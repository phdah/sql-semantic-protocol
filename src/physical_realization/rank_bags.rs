//! TASK-125: closed-world portable ranking over complete physical source bags.
//!
//! Ranking uses canonical window expressions, not SQL reparsing. Peers have
//! deterministic RANK/DENSE_RANK but no deterministic ROW_NUMBER identity.

use super::write_states::{physical_closed_world_schema_plan, PhysicalWriteStatePlan};
use super::*;
use crate::protocol::{
    ComparisonOperator, LiteralExpression, LiteralType, LiteralValue, SetMode, ValueDomain,
    WindowFunctionExpression, WindowSpecification,
};
use std::cmp::Ordering;

/// Complete identity and ranking outcome for one supplied physical row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRankRow {
    /// Physical source row identity, never collapsed for SQL duplicates.
    pub source_row_id: String,
    /// ROW_NUMBER only for partitions with unique ordering tuples; peers are ambiguous.
    pub row_number: Option<u64>,
    /// One plus the number of preceding rows, including preceding peers.
    pub rank: u64,
    /// One plus the number of distinct preceding peer groups.
    pub dense_rank: u64,
    /// TRUE when the row survives an optional QUALIFY predicate.
    pub survives: bool,
}

/// Complete source-backed ranking output with exact finite terminal domains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteRankExact {
    /// Canonical physical dependency graph.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Physical table whose whole bag was validated.
    pub source: String,
    /// Every source identity exactly once, including QUALIFY rejected rows.
    pub ranked_rows: Vec<PhysicalRankRow>,
    /// Complete SQL terminal bag, independent of unspecified output row order.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact finite output domains, including nullable fields and rank bounds.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// Distinguishes complete physical proof from contradictory or missing evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteRankProof {
    /// Exact closed-world source identities and output bag.
    Exact(Box<PhysicalFiniteRankExact>),
    /// Supplied complete source contradicts enforced physical schema.
    Impossible { reason: String },
    /// Unsupported SQL/session/order law prevents a proof.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalFiniteRankProof {
    PhysicalFiniteRankProof::Residual {
        reason: reason.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RankFunction {
    RowNumber,
    Rank,
    DenseRank,
}

fn rank_function(window: &WindowFunctionExpression) -> Option<RankFunction> {
    if !window.function().arguments().is_empty() || window.function().distinct() {
        return None;
    }
    match window.function().name().to_ascii_lowercase().as_str() {
        "row_number" => Some(RankFunction::RowNumber),
        "rank" => Some(RankFunction::Rank),
        "dense_rank" => Some(RankFunction::DenseRank),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RankSort {
    pub(super) column: String,
    pub(super) ascending: bool,
    pub(super) nulls_first: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RankKeys {
    pub(super) partition: Vec<String>,
    pub(super) sort: Vec<RankSort>,
}

fn source_column(
    expression: &Expression,
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
) -> Result<String, &'static str> {
    let Expression::Column(column) = expression else {
        return Err("rank keys must be physical source columns");
    };
    if column
        .relation()
        .is_some_and(|name| name != source.name() && Some(name) != source.alias())
    {
        return Err("rank key refers to an unresolved relation");
    }
    let Some(declared) = schema.columns().iter().find(|c| c.name() == column.name()) else {
        return Err("rank key is absent from authoritative source schema");
    };
    if !super::aggregate_bags::signed_integer(declared.data_type()) {
        return Err("rank key lacks portable signed integer/NULL comparison semantics");
    }
    Ok(column.name().to_string())
}

pub(super) fn rank_keys(
    spec: &WindowSpecification,
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
    allow_frame: bool,
) -> Result<RankKeys, &'static str> {
    if (spec.frame().is_some() && !allow_frame) || spec.order_by().is_empty() {
        return Err("ranking requires ORDER BY without unproved window frames");
    }
    let mut partition = Vec::new();
    for key in spec.partition_by() {
        partition.push(source_column(key, source, schema)?);
    }
    let mut sort = Vec::new();
    for key in spec.order_by() {
        let Some(nulls_first) = key.nulls_first() else {
            return Err("implicit NULL ordering depends on dialect or session");
        };
        sort.push(RankSort {
            column: source_column(key.expression(), source, schema)?,
            ascending: key.ascending().unwrap_or(true),
            nulls_first,
        });
    }
    Ok(RankKeys { partition, sort })
}

fn numeric(value: &LiteralValue) -> Result<Option<i128>, &'static str> {
    super::aggregate_bags::numeric(value)
}

fn rank_filter(
    predicate: &Predicate,
    spec: &WindowSpecification,
) -> Result<(RankFunction, ComparisonOperator, i128), &'static str> {
    let Predicate::Comparison(compare) = predicate else {
        return Err("QUALIFY requires a single ranking comparison");
    };
    let (window, operator, literal) = match (compare.left(), compare.right()) {
        (Expression::WindowFunction(window), Expression::Literal(literal)) => {
            (window, compare.operator(), literal)
        }
        (Expression::Literal(literal), Expression::WindowFunction(window)) => {
            (window, compare.operator().reversed(), literal)
        }
        _ => return Err("QUALIFY does not compare a canonical rank and integer"),
    };
    if window.window() != spec {
        return Err("QUALIFY ranking window differs from the projected ranking window");
    }
    if literal.literal_type() != LiteralType::Integer {
        return Err("QUALIFY rank threshold is not an integer");
    }
    let Some(value) = numeric(literal.value())? else {
        return Err("QUALIFY rank threshold is NULL");
    };
    Ok((
        rank_function(window).ok_or("unsupported QUALIFY ranking function")?,
        operator,
        value,
    ))
}

