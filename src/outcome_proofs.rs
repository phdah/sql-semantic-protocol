//! Constructive final-output goals sourced from canonical SQL witness contracts.
//!
//! An exact row-membership witness alone is not a proof of final cardinality.
//! These plans additionally control whole physical input relations and explicitly
//! rule out unmodeled row-shaping, constraints and finite key domains.

use crate::bundle::{
    AnalysisBundle, ComposedJoinColumn, ComposedSemantics, RelationResolution, TransformationLayer,
};
use crate::constraints::ConstraintValue;
use crate::data_type::DataType;
use crate::group_witness::{GroupAggregate, GroupWitnessDirection};
use crate::join_witness::{JoinWitnessDirection, JoinWitnessShape};
use crate::outcome_goals::{OutcomeGoal, OutputValueCount};
use crate::protocol::{
    ColumnRef, ComparisonOperator, Expression, JoinKind, QueryStatement,
    SetMultiplicityRule, SetWitnessCase, SetWitnessDirection,
};
use crate::relation::RelationSchema;
use crate::window_witness::{WindowOrderKey, WindowWitnessDirection};

/// Exact source-column histogram to materialize for a direct projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceColumnValues {
    column: String,
    values: Vec<OutputValueCount>,
}

impl SourceColumnValues {
    /// Physical column receiving the complete requested values.
    pub fn column(&self) -> &str {
        &self.column
    }
    /// Typed frequencies, in canonical value order.
    pub fn values(&self) -> &[OutputValueCount] {
        &self.values
    }
}

/// Constructive, parser-independent obligations for one feasible output goal.
///
/// All other source columns can be assigned valid arbitrary values. The sequence
/// 0..count-1 is used for distinct integer keys where no values are supplied.
/// The caller must materialize each named source as its complete stated contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutcomeWitness {
    /// A source-free constant SELECT or a guaranteed one-row global aggregate.
    Singleton,
    /// All listed physical sources must be empty.
    EmptySources { relations: Vec<String> },
    /// Exactly the stated source rows, with complete typed column frequencies.
    SourceRows {
        relation: String,
        rows: u64,
        columns: Vec<SourceColumnValues>,
    },
    /// Two independent physical sources have exactly one matched row per key.
    /// Both equality columns receive the distinct integer keys 0..pairs-1.
    JoinPairs {
        left: ComposedJoinColumn,
        right: ComposedJoinColumn,
        pairs: u64,
    },
    /// Each group has a distinct integer key, the specified source-row count,
    /// and no other source rows. The GROUP BY/HAVING witness proves survival.
    Groups {
        relation: String,
        key: ColumnRef,
        groups: u64,
        rows_per_group: u64,
    },
    /// For a partitioned window, one row per distinct integer partition key.
    /// For an unpartitioned window, rows receive strictly ordered integer keys.
    /// Each row passes the proven ROW_NUMBER filter.
    Ranked {
        relation: String,
        partition_key: Option<ColumnRef>,
        order_by: Vec<WindowOrderKey>,
        rows: u64,
    },
    /// For every distinct tuple, apply these complete per-branch counts.
    /// If provided, `values` is the complete requested one-column output
    /// histogram; otherwise keys are 0..tuples-1.
    SetTuples {
        tuples: u64,
        case: SetWitnessCase,
        values: Vec<OutputValueCount>,
    },
}

/// Construct a whole-input witness for an explicitly supported class.
pub(crate) fn construct(
    bundle: &AnalysisBundle,
    layer: &TransformationLayer,
    query: &QueryStatement,
    goal: &OutcomeGoal,
) -> Option<OutcomeWitness> {
    let rows = goal.rows()?;
    let ComposedSemantics::Resolved(resolved) = layer.composed_semantics() else {
        return None;
    };
    if layer
        .write_kind()
        .is_some_and(|kind| !kind.fully_defines_relation())
        || !resolved.diagnostics().is_empty()
        || !external_inputs(bundle, layer)
        || !resolved.subquery_witnesses().is_empty()
        || !resolved.boolean_witnesses().is_empty()
    {
        return None;
    }

    if let Some(witness) = construct_set(bundle, query, goal, rows) {
        return Some(witness);
    }
    if let Some(witness) = construct_group(bundle, query, goal, rows) {
        return Some(witness);
    }
    if let Some(witness) = construct_rank(bundle, query, goal, rows) {
        return Some(witness);
    }
    if let Some(witness) = construct_join(bundle, layer, query, goal, rows) {
        return Some(witness);
    }
    construct_source(bundle, query, goal, rows)
}

