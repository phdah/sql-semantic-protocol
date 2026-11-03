//! Typed multi-column output goals over a shared closed-world terminal proof.
//!
//! Tuple frequencies are checked against already executed canonical terminal
//! bags, never inferred by multiplying independently valid column marginals.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::bundle::AnalysisBundle;
use crate::constraints::ConstraintValue;
use crate::outcome_goals::OutcomeGoal;
use crate::protocol::LiteralValue;

use super::{
    physical_finite_page_bag_plan, physical_finite_rank_bag_plan, physical_finite_window_bag_plan,
    physical_joint_terminal_bag_plan, PhysicalFinitePageExact, PhysicalFinitePageProof,
    PhysicalFiniteRankExact, PhysicalFiniteRankProof, PhysicalFiniteWindowExact,
    PhysicalFiniteWindowProof, PhysicalJointOtherExact, PhysicalJointTerminalProof,
    PhysicalWriteState,
};

/// An exact requested multiplicity for a typed, positional output tuple.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalTupleBucket {
    values: Vec<ConstraintValue>,
    rows: u64,
}

impl PhysicalTupleBucket {
    /// Construct an independently requested tuple frequency, including SQL NULL.
    pub fn new(values: Vec<ConstraintValue>, rows: u64) -> Self {
        Self { values, rows }
    }

    /// Positional values corresponding to the histogram's output columns.
    pub fn values(&self) -> &[ConstraintValue] {
        &self.values
    }

    /// Requested number of complete output rows with these values.
    pub fn rows(&self) -> u64 {
        self.rows
    }
}

/// A full correlated histogram for one terminal's named output columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTupleHistogramGoal {
    layer_id: String,
    columns: Vec<String>,
    buckets: Vec<PhysicalTupleBucket>,
}

impl PhysicalTupleHistogramGoal {
    /// Validate a complete, deterministic, multi-column frequency request.
    ///
    /// Column order determines tuple positions; values cannot be rearranged
    /// independently, and omitted buckets carry an implicit frequency of zero.
    pub fn new(
        layer_id: impl Into<String>,
        columns: Vec<String>,
        mut buckets: Vec<PhysicalTupleBucket>,
    ) -> Result<Self, PhysicalTupleGoalError> {
        let layer_id = layer_id.into();
        if layer_id.trim().is_empty() {
            return Err(PhysicalTupleGoalError::EmptyLayer);
        }
        if columns.len() < 2 || columns.iter().any(|column| column.trim().is_empty()) {
            return Err(PhysicalTupleGoalError::InvalidColumns);
        }
        let distinct = columns.iter().collect::<BTreeSet<_>>();
        if distinct.len() != columns.len() {
            return Err(PhysicalTupleGoalError::DuplicateColumn);
        }
        if buckets
            .iter()
            .any(|bucket| bucket.values.len() != columns.len())
        {
            return Err(PhysicalTupleGoalError::TupleArityMismatch);
        }
        buckets.sort_by(|left, right| left.values.cmp(&right.values));
        if buckets
            .windows(2)
            .any(|pair| pair[0].values == pair[1].values)
        {
            return Err(PhysicalTupleGoalError::DuplicateTuple);
        }
        if buckets
            .iter()
            .try_fold(0_u64, |sum, bucket| sum.checked_add(bucket.rows))
            .is_none()
        {
            return Err(PhysicalTupleGoalError::CountOverflow);
        }
        Ok(Self {
            layer_id,
            columns,
            buckets,
        })
    }

    /// Exact, stable terminal identity from the analysis bundle.
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }

    /// Ordered output columns, not individually independent marginals.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Complete requested frequencies, sorted by canonical typed tuple.
    pub fn buckets(&self) -> &[PhysicalTupleBucket] {
        &self.buckets
    }
}

/// Why a typed tuple histogram cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalTupleGoalError {
    /// The target layer is empty.
    EmptyLayer,
    /// At least two nonempty column names are required.
    InvalidColumns,
    /// The same output column is named more than once.
    DuplicateColumn,
    /// A tuple has a different width from the column list.
    TupleArityMismatch,
    /// A canonical tuple was supplied more than once.
    DuplicateTuple,
    /// Sum of requested frequencies exceeds the supported row-count type.
    CountOverflow,
}

impl fmt::Display for PhysicalTupleGoalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid physical tuple histogram: {self:?}")
    }
}

impl std::error::Error for PhysicalTupleGoalError {}

