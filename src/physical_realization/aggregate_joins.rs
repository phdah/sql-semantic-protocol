//! Closed-world two-source INNER JOIN to GROUP/HAVING proof (TASK-123).
//!
//! The complete caller-supplied physical bags are the only source of row
//! existence and multiplicity. Every Cartesian pair is classified under SQL
//! three-valued truth, then surviving identities flow into typed aggregates.
//! Unproved operators, types, grouping extensions and computed scalars fail closed.

use super::aggregate_bags::{
    numeric, predicate_shape, predicate_truth, signed_integer, PhysicalAggregateGroup,
    PhysicalAggregateTruth,
};
use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::protocol::{
    AggregateArgument, AggregateFunctionExpression, ColumnExpression, JoinKind, LiteralExpression,
    LiteralType, LiteralValue, SetMode, ValueDomain,
};

/// Exact, reproducible two-source SQL bag and all rejected physical pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalJoinedAggregateExact {
    /// Canonical physical source and producer DAG.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Physical relation names in SQL order.
    pub sources: [String; 2],
    /// Every pair whose ON and WHERE predicates are both TRUE.
    pub selected: Vec<[String; 2]>,
    /// Every Cartesian pair rejected by ON or WHERE (FALSE or UNKNOWN).
    pub rejected: Vec<[String; 2]>,
    /// Every group including groups rejected by HAVING.
    pub groups: Vec<PhysicalAggregateGroup>,
    /// Complete terminal output bag, including duplicate rows and SQL NULL.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact terminal column value sets; each finite numeric bound is inclusive.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// A proof, a contradictory physical source bag, or an explicitly unproved law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalJoinedAggregateProof {
    /// Exact source-backed output and complete TRUE/NOT TRUE classifications.
    Exact(Box<PhysicalJoinedAggregateExact>),
    /// Source rows contradict enforced physical schema evidence.
    Impossible { reason: String },
    /// Missing parser, execution, datatype or source evidence.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalJoinedAggregateProof {
    PhysicalJoinedAggregateProof::Residual {
        reason: reason.into(),
    }
}

