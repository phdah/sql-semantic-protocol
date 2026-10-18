//! Closed-world finite join-chain evaluation from canonical parsed semantics.
//!
//! Input bags are explicit caller-controlled source evidence, never inferred
//! from output cardinality. Only typed scalar equalities and filters are
//! executable here; unproved SQL laws stay residual.

use std::collections::{BTreeMap, BTreeSet};

use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::protocol::{
    AggregateArgument, ComparisonOperator, GroupingExpression, JoinKind, LiteralValue,
};

/// Exact terminal scalar domain computed from the complete constructed bag.
/// All finite non-NULL values are enumerated; bounds are inclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteChainColumnDomain {
    allowed: Vec<i128>,
    null_allowed: bool,
}

impl PhysicalFiniteChainColumnDomain {
    /// Ordered distinct signed-integer values appearing in the complete output.
    pub fn allowed(&self) -> &[i128] {
        &self.allowed
    }

    /// Whether SQL NULL actually appears in the terminal bag.
    pub fn null_allowed(&self) -> bool {
        self.null_allowed
    }

    /// Inclusive lowest and highest values; None for an empty integer domain.
    pub fn bounds(&self) -> Option<(i128, i128)> {
        Some((*self.allowed.first()?, *self.allowed.last()?))
    }
}

fn exact_terminal_domains(
    terminal: &[BTreeMap<String, LiteralValue>],
    columns: &[&str],
) -> Option<BTreeMap<String, PhysicalFiniteChainColumnDomain>> {
    let mut domains = BTreeMap::new();
    for name in columns {
        let mut values = BTreeSet::new();
        let mut null_allowed = false;
        for row in terminal {
            match row.get(*name)? {
                LiteralValue::Null => null_allowed = true,
                LiteralValue::Number(number) => {
                    values.insert(number.parse::<i128>().ok()?);
                }
                _ => return None,
            }
        }
        domains.insert(
            name.to_string(),
            PhysicalFiniteChainColumnDomain {
                allowed: values.into_iter().collect(),
                null_allowed,
            },
        );
    }
    Some(domains)
}

/// Exact physical row identities together with their surviving source-backed
/// terminal bag. NULL and bag multiplicity are preserved without SQL reparsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteChainProof {
    /// All three supplied complete source bags satisfy attested schema laws
    /// and the normalized SQL predicates have an exact integer comparison law.
    Exact {
        /// Canonical graph used for both the source bags and the terminal.
        nodes: Vec<PhysicalPlanNode>,
        /// Named source instances in SQL left-to-right order.
        sources: [String; 3],
        /// Source identities for rows whose independent left WHERE is TRUE.
        filtered_true_ids: Vec<String>,
        /// Source identities whose left WHERE is FALSE or UNKNOWN.
        filtered_not_true_ids: Vec<String>,
        /// Selected physical identity triplets, including duplicates by row.
        selected: Vec<[String; 3]>,
        /// SQL FALSE or UNKNOWN triplets over the full Cartesian source product.
        rejected: Vec<[String; 3]>,
        /// Exact terminal SQL bag with named, typed projected columns.
        terminal: Vec<BTreeMap<String, LiteralValue>>,
        /// Domains inferred from that entire bag, with inclusive bounds.
        terminal_domains: BTreeMap<String, PhysicalFiniteChainColumnDomain>,
    },
    /// Input rows violate an attested necessary physical constraint.
    Impossible { reason: String },
    /// Parser, source, type, or comparison evidence is incomplete.
    Residual { reason: String },
}

fn residual(reason: &str) -> PhysicalFiniteChainProof {
    PhysicalFiniteChainProof::Residual {
        reason: reason.to_string(),
    }
}

type ChainRows<'a> = [(&'a str, &'a crate::physical_realization::PhysicalWriteRow); 3];

fn source_id(row: &crate::physical_realization::PhysicalWriteRow) -> Option<String> {
    match row.values().get("id")? {
        LiteralValue::Text(value) if !value.is_empty() => Some(value.clone()),
        _ => None,
    }
}

