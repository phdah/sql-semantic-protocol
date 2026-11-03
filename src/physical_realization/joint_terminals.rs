//! Joint full-bag certification for grouped JOINs and window/set terminals (TASK-167).
//!
//! Both terminal interpreters consume the SAME caller-attested complete source
//! snapshots. A locally feasible output never substitutes for a compatible
//! physical assignment. Only exact interpreter results can refute a goal.

use super::*;
use crate::constraints::ConstraintValue;
use crate::outcome_goals::OutcomeGoal;
use crate::protocol::LiteralValue;

/// The exact second terminal interpreted against the join's physical sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalJointOtherExact {
    /// A complete deterministic window result, including rejected identities.
    Window(Box<PhysicalFiniteWindowExact>),
    /// A complete positional set-tree result, including NULL tuple membership.
    Set(Box<PhysicalFiniteSetTreeExact>),
}

impl PhysicalJointOtherExact {
    fn terminal(&self) -> &[BTreeMap<String, LiteralValue>] {
        match self {
            Self::Window(proof) => &proof.terminal,
            Self::Set(proof) => &proof.terminal,
        }
    }
}

/// Two exact terminal results over one closed physical source assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalJointTerminalExact {
    /// The source bags in stable relation order. No terminal may replace them.
    pub sources: Vec<PhysicalWriteState>,
    /// JOIN, WHERE, GROUP and HAVING results and TRUE/NOT TRUE pair identities.
    pub grouped: Box<PhysicalJoinedAggregateExact>,
    /// The second terminal's full bag and canonical proof.
    pub other: PhysicalJointOtherExact,
}

/// Classification for two simultaneous requested terminal results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalJointTerminalProof {
    /// One complete assignment satisfies both terminal goals.
    Exact(Box<PhysicalJointTerminalExact>),
    /// An attested source or a fully interpreted terminal contradicts a goal.
    Impossible { reason: String },
    /// Missing schema, source, parser or execution evidence cannot prove either.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalJointTerminalProof {
    PhysicalJointTerminalProof::Residual {
        reason: reason.into(),
    }
}

fn snapshots_for_join(
    bundle: &AnalysisBundle,
    goal: &OutcomeGoal,
    snapshots: &[PhysicalWriteState],
) -> Option<[PhysicalWriteState; 2]> {
    let layer = bundle
        .layers()
        .iter()
        .find(|layer| layer.id() == goal.layer_id())?;
    let query = query_for(bundle, layer)?;
    if query.joins().len() != 1 || query.aggregation().is_none() {
        return None;
    }
    let [left, right] = query.sources() else {
        return None;
    };
    if left.name() == right.name() {
        return None;
    }
    Some([
        snapshots
            .iter()
            .find(|item| item.relation() == left.name())?
            .clone(),
        snapshots
            .iter()
            .find(|item| item.relation() == right.name())?
            .clone(),
    ])
}

enum GoalMatch {
    Matching,
    Contradictory,
    Unproved,
}

/// Compare complete requested frequencies with the actual full terminal bag.
///
/// Source row count and typed NULL are not inferred from per-column intervals.
/// The final row count is checked first, so comparing complete distributions
/// cannot accidentally omit an unmentioned output row.
fn matches_goal(
    bundle: &AnalysisBundle,
    goal: &OutcomeGoal,
    rows: &[BTreeMap<String, LiteralValue>],
    grouped: bool,
) -> GoalMatch {
    let Some(wanted) = goal.rows() else {
        return GoalMatch::Unproved;
    };
    let Ok(actual) = u64::try_from(rows.len()) else {
        return GoalMatch::Unproved;
    };
    if actual != wanted
        || goal
            .groups()
            .is_some_and(|groups| !grouped || groups != actual)
    {
        return GoalMatch::Contradictory;
    }
    let Some(layer) = bundle
        .layers()
        .iter()
        .find(|layer| layer.id() == goal.layer_id())
    else {
        return GoalMatch::Unproved;
    };
    let Some(query) = query_for(bundle, layer) else {
        return GoalMatch::Unproved;
    };
    for distribution in goal.distributions() {
        if query
            .output()
            .columns()
            .iter()
            .filter(|column| column.name() == distribution.column())
            .count()
            != 1
        {
            return GoalMatch::Unproved;
        }
        if distribution
            .values()
            .iter()
            .try_fold(0_u64, |sum, bucket| sum.checked_add(bucket.rows()))
            != Some(wanted)
        {
            return GoalMatch::Contradictory;
        }
        for bucket in distribution.values() {
            let scalar = match bucket.value() {
                ConstraintValue::Integer(value) => LiteralValue::Number(value.to_string()),
                ConstraintValue::Null => LiteralValue::Null,
                _ => return GoalMatch::Unproved,
            };
            let observed = rows
                .iter()
                .filter(|row| row.get(distribution.column()) == Some(&scalar))
                .count();
            let Ok(observed) = u64::try_from(observed) else {
                return GoalMatch::Unproved;
            };
            if observed != bucket.rows() {
                return GoalMatch::Contradictory;
            }
        }
    }
    GoalMatch::Matching
}

