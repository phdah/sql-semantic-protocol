//! TASK-121 closed-world basic integer aggregate laws.
//!
//! A complete, schema-attested physical bag is the evidence. We interpret only
//! canonical parsed aggregate expressions, never input SQL or presumed row
//! counts. Missing type, scale, collation and overflow laws fail closed.

use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::data_type::DataType;
use crate::protocol::{
    AggregateArgument, ColumnExpression, LiteralExpression, LiteralType, LiteralValue, SetMode,
    ValueDomain,
};

/// Exact membership of a group, keeping duplicates as separate source rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalAggregateGroup {
    /// GROUP BY key. None is either SQL NULL or the synthetic global group.
    pub key: Option<i128>,
    /// All physical rows contributing to this group, including NULL inputs.
    pub source_row_ids: Vec<String>,
    /// Non-NULL contributors for each named aggregate (COUNT(*) includes all).
    pub aggregate_row_ids: BTreeMap<String, Vec<String>>,
}

/// One fully evaluated source-backed aggregate query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteAggregateExact {
    /// Canonical graph reached from the requested terminal layer.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Physical relation whose complete bag was supplied.
    pub source: String,
    /// True for SQL GROUP BY, false for the global singleton group.
    pub grouped: bool,
    /// All groups including groups not surviving any NULL aggregate values.
    pub groups: Vec<PhysicalAggregateGroup>,
    /// Complete resulting terminal bag with SQL NULL values preserved.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Canonical exact finite domains of each terminal output column.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// Proof strength, with contradiction distinct from absent execution evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteAggregateProof {
    /// Fully closed-world input and exact SQL terminal bag.
    Exact(Box<PhysicalFiniteAggregateExact>),
    /// Enforced physical schema evidence contradicts the supplied population.
    Impossible { reason: String },
    /// Unproven operator or runtime law. Never an empty-output proof.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalFiniteAggregateProof {
    PhysicalFiniteAggregateProof::Residual {
        reason: reason.into(),
    }
}

fn signed_integer(data_type: &DataType) -> bool {
    match data_type {
        DataType::SignedInteger { bits } => bits.is_none_or(|bits| bits <= 128),
        DataType::Nullable(inner) => signed_integer(inner),
        _ => false,
    }
}

fn matches_source(column: &ColumnExpression, name: &str, alias: Option<&str>) -> bool {
    column
        .relation()
        .is_none_or(|reference| reference == name || Some(reference) == alias)
}

fn numeric(value: &LiteralValue) -> Result<Option<i128>, &'static str> {
    match value {
        LiteralValue::Null => Ok(None),
        LiteralValue::Number(v) => v
            .parse::<i128>()
            .map(Some)
            .map_err(|_| "source value is not an exact signed integer"),
        _ => Err("source value has no attested signed integer execution law"),
    }
}

fn aggregate_value(
    function: &crate::protocol::AggregateFunctionExpression,
    members: &[usize],
    snapshot: &PhysicalWriteState,
    schema: &crate::relation::RelationSchema,
    source: &str,
    alias: Option<&str>,
) -> Result<(LiteralValue, Vec<String>), &'static str> {
    if function.distinct() || function.filter().is_some() {
        return Err("DISTINCT and FILTER need separate grouped proofs");
    }
    let count_all = function.name().eq_ignore_ascii_case("count")
        && matches!(function.arguments(), [AggregateArgument::Wildcard]);
    let column = if count_all {
        None
    } else {
        let [AggregateArgument::Expression(Expression::Column(column))] = function.arguments()
        else {
            return Err("only COUNT(*) or a single physical scalar argument is certified");
        };
        if !matches_source(column, source, alias) {
            return Err("aggregate argument is not a direct physical source column");
        }
        let schema_column = schema.columns().iter().find(|item| item.name() == column.name())
            .ok_or("aggregate argument has no source schema")?;
        if !signed_integer(schema_column.data_type()) {
            return Err("aggregate source needs an exact signed integer type law");
        }
        Some(column.name())
    };

    let mut numbers = Vec::new();
    let mut contributors = Vec::new();
    for index in members {
        let row = &snapshot.rows()[*index];
        let id = format!("{source}#{index}");
        if count_all {
            contributors.push(id);
            continue;
        }
        let value = row.values().get(column.ok_or("missing aggregate column")?)
            .ok_or("aggregate source row lacks a declared column")?;
        if let Some(value) = numeric(value)? {
            numbers.push(value);
            contributors.push(id);
        }
    }

    let value = match function.name().to_ascii_lowercase().as_str() {
        "count" => LiteralValue::Number(contributors.len().to_string()),
        "sum" | "avg" => {
            if numbers.is_empty() {
                LiteralValue::Null
            } else {
                let sum = numbers.iter().try_fold(0_i128, |acc, n| acc.checked_add(*n))
                    .ok_or("aggregate accumulator overflow without an attested wider SQL type")?;
                if function.name().eq_ignore_ascii_case("sum") {
                    LiteralValue::Number(sum.to_string())
                } else {
                    let count = i128::try_from(numbers.len())
                        .map_err(|_| "aggregate contributor count exceeds signed integer range")?;
                    if sum % count != 0 {
                        return Err("nonintegral AVG needs an attested decimal scale and rounding law");
                    }
                    LiteralValue::Number((sum / count).to_string())
                }
            }
        }
        "min" => numbers.iter().min().map(|n| LiteralValue::Number(n.to_string()))
            .unwrap_or(LiteralValue::Null),
        "max" => numbers.iter().max().map(|n| LiteralValue::Number(n.to_string()))
            .unwrap_or(LiteralValue::Null),
        _ => return Err("aggregate function has no portable closed-world proof"),
    };
    Ok((value, contributors))
}