enum TupleMatch {
    Matching,
    Contradictory,
    Unproved,
}

fn actual_tuple_frequencies(
    rows: &[BTreeMap<String, LiteralValue>],
    columns: &[String],
) -> Option<BTreeMap<Vec<ConstraintValue>, u64>> {
    let mut observed = BTreeMap::<Vec<ConstraintValue>, u64>::new();
    for row in rows {
        let values = columns
            .iter()
            .map(|column| match row.get(column)? {
                LiteralValue::Null => Some(ConstraintValue::Null),
                LiteralValue::Number(text) => {
                    text.parse::<i64>().ok().map(ConstraintValue::Integer)
                }
                _ => None, // No text collation, float, or vendor comparison guess.
            })
            .collect::<Option<Vec<_>>>()?;
        let count = observed.entry(values).or_insert(0);
        *count = count.checked_add(1)?;
    }
    Some(observed)
}

fn compare_tuple_goal(
    goal: &PhysicalTupleHistogramGoal,
    rows: &[BTreeMap<String, LiteralValue>],
) -> TupleMatch {
    // Empty results do not attest their column identities; refuse false exactness.
    if rows.is_empty() {
        return TupleMatch::Unproved;
    }
    if goal
        .buckets
        .iter()
        .flat_map(|bucket| bucket.values.iter())
        .any(|value| !matches!(value, ConstraintValue::Integer(_) | ConstraintValue::Null))
    {
        return TupleMatch::Unproved;
    }
    let Some(observed) = actual_tuple_frequencies(rows, &goal.columns) else {
        return TupleMatch::Unproved;
    };
    let requested = goal
        .buckets
        .iter()
        .filter(|bucket| bucket.rows > 0)
        .map(|bucket| (bucket.values.clone(), bucket.rows))
        .collect::<BTreeMap<_, _>>();
    if observed == requested {
        TupleMatch::Matching
    } else {
        TupleMatch::Contradictory
    }
}

/// Certify joint tuple correlations against the SAME complete physical bags.
///
/// First prove the grouped JOIN and window/set terminals with ordinary row
/// and scalar goals. Then require every specified complete multi-column
/// histogram to match the already interpreted terminal bag. Unsupported scalar
/// comparison laws and unattested output columns remain Residual; a mismatch
/// against exact INTEGER/NULL output is Impossible. This API does not yet prove
/// arbitrary LIMIT/QUALIFY fanout or construct physical bags from tuple goals.
pub fn physical_joint_tuple_histogram_plan(
    bundle: &AnalysisBundle,
    goals: &[OutcomeGoal],
    snapshots: &[PhysicalWriteState],
    tuple_goals: &[PhysicalTupleHistogramGoal],
) -> PhysicalJointTerminalProof {
    if tuple_goals.is_empty() {
        return PhysicalJointTerminalProof::Residual {
            reason: "no correlated output tuple histogram was requested".into(),
        };
    }
    let proof = physical_joint_terminal_bag_plan(bundle, goals, snapshots);
    let PhysicalJointTerminalProof::Exact(exact) = &proof else {
        return proof;
    };
    let mut seen = BTreeSet::new();
    for target in tuple_goals {
        if !seen.insert(target.layer_id()) {
            return PhysicalJointTerminalProof::Residual {
                reason: "duplicate tuple histogram for one terminal".into(),
            };
        }
        let terminal = if target.layer_id() == exact.grouped_layer_id {
            &exact.grouped.terminal
        } else if target.layer_id() == exact.other_layer_id {
            match &exact.other {
                PhysicalJointOtherExact::Window(window) => &window.terminal,
                PhysicalJointOtherExact::Set(set) => &set.terminal,
            }
        } else {
            return PhysicalJointTerminalProof::Residual {
                reason: "tuple histogram references an unattested terminal".into(),
            };
        };
        match compare_tuple_goal(target, terminal) {
            TupleMatch::Matching => {}
            TupleMatch::Contradictory => {
                return PhysicalJointTerminalProof::Impossible {
                    reason:
                        "requested correlated tuple histogram contradicts the complete terminal bag"
                            .into(),
                };
            }
            TupleMatch::Unproved => {
                return PhysicalJointTerminalProof::Residual {
                    reason:
                        "tuple histogram requires unsupported scalar laws or unattested columns"
                            .into(),
                };
            }
        }
    }
    proof
}

