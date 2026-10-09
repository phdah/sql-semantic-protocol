//! Caller-requested result goals, evaluated separately from SQL semantic facts.
//!
//! A feasible verdict is a constructive proof for the explicitly supported cases.
//! Missing SQL cardinality or distribution proofs always remain residual.

use std::collections::BTreeSet;
use std::fmt;

use crate::bundle::{AnalysisBundle, ComposedSemantics, RelationResolution, TransformationLayer};
use crate::constraints::ConstraintValue;
use crate::protocol::{Expression, GroupBy, GroupingExpression, ProtocolStatement, QueryStatement};

/// A requested count for one output scalar, including SQL NULL.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutputValueCount {
    value: ConstraintValue,
    rows: u64,
}

impl OutputValueCount {
    /// Request exactly `rows` occurrences of a typed output value.
    pub fn new(value: ConstraintValue, rows: u64) -> Self {
        Self { value, rows }
    }

    /// Canonical scalar value, with NULL represented separately from non-NULL values.
    pub fn value(&self) -> &ConstraintValue {
        &self.value
    }

    /// Requested number of output rows containing this value.
    pub fn rows(&self) -> u64 {
        self.rows
    }
}

/// A complete, typed frequency distribution for one projected output column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutputDistribution {
    column: String,
    values: Vec<OutputValueCount>,
}

impl OutputDistribution {
    /// Construct a deterministic histogram. Each value may appear only once.
    pub fn new(column: impl Into<String>, mut values: Vec<OutputValueCount>) -> Result<Self, OutcomeGoalError> {
        let column = column.into();
        if column.trim().is_empty() {
            return Err(OutcomeGoalError::EmptyColumn);
        }
        values.sort_by(|left, right| left.value.cmp(&right.value));
        if values.windows(2).any(|pair| pair[0].value == pair[1].value) {
            return Err(OutcomeGoalError::DuplicateValue { column });
        }
        Ok(Self { column, values })
    }

    /// Output column name, not a physical-source column identity.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// All output values and their exact frequencies (including NULL when requested).
    pub fn values(&self) -> &[OutputValueCount] {
        &self.values
    }
}

/// Optional caller-requested output counts, addressed by a stable layer identity.
///
/// The distribution describes the entire output, not a sampled subset. A row count
/// must be provided when one or more distributions are requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeGoal {
    layer_id: String,
    rows: Option<u64>,
    groups: Option<u64>,
    distributions: Vec<OutputDistribution>,
}

impl OutcomeGoal {
    /// Construct a nonempty goal with uniquely named column distributions.
    pub fn new(
        layer_id: impl Into<String>,
        rows: Option<u64>,
        groups: Option<u64>,
        mut distributions: Vec<OutputDistribution>,
    ) -> Result<Self, OutcomeGoalError> {
        let layer_id = layer_id.into();
        if layer_id.trim().is_empty() {
            return Err(OutcomeGoalError::EmptyLayer);
        }
        if rows.is_none() && groups.is_none() {
            return Err(OutcomeGoalError::EmptyGoal);
        }
        if !distributions.is_empty() && rows.is_none() {
            return Err(OutcomeGoalError::DistributionRequiresRows);
        }
        distributions.sort_by(|left, right| left.column.cmp(&right.column));
        if distributions.windows(2).any(|pair| pair[0].column == pair[1].column) {
            return Err(OutcomeGoalError::DuplicateColumn);
        }
        Ok(Self { layer_id, rows, groups, distributions })
    }

    /// Stable identity of the query layer whose *final* output is targeted.
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }

    /// Exact requested output rows, independently of physical source row counts.
    pub fn rows(&self) -> Option<u64> {
        self.rows
    }

    /// Exact requested groups surviving HAVING, not the count of source rows.
    pub fn groups(&self) -> Option<u64> {
        self.groups
    }

    /// Complete requested output histograms.
    pub fn distributions(&self) -> &[OutputDistribution] {
        &self.distributions
    }
}

