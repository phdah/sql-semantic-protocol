//! Typed source-group witness plans for simple HAVING predicates.
//!
//! Witnesses are sufficient plans, not claims that arbitrary SQL expressions can be inverted.
//! Unsupported grouping and non-row-preserving inputs stay explicitly residual.

use crate::protocol::{
    AggregateArgument, Bound, ColumnRef, ComparisonOperator, Expression, GroupBy,
    GroupingExpression, LiteralExpression, LiteralType, LiteralValue, Output, Predicate,
    QueryStatement, SetMode, ValueDomain, ValueRange,
};

/// Aggregate computation whose source-row contributions a consumer must construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupAggregate {
    /// COUNT(*) counts all source rows, including rows with NULL values.
    CountRows,
    /// COUNT(column) counts only non-NULL source values.
    CountValues,
    /// SUM(column) adds non-NULL values, producing NULL for no contributors.
    Sum,
    /// MIN(column) chooses the least non-NULL value.
    Min,
    /// MAX(column) chooses the greatest non-NULL value.
    Max,
}

impl GroupAggregate {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CountRows => "count_rows",
            Self::CountValues => "count_values",
            Self::Sum => "sum",
            Self::Min => "min",
            Self::Max => "max",
        }
    }
}

/// A requirement on the non-NULL values contributing to one aggregate.
///
/// 'Every' and 'Some' are evaluated over the group after NULLs are excluded.
/// A case separately requires a nonzero contributor count when necessary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupValueTest {
    /// Every contributing value satisfies the comparison.
    Every {
        operator: ComparisonOperator,
        bound: LiteralExpression,
    },
    /// At least one contributing value satisfies the comparison.
    Some {
        operator: ComparisonOperator,
        bound: LiteralExpression,
    },
    /// The sum of non-NULL contributing values satisfies the comparison.
    Sum {
        operator: ComparisonOperator,
        bound: LiteralExpression,
    },
}

impl GroupValueTest {
    /// Return the test operator.
    pub fn operator(&self) -> ComparisonOperator {
        match self {
            Self::Every { operator, .. }
            | Self::Some { operator, .. }
            | Self::Sum { operator, .. } => *operator,
        }
    }

    /// Return the bound in this scalar comparison.
    pub fn bound(&self) -> &LiteralExpression {
        match self {
            Self::Every { bound, .. } | Self::Some { bound, .. } | Self::Sum { bound, .. } => bound,
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Every { .. } => "every",
            Self::Some { .. } => "some",
            Self::Sum { .. } => "sum",
        }
    }
}

/// One independently sufficient input-group construction.
///
/// A consumer must create a group with the indicated identity and exactly the
/// prescribed bounds; any unspecified upper bound is unbounded. When the
/// aggregate uses a column, non-NULL counts refer only to that column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupWitnessCase {
    min_rows: u64,
    max_rows: Option<u64>,
    min_non_null: u64,
    max_non_null: Option<u64>,
    tests: Vec<GroupValueTest>,
}

impl GroupWitnessCase {
    fn new(
        min_rows: u64,
        max_rows: Option<u64>,
        min_non_null: u64,
        max_non_null: Option<u64>,
        tests: Vec<GroupValueTest>,
    ) -> Self {
        Self {
            min_rows,
            max_rows,
            min_non_null,
            max_non_null,
            tests,
        }
    }
    /// Minimum number of rows with this group key.
    pub fn min_rows(&self) -> u64 {
        self.min_rows
    }
    /// Optional maximum number of rows with this group key.
    pub fn max_rows(&self) -> Option<u64> {
        self.max_rows
    }
    /// Minimum number of non-NULL values for the aggregate's argument.
    pub fn min_non_null(&self) -> u64 {
        self.min_non_null
    }
    /// Optional maximum number of non-NULL values for the aggregate's argument.
    pub fn max_non_null(&self) -> Option<u64> {
        self.max_non_null
    }
    /// Conjunctive contributor constraints; distinct cases are alternatives.
    pub fn tests(&self) -> &[GroupValueTest] {
        &self.tests
    }
}