/// Prove a direct single-source GROUP BY or global aggregation on the complete
/// physical integer bag. Both SQL's empty-global singleton and empty-grouped
/// zero-row result are evaluated from the same source identity. Runtime-specific
/// AVG scale, overflowing SUM, HAVING, DISTINCT and other operators stay Residual.
pub fn physical_finite_aggregate_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteAggregateProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteAggregateProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("unknown aggregate layer identity");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("aggregate layer is not a parsed canonical query");
    };
    let [source] = query.sources() else {
        return residual("exact basic aggregation requires one direct physical source");
    };
    let relation = source.name();
    let [snapshot] = snapshots else {
        return residual("complete aggregate proof needs exactly one source snapshot");
    };
    if snapshot.relation() != relation
        || !walker.sources.contains(relation)
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.joins().is_empty()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
    {
        return residual("aggregate is not a direct unfiltered physical source");
    }
    let Some(schema) = bundle.source_schemas().iter().find(|s| s.relation() == relation) else {
        return residual("missing authoritative physical source schema");
    };
    let grouped_key = match query.aggregation() {
        None => None,
        Some(aggregation) if aggregation.distinct() || !aggregation.distinct_on().is_empty() =>
            return residual("aggregate DISTINCT has an unproved bag law"),
        Some(aggregation) => match aggregation.group_by() {
            Some(GroupBy::Expressions(keys)) if keys.len() == 1 => {
                let GroupingExpression::Expression(Expression::Column(column)) = &keys[0] else {
                    return residual("group key is not a simple physical column");
                };
                if !matches_source(column, relation, source.alias()) {
                    return residual("group key source identity is unresolved");
                }
                let Some(schema_column) = schema.columns().iter().find(|c| c.name() == column.name())
                else {
                    return residual("group key lacks typed physical schema");
                };
                if !signed_integer(schema_column.data_type()) {
                    return residual("group key requires exact signed integer type law");
                }
                Some(column.name())
            }
            _ => return residual("basic aggregate proof excludes complex grouping"),
        },
    };
    if grouped_key.is_some() && !query.group_rows_match_surviving_groups() {
        return residual("parser did not certify the grouped terminal row shape");
    }
    let mut groups = BTreeMap::<Option<i128>, Vec<usize>>::new();
    if grouped_key.is_none() {
        groups.insert(None, (0..snapshot.rows().len()).collect());
    } else {
        for (index, row) in snapshot.rows().iter().enumerate() {
            let Some(value) = row.values().get(grouped_key.unwrap_or("")) else {
                return residual("source row lacks its group key");
            };
            let key = match numeric(value) {
                Ok(key) => key,
                Err(reason) => return residual(reason),
            };
            groups.entry(key).or_default().push(index);
        }
    }
    let mut terminal = Vec::new();
    let mut identities = Vec::new();
    let mut has_aggregate = false;
    for (key, members) in groups {
        let mut projected = BTreeMap::new();
        let mut aggregate_row_ids = BTreeMap::new();
        for output in query.output().columns() {
            let scalar = match output.expression() {
                Expression::Column(column)
                    if grouped_key == Some(column.name())
                        && matches_source(column, relation, source.alias()) =>
                {
                    key.map(|value| LiteralValue::Number(value.to_string()))
                        .unwrap_or(LiteralValue::Null)
                }
                Expression::AggregateFunction(function) => {
                    has_aggregate = true;
                    match aggregate_value(function, &members, snapshot, schema, relation, source.alias()) {
                        Ok((value, contributors)) => {
                            aggregate_row_ids.insert(output.name().to_string(), contributors);
                            value
                        }
                        Err(reason) => return residual(reason),
                    }
                }
                _ => return residual("output needs a direct group key or basic aggregate"),
            };
            if projected.insert(output.name().to_string(), scalar).is_some() {
                return residual("duplicate output column names prevent exact terminal identity");
            }
        }
        identities.push(PhysicalAggregateGroup {
            key,
            source_row_ids: members.iter().map(|index| format!("{relation}#{index}")).collect(),
            aggregate_row_ids,
        });
        terminal.push(projected);
    }
    // For GROUP BY on an empty bag the loop above has no groups. Detect
    // aggregate shape from the canonical output instead of requiring a row.
    if !has_aggregate
        && !query.output().columns().iter().any(|col| {
            matches!(col.expression(), Expression::AggregateFunction(_))
        })
    {
        return residual("query contains no basic aggregate expression");
    }
    let mut terminal_domains = BTreeMap::new();
    for output in query.output().columns() {
        let name = output.name();
        let mut exact_values = Vec::new();
        for row in &terminal {
            let Some(value) = row.get(name) else {
                return residual("terminal output identity missing");
            };
            if !exact_values.contains(value) {
                let literal_type = if matches!(value, LiteralValue::Null) {
                    LiteralType::Null
                } else {
                    LiteralType::Integer
                };
                exact_values.push(LiteralExpression::new(literal_type, value.clone()));
            }
        }
        terminal_domains.insert(name.to_string(), ValueDomain::set(SetMode::Include, exact_values));
    }
    PhysicalFiniteAggregateProof::Exact(Box::new(PhysicalFiniteAggregateExact {
        nodes: walker.nodes,
        source: relation.to_string(),
        grouped: grouped_key.is_some(),
        groups: identities,
        terminal,
        terminal_domains,
    }))
}