/// Explicit failure to attach an invalid or ambiguous goal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutcomeGoalError {
    /// Layer identifier is blank.
    EmptyLayer,
    /// Goal did not request either rows or groups.
    EmptyGoal,
    /// Histogram column name is blank.
    EmptyColumn,
    /// A goal names one output column more than once.
    DuplicateColumn,
    /// A histogram repeats the same typed scalar.
    DuplicateValue { column: String },
    /// A histogram cannot be complete without an output-row target.
    DistributionRequiresRows,
    /// An outcome layer was not found in this bundle.
    UnknownLayer { layer_id: String },
    /// Multiple goal requests target the same layer.
    DuplicateLayer { layer_id: String },
    /// A goal references a column not present in the projected output.
    UnknownOutputColumn { layer_id: String, column: String },
}

impl fmt::Display for OutcomeGoalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for OutcomeGoalError {}

/// Feasibility classification, separate from row-condition exactness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeGoalStatus {
    /// There is a constructive witness for the complete requested goal.
    Feasible,
    /// The requested goal contradicts a proven SQL or arithmetic invariant.
    Unsatisfiable,
    /// Feasibility cannot be proven without unsupported row-shape or value evidence.
    Residual,
}

impl OutcomeGoalStatus {
    /// Stable JSON discriminator for feasibility.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Feasible => "feasible",
            Self::Unsatisfiable => "unsatisfiable",
            Self::Residual => "residual",
        }
    }
}

/// Proof status and the strongest safely proven output-row bounds for a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedOutcomeGoal {
    goal: OutcomeGoal,
    status: OutcomeGoalStatus,
    reason: String,
    min_rows: u64,
    max_rows: Option<u64>,
}

impl EvaluatedOutcomeGoal {
    /// Caller-originated request retained unmodified.
    pub fn goal(&self) -> &OutcomeGoal {
        &self.goal
    }
    /// Whether the request is constructively feasible, contradictory or residual.
    pub fn status(&self) -> OutcomeGoalStatus {
        self.status
    }
    /// Specific proof or conservative limitation for this classification.
    pub fn reason(&self) -> &str {
        &self.reason
    }
    /// Proven minimum possible final output rows.
    pub fn min_rows(&self) -> u64 {
        self.min_rows
    }
    /// Proven maximum final output rows, or None when not bounded.
    pub fn max_rows(&self) -> Option<u64> {
        self.max_rows
    }
}

pub(crate) fn evaluate(bundle: &AnalysisBundle, goals: &[OutcomeGoal]) -> Result<Vec<EvaluatedOutcomeGoal>, OutcomeGoalError> {
    let mut ordered = goals.to_vec();
    ordered.sort_by(|left, right| left.layer_id.cmp(&right.layer_id));
    for pair in ordered.windows(2) {
        if pair[0].layer_id == pair[1].layer_id {
            return Err(OutcomeGoalError::DuplicateLayer { layer_id: pair[0].layer_id.clone() });
        }
    }

    ordered.into_iter().map(|goal| {
        let layer = bundle.layers().iter().find(|layer| layer.id() == goal.layer_id)
            .ok_or_else(|| OutcomeGoalError::UnknownLayer { layer_id: goal.layer_id.clone() })?;
        let query = bundle.inputs().iter().find(|input| input.id() == layer.input_id())
            .and_then(|input| input.statements().get(layer.statement_index()))
            .and_then(|statement| match statement {
                ProtocolStatement::Query(query) => Some(query),
                ProtocolStatement::Unsupported(_) => None,
            });
        assess_goal(bundle, layer, query, goal)
    }).collect()
}

fn assessed(goal: OutcomeGoal, status: OutcomeGoalStatus, reason: &str, min_rows: u64, max_rows: Option<u64>) -> EvaluatedOutcomeGoal {
    EvaluatedOutcomeGoal { goal, status, reason: reason.to_string(), min_rows, max_rows }
}

