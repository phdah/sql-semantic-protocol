//! Generator-consumable ROW_NUMBER partition and rank witness obligations.
//!
//! A witness targets the candidate input row at one relation boundary. Earlier rows
//! share its partition key and precede it under the stated strict ordering. This
//! avoids assuming which row wins a tie or a dialect's default NULL placement.

use crate::protocol::{
    Bound, ColumnRef, ComparisonOperator, Expression, LiteralExpression, LiteralType, LiteralValue,
    Output, Predicate, QueryStatement, SourceRelation, ValueDomain, ValueRange,
};

/// Order key at the controllable source boundary, in window ORDER BY priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOrderKey {
    column: ColumnRef,
    ascending: bool,
    nulls_first: bool,
}

impl WindowOrderKey {
    /// Physical or intermediate input column being ordered.
    pub fn column(&self) -> &ColumnRef {
        &self.column
    }
    /// True for ascending ORDER BY.
    pub fn ascending(&self) -> bool {
        self.ascending
    }
    /// Explicit placement of NULL values.
    pub fn nulls_first(&self) -> bool {
        self.nulls_first
    }
}

/// Sufficient source rows preceding one candidate within its partition.
///
/// Construct one candidate and `min_preceding..=max_preceding` strictly ordered
/// earlier rows with equal partition keys. The maximum is optional (unbounded).
/// Distinct order tuples are required, so execution never depends on tie breaking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRankCase {
    min_preceding: u64,
    max_preceding: Option<u64>,
}

impl WindowRankCase {
    fn new(min_preceding: u64, max_preceding: Option<u64>) -> Self {
        Self {
            min_preceding,
            max_preceding,
        }
    }
    /// Minimum number of rows ordered before the candidate.
    pub fn min_preceding(&self) -> u64 {
        self.min_preceding
    }
    /// Inclusive maximum; None means no upper bound.
    pub fn max_preceding(&self) -> Option<u64> {
        self.max_preceding
    }
}

/// Whether the candidate can be constructed to pass or fail a rank filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowWitnessDirection {
    /// The stated source-row obligation is sufficient for this classification.
    Exact(WindowRankCase),
    /// No candidate can have a rank in the requested range.
    Impossible,
    /// The SQL/window behavior cannot be proven from the supported evidence.
    Residual { reason: &'static str },
}

/// Typed, source-independent witness contract for one ROW_NUMBER rank predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowWitness {
    boundary: Option<String>,
    partition_by: Vec<ColumnRef>,
    order_by: Vec<WindowOrderKey>,
    operator: Option<ComparisonOperator>,
    limit: Option<u64>,
    qualifying: WindowWitnessDirection,
    rejected: WindowWitnessDirection,
}

impl WindowWitness {
    /// Single controllable input relation for the window partition, if proven.
    pub fn boundary(&self) -> Option<&str> {
        self.boundary.as_deref()
    }
    /// Physical or intermediate keys whose values must match the candidate.
    pub fn partition_by(&self) -> &[ColumnRef] {
        &self.partition_by
    }
    /// Strict order required for the candidate and its preceding witnesses.
    pub fn order_by(&self) -> &[WindowOrderKey] {
        &self.order_by
    }
    /// Supported ranking comparison operator.
    pub fn operator(&self) -> Option<ComparisonOperator> {
        self.operator
    }
    /// Right-hand integer ranking threshold.
    pub fn limit(&self) -> Option<u64> {
        self.limit
    }
    /// Constructible candidate which survives QUALIFY or a proven rank filter.
    pub fn qualifying(&self) -> &WindowWitnessDirection {
        &self.qualifying
    }
    /// Constructible candidate which fails QUALIFY or a proven rank filter.
    pub fn rejected(&self) -> &WindowWitnessDirection {
        &self.rejected
    }
    pub(crate) fn is_exact(&self) -> bool {
        !matches!(self.qualifying, WindowWitnessDirection::Residual { .. })
            && !matches!(self.rejected, WindowWitnessDirection::Residual { .. })
    }
}

fn residual(reason: &'static str) -> WindowWitnessDirection {
    WindowWitnessDirection::Residual { reason }
}