fn qualifies(rank: i128, op: ComparisonOperator, threshold: i128) -> bool {
    match op {
        ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => rank == threshold,
        ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => rank != threshold,
        ComparisonOperator::Lt => rank < threshold,
        ComparisonOperator::Lte => rank <= threshold,
        ComparisonOperator::Gt => rank > threshold,
        ComparisonOperator::Gte => rank >= threshold,
    }
}

pub(super) fn key_value(row: &PhysicalWriteRow, key: &str) -> Result<Option<i128>, &'static str> {
    numeric(
        row.values()
            .get(key)
            .ok_or("rank key absent from source row")?,
    )
}

pub(super) fn compare_key(a: Option<i128>, b: Option<i128>, key: &RankSort) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => {
            if key.nulls_first {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
        (Some(_), None) => {
            if key.nulls_first {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        }
        (Some(left), Some(right)) => {
            let order = left.cmp(&right);
            if key.ascending {
                order
            } else {
                order.reverse()
            }
        }
    }
}

pub(super) fn exact_domain(values: impl Iterator<Item = LiteralValue>) -> ValueDomain {
    let mut unique = Vec::<LiteralExpression>::new();
    for value in values {
        if unique.iter().any(|existing| existing.value() == &value) {
            continue;
        }
        let kind = match &value {
            LiteralValue::Null => LiteralType::Null,
            LiteralValue::Number(number) if number.contains('.') => LiteralType::Decimal,
            LiteralValue::Number(_) => LiteralType::Integer,
            LiteralValue::Text(_) => LiteralType::String,
            LiteralValue::Boolean(_) => LiteralType::Boolean,
        };
        unique.push(LiteralExpression::new(kind, value));
    }
    ValueDomain::set(SetMode::Include, unique)
}

/// Prove portable ROW_NUMBER, RANK and DENSE_RANK against an entire attested
/// direct source. Tied ROW_NUMBER identity is residual, never source-order
/// guessed. Peer-aware rank and dense rank do not depend on tie order.
pub fn physical_finite_rank_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteRankProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteRankProof::Impossible { reason }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let [snapshot] = snapshots else {
        return residual("ranking requires one complete physical source bag");
    };
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("unknown rank target layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("rank target has no canonical query");
    };
    let [source] = query.sources() else {
        return residual("ranking requires a direct single source");
    };
    if snapshot.relation() != source.name()
        || !walker.sources.contains(source.name())
        || !query.joins().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
        || (query.predicates().qualify_predicate().is_some() && !query.ranked_goal_output_shape())
        || (query.predicates().qualify_predicate().is_none() && !query.plain_goal_output_shape())
    {
        return residual("query has unproved ranking row-shaping or source identity");
    }
    let Some(schema) = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == source.name())
    else {
        return residual("ranking needs authoritative physical source types");
    };

    let mut spec = None::<&WindowSpecification>;
    let mut seen_names = BTreeSet::new();
    let mut contains_row_number = false;
    let mut has_rank = false;
    for column in query.output().columns() {
        if !seen_names.insert(column.name()) {
            return residual("duplicate ranking output column names");
        }
        match column.expression() {
            Expression::Column(expr) => {
                if expr
                    .relation()
                    .is_some_and(|name| name != source.name() && Some(name) != source.alias())
                    || !schema
                        .columns()
                        .iter()
                        .any(|schema_col| schema_col.name() == expr.name())
                {
                    return residual("unproved direct ranking projection");
                }
            }
            Expression::WindowFunction(window) => {
                let Some(kind) = rank_function(window) else {
                    return residual("unsupported ranking function or arguments");
                };
                if spec.is_some_and(|earlier| earlier != window.window()) {
                    return residual("multiple incompatible ranking window specifications");
                }
                spec = Some(window.window());
                has_rank = true;
                contains_row_number |= kind == RankFunction::RowNumber;
            }
            _ => return residual("rank terminal contains unproved computed output"),
        }
    }
    if !has_rank {
        return residual("rank query has no supported ranking function");
    }
    let Some(spec) = spec else {
        return residual("missing ranking window specification");
    };
    let keys = match rank_keys(spec, source, schema, false) {
        Ok(value) => value,
        Err(reason) => return residual(reason),
    };
    let filter = match query.predicates().qualify_predicate() {
        Some(predicate) => match rank_filter(predicate, spec) {
            Ok(value) => {
                contains_row_number |= value.0 == RankFunction::RowNumber;
                Some(value)
            }
            Err(reason) => return residual(reason),
        },
        None => None,
    };
    // SQL output rank values cannot fit their i128 representation in a
    // pathological source bag with more than i128::MAX rows.
    if i128::try_from(snapshot.rows().len()).is_err() {
        return residual("ranking cardinality exceeds signed domain");
    }
    let mut partitions = BTreeMap::<Vec<Option<i128>>, Vec<(usize, Vec<Option<i128>>)>>::new();
    for (index, row) in snapshot.rows().iter().enumerate() {
        let partition = match keys
            .partition
            .iter()
            .map(|key| key_value(row, key))
            .collect()
        {
            Ok(values) => values,
            Err(reason) => return residual(reason),
        };
        let order = match keys
            .sort
            .iter()
            .map(|key| key_value(row, &key.column))
            .collect()
        {
            Ok(values) => values,
            Err(reason) => return residual(reason),
        };
        partitions
            .entry(partition)
            .or_default()
            .push((index, order));
    }
    let mut ranked = vec![None; snapshot.rows().len()];
    for rows in partitions.values_mut() {
        rows.sort_by(|(_, a), (_, b)| {
            a.iter()
                .zip(b.iter())
                .zip(&keys.sort)
                .map(|((left, right), sort)| compare_key(*left, *right, sort))
                .find(|result| *result != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        });
        let has_peers = rows.windows(2).any(|pair| pair[0].1 == pair[1].1);
        if has_peers && contains_row_number {
            return residual("ROW_NUMBER identity among equal ORDER BY peers is nondeterministic");
        }
        let mut first_peer: usize = 0;
        let mut distinct_peers: u64 = 0;
        for index in 0..rows.len() {
            if index == 0 || rows[index].1 != rows[index - 1].1 {
                first_peer = index;
                distinct_peers += 1;
            }
            let rank = match u64::try_from(first_peer + 1) {
                Ok(value) => value,
                Err(_) => return residual("rank exceeds u64"),
            };
            let row_number = match u64::try_from(index + 1) {
                Ok(value) => value,
                Err(_) => return residual("row number exceeds u64"),
            };
            let selected = filter.is_none_or(|(kind, operator, threshold)| {
                let value = match kind {
                    RankFunction::RowNumber => row_number,
                    RankFunction::Rank => rank,
                    RankFunction::DenseRank => distinct_peers,
                };
                qualifies(i128::from(value), operator, threshold)
            });
            ranked[rows[index].0] = Some(PhysicalRankRow {
                source_row_id: format!("{}#{}", source.name(), rows[index].0),
                row_number: (!has_peers).then_some(row_number),
                rank,
                dense_rank: distinct_peers,
                survives: selected,
            });
        }
    }
    let Some(ranked_rows) = ranked.into_iter().collect::<Option<Vec<_>>>() else {
        return residual("incomplete ranking row assignment");
    };
    let mut terminal = Vec::new();
    for (row, rank) in snapshot.rows().iter().zip(&ranked_rows) {
        if !rank.survives {
            continue;
        }
        let mut projection = BTreeMap::new();
        for column in query.output().columns() {
            let value = match column.expression() {
                Expression::Column(expr) => match row.values().get(expr.name()) {
                    Some(value) => value.clone(),
                    None => return residual("projected source field absent"),
                },
                Expression::WindowFunction(window) => {
                    let value = match rank_function(window) {
                        Some(RankFunction::RowNumber) => match rank.row_number {
                            Some(value) => value,
                            None => return residual("ROW_NUMBER peer identity is not exact"),
                        },
                        Some(RankFunction::Rank) => rank.rank,
                        Some(RankFunction::DenseRank) => rank.dense_rank,
                        None => return residual("unsupported ranking output"),
                    };
                    LiteralValue::Number(value.to_string())
                }
                _ => return residual("unsupported ranking output shape"),
            };
            projection.insert(column.name().to_string(), value);
        }
        terminal.push(projection);
    }
    let mut domains = BTreeMap::new();
    for column in query.output().columns() {
        domains.insert(
            column.name().to_string(),
            exact_domain(
                terminal
                    .iter()
                    .filter_map(|row| row.get(column.name()).cloned()),
            ),
        );
    }
    PhysicalFiniteRankProof::Exact(Box::new(PhysicalFiniteRankExact {
        nodes: walker.nodes,
        source: source.name().to_string(),
        ranked_rows,
        terminal,
        terminal_domains: domains,
    }))
}