struct JoinedSources<'a> {
    names: [&'a str; 2],
    aliases: [&'a str; 2],
    schemas: [&'a crate::relation::RelationSchema; 2],
    states: [&'a PhysicalWriteState; 2],
}

fn source_index(
    column: &ColumnExpression,
    sources: &JoinedSources<'_>,
) -> Result<usize, &'static str> {
    let Some(relation) = column.relation() else {
        return Err("unqualified joined column lacks unique source identity");
    };
    let matches = (0..2)
        .filter(|index| relation == sources.names[*index] || relation == sources.aliases[*index])
        .collect::<Vec<_>>();
    let [index] = matches.as_slice() else {
        return Err("joined column does not identify exactly one physical source");
    };
    let Some(declared) = sources.schemas[*index]
        .columns()
        .iter()
        .find(|schema| schema.name() == column.name())
    else {
        return Err("joined column lacks source schema evidence");
    };
    if !signed_integer(declared.data_type()) {
        return Err("joined column has no portable signed integer execution law");
    }
    Ok(*index)
}

fn scalar(
    expr: &Expression,
    pair: Option<[usize; 2]>,
    sources: &JoinedSources<'_>,
) -> Result<LiteralValue, &'static str> {
    match expr {
        Expression::Column(column) => {
            let index = source_index(column, sources)?;
            match pair {
                Some(pair) => sources.states[index]
                    .rows()
                    .get(pair[index])
                    .ok_or("joined physical row identity is outside the attested snapshot")?
                    .values()
                    .get(column.name())
                    .cloned()
                    .ok_or("physical joined row is missing a declared column"),
                None => Ok(LiteralValue::Null),
            }
        }
        Expression::Literal(literal) => match literal.literal_type() {
            LiteralType::Integer | LiteralType::Boolean | LiteralType::Null => {
                Ok(literal.value().clone())
            }
            _ => Err("joined scalar literal has no exact numeric truth law"),
        },
        _ => Err("joined scalar expression has no certified execution law"),
    }
}

fn aggregate(
    function: &AggregateFunctionExpression,
    members: &[[usize; 2]],
    sources: &JoinedSources<'_>,
) -> Result<(LiteralValue, Vec<String>), &'static str> {
    let name = function.name().to_ascii_lowercase();
    if !matches!(name.as_str(), "count" | "sum" | "min" | "max" | "avg") {
        return Err("aggregate function lacks exact physical execution semantics");
    }
    if function.distinct() && name != "count" {
        return Err("DISTINCT is certified only for COUNT(column)");
    }
    let argument = match function.arguments() {
        [AggregateArgument::Wildcard] if name == "count" && !function.distinct() => None,
        [AggregateArgument::Expression(Expression::Column(column))] => {
            source_index(column, sources)?;
            Some(column)
        }
        _ => return Err("aggregate argument is not one attested source column or COUNT(*)"),
    };
    if let Some(filter) = function.filter() {
        predicate_shape(filter, &|expr| scalar(expr, None, sources))?;
    }

    let mut contributors = Vec::new();
    let mut numbers = Vec::new();
    let mut distinct = BTreeSet::new();
    for pair in members {
        if let Some(filter) = function.filter() {
            if predicate_truth(filter, &|expr| scalar(expr, Some(*pair), sources))?
                != PhysicalAggregateTruth::True
            {
                continue;
            }
        }
        let value = match argument {
            Some(column) => scalar(&Expression::Column(column.clone()), Some(*pair), sources)?,
            None => LiteralValue::Number("1".to_string()),
        };
        let Some(number) = numeric(&value)? else {
            continue;
        };
        if function.distinct() && !distinct.insert(number) {
            continue;
        }
        let identity = format!(
            "{}#{}|{}#{}",
            sources.names[0], pair[0], sources.names[1], pair[1]
        );
        contributors.push(identity);
        numbers.push(number);
    }
    let value = match name.as_str() {
        "count" => LiteralValue::Number(contributors.len().to_string()),
        "sum" | "avg" => {
            if numbers.is_empty() {
                LiteralValue::Null
            } else {
                let sum = numbers
                    .iter()
                    .try_fold(0_i128, |total, value| total.checked_add(*value))
                    .ok_or("joined SUM/AVG overflow lacks a certified SQL accumulator law")?;
                if name == "sum" {
                    LiteralValue::Number(sum.to_string())
                } else {
                    let n = i128::try_from(numbers.len())
                        .map_err(|_| "aggregate count exceeds signed integer range")?;
                    if sum % n != 0 {
                        return Err("nonintegral AVG lacks an attested decimal scale");
                    }
                    LiteralValue::Number((sum / n).to_string())
                }
            }
        }
        "min" => numbers
            .iter()
            .min()
            .map(|n| LiteralValue::Number(n.to_string()))
            .unwrap_or(LiteralValue::Null),
        "max" => numbers
            .iter()
            .max()
            .map(|n| LiteralValue::Number(n.to_string()))
            .unwrap_or(LiteralValue::Null),
        _ => return Err("aggregate function is unsupported"),
    };
    Ok((value, contributors))
}

fn having_scalar(
    expr: &Expression,
    projected: &BTreeMap<String, LiteralValue>,
    key: Option<i128>,
    group_column: Option<&ColumnExpression>,
    members: &[[usize; 2]],
    sources: &JoinedSources<'_>,
) -> Result<LiteralValue, &'static str> {
    match expr {
        Expression::AggregateFunction(function) => aggregate(function, members, sources).map(|v| v.0),
        Expression::Column(column) => {
            if group_column.is_some_and(|group| group == column) {
                return Ok(key.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())));
            }
            if column.relation().is_none() {
                return projected
                    .get(column.name())
                    .cloned()
                    .ok_or("HAVING does not name a projected aggregate");
            }
            Err("HAVING source column is not its certified grouping key")
        }
        Expression::Literal(literal) => match literal.literal_type() {
            LiteralType::Integer | LiteralType::Boolean | LiteralType::Null => {
                Ok(literal.value().clone())
            }
            _ => Err("HAVING literal type is unproved"),
        },
        _ => Err("HAVING scalar has no exact joined aggregate law"),
    }
}

fn domains(
    output_names: &[String],
    terminal: &[BTreeMap<String, LiteralValue>],
) -> Result<BTreeMap<String, ValueDomain>, &'static str> {
    let mut result = BTreeMap::new();
    for name in output_names {
        let mut values = Vec::new();
        for row in terminal {
            let value = row.get(name).ok_or("terminal projection is incomplete")?;
            if !values.iter().any(|candidate: &LiteralExpression| candidate.value() == value) {
                values.push(LiteralExpression::new(
                    if value == &LiteralValue::Null { LiteralType::Null } else { LiteralType::Integer },
                    value.clone(),
                ));
            }
        }
        result.insert(name.clone(), ValueDomain::set(SetMode::Include, values));
    }
    Ok(result)
}