fn source_column(
    column: &crate::protocol::ColumnExpression,
    source: &SourceRelation,
) -> Option<ColumnRef> {
    if column
        .relation()
        .is_some_and(|name| name != source.name() && Some(name) != source.alias())
    {
        return None;
    }
    Some(ColumnRef::new(
        Some(source.name().to_string()),
        column.name().to_string(),
    ))
}

/// Analyze a direct QUALIFY comparison against one ROW_NUMBER result.
///
/// The required strict, distinct ordering is a generator obligation, not an
/// assertion that arbitrary existing source rows contain no ties.
pub(crate) fn analyze(query: &QueryStatement) -> Option<WindowWitness> {
    analyze_predicate(query, query.predicates().qualify_predicate()?)
}

/// Analyze a projected rank value filtered by an enclosing WHERE scope.
/// The caller must prove a simple one-to-one nested-query boundary.
pub(crate) fn analyze_projected(
    inner: &QueryStatement,
    filter: &Predicate,
    outer_source: &SourceRelation,
) -> Option<(WindowWitness, String)> {
    // Earlier row-set shaping invalidates predecessor cardinalities at this boundary.
    if inner.predicates().qualify_predicate().is_some() || !inner.condition_exactness().is_exact() {
        return None;
    }
    let Predicate::Comparison(compare) = filter else {
        return None;
    };
    let (column, operator, bound) = match (compare.left(), compare.right()) {
        (Expression::Column(column), Expression::Literal(bound)) => {
            (column, compare.operator(), bound)
        }
        (Expression::Literal(bound), Expression::Column(column)) => {
            (column, compare.operator().reversed(), bound)
        }
        _ => return None,
    };
    if column
        .relation()
        .is_some_and(|name| name != outer_source.name() && Some(name) != outer_source.alias())
    {
        return None;
    }
    let output = inner
        .output()
        .columns()
        .iter()
        .find(|item| item.name() == column.name())?;
    let Expression::WindowFunction(window) = output.expression() else {
        return None;
    };
    let predicate = Predicate::Comparison(crate::protocol::ComparisonPredicate::new(
        Expression::WindowFunction(window.clone()),
        operator,
        Expression::Literal(bound.clone()),
    ));
    let witness = analyze_predicate(inner, &predicate)?;
    Some((witness, column.name().to_string()))
}

fn analyze_predicate(query: &QueryStatement, predicate: &Predicate) -> Option<WindowWitness> {
    let mut result = WindowWitness {
        boundary: None,
        partition_by: Vec::new(),
        order_by: Vec::new(),
        operator: None,
        limit: None,
        qualifying: residual("unsupported_qualify_predicate"),
        rejected: residual("unsupported_qualify_predicate"),
    };
    let Predicate::Comparison(compare) = predicate else {
        return Some(result);
    };
    let (window, operator, bound) = match (compare.left(), compare.right()) {
        (Expression::WindowFunction(window), Expression::Literal(bound)) => {
            (window, compare.operator(), bound)
        }
        (Expression::Literal(bound), Expression::WindowFunction(window)) => {
            (window, compare.operator().reversed(), bound)
        }
        _ => return Some(result),
    };
    if !window.function().name().eq_ignore_ascii_case("row_number") {
        result.qualifying = residual("unsupported_rank_function");
        result.rejected = residual("unsupported_rank_function");
        return Some(result);
    }
    if !window.function().arguments().is_empty() || window.function().distinct() {
        result.qualifying = residual("unsupported_window_function_arguments");
        result.rejected = residual("unsupported_window_function_arguments");
        return Some(result);
    }
    let LiteralValue::Number(text) = bound.value() else {
        return Some(result);
    };
    if bound.literal_type() != LiteralType::Integer {
        return Some(result);
    }
    let Ok(threshold) = text.parse::<u64>() else {
        return Some(result);
    };
    if !matches!(operator, ComparisonOperator::Lte)
        && !(operator == ComparisonOperator::Eq && threshold == 1)
    {
        result.qualifying = residual("unsupported_rank_comparison");
        result.rejected = residual("unsupported_rank_comparison");
        return Some(result);
    }
    result.operator = Some(operator);
    result.limit = Some(threshold);

    let [source] = query.sources() else {
        result.qualifying = residual("unproven_window_boundary");
        result.rejected = residual("unproven_window_boundary");
        return Some(result);
    };
    result.boundary = Some(source.name().to_string());
    if !query.joins().is_empty()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || !query.diagnostics().is_empty()
    {
        result.qualifying = residual("unproven_window_boundary");
        result.rejected = residual("unproven_window_boundary");
        return Some(result);
    }
    let window_spec = window.window();
    if window_spec.frame().is_some() || window_spec.order_by().is_empty() {
        result.qualifying = residual("unproven_window_frame_or_order");
        result.rejected = residual("unproven_window_frame_or_order");
        return Some(result);
    }
    for key in window_spec.partition_by() {
        let Expression::Column(column) = key else {
            result.qualifying = residual("computed_partition_key");
            result.rejected = residual("computed_partition_key");
            return Some(result);
        };
        let Some(column) = source_column(column, source) else {
            return Some(result);
        };
        result.partition_by.push(column);
    }
    for order in window_spec.order_by() {
        let Expression::Column(column) = order.expression() else {
            result.qualifying = residual("computed_order_key");
            result.rejected = residual("computed_order_key");
            return Some(result);
        };
        let (Some(column), Some(nulls_first)) =
            (source_column(column, source), order.nulls_first())
        else {
            result.qualifying = residual("implicit_null_ordering_or_unresolved_column");
            result.rejected = residual("implicit_null_ordering_or_unresolved_column");
            return Some(result);
        };
        result.order_by.push(WindowOrderKey {
            column,
            ascending: order.ascending().unwrap_or(true),
            nulls_first,
        });
    }

    result.qualifying = if threshold == 0 {
        WindowWitnessDirection::Impossible
    } else {
        WindowWitnessDirection::Exact(WindowRankCase::new(0, Some(threshold - 1)))
    };
    result.rejected = WindowWitnessDirection::Exact(WindowRankCase::new(threshold, None));
    Some(result)
}