fn numeric_value(value: &LiteralValue) -> Result<Option<i128>, &'static str> {
    match value {
        LiteralValue::Number(value) => value
            .parse::<i128>()
            .map(Some)
            .map_err(|_| "source number cannot be represented as a signed integer"),
        LiteralValue::Null => Ok(None),
        _ => Err("unattested non-integer comparison operand"),
    }
}

fn expression_value(
    expression: &Expression,
    rows: &ChainRows<'_>,
    names: &[String; 3],
) -> Result<Option<i128>, &'static str> {
    match expression {
        Expression::Literal(literal) => numeric_value(literal.value()),
        Expression::Column(column) => {
            let relation = column
                .relation()
                .ok_or("unqualified or ambiguous physical column")?;
            let matches = names
                .iter()
                .enumerate()
                .filter(|(index, alias)| alias.as_str() == relation || rows[*index].0 == relation)
                .collect::<Vec<_>>();
            let [(index, _)] = matches.as_slice() else {
                return Err("physical column does not identify exactly one source instance");
            };
            let value = rows[*index]
                .1
                .values()
                .get(column.name())
                .ok_or("physical row lacks the referenced source column")?;
            numeric_value(value)
        }
        _ => Err("expression lacks a certified exact finite scalar law"),
    }
}

fn compare(
    operator: ComparisonOperator,
    left: Option<i128>,
    right: Option<i128>,
) -> Result<Option<bool>, &'static str> {
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(None);
    };
    Ok(Some(match operator {
        ComparisonOperator::Eq => left == right,
        ComparisonOperator::Neq => left != right,
        ComparisonOperator::Lt => left < right,
        ComparisonOperator::Lte => left <= right,
        ComparisonOperator::Gt => left > right,
        ComparisonOperator::Gte => left >= right,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
            return Err("NULL-safe and vendor-dependent comparisons not certified by this law");
        }
    }))
}

fn predicate_truth(
    predicate: &Predicate,
    rows: &ChainRows<'_>,
    names: &[String; 3],
) -> Result<Option<bool>, &'static str> {
    match predicate {
        Predicate::Comparison(comparison) => compare(
            comparison.operator(),
            expression_value(comparison.left(), rows, names)?,
            expression_value(comparison.right(), rows, names)?,
        ),
        Predicate::And(and) => {
            let mut unknown = false;
            for operand in and.operands() {
                match predicate_truth(operand, rows, names)? {
                    Some(false) => return Ok(Some(false)),
                    Some(true) => {}
                    None => unknown = true,
                }
            }
            Ok(if unknown { None } else { Some(true) })
        }
        _ => Err("unsupported predicate cannot be treated as SQL FALSE"),
    }
}

fn terminal_group(
    query: &QueryStatement,
    key: &str,
    counts: &BTreeMap<i128, u64>,
) -> Result<Vec<BTreeMap<String, LiteralValue>>, &'static str> {
    let Some(crate::protocol::GroupBy::Expressions(group_keys)) = query
        .aggregation()
        .and_then(|aggregation| aggregation.group_by())
    else {
        return Err("missing exact ordinary group key");
    };
    let [GroupingExpression::Expression(Expression::Column(group_key))] = group_keys.as_slice()
    else {
        return Err("unsupported grouped key shape");
    };
    if group_key.name() != key || query.output().columns().len() != 2 {
        return Err("grouped output does not preserve the joined physical key");
    }
    let mut output_key = None;
    let mut output_count = None;
    for column in query.output().columns() {
        match column.expression() {
            Expression::Column(expression) if expression.name() == key => {
                output_key = Some(column.name());
            }
            Expression::AggregateFunction(function)
                if function.name().eq_ignore_ascii_case("count")
                    && matches!(function.arguments(), [AggregateArgument::Wildcard])
                    && !function.distinct()
                    && function.filter().is_none() =>
            {
                output_count = Some(column.name());
            }
            _ => return Err("grouped projection has an unsupported aggregate"),
        }
    }
    let (Some(output_key), Some(output_count)) = (output_key, output_count) else {
        return Err("missing key or COUNT(*) projected output");
    };

    let having = match query.predicates().having_predicate() {
        None => None,
        Some(Predicate::Comparison(comparison)) => {
            let (Expression::AggregateFunction(function), Expression::Literal(literal)) =
                (comparison.left(), comparison.right())
            else {
                return Err("unsupported HAVING expression");
            };
            if !function.name().eq_ignore_ascii_case("count")
                || !matches!(function.arguments(), [AggregateArgument::Wildcard])
                || function.distinct()
                || function.filter().is_some()
            {
                return Err("HAVING is not ordinary COUNT(*)");
            }
            let Some(threshold) = numeric_value(literal.value())? else {
                return Err("HAVING NULL has no certified count truth");
            };
            Some((comparison.operator(), threshold))
        }
        Some(_) => return Err("unsupported HAVING truth law"),
    };
    let mut terminal = Vec::new();
    for (key_value, count) in counts {
        if let Some((operator, threshold)) = having {
            if compare(operator, Some(i128::from(*count)), Some(threshold))? != Some(true) {
                continue;
            }
        }
        terminal.push(BTreeMap::from([
            (
                output_key.to_string(),
                LiteralValue::Number(key_value.to_string()),
            ),
            (
                output_count.to_string(),
                LiteralValue::Number(count.to_string()),
            ),
        ]));
    }
    Ok(terminal)
}