/// An exact page, window or ranking proof, including original source row IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalSingleTupleExact {
    /// Deterministic ORDER BY/OFFSET/LIMIT full page bag and rejected identities.
    Page(Box<PhysicalFinitePageExact>),
    /// Exact window/QUALIFY full output and TRUE/NOT TRUE physical membership.
    Window(Box<PhysicalFiniteWindowExact>),
    /// Exact ranked ROW_NUMBER/RANK/DENSE_RANK output and rejected identities.
    Rank(Box<PhysicalFiniteRankExact>),
}

/// Classification of a correlated histogram against one page or QUALIFY goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalSingleTupleProof {
    /// All typed tuple frequencies match a fully evaluated terminal bag.
    Exact(PhysicalSingleTupleExact),
    /// The complete source bag violates an enforced constraint or tuple request.
    Impossible { reason: String },
    /// A row order, column identity, SQL type or execution law is unproved.
    Residual { reason: String },
}

/// Check a full typed tuple request against canonical LIMIT and QUALIFY execution.
///
/// Reuses the existing single-source page and window/rank interpreters instead
/// of independently guessing SELECT/QUALIFY semantics or multiplying marginals.
/// The complete ordered input, output, rejected physical rows and typed outcome
/// bounds are retained in the returned proof. Unsupported dialect/session laws
/// fail closed as Residual rather than a fabricated exact positive.
pub fn physical_single_tuple_histogram_plan(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalWriteState],
    goal: &PhysicalTupleHistogramGoal,
) -> PhysicalSingleTupleProof {
    let layer = bundle
        .layers()
        .iter()
        .find(|layer| layer.id() == goal.layer_id());
    let Some(layer) = layer else {
        return PhysicalSingleTupleProof::Residual {
            reason: "tuple request references an unknown output layer".into(),
        };
    };
    let Some(query) = super::query_for(bundle, layer) else {
        return PhysicalSingleTupleProof::Residual {
            reason: "tuple request has no canonical SQL query".into(),
        };
    };
    if goal.columns.iter().any(|name| {
        query
            .output()
            .columns()
            .iter()
            .filter(|col| col.name() == name)
            .count()
            != 1
    }) {
        return PhysicalSingleTupleProof::Residual {
            reason: "tuple request lacks unique canonical projected column identities".into(),
        };
    }

    let page = physical_finite_page_bag_plan(bundle, goal.layer_id(), snapshots);
    let exact = match page {
        PhysicalFinitePageProof::Exact(proof) => PhysicalSingleTupleExact::Page(proof),
        PhysicalFinitePageProof::Impossible { reason } => {
            return PhysicalSingleTupleProof::Impossible { reason };
        }
        PhysicalFinitePageProof::Residual { .. } => {
            match physical_finite_window_bag_plan(bundle, goal.layer_id(), snapshots) {
                PhysicalFiniteWindowProof::Exact(proof) => PhysicalSingleTupleExact::Window(proof),
                PhysicalFiniteWindowProof::Impossible { reason } => {
                    return PhysicalSingleTupleProof::Impossible { reason };
                }
                PhysicalFiniteWindowProof::Residual { .. } => {
                    match physical_finite_rank_bag_plan(bundle, goal.layer_id(), snapshots) {
                        PhysicalFiniteRankProof::Exact(proof) => {
                            PhysicalSingleTupleExact::Rank(proof)
                        }
                        PhysicalFiniteRankProof::Impossible { reason } => {
                            return PhysicalSingleTupleProof::Impossible { reason };
                        }
                        PhysicalFiniteRankProof::Residual { reason } => {
                            return PhysicalSingleTupleProof::Residual { reason };
                        }
                    }
                }
            }
        }
    };
    let terminal = match &exact {
        PhysicalSingleTupleExact::Page(page) => &page.terminal,
        PhysicalSingleTupleExact::Window(window) => &window.terminal,
        PhysicalSingleTupleExact::Rank(rank) => &rank.terminal,
    };
    match compare_tuple_goal(goal, terminal) {
        TupleMatch::Matching => PhysicalSingleTupleProof::Exact(exact),
        TupleMatch::Contradictory => PhysicalSingleTupleProof::Impossible {
            reason: "requested tuple histogram contradicts the complete page or QUALIFY bag".into(),
        },
        TupleMatch::Unproved => PhysicalSingleTupleProof::Residual {
            reason: "correlated tuple comparison needs unproved scalar or output evidence".into(),
        },
    }
}