fn external_inputs(bundle: &AnalysisBundle, layer: &TransformationLayer) -> bool {
    let edges = bundle
        .graph()
        .edges()
        .iter()
        .filter(|edge| edge.consumer_layer_id() == layer.id())
        .collect::<Vec<_>>();
    edges.len() == layer.consumes().len()
        && edges
            .iter()
            .all(|edge| edge.resolution() == RelationResolution::External)
}

fn unconstrained(bundle: &AnalysisBundle, relations: &[&str]) -> bool {
    relations.iter().all(|relation| {
        !bundle.relation_constraints().iter().any(|set| {
            set.relation() == *relation
                && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
        })
    })
}

fn schema<'a>(bundle: &'a AnalysisBundle, relation: &str) -> Option<&'a RelationSchema> {
    bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == relation)
}

fn integer_key(bundle: &AnalysisBundle, relation: &str, column: &str, count: u64) -> bool {
    let Some(schema) = schema(bundle, relation) else {
        return false;
    };
    let Some(column) = schema.columns().iter().find(|item| item.name() == column) else {
        return false;
    };
    match column.data_type() {
        DataType::SignedInteger { bits } => match bits {
            Some(0) => false,
            Some(width) if *width < 64 => count <= (1_u64 << (width - 1)),
            _ => count <= (i64::MAX as u64) + 1,
        },
        DataType::UnsignedInteger { bits } => match bits {
            Some(0) => false,
            Some(width) if *width < 64 => count <= (1_u64 << width),
            _ => true,
        },
        _ => false,
    }
}

fn value_fits(bundle: &AnalysisBundle, relation: &str, column: &str, value: &ConstraintValue) -> bool {
    let Some(schema) = schema(bundle, relation) else { return false };
    let Some(column) = schema.columns().iter().find(|item| item.name() == column) else { return false };
    match (column.data_type(), value) {
        (DataType::SignedInteger { bits }, ConstraintValue::Integer(value)) => {
            bits.is_none_or(|width| width >= 64 || (width > 0 && {
                let half = 1_i128 << (width - 1);
                i128::from(*value) >= -half && i128::from(*value) < half
            }))
        }
        (DataType::UnsignedInteger { bits }, ConstraintValue::UnsignedInteger(value)) => {
            bits.is_none_or(|width| width >= 64 || (width > 0 && u128::from(*value) < (1_u128 << width)))
        }
        (DataType::UnsignedInteger { bits }, ConstraintValue::Integer(value)) if *value >= 0 => {
            bits.is_none_or(|width| width >= 64 || (width > 0 && (*value as u128) < (1_u128 << width)))
        }
        (DataType::SignedInteger { .. } | DataType::UnsignedInteger { .. }, ConstraintValue::Null) => true,
        _ => false,
    }
}

fn projections_are_plain(query: &QueryStatement) -> bool {
    query.output().columns().iter().all(|column| {
        matches!(
            column.expression(),
            Expression::Column(_) | Expression::Literal(_)
        )
    })
}

fn construct_source(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    goal: &OutcomeGoal,
    rows: u64,
) -> Option<OutcomeWitness> {
    if !query.plain_goal_output_shape()
        || query.joins().len() != 0
        || query.sources().len() != 1
        || query.dependencies().len() != 1
        || !projections_are_plain(query)
        || !query.diagnostics().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || goal.groups().is_some()
    {
        return None;
    }
    let relation = query.dependencies().first()?.as_str();
    if rows == 0 && goal.distributions().iter().all(|d| d.values().is_empty()) {
        return Some(OutcomeWitness::EmptySources {
            relations: vec![relation.to_string()],
        });
    }
    if !unconstrained(bundle, &[relation]) || schema(bundle, relation).is_none() {
        return None;
    }
    let mut columns = Vec::new();
    for distribution in goal.distributions() {
        let projected = query
            .output()
            .columns()
            .iter()
            .find(|item| item.name() == distribution.column())?;
        let source = projected.plain_copy_source()?;
        if source.relation() != relation {
            return None;
        }
        if distribution
            .values()
            .iter()
            .any(|entry| !value_fits(bundle, relation, source.column(), entry.value()))
        {
            return None;
        }
        if columns
            .iter()
            .any(|item: &SourceColumnValues| item.column == source.column())
        {
            return None;
        }
        columns.push(SourceColumnValues {
            column: source.column().to_string(),
            values: distribution.values().to_vec(),
        });
    }
    Some(OutcomeWitness::SourceRows {
        relation: relation.to_string(),
        rows,
        columns,
    })
}