/// Proof status for one direction of HAVING group membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupWitnessDirection {
    /// Each case guarantees the advertised membership direction; an empty list is impossible.
    Exact(Vec<GroupWitnessCase>),
    /// Source group construction is not proven safe.
    Residual { reason: &'static str },
}

/// Generator-consumable positive and negative obligations for an aggregate HAVING comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupWitness {
    boundary: Option<String>,
    group_keys: Vec<ColumnRef>,
    aggregate: Option<GroupAggregate>,
    distinct: bool,
    argument: Option<ColumnRef>,
    predicate: Option<(ComparisonOperator, LiteralExpression)>,
    qualifying: GroupWitnessDirection,
    rejected: GroupWitnessDirection,
}

impl GroupWitness {
    /// Input relation at which group obligations hold, physical or intermediate.
    /// The composed graph classifies the boundary before physical generation.
    pub fn boundary(&self) -> Option<&str> {
        self.boundary.as_deref()
    }
    /// Group key columns at the input relation boundary, in GROUP BY order.
    pub fn group_keys(&self) -> &[ColumnRef] {
        &self.group_keys
    }
    /// Aggregate reduction to perform over the input group.
    pub fn aggregate(&self) -> Option<GroupAggregate> {
        self.aggregate
    }
    /// Whether the aggregate applies DISTINCT to its arguments.
    pub fn distinct(&self) -> bool {
        self.distinct
    }
    /// Input relation column for COUNT(column), SUM, MIN, or MAX. None for COUNT(*).
    pub fn argument(&self) -> Option<&ColumnRef> {
        self.argument.as_ref()
    }
    /// The normalized aggregate-result comparison, when supported.
    pub fn predicate(&self) -> Option<(ComparisonOperator, &LiteralExpression)> {
        self.predicate
            .as_ref()
            .map(|(operator, bound)| (*operator, bound))
    }
    /// Sufficient constructions for groups which survive HAVING.
    pub fn qualifying(&self) -> &GroupWitnessDirection {
        &self.qualifying
    }
    /// Sufficient constructions for groups which are rejected by HAVING, including SQL UNKNOWN.
    pub fn rejected(&self) -> &GroupWitnessDirection {
        &self.rejected
    }
}

fn residual(reason: &'static str) -> GroupWitnessDirection {
    GroupWitnessDirection::Residual { reason }
}

fn negated(operator: ComparisonOperator) -> Option<ComparisonOperator> {
    match operator {
        ComparisonOperator::Eq => Some(ComparisonOperator::Neq),
        ComparisonOperator::Neq => Some(ComparisonOperator::Eq),
        ComparisonOperator::Lt => Some(ComparisonOperator::Gte),
        ComparisonOperator::Lte => Some(ComparisonOperator::Gt),
        ComparisonOperator::Gt => Some(ComparisonOperator::Lte),
        ComparisonOperator::Gte => Some(ComparisonOperator::Lt),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => None,
    }
}

fn integer_intervals(
    operator: ComparisonOperator,
    bound: u64,
    floor: u64,
) -> Vec<(u64, Option<u64>)> {
    let mut candidates = Vec::new();
    match operator {
        ComparisonOperator::Eq => candidates.push((bound, Some(bound))),
        ComparisonOperator::Neq => {
            if let Some(below) = bound.checked_sub(1) {
                candidates.push((0, Some(below)));
            }
            if let Some(above) = bound.checked_add(1) {
                candidates.push((above, None));
            }
        }
        ComparisonOperator::Lt => {
            if let Some(below) = bound.checked_sub(1) {
                candidates.push((0, Some(below)));
            }
        }
        ComparisonOperator::Lte => candidates.push((0, Some(bound))),
        ComparisonOperator::Gt => {
            if let Some(above) = bound.checked_add(1) {
                candidates.push((above, None));
            }
        }
        ComparisonOperator::Gte => candidates.push((bound, None)),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {}
    }
    candidates
        .into_iter()
        .filter_map(|(min, max)| {
            let min = min.max(floor);
            if max.is_some_and(|upper| upper < min) {
                None
            } else {
                Some((min, max))
            }
        })
        .collect()
}