/// Prove a grouped two-source INNER JOIN and a second window or set terminal
/// against one caller-supplied complete physical assignment.
///
/// The bounded API requires two distinct terminal goals, each with an exact
/// row target, and exactly two distinct physical source snapshots. The joined
/// interpreter owns SQL ON/WHERE/HAVING truth and grouped output domains;
/// the window/set interpreter owns the second terminal's exact bag and domains.
/// An unsupported operator or type yields Residual, never an invented output.
/// This is snapshot-relative certification, not existential construction of
/// input bags from requested histograms.
pub fn physical_joint_terminal_bag_plan(
    bundle: &AnalysisBundle,
    goals: &[OutcomeGoal],
    snapshots: &[PhysicalWriteState],
) -> PhysicalJointTerminalProof {
    let [first, second] = goals else {
        return residual("TASK-167 requires exactly two terminal goals");
    };
    if first.layer_id() == second.layer_id() || first.rows().is_none() || second.rows().is_none() {
        return residual("joint goals require distinct layer IDs and exact row targets");
    }
    let [source_a, source_b] = snapshots else {
        return residual("joint terminals require exactly two complete physical source bags");
    };
    if source_a.relation() == source_b.relation() {
        return residual("one physical identity cannot have independent source snapshots");
    }
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalJointTerminalProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }

    let mut ordered = goals.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.layer_id().cmp(right.layer_id()));
    let Some((group_index, joined_sources)) =
        ordered.iter().enumerate().find_map(|(index, goal)| {
            snapshots_for_join(bundle, goal, snapshots).map(|sources| (index, sources))
        })
    else {
        return residual("no terminal is a bounded two-source grouped JOIN");
    };
    let grouped_goal = ordered[group_index];
    let other_goal = ordered[1 - group_index];
    let grouped =
        match physical_finite_join_aggregate_plan(bundle, grouped_goal.layer_id(), &joined_sources)
        {
            PhysicalJoinedAggregateProof::Exact(proof) => proof,
            PhysicalJoinedAggregateProof::Impossible { reason } => {
                return PhysicalJointTerminalProof::Impossible { reason };
            }
            PhysicalJoinedAggregateProof::Residual { reason } => return residual(reason),
        };

    let second_sources = physical_source_plan(bundle, other_goal.layer_id());
    if second_sources.sources().is_empty()
        || second_sources.sources().iter().any(|source| {
            !joined_sources
                .iter()
                .any(|state| state.relation() == source)
        })
    {
        return residual("second terminal requires an unattested physical source");
    }
    let second_snapshots = second_sources
        .sources()
        .iter()
        .filter_map(|source| {
            joined_sources
                .iter()
                .find(|state| state.relation() == source)
        })
        .cloned()
        .collect::<Vec<_>>();
    let other =
        match physical_finite_window_bag_plan(bundle, other_goal.layer_id(), &second_snapshots) {
            PhysicalFiniteWindowProof::Exact(proof) => PhysicalJointOtherExact::Window(proof),
            PhysicalFiniteWindowProof::Impossible { reason } => {
                return PhysicalJointTerminalProof::Impossible { reason };
            }
            PhysicalFiniteWindowProof::Residual { .. } => {
                match physical_finite_set_tree_plan(
                    bundle,
                    other_goal.layer_id(),
                    &second_snapshots,
                ) {
                    PhysicalFiniteSetTreeProof::Exact(proof) => PhysicalJointOtherExact::Set(proof),
                    PhysicalFiniteSetTreeProof::Impossible { reason } => {
                        return PhysicalJointTerminalProof::Impossible { reason };
                    }
                    PhysicalFiniteSetTreeProof::Residual { reason } => return residual(reason),
                }
            }
        };

    // Do not publish a positive proof based on either operator alone. The
    // complete source bags are shared and both terminal histograms are checked.
    let group_match = matches_goal(bundle, grouped_goal, &grouped.terminal, true);
    let other_match = matches_goal(bundle, other_goal, other.terminal(), false);
    match (group_match, other_match) {
        (GoalMatch::Contradictory, _) | (_, GoalMatch::Contradictory) => {
            PhysicalJointTerminalProof::Impossible {
                reason:
                    "requested terminal count or typed histogram contradicts the complete shared source bags"
                        .into(),
            }
        }
        (GoalMatch::Matching, GoalMatch::Matching) => {
            let mut sources = joined_sources.to_vec();
            sources.sort_by(|left, right| left.relation().cmp(right.relation()));
            PhysicalJointTerminalProof::Exact(Box::new(PhysicalJointTerminalExact {
                sources,
                grouped,
                other,
            }))
        }
        _ => residual(
            "requested distribution uses a scalar with no certified exact comparison law",
        ),
    }
}