/// Apply a proved rank bound to an outer projection of the nested rank alias.
pub(crate) fn refine_projected_output(
    output: &Output,
    alias: &str,
    witness: &WindowWitness,
) -> Output {
    let Some(limit) = witness.limit() else {
        return output.clone();
    };
    let bounded = rank_domain(limit);
    Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .map(|column| {
                if matches!(column.expression(), Expression::Column(expr) if expr.name() == alias) {
                    let domain = if matches!(column.domain(), ValueDomain::Unknown(_)) {
                        bounded.clone()
                    } else {
                        crate::domain::intersect_domains(column.domain(), &bounded)
                    };
                    column.with_domain(domain)
                } else {
                    column
                }
            })
            .collect(),
    )
}

fn rank_domain(limit: u64) -> ValueDomain {
    if limit == 0 {
        ValueDomain::Empty
    } else {
        let lower =
            LiteralExpression::new(LiteralType::Integer, LiteralValue::Number("1".to_string()));
        let upper = LiteralExpression::new(
            LiteralType::Integer,
            LiteralValue::Number(limit.to_string()),
        );
        ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(lower, true)),
            Some(Bound::new(upper, true)),
        )])
    }
}

/// Intersect the projected ROW_NUMBER domain with its proven QUALIFY bound.
pub(crate) fn refine_output(query: &QueryStatement) -> Output {
    let Some(witness) = query.window_witness().filter(|witness| witness.is_exact()) else {
        return query.output().clone();
    };
    let Some(limit) = witness.limit() else {
        return query.output().clone();
    };
    let Some(Predicate::Comparison(compare)) = query.predicates().qualify_predicate() else {
        return query.output().clone();
    };
    let expression = match (compare.left(), compare.right()) {
        (Expression::WindowFunction(window), Expression::Literal(_))
        | (Expression::Literal(_), Expression::WindowFunction(window)) => window,
        _ => return query.output().clone(),
    };
    let bounded = rank_domain(limit);
    Output::new(query.output().columns().iter().cloned().map(|column| {
        if matches!(column.expression(), Expression::WindowFunction(candidate) if candidate == expression) {
            let domain = crate::domain::intersect_domains(column.domain(), &bounded);
            column.with_domain(domain)
        } else { column }
    }).collect())
}