fn count_cases(
    aggregate: GroupAggregate,
    operator: ComparisonOperator,
    bound: &LiteralExpression,
    grouped: bool,
) -> Option<Vec<GroupWitnessCase>> {
    let LiteralValue::Number(value) = bound.value() else {
        return None;
    };
    if bound.literal_type() != LiteralType::Integer {
        return None;
    }
    let threshold = value.parse::<u64>().ok()?;
    let floor = if grouped && aggregate == GroupAggregate::CountRows {
        1
    } else {
        0
    };
    let mut cases = Vec::new();
    for (min, max) in integer_intervals(operator, threshold, floor) {
        let case = if aggregate == GroupAggregate::CountRows {
            GroupWitnessCase::new(min, max, 0, None, Vec::new())
        } else {
            GroupWitnessCase::new(
                if grouped { min.max(1) } else { min },
                None,
                min,
                max,
                Vec::new(),
            )
        };
        cases.push(case);
    }
    Some(cases)
}

fn aggregate_tests(
    kind: GroupAggregate,
    operator: ComparisonOperator,
    bound: &LiteralExpression,
) -> Vec<Vec<GroupValueTest>> {
    use ComparisonOperator::{Eq, Gt, Gte, Lt, Lte, Neq};
    let every = |operator| GroupValueTest::Every {
        operator,
        bound: bound.clone(),
    };
    let some = |operator| GroupValueTest::Some {
        operator,
        bound: bound.clone(),
    };
    match kind {
        GroupAggregate::Sum => vec![vec![GroupValueTest::Sum {
            operator,
            bound: bound.clone(),
        }]],
        GroupAggregate::Min => match operator {
            Lt | Lte => vec![vec![some(operator)]],
            Gt | Gte => vec![vec![every(operator)]],
            Eq => vec![vec![every(Gte), some(Eq)]],
            Neq => vec![vec![some(Lt)], vec![every(Gt)]],
            ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                Vec::new()
            }
        },
        GroupAggregate::Max => match operator {
            Gt | Gte => vec![vec![some(operator)]],
            Lt | Lte => vec![vec![every(operator)]],
            Eq => vec![vec![every(Lte), some(Eq)]],
            Neq => vec![vec![some(Gt)], vec![every(Lt)]],
            ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                Vec::new()
            }
        },
        GroupAggregate::CountRows | GroupAggregate::CountValues => Vec::new(),
    }
}

fn value_cases(
    kind: GroupAggregate,
    operator: ComparisonOperator,
    bound: &LiteralExpression,
) -> Option<Vec<GroupWitnessCase>> {
    if !matches!(
        bound.literal_type(),
        LiteralType::Integer | LiteralType::Decimal
    ) {
        return None;
    }
    let tests = aggregate_tests(kind, operator, bound);
    Some(
        tests
            .into_iter()
            .map(|tests| GroupWitnessCase::new(1, None, 1, None, tests))
            .collect(),
    )
}