fn construct_group(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    goal: &OutcomeGoal,
    rows: u64,
) -> Option<OutcomeWitness> {
    if !query.group_rows_match_surviving_groups()
        || !goal.distributions().is_empty()
        || !query.diagnostics().is_empty()
        || !query.joins().is_empty()
        || query.sources().len() != 1
        || query.dependencies().len() != 1
        || query.predicates().where_predicate().is_some()
        || query.output().columns().iter().any(|c| {
            !matches!(
                c.expression(),
                Expression::Column(_) | Expression::AggregateFunction(_)
            )
        })
        || goal.groups().is_some_and(|groups| groups != rows)
    {
        return None;
    }
    let relation = query.dependencies().first()?.as_str();
    if rows == 0 {
        return Some(OutcomeWitness::EmptySources {
            relations: vec![relation.to_string()],
        });
    }
    if !unconstrained(bundle, &[relation]) {
        return None;
    }
    let group_key = match query.aggregation()?.group_by()? {
        crate::protocol::GroupBy::Expressions(grouping) if grouping.len() == 1 => {
            match &grouping[0] {
                crate::protocol::GroupingExpression::Expression(Expression::Column(column)) => {
                    column
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let key = query
        .output()
        .columns()
        .iter()
        .filter_map(|output| output.plain_copy_source())
        .find(|source| source.column() == group_key.name() && source.relation() == relation)?;
    if !integer_key(bundle, relation, key.column(), rows) {
        return None;
    }

    let mut rows_per_group = 1;
    if query.predicates().having_predicate().is_some() {
        let witness = query.group_witness()?;
        if witness.boundary() != Some(relation)
            || witness.aggregate() != Some(GroupAggregate::CountRows)
            || witness.group_keys().len() != 1
            || witness.group_keys()[0].name() != key.column()
        {
            return None;
        }
        let GroupWitnessDirection::Exact(cases) = witness.qualifying() else {
            return None;
        };
        rows_per_group = cases
            .iter()
            .filter_map(|case| {
                let min_rows = case.min_rows().max(1);
                if case.max_rows().is_some_and(|max| min_rows > max) {
                    None
                } else {
                    Some(min_rows)
                }
            })
            .min()?;
    }
    rows.checked_mul(rows_per_group)?;
    Some(OutcomeWitness::Groups {
        relation: relation.to_string(),
        key: ColumnRef::new(Some(relation.to_string()), key.column().to_string()),
        groups: rows,
        rows_per_group,
    })
}

fn construct_rank(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    goal: &OutcomeGoal,
    rows: u64,
) -> Option<OutcomeWitness> {
    if !query.ranked_goal_output_shape()
        || !goal.distributions().is_empty()
        || goal.groups().is_some()
        || query.sources().len() != 1
        || query.dependencies().len() != 1
        || !query.diagnostics().is_empty()
        || query.output().columns().iter().any(|c| {
            !matches!(
                c.expression(),
                Expression::Column(_) | Expression::WindowFunction(_)
            )
        })
    {
        return None;
    }
    let relation = query.dependencies().first()?.as_str();
    let witness = query.window_witness()?;
    if witness.boundary() != Some(relation) {
        return None;
    }
    if rows == 0 {
        return Some(OutcomeWitness::EmptySources {
            relations: vec![relation.to_string()],
        });
    }
    if !unconstrained(bundle, &[relation])
        || witness.order_by().len() != 1
        || !integer_key(
            bundle,
            relation,
            witness.order_by()[0].column().name(),
            rows,
        )
    {
        return None;
    }
    let WindowWitnessDirection::Exact(case) = witness.qualifying() else {
        return None;
    };
    if case.min_preceding() != 0 {
        return None;
    }
    let limit = witness.limit()?;
    if limit == 0 {
        return None;
    }
    let partition_key = match witness.partition_by() {
        [] if rows <= limit => None,
        [key] if integer_key(bundle, relation, key.name(), rows) => Some(key.clone()),
        _ => return None,
    };
    Some(OutcomeWitness::Ranked {
        relation: relation.to_string(),
        partition_key,
        order_by: witness.order_by().to_vec(),
        rows,
    })
}

fn construct_join(
    bundle: &AnalysisBundle,
    layer: &TransformationLayer,
    query: &QueryStatement,
    goal: &OutcomeGoal,
    rows: u64,
) -> Option<OutcomeWitness> {
    if !query.plain_goal_output_shape()
        || !goal.distributions().is_empty()
        || goal.groups().is_some()
        || query.joins().len() != 1
        || query.sources().len() != 2
        || query.dependencies().len() != 2
        || !query.diagnostics().is_empty()
        || !projections_are_plain(query)
        || query.aggregation().is_some()
        || query.set_operation().is_some()
    {
        return None;
    }
    let ComposedSemantics::Resolved(resolved) = layer.composed_semantics() else {
        return None;
    };
    let [witness] = resolved.join_witnesses() else {
        return None;
    };
    if witness.origin_layer_id() != layer.id()
        || !matches!(
            witness.kind(),
            JoinKind::Inner | JoinKind::Left | JoinKind::Right | JoinKind::Full
        )
        || witness.comparison() != Some(ComparisonOperator::Eq)
        || !matches!(witness.qualifying(), JoinWitnessDirection::Exact(cases)
            if cases.iter().any(|case| case.shape() == JoinWitnessShape::Matched))
    {
        return None;
    }
    let left = witness.left()?;
    let right = witness.right()?;
    if left.relation() == right.relation() {
        return None;
    }
    if rows == 0 {
        return Some(OutcomeWitness::EmptySources {
            relations: vec![left.relation().to_string(), right.relation().to_string()],
        });
    }
    if !unconstrained(bundle, &[left.relation(), right.relation()])
        || !integer_key(bundle, left.relation(), left.column(), rows)
        || !integer_key(bundle, right.relation(), right.column(), rows)
    {
        return None;
    }
    Some(OutcomeWitness::JoinPairs {
        left: left.clone(),
        right: right.clone(),
        pairs: rows,
    })
}

fn construct_set(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    goal: &OutcomeGoal,
    rows: u64,
) -> Option<OutcomeWitness> {
    if goal.groups().is_some()
        || !query.diagnostics().is_empty()
        || query.output().columns().len() != 1
    {
        return None;
    }
    let operation = query.set_operation()?;
    let branches = operation.branches();
    if branches.len() != 2 {
        return None;
    }
    let mut relations = Vec::new();
    for branch in branches {
        if branch.output().columns().len() != 1
            || !matches!(
                branch.output().columns()[0].expression(),
                Expression::Column(_)
            )
            || branch.dependencies().len() != 1
            || branch.predicates().where_predicate().is_some()
            || branch.predicates().having_predicate().is_some()
            || branch.predicates().qualify_predicate().is_some()
            || !branch.condition_exactness().is_exact()
        {
            return None;
        }
        let boundary = branch.witness_boundary()?;
        if boundary.is_intermediate()
            || boundary.tuple_columns().len() != 1
            || branch.dependencies()[0] != boundary.relation()
        {
            return None;
        }
        relations.push(boundary.relation());
    }
    if relations[0] == relations[1] {
        return None;
    }
    let (positive, negative) = operation.witness_directions();
    if rows == 0 {
        if let SetWitnessDirection::Exact(cases) = negative {
            if cases.iter().any(|case| {
                case.output_tuple_count() == 0
                    && case
                        .obligations()
                        .iter()
                        .all(|obligation| obligation.matching_tuple_count() == 0)
            }) {
                return Some(OutcomeWitness::EmptySources {
                    relations: relations.iter().map(|r| (*r).to_string()).collect(),
                });
            }
        }
        return None;
    }
    if !unconstrained(bundle, &relations) {
        return None;
    }
    let values = match goal.distributions() {
        [] => Vec::new(),
        [distribution] if distribution.column() == query.output().columns()[0].name() => {
            if operation.multiplicity_rule().is_some_and(|rule| {
                matches!(
                    rule,
                    SetMultiplicityRule::UnionDistinct
                        | SetMultiplicityRule::IntersectDistinct
                        | SetMultiplicityRule::ExceptDistinct
                )
            }) && distribution.values().iter().any(|item| item.rows() > 1)
            {
                return None;
            }
            distribution.values().to_vec()
        }
        _ => return None,
    };
    for branch in branches {
        let boundary = branch.witness_boundary()?;
        let column = &boundary.tuple_columns()[0];
        if !integer_key(bundle, boundary.relation(), column, rows) {
            return None;
        }
        if values
            .iter()
            .any(|item| !value_fits(bundle, boundary.relation(), column, item.value()))
        {
            return None;
        }
    }
    let SetWitnessDirection::Exact(cases) = positive else {
        return None;
    };
    let case = cases
        .into_iter()
        .find(|case| case.output_tuple_count() == 1 && case.obligations().len() == 2)?;
    Some(OutcomeWitness::SetTuples {
        tuples: rows,
        case,
        values,
    })
}

/// Max cardinality provable from an unpartitioned, directly filtered ROW_NUMBER.
pub(crate) fn rank_upper_bound(query: &QueryStatement) -> Option<u64> {
    if !query.ranked_goal_output_shape() {
        return None;
    }
    let witness = query.window_witness()?;
    if !witness.partition_by().is_empty() {
        return None;
    }
    match witness.qualifying() {
        WindowWitnessDirection::Impossible => Some(0),
        WindowWitnessDirection::Exact(_) => match witness.operator()? {
            ComparisonOperator::Eq | ComparisonOperator::Lte => witness.limit(),
            _ => None,
        },
        WindowWitnessDirection::Residual { .. } => None,
    }
}