fn assess_goal(
    bundle: &AnalysisBundle,
    layer: &TransformationLayer,
    query: Option<&QueryStatement>,
    goal: OutcomeGoal,
) -> Result<EvaluatedOutcomeGoal, OutcomeGoalError> {
    let resolved = match layer.composed_semantics() {
        ComposedSemantics::Resolved(resolved) => resolved,
        ComposedSemantics::Unresolved(_) => {
            return Ok(assessed(goal, OutcomeGoalStatus::Residual, "target layer composition is unresolved", 0, None))
        }
    };
    let output = resolved.output();
    for distribution in &goal.distributions {
        if !output.columns().iter().any(|column| column.name() == distribution.column()) {
            return Err(OutcomeGoalError::UnknownOutputColumn {
                layer_id: goal.layer_id.clone(),
                column: distribution.column().to_string(),
            });
        }
    }

    let singleton = query.is_some_and(QueryStatement::proven_single_row_output)
        && resolved.dependencies().is_empty()
        && resolved.diagnostics().is_empty();
    let min_rows = u64::from(singleton);
    let max_rows = singleton.then_some(1);
    if let Some(rows) = goal.rows {
        if singleton && rows != 1 {
            return Ok(assessed(goal, OutcomeGoalStatus::Unsatisfiable, "query provably produces exactly one output row", min_rows, max_rows));
        }
    }

    for distribution in &goal.distributions {
        let mut sum = 0_u64;
        for item in distribution.values() {
            match sum.checked_add(item.rows()) {
                Some(next) => sum = next,
                None => {
                    return Ok(assessed(goal, OutcomeGoalStatus::Unsatisfiable, "distribution count overflows the representable output row count", min_rows, max_rows));
                }
            }
        }
        if Some(sum) != goal.rows {
            return Ok(assessed(goal, OutcomeGoalStatus::Unsatisfiable, "complete distribution does not sum to requested output rows", min_rows, max_rows));
        }
    }

    if let Some(query) = query {
        let simple_groups = query.aggregation().and_then(|aggregation| aggregation.group_by())
            .is_some_and(|group_by| matches!(group_by, GroupBy::Expressions(items)
                if !items.is_empty() && items.iter().all(|item| matches!(item, GroupingExpression::Expression(_)))));
        if simple_groups && goal.rows.is_some() && goal.groups != goal.rows && goal.groups.is_some() {
            return Ok(assessed(goal, OutcomeGoalStatus::Unsatisfiable, "an ordinary GROUP BY emits exactly one output row per surviving group", min_rows, max_rows));
        }
        if singleton && goal.groups.is_some() {
            // Explicit GROUP BY / aggregate group feasibility must not be inferred from
            // the singleton row bound alone.
            return Ok(assessed(goal, OutcomeGoalStatus::Residual, "group cardinality is not proved by a singleton output bound", min_rows, max_rows));
        }

        let distinct_one_column = output.columns().len() == 1
            && query.aggregation().is_some_and(|aggregation| aggregation.distinct() && aggregation.distinct_on().is_empty());
        if distinct_one_column && goal.distributions().iter().any(|distribution| {
            distribution.values().iter().any(|item| item.rows() > 1)
        }) {
            return Ok(assessed(goal, OutcomeGoalStatus::Unsatisfiable, "DISTINCT permits at most one row per value, including NULL", min_rows, max_rows));
        }

        if singleton && goal.distributions.is_empty() && goal.groups.is_none() {
            return Ok(assessed(goal, OutcomeGoalStatus::Feasible, "SQL proves a singleton output independently of source cardinality", min_rows, max_rows));
        }

        // An empty physical source is a constructive witness for zero rows only
        // for an ordinary, direct, predicate-free and projection-preserving query.
        let direct_external = layer.consumes().len() == 1
            && bundle.graph().edges().iter().filter(|edge| edge.consumer_layer_id() == layer.id())
                .all(|edge| edge.resolution() == RelationResolution::External);
        let simple_projection = query.row_preserving_projection()
            && query.output().columns().iter().all(|column| matches!(column.expression(), Expression::Column(_) | Expression::Literal(_)))
            && query.diagnostics().is_empty()
            && resolved.diagnostics().is_empty()
            && direct_external;
        if simple_projection && goal.rows == Some(0) && goal.groups.is_none()
            && goal.distributions.iter().all(|distribution| distribution.values().is_empty()) {
            return Ok(assessed(goal, OutcomeGoalStatus::Feasible, "an empty external source yields an empty row-preserving projection", min_rows, max_rows));
        }
    }

    Ok(assessed(goal, OutcomeGoalStatus::Residual, "SQL cardinality, grouping, join multiplicity, window and distribution witnesses are not sufficient to prove this request", min_rows, max_rows))
}