/// Analyze the normalized HAVING predicate without depending on sqlparser AST types.
///
/// Exact plans require one direct relation boundary, plain grouping columns
/// and an isolated aggregate comparison. The composed graph distinguishes external
/// physical relations from intermediate producer outputs. Realizing the latter
/// at physical sources requires additional upstream proof; the plans are local.
pub(crate) fn analyze(query: &QueryStatement) -> Option<GroupWitness> {
    let having = query.predicates().having_predicate()?;
    let source = match (query.sources(), query.dependencies()) {
        ([source], [dependency]) if source.name() == dependency && query.joins().is_empty() => {
            Some(source.name().to_string())
        }
        _ => None,
    };
    let group_keys = match query.aggregation().and_then(|a| a.group_by()) {
        Some(GroupBy::Expressions(grouping)) => grouping
            .iter()
            .map(|item| match item {
                GroupingExpression::Expression(Expression::Column(column)) => source
                    .as_ref()
                    .map(|table| ColumnRef::new(Some(table.clone()), column.name().to_string())),
                _ => None,
            })
            .collect::<Option<Vec<_>>>(),
        None => Some(Vec::new()),
        Some(GroupBy::All) => None,
    };
    let mut result = GroupWitness {
        boundary: source.clone(),
        group_keys: group_keys.clone().unwrap_or_default(),
        aggregate: None,
        distinct: false,
        argument: None,
        predicate: None,
        qualifying: residual("unsupported_having"),
        rejected: residual("unsupported_having"),
    };
    let Predicate::Comparison(compare) = having else {
        return Some(result);
    };
    let (function, op, bound) = match (compare.left(), compare.right()) {
        (Expression::AggregateFunction(function), Expression::Literal(bound)) => {
            (function, compare.operator(), bound.clone())
        }
        (Expression::Literal(bound), Expression::AggregateFunction(function)) => {
            (function, compare.operator().reversed(), bound.clone())
        }
        _ => return Some(result),
    };
    result.predicate = Some((op, bound.clone()));
    let kind = match function.name().to_ascii_uppercase().as_str() {
        "COUNT" if matches!(function.arguments(), [AggregateArgument::Wildcard]) => {
            GroupAggregate::CountRows
        }
        "COUNT"
            if matches!(
                function.arguments(),
                [AggregateArgument::Expression(Expression::Column(_))]
            ) =>
        {
            GroupAggregate::CountValues
        }
        "SUM" => GroupAggregate::Sum,
        "MIN" => GroupAggregate::Min,
        "MAX" => GroupAggregate::Max,
        _ => return Some(result),
    };
    result.aggregate = Some(kind);
    result.distinct = function.distinct();
    result.argument = match function.arguments() {
        [AggregateArgument::Expression(Expression::Column(column))] => source
            .as_ref()
            .map(|table| ColumnRef::new(Some(table.clone()), column.name().to_string())),
        [AggregateArgument::Wildcard] if kind == GroupAggregate::CountRows => None,
        _ => return Some(result),
    };
    if function.distinct() || function.filter().is_some() {
        result.qualifying = residual("distinct_or_filtered_aggregate");
        result.rejected = residual("distinct_or_filtered_aggregate");
        return Some(result);
    }
    if source.is_none()
        || group_keys.is_none()
        || query.predicates().where_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || query.set_operation().is_some()
        || !query.diagnostics().is_empty()
    {
        result.qualifying = residual("unproven_source_group_boundary");
        result.rejected = residual("unproven_source_group_boundary");
        return Some(result);
    }
    if matches!(
        kind,
        GroupAggregate::Sum | GroupAggregate::Min | GroupAggregate::Max
    ) && result.argument.is_none()
    {
        result.qualifying = residual("unsupported_aggregate_argument");
        result.rejected = residual("unsupported_aggregate_argument");
        return Some(result);
    }
    let Some(reversed) = negated(op) else {
        result.qualifying = residual("unsupported_comparison");
        result.rejected = residual("unsupported_comparison");
        return Some(result);
    };
    if kind == GroupAggregate::CountRows || kind == GroupAggregate::CountValues {
        let grouped = query.aggregation().and_then(|a| a.group_by()).is_some();
        match (
            count_cases(kind, op, &bound, grouped),
            count_cases(kind, reversed, &bound, grouped),
        ) {
            (Some(positive), Some(negative)) => {
                result.qualifying = GroupWitnessDirection::Exact(positive);
                result.rejected = GroupWitnessDirection::Exact(negative);
            }
            _ => {
                result.qualifying = residual("unsupported_count_bound");
                result.rejected = residual("unsupported_count_bound");
            }
        }
    } else {
        match (
            value_cases(kind, op, &bound),
            value_cases(kind, reversed, &bound),
        ) {
            (Some(positive), Some(mut negative)) => {
                // SQL SUM/MIN/MAX over only NULL inputs returns NULL. HAVING rejects UNKNOWN.
                negative.push(GroupWitnessCase::new(1, None, 0, Some(0), Vec::new()));
                result.qualifying = GroupWitnessDirection::Exact(positive);
                result.rejected = GroupWitnessDirection::Exact(negative);
            }
            _ => {
                result.qualifying = residual("unsupported_aggregate_bound");
                result.rejected = residual("unsupported_aggregate_bound");
            }
        }
    }
    Some(result)
}