/// Evaluate INNER JOIN, WHERE, GROUP BY and HAVING over two complete typed
/// physical source bags. Supports ordinary one-column or global grouping,
/// COUNT(*), COUNT(column), COUNT(DISTINCT column), FILTER and exact integer
/// SUM/MIN/MAX/AVG. GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL, opaque types
/// and unproved dialect/session laws deliberately remain typed Residual.
///
/// Row pair identities and terminal domains are evaluated from the same
/// Cartesian population. Empty global aggregation emits one row; empty
/// ordinary grouping emits none.
pub fn physical_finite_join_aggregate_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalJoinedAggregateProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalJoinedAggregateProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("unknown join aggregate layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("joined aggregate is not a canonical parsed query");
    };
    let [left, right] = query.sources() else {
        return residual("joined aggregation requires exactly two physical sources");
    };
    let [join] = query.joins() else {
        return residual("joined aggregation requires exactly one join");
    };
    if join.kind() != JoinKind::Inner
        || query.predicates().qualify_predicate().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
        || query.aggregation().is_some_and(|a| a.distinct() || !a.distinct_on().is_empty())
    {
        return residual("joined aggregate has an unsupported operator or SQL row-shaping law");
    }
    let source_names = [left.name(), right.name()];
    if source_names[0] == source_names[1] {
        return residual("self-joins require distinct attested physical source-instance evidence");
    }
    let [left_state, right_state] = snapshots else {
        return residual("joined aggregation requires exactly two complete source bags");
    };
    if left_state.relation() != source_names[0] || right_state.relation() != source_names[1] {
        return residual("physical source order or identity disagrees with canonical join");
    }
    let Some(left_schema) = bundle.source_schemas().iter().find(|s| s.relation() == source_names[0]) else {
        return residual("left source lacks attested schema");
    };
    let Some(right_schema) = bundle.source_schemas().iter().find(|s| s.relation() == source_names[1]) else {
        return residual("right source lacks attested schema");
    };
    let sources = JoinedSources {
        names: source_names,
        aliases: [left.alias().unwrap_or(left.name()), right.alias().unwrap_or(right.name())],
        schemas: [left_schema, right_schema],
        states: [left_state, right_state],
    };
    if !walker.sources.contains(source_names[0]) || !walker.sources.contains(source_names[1]) {
        return residual("join producer does not close over supplied physical sources");
    }
    let Some(on) = join.condition() else {
        return residual("INNER JOIN requires a certified ON predicate");
    };
    for predicate in [Some(on), query.predicates().where_predicate()] {
        if let Some(predicate) = predicate {
            if let Err(reason) = predicate_shape(predicate, &|expr| scalar(expr, None, &sources)) {
                return residual(reason);
            }
        }
    }

    let group_column = match query.aggregation().and_then(|a| a.group_by()) {
        Some(GroupBy::Expressions(keys)) => {
            let [GroupingExpression::Expression(Expression::Column(column))] = keys.as_slice() else {
                return residual("GROUPING SETS, ROLLUP, CUBE or multi-key groups require explicit grouping identities");
            };
            if let Err(reason) = source_index(column, &sources) {
                return residual(reason);
            }
            Some(column)
        }
        Some(GroupBy::All) => {
            return residual("GROUP BY ALL needs a dialect-certified inferred grouping identity");
        }
        None => None,
    };
    if group_column.is_some() && !query.group_rows_match_surviving_groups() {
        return residual("parser did not certify joined grouping terminal shape");
    }
    if group_column.is_none() && !query.proven_single_row_output() {
        return residual("parser did not certify global aggregate singleton");
    }

    let mut output_names = Vec::new();
    let mut has_aggregate = false;
    for output in query.output().columns() {
        if output_names.iter().any(|name: &String| name == output.name()) {
            return residual("duplicate terminal output names");
        }
        output_names.push(output.name().to_string());
        match output.expression() {
            Expression::Column(column) if group_column == Some(column) => {}
            Expression::AggregateFunction(function) => {
                has_aggregate = true;
                if let Err(reason) = aggregate(function, &[], &sources) {
                    return residual(reason);
                }
            }
            _ => return residual("joined aggregate projection is not a group key or certified aggregate"),
        }
    }
    if !has_aggregate {
        return residual("joined group contains no certified aggregate");
    }

    if let Some(having) = query.predicates().having_predicate() {
        let prototype = output_names
            .iter()
            .map(|name| (name.to_string(), LiteralValue::Null))
            .collect::<BTreeMap<_, _>>();
        let shape = |expr: &Expression| having_scalar(expr, &prototype, None, group_column, &[], &sources);
        if let Err(reason) = predicate_shape(having, &shape) {
            return residual(reason);
        }
    }

    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut groups = BTreeMap::<Option<i128>, Vec<[usize; 2]>>::new();
    if group_column.is_none() {
        groups.insert(None, Vec::new());
    }
    for l in 0..left_state.rows().len() {
        for r in 0..right_state.rows().len() {
            let pair = [l, r];
            let left_id = format!("{}#{l}", source_names[0]);
            let right_id = format!("{}#{r}", source_names[1]);
            let on_truth = predicate_truth(on, &|expr| scalar(expr, Some(pair), &sources));
            let where_truth = query.predicates().where_predicate().map_or(
                Ok(PhysicalAggregateTruth::True),
                |predicate| predicate_truth(predicate, &|expr| scalar(expr, Some(pair), &sources)),
            );
            let (Ok(on_truth), Ok(where_truth)) = (on_truth, where_truth) else {
                return residual("joined predicate evaluation requires unproved data type semantics");
            };
            if on_truth == PhysicalAggregateTruth::True && where_truth == PhysicalAggregateTruth::True {
                let key = match group_column {
                    Some(column) => match numeric(&match scalar(
                        &Expression::Column(column.clone()), Some(pair), &sources
                    ) {
                        Ok(value) => value,
                        Err(reason) => return residual(reason),
                    }) {
                        Ok(value) => value,
                        Err(reason) => return residual(reason),
                    },
                    None => None,
                };
                groups.entry(key).or_default().push(pair);
                selected.push([left_id, right_id]);
            } else {
                rejected.push([left_id, right_id]);
            }
        }
    }
    let mut identities = Vec::new();
    let mut terminal = Vec::new();
    for (key, members) in groups {
        let mut projected = BTreeMap::new();
        let mut contributor_ids = BTreeMap::new();
        for output in query.output().columns() {
            let value = match output.expression() {
                Expression::Column(column) if group_column == Some(column) => key
                    .map(|v| LiteralValue::Number(v.to_string()))
                    .unwrap_or(LiteralValue::Null),
                Expression::AggregateFunction(function) => match aggregate(function, &members, &sources) {
                    Ok((value, ids)) => {
                        contributor_ids.insert(output.name().to_string(), ids);
                        value
                    }
                    Err(reason) => return residual(reason),
                },
                _ => return residual("unproved joined aggregate output"),
            };
            projected.insert(output.name().to_string(), value);
        }
        let having_truth = match query.predicates().having_predicate() {
            None => None,
            Some(predicate) => {
                let evaluate = |expr: &Expression| having_scalar(
                    expr, &projected, key, group_column, &members, &sources
                );
                match predicate_truth(predicate, &evaluate) {
                    Ok(value) => Some(value),
                    Err(reason) => return residual(reason),
                }
            }
        };
        let member_ids = members.iter().map(|pair| {
            format!("{}#{}|{}#{}", source_names[0], pair[0], source_names[1], pair[1])
        }).collect();
        identities.push(PhysicalAggregateGroup {
            key,
            source_row_ids: member_ids,
            aggregate_row_ids: contributor_ids,
            having_truth,
        });
        if having_truth.is_none_or(|truth| truth == PhysicalAggregateTruth::True) {
            terminal.push(projected);
        }
    }
    let terminal_domains = match domains(&output_names, &terminal) {
        Ok(domains) => domains,
        Err(reason) => return residual(reason),
    };
    PhysicalJoinedAggregateProof::Exact(Box::new(PhysicalJoinedAggregateExact {
        nodes: walker.nodes,
        sources: source_names.map(str::to_string),
        selected,
        rejected,
        groups: identities,
        terminal,
        terminal_domains,
    }))
}