/// Evaluate a complete caller-supplied three-source INNER chain over canonical
/// SQL and attested physical schemas. Unlike a sufficient homogeneous count
/// witness, this certifies each selected and NOT TRUE physical identity
/// triplet, including NULL/duplicate/rejected rows and downstream COUNT(*).
///
/// The implementation admits only the supported finite SQL shape and never
/// guesses from source counts or silently ignores an unsupported predicate.
/// Schema PK/FK feasibility is delegated to the authoritative physical-state
/// validator. It is an evidence-driven proof, not automatic row synthesis.
pub fn physical_finite_chain_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteChainProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteChainProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }

    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(target) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing canonical target layer");
    };
    let Some(query) = query_for(bundle, target) else {
        return residual("missing canonical query");
    };
    let grouped = query.joins().is_empty();
    let (join_query, grouped_query) = if grouped {
        let Some(node) = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(target_layer_id.to_string()))
        else {
            return residual("missing group producer");
        };
        let [PhysicalPlanRef::Layer(producer)] = node.inputs() else {
            return residual("GROUP requires one in-bundle joined producer");
        };
        let Some(layer) = walker.layers.get(producer.as_str()).copied() else {
            return residual("group join producer is absent");
        };
        let Some(join_query) = query_for(bundle, layer) else {
            return residual("group join producer has no canonical query");
        };
        if untransported_materialization_laws(
            bundle,
            query.sources().first().map_or("", |source| source.name()),
        ) {
            return residual("group stage carries untransported materialization constraints");
        }
        (join_query, Some(query))
    } else {
        (query, None)
    };
    let [left, middle, right] = join_query.sources() else {
        return residual("finite proof requires three original physical source instances");
    };
    let [first_join, second_join] = join_query.joins() else {
        return residual("finite proof requires exactly two joins");
    };
    if first_join.kind() != JoinKind::Inner
        || second_join.kind() != JoinKind::Inner
        || join_query.aggregation().is_some()
        || join_query.predicates().having_predicate().is_some()
        || join_query.predicates().qualify_predicate().is_some()
        || !join_query.diagnostics().is_empty()
        || join_query.set_operation().is_some()
        || !join_query.subquery_witnesses().is_empty()
        || join_query.window_witness().is_some()
    {
        return residual("unproved join-chain operator or conditional SQL law");
    }
    let sources = [left, middle, right];
    let names = sources.map(|source| source.alias().unwrap_or(source.name()).to_string());
    let relations = sources.map(|source| source.name());
    if BTreeSet::from(relations).len() != 3
        || names.iter().collect::<BTreeSet<_>>().len() != 3
        || snapshots.len() != 3
        || relations
            .iter()
            .any(|relation| !snapshots.iter().any(|state| state.relation() == *relation))
    {
        return residual("the exact source population must close three distinct physical bags");
    }
    let mut ordered = Vec::new();
    for relation in relations {
        let Some(state) = snapshots.iter().find(|state| state.relation() == relation) else {
            return residual("missing complete source population");
        };
        ordered.push(state);
        let mut ids = BTreeSet::new();
        for row in state.rows() {
            let Some(id) = source_id(row) else {
                return residual("missing stable string row identity");
            };
            if !ids.insert(id) {
                return PhysicalFiniteChainProof::Impossible {
                    reason: "repeated physical row identity".to_string(),
                };
            }
        }
    }
    let Some(first_on) = first_join.condition() else {
        return residual("missing first normalized ON predicate");
    };
    let Some(second_on) = second_join.condition() else {
        return residual("missing second normalized ON predicate");
    };
    let projected = match join_query.output().columns() {
        [column] => match column.expression() {
            Expression::Column(reference) => Some((column.name(), reference)),
            _ => None,
        },
        _ => None,
    };
    let Some((output_name, key_expression)) = projected else {
        return residual("finite terminal needs one plain copied physical key");
    };
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut counts = BTreeMap::<i128, u64>::new();
    let mut terminal = Vec::new();
    let mut filtered_true_ids = Vec::new();
    let mut filtered_not_true_ids = Vec::new();
    for l in ordered[0].rows() {
        let mut left_truth = None;
        for r in ordered[1].rows() {
            for t in ordered[2].rows() {
                let rows = [(relations[0], l), (relations[1], r), (relations[2], t)];
                let where_truth = match join_query.predicates().where_predicate() {
                    None => Ok(Some(true)),
                    Some(predicate) => predicate_truth(predicate, &rows, &names),
                };
                let Ok(where_truth) = where_truth else {
                    return residual("WHERE lacks an exact signed-integer truth law");
                };
                if left_truth.is_some_and(|previous| previous != where_truth) {
                    return residual("WHERE depends on more than the independent left source");
                }
                left_truth = Some(where_truth);
                let (Ok(first), Ok(second)) = (
                    predicate_truth(first_on, &rows, &names),
                    predicate_truth(second_on, &rows, &names),
                ) else {
                    return residual("ON predicate lacks an exact signed-integer equality law");
                };
                let identities = [
                    source_id(l).unwrap_or_default(),
                    source_id(r).unwrap_or_default(),
                    source_id(t).unwrap_or_default(),
                ];
                if where_truth == Some(true) && first == Some(true) && second == Some(true) {
                    let Ok(Some(value)) = expression_value(
                        &Expression::Column(key_expression.clone()),
                        &rows,
                        &names,
                    ) else {
                        return residual("selected projection lacks an exact integer value");
                    };
                    let Some(next) = counts.get(&value).copied().unwrap_or(0).checked_add(1) else {
                        return residual("joined contribution multiplicity overflow");
                    };
                    counts.insert(value, next);
                    if grouped_query.is_none() {
                        terminal.push(BTreeMap::from([(
                            output_name.to_string(),
                            LiteralValue::Number(value.to_string()),
                        )]));
                    }
                    selected.push(identities);
                } else {
                    rejected.push(identities);
                }
            }
        }
        if let Some(truth) = left_truth {
            let Some(id) = source_id(l) else {
                return residual("unidentified left source row");
            };
            if truth == Some(true) {
                filtered_true_ids.push(id);
            } else {
                filtered_not_true_ids.push(id);
            }
        } else if !ordered[1].rows().is_empty() || !ordered[2].rows().is_empty() {
            return residual("missing complete left predicate evaluation");
        }
    }
    if let Some(group_query) = grouped_query {
        let key = output_name;
        match terminal_group(group_query, key, &counts) {
            Ok(result) => terminal = result,
            Err(reason) => return residual(reason),
        }
    }
    let columns = if let Some(group_query) = grouped_query {
        group_query
            .output()
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>()
    } else {
        vec![output_name]
    };
    let Some(terminal_domains) = exact_terminal_domains(&terminal, &columns) else {
        return residual("terminal domain has an unsupported scalar or incomplete projection");
    };
    PhysicalFiniteChainProof::Exact {
        nodes: walker.nodes,
        sources: names,
        filtered_true_ids,
        filtered_not_true_ids,
        selected,
        rejected,
        terminal,
        terminal_domains,
    }
}