/// Refine only a projected aggregate identical to the HAVING operand.
/// HAVING excludes SQL NULL, so a supported scalar comparison bounds its surviving result.
pub(crate) fn refine_output(query: &QueryStatement) -> Output {
    let Some(GroupWitness {
        predicate: Some((operator, bound)),
        aggregate: Some(kind),
        ..
    }) = query.group_witness()
    else {
        return query.output().clone();
    };
    let Some(Predicate::Comparison(compare)) = query.predicates().having_predicate() else {
        return query.output().clone();
    };
    let function = match (compare.left(), compare.right()) {
        (Expression::AggregateFunction(function), Expression::Literal(_))
        | (Expression::Literal(_), Expression::AggregateFunction(function)) => function,
        _ => return query.output().clone(),
    };
    let domain = if matches!(
        kind,
        GroupAggregate::CountRows | GroupAggregate::CountValues
    ) {
        let LiteralValue::Number(number) = bound.value() else {
            return query.output().clone();
        };
        let Ok(threshold) = number.parse::<u64>() else {
            return query.output().clone();
        };
        let grouped = query.aggregation().and_then(|a| a.group_by()).is_some();
        let floor = if *kind == GroupAggregate::CountRows && grouped {
            1
        } else {
            0
        };
        let ranges = integer_intervals(*operator, threshold, floor)
            .into_iter()
            .map(|(min, max)| {
                let min = LiteralExpression::new(
                    LiteralType::Integer,
                    LiteralValue::Number(min.to_string()),
                );
                let max = max.map(|max| {
                    Bound::new(
                        LiteralExpression::new(
                            LiteralType::Integer,
                            LiteralValue::Number(max.to_string()),
                        ),
                        true,
                    )
                });
                ValueRange::new(Some(Bound::new(min, true)), max)
            })
            .collect();
        ValueDomain::ranges(ranges)
    } else {
        // Without a numeric bound the warehouse may apply unknown coercions.
        // Do not claim a numerical aggregate output interval for string/NULL literals.
        if !matches!(bound.literal_type(), LiteralType::Integer | LiteralType::Decimal) {
            return query.output().clone();
        }
        match operator {
            ComparisonOperator::Eq => ValueDomain::set(SetMode::Include, vec![bound.clone()]),
            ComparisonOperator::Neq => ValueDomain::ranges(vec![
                ValueRange::new(None, Some(Bound::new(bound.clone(), false))),
                ValueRange::new(Some(Bound::new(bound.clone(), false)), None),
            ]),
            ComparisonOperator::Lt | ComparisonOperator::Lte => {
                ValueDomain::ranges(vec![ValueRange::new(
                    None,
                    Some(Bound::new(
                        bound.clone(),
                        *operator == ComparisonOperator::Lte,
                    )),
                )])
            }
            ComparisonOperator::Gt | ComparisonOperator::Gte => {
                ValueDomain::ranges(vec![ValueRange::new(
                    Some(Bound::new(
                        bound.clone(),
                        *operator == ComparisonOperator::Gte,
                    )),
                    None,
                )])
            }
            ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                return query.output().clone()
            }
        }
    };
    Output::new(query.output().columns().iter().cloned().map(|column| {
        if matches!(column.expression(), Expression::AggregateFunction(candidate) if candidate == function) {
            // Existing aggregate domains are Unknown for SUM/MIN/MAX; the HAVING bound is known.
            let domain = if matches!(column.domain(), ValueDomain::Unknown(_)) {
                domain.clone()
            } else {
                crate::domain::intersect_domains(column.domain(), &domain)
            };
            column.with_domain(domain)
        } else { column }
    }).collect())
}
