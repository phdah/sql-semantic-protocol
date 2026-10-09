//! Conservative, typed bag-count transfer laws.
//!
//! A count refers either to an entire relation or to one complete equivalence
//! class of tuples. It never describes merely one sampled row. A proof here is
//! a cardinality theorem, not a construction of the underlying physical rows:
//! joint source realizability remains the responsibility of DAG composition.

use crate::constructive::CountBounds;
use crate::protocol::{JoinKind, SetMultiplicityRule};

/// Whether a count covers the complete relation or one SQL-equal tuple class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagScope {
    /// Every row in one controlled relation or window partition.
    CompleteRelation,
    /// Every occurrence of one projected tuple, with NULLs equal for set laws.
    CandidateTuple,
}

/// Input evidence for a bag transfer: an inclusive count and its closed-world scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagEvidence {
    bounds: CountBounds,
    scope: BagScope,
    closed_world: bool,
}

impl BagEvidence {
    /// Construct scoped evidence. Open-world evidence is legal but cannot prove
    /// exact absence, duplicate multiplicity, or source-wide cardinality.
    pub fn new(bounds: CountBounds, scope: BagScope, closed_world: bool) -> Self {
        Self {
            bounds,
            scope,
            closed_world,
        }
    }

    /// Count interval of the controlled scope.
    pub fn bounds(self) -> CountBounds {
        self.bounds
    }

    /// Whether this is a complete relation or an entire matching tuple class.
    pub fn scope(self) -> BagScope {
        self.scope
    }

    /// Whether all candidates, including absent candidates, are controlled.
    pub fn closed_world(self) -> bool {
        self.closed_world
    }
}

/// A proven relationship between every candidate key on two complete inputs.
/// No key relationship may be inferred from two column domains alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagJoinKeys {
    /// Every candidate pair has the same non-NULL, comparable key.
    EqualNonNull,
    /// All candidate comparisons are nonmatching, including SQL NULL = NULL.
    NeverMatch,
    /// Key matching or NULL ordering has not been proved.
    Unknown,
}

/// Which complete-bag transfer law the caller has *already established* applies.
///
/// In particular, a generic WHERE, non-injective projection, unproved join
/// predicate, window tie, or partial mutation must not be rewritten as one of
/// these laws simply because the SQL parser recognizes its syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagLaw {
    /// Plain projection that neither filters nor multiplies rows.
    RowPreservingProjection,
    /// DISTINCT multiplicity of one complete projected tuple class.
    DistinctTuple,
    /// SQL set multiplicity of one complete tuple class; NULL equals NULL.
    SetTuple(SetMultiplicityRule),
    /// One GROUP BY key survives iff it has at least one contributor.
    GroupKey,
    /// An ungrouped aggregate emits one row, even over an empty input.
    GlobalAggregate,
    /// ROW_NUMBER() <= limit with deterministic strict total ordering.
    RankedPrefix {
        limit: u64,
        strict_total_order: bool,
    },
    /// Equijoin with the stated complete key-match law, no other predicates.
    EquiJoin { kind: JoinKind, keys: BagJoinKeys },
    /// Pure INSERT append with no conflicting constraints/triggers.
    AppendRows,
    /// Delete exactly the specified complete matching subset.
    DeleteRows,
    /// Update existing rows without inserts, deletes, or triggers.
    UpdateRows,
}

/// Cardinality bounds, impossibility from contradictory input evidence, or an
/// explicit unproved precondition. Bounds do *not* imply constructible sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagCountProof {
    /// All possible outputs lie inside this inclusive interval.
    Bounds(CountBounds),
    /// The declared complete source/mutation counts cannot coexist.
    Impossible,
    /// An exact safe transfer cannot be established from the supplied facts.
    Residual { reason: &'static str },
}

/// Whether an independently supplied target count is entailed or contradicted.
///
/// Entailed asserts only that the count goal follows from the supplied complete
/// evidence, not that the evidence can be generated across the entire DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagCountTarget {
    /// All possible counts satisfy the requested target interval.
    Entailed,
    /// No possible count satisfies the requested target interval.
    Impossible,
    /// The ranges overlap but there is no proof of the requested count.
    Residual,
}

impl BagCountProof {
    /// Evaluate a requested inclusive count goal without treating overlap as
    /// constructive feasibility.
    pub fn assess(self, target: CountBounds) -> BagCountTarget {
        match self {
            Self::Impossible => BagCountTarget::Impossible,
            Self::Residual { .. } => BagCountTarget::Residual,
            Self::Bounds(actual) => {
                if disjoint(actual, target) {
                    BagCountTarget::Impossible
                } else if actual.minimum() >= target.minimum()
                    && match target.maximum() {
                        Some(upper) => actual.maximum().is_some_and(|max| max <= upper),
                        None => true,
                    }
                {
                    BagCountTarget::Entailed
                } else {
                    BagCountTarget::Residual
                }
            }
        }
    }
}

fn disjoint(a: CountBounds, b: CountBounds) -> bool {
    a.maximum().is_some_and(|max| max < b.minimum())
        || b.maximum().is_some_and(|max| max < a.minimum())
}

fn bounds(minimum: u64, maximum: Option<u64>) -> BagCountProof {
    // Overflow of a finite maximum means no finite u64 upper bound is proved.
    match CountBounds::new(minimum, maximum) {
        Some(bounds) => BagCountProof::Bounds(bounds),
        None => BagCountProof::Residual {
            reason: "invalid_transferred_bounds",
        },
    }
}

fn sum(a: CountBounds, b: CountBounds) -> BagCountProof {
    let Some(min) = a.minimum().checked_add(b.minimum()) else {
        return BagCountProof::Residual {
            reason: "minimum_count_overflow",
        };
    };
    let max = a
        .maximum()
        .zip(b.maximum())
        .and_then(|(x, y)| x.checked_add(y));
    bounds(min, max)
}

fn product(a: CountBounds, b: CountBounds) -> BagCountProof {
    let Some(min) = a.minimum().checked_mul(b.minimum()) else {
        return BagCountProof::Residual {
            reason: "minimum_count_overflow",
        };
    };
    // Zero times an unbounded count is provably zero.
    let max = if a.maximum() == Some(0) || b.maximum() == Some(0) {
        Some(0)
    } else {
        a.maximum()
            .zip(b.maximum())
            .and_then(|(x, y)| x.checked_mul(y))
    };
    bounds(min, max)
}

fn min_upper(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn presence(count: CountBounds) -> CountBounds {
    let min = u64::from(count.minimum() > 0);
    let max = Some(u64::from(count.maximum() != Some(0)));
    // Both values are in {0,1}, so this is always valid.
    CountBounds::new(min, max).unwrap_or(count)
}

fn set_count(rule: SetMultiplicityRule, left: CountBounds, right: CountBounds) -> BagCountProof {
    match rule {
        SetMultiplicityRule::Sum => sum(left, right),
        SetMultiplicityRule::Minimum => bounds(
            left.minimum().min(right.minimum()),
            min_upper(left.maximum(), right.maximum()),
        ),
        SetMultiplicityRule::SaturatingDifference => bounds(
            left.minimum()
                .saturating_sub(right.maximum().unwrap_or(u64::MAX)),
            left.maximum()
                .map(|max| max.saturating_sub(right.minimum())),
        ),
        SetMultiplicityRule::UnionDistinct => {
            let lo = u64::from(left.minimum() > 0 || right.minimum() > 0);
            let hi = Some(u64::from(
                left.maximum() != Some(0) || right.maximum() != Some(0),
            ));
            bounds(lo, hi)
        }
        SetMultiplicityRule::IntersectDistinct => {
            let lo = u64::from(left.minimum() > 0 && right.minimum() > 0);
            let hi = Some(u64::from(
                left.maximum() != Some(0) && right.maximum() != Some(0),
            ));
            bounds(lo, hi)
        }
        SetMultiplicityRule::ExceptDistinct => {
            let lo = u64::from(left.minimum() > 0 && right.maximum() == Some(0));
            let hi = Some(u64::from(left.maximum() != Some(0) && right.minimum() == 0));
            bounds(lo, hi)
        }
    }
}

fn matching_join(kind: JoinKind, left: CountBounds, right: CountBounds) -> BagCountProof {
    use JoinKind::*;
    match kind {
        Inner | Cross => product(left, right),
        Left => product(
            left,
            CountBounds::new(right.minimum().max(1), right.maximum().map(|n| n.max(1)))
                .unwrap_or(right),
        ),
        Right => product(
            right,
            CountBounds::new(left.minimum().max(1), left.maximum().map(|n| n.max(1)))
                .unwrap_or(left),
        ),
        Full => {
            // If either side might be empty, FULL has piecewise behavior:
            // matching pairs when both nonempty, otherwise all other-side rows.
            // Do not infer an interval from independent endpoints without a
            // proof of which branch is active.
            match (
                left.minimum() > 0,
                right.minimum() > 0,
                left.maximum() == Some(0),
                right.maximum() == Some(0),
            ) {
                (_, _, true, _) => bounds(right.minimum(), right.maximum()),
                (_, _, _, true) => bounds(left.minimum(), left.maximum()),
                (true, true, _, _) => product(left, right),
                _ => BagCountProof::Residual {
                    reason: "full_join_possible_empty_branch",
                },
            }
        }
        LeftSemi => {
            if right.minimum() > 0 {
                bounds(left.minimum(), left.maximum())
            } else if right.maximum() == Some(0) {
                bounds(0, Some(0))
            } else {
                bounds(0, left.maximum())
            }
        }
        RightSemi => {
            if left.minimum() > 0 {
                bounds(right.minimum(), right.maximum())
            } else if left.maximum() == Some(0) {
                bounds(0, Some(0))
            } else {
                bounds(0, right.maximum())
            }
        }
        LeftAnti => {
            if right.minimum() > 0 {
                bounds(0, Some(0))
            } else if right.maximum() == Some(0) {
                bounds(left.minimum(), left.maximum())
            } else {
                bounds(0, left.maximum())
            }
        }
        RightAnti => {
            if left.minimum() > 0 {
                bounds(0, Some(0))
            } else if left.maximum() == Some(0) {
                bounds(right.minimum(), right.maximum())
            } else {
                bounds(0, right.maximum())
            }
        }
        Unknown => BagCountProof::Residual {
            reason: "unknown_join_kind",
        },
    }
}

fn nonmatching_join(kind: JoinKind, left: CountBounds, right: CountBounds) -> BagCountProof {
    use JoinKind::*;
    match kind {
        Inner | LeftSemi | RightSemi => bounds(0, Some(0)),
        Cross => product(left, right),
        Left | LeftAnti => bounds(left.minimum(), left.maximum()),
        Right | RightAnti => bounds(right.minimum(), right.maximum()),
        Full => sum(left, right),
        Unknown => BagCountProof::Residual {
            reason: "unknown_join_kind",
        },
    }
}

impl BagLaw {
    /// Apply a law to proven complete input bags. A right operand is required
    /// only for two-input operators. For equijoins, the key law applies to *all*
    /// candidate rows, including duplicates and SQL NULL comparisons.
    pub fn transfer(self, left: BagEvidence, right: Option<BagEvidence>) -> BagCountProof {
        use BagLaw::*;
        if !left.closed_world() {
            return BagCountProof::Residual {
                reason: "left_open_world",
            };
        }
        let pair = match self {
            SetTuple(_) | EquiJoin { .. } | AppendRows | DeleteRows | UpdateRows => {
                let Some(other) = right else {
                    return BagCountProof::Residual {
                        reason: "missing_right_input",
                    };
                };
                if !other.closed_world() {
                    return BagCountProof::Residual {
                        reason: "right_open_world",
                    };
                }
                Some(other)
            }
            _ => None,
        };
        match self {
            RowPreservingProjection if left.scope() == BagScope::CompleteRelation => {
                BagCountProof::Bounds(left.bounds())
            }
            DistinctTuple if left.scope() == BagScope::CandidateTuple => {
                BagCountProof::Bounds(presence(left.bounds()))
            }
            SetTuple(rule)
                if left.scope() == BagScope::CandidateTuple
                    && pair.is_some_and(|r| r.scope() == BagScope::CandidateTuple) =>
            {
                // The checked pair is present by construction.
                if let Some(right) = pair {
                    set_count(rule, left.bounds(), right.bounds())
                } else {
                    BagCountProof::Residual {
                        reason: "missing_right_input",
                    }
                }
            }
            GroupKey if left.scope() == BagScope::CandidateTuple => {
                BagCountProof::Bounds(presence(left.bounds()))
            }
            GlobalAggregate if left.scope() == BagScope::CompleteRelation => bounds(1, Some(1)),
            RankedPrefix {
                limit,
                strict_total_order: true,
            } if left.scope() == BagScope::CompleteRelation => bounds(
                left.bounds().minimum().min(limit),
                left.bounds().maximum().map(|n| n.min(limit)),
            ),
            RankedPrefix {
                strict_total_order: false,
                ..
            } => BagCountProof::Residual {
                reason: "unproved_rank_tie_order",
            },
            EquiJoin { kind, keys }
                if left.scope() == BagScope::CompleteRelation
                    && pair.is_some_and(|r| r.scope() == BagScope::CompleteRelation) =>
            {
                if let Some(right) = pair {
                    match keys {
                        BagJoinKeys::EqualNonNull => {
                            matching_join(kind, left.bounds(), right.bounds())
                        }
                        BagJoinKeys::NeverMatch => {
                            nonmatching_join(kind, left.bounds(), right.bounds())
                        }
                        BagJoinKeys::Unknown => BagCountProof::Residual {
                            reason: "unproved_join_key_relationship",
                        },
                    }
                } else {
                    BagCountProof::Residual {
                        reason: "missing_right_input",
                    }
                }
            }
            AppendRows
                if left.scope() == BagScope::CompleteRelation
                    && pair.is_some_and(|r| r.scope() == BagScope::CompleteRelation) =>
            {
                if let Some(right) = pair {
                    sum(left.bounds(), right.bounds())
                } else {
                    BagCountProof::Residual {
                        reason: "missing_right_input",
                    }
                }
            }
            DeleteRows
                if left.scope() == BagScope::CompleteRelation
                    && pair.is_some_and(|r| r.scope() == BagScope::CompleteRelation) =>
            {
                if let Some(right) = pair {
                    if left
                        .bounds()
                        .maximum()
                        .is_some_and(|max| right.bounds().minimum() > max)
                    {
                        BagCountProof::Impossible
                    } else if right
                        .bounds()
                        .maximum()
                        .is_none_or(|max| max > left.bounds().minimum())
                    {
                        BagCountProof::Residual {
                            reason: "deleted_subset_not_proved",
                        }
                    } else {
                        bounds(
                            left.bounds()
                                .minimum()
                                .saturating_sub(right.bounds().maximum().unwrap_or(0)),
                            left.bounds()
                                .maximum()
                                .map(|max| max.saturating_sub(right.bounds().minimum())),
                        )
                    }
                } else {
                    BagCountProof::Residual {
                        reason: "missing_right_input",
                    }
                }
            }
            UpdateRows
                if left.scope() == BagScope::CompleteRelation
                    && pair.is_some_and(|r| r.scope() == BagScope::CompleteRelation) =>
            {
                if let Some(right) = pair {
                    if left
                        .bounds()
                        .maximum()
                        .is_some_and(|max| right.bounds().minimum() > max)
                    {
                        BagCountProof::Impossible
                    } else if right
                        .bounds()
                        .maximum()
                        .is_none_or(|max| max > left.bounds().minimum())
                    {
                        BagCountProof::Residual {
                            reason: "updated_subset_not_proved",
                        }
                    } else {
                        BagCountProof::Bounds(left.bounds())
                    }
                } else {
                    BagCountProof::Residual {
                        reason: "missing_right_input",
                    }
                }
            }
            _ => BagCountProof::Residual {
                reason: "incompatible_bag_scope_or_law",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(min: u64, max: Option<u64>, scope: BagScope) -> BagEvidence {
        BagEvidence::new(
            CountBounds::new(min, max).expect("valid counts"),
            scope,
            true,
        )
    }

    fn exact(n: u64, scope: BagScope) -> BagEvidence {
        evidence(n, Some(n), scope)
    }

    fn number(proof: BagCountProof) -> CountBounds {
        match proof {
            BagCountProof::Bounds(value) => value,
            other => panic!("not proved: {other:?}"),
        }
    }

    #[test]
    fn set_all_and_distinct_laws_preserve_null_safe_duplicate_counts() {
        let l = exact(3, BagScope::CandidateTuple);
        let r = exact(2, BagScope::CandidateTuple);
        for (law, expected) in [
            (SetMultiplicityRule::Sum, 5),
            (SetMultiplicityRule::UnionDistinct, 1),
            (SetMultiplicityRule::Minimum, 2),
            (SetMultiplicityRule::IntersectDistinct, 1),
            (SetMultiplicityRule::SaturatingDifference, 1),
            (SetMultiplicityRule::ExceptDistinct, 0),
        ] {
            assert_eq!(
                number(BagLaw::SetTuple(law).transfer(l, Some(r))).minimum(),
                expected
            );
        }
        assert_eq!(
            number(BagLaw::DistinctTuple.transfer(l, None)).maximum(),
            Some(1)
        );
    }

    #[test]
    fn empty_tuple_absence_requires_a_closed_world() {
        let tuple = exact(0, BagScope::CandidateTuple);
        assert_eq!(
            number(BagLaw::DistinctTuple.transfer(tuple, None)).maximum(),
            Some(0)
        );
        let open = BagEvidence::new(tuple.bounds(), tuple.scope(), false);
        assert_eq!(
            BagLaw::DistinctTuple.transfer(open, None),
            BagCountProof::Residual {
                reason: "left_open_world"
            }
        );
        assert_eq!(
            BagLaw::SetTuple(SetMultiplicityRule::Minimum).transfer(tuple, Some(open)),
            BagCountProof::Residual {
                reason: "right_open_world"
            }
        );
    }

    #[test]
    fn joins_count_duplicate_pairs_and_treat_null_equalities_as_nonmatching() {
        let l = exact(3, BagScope::CompleteRelation);
        let r = exact(2, BagScope::CompleteRelation);
        let join = |kind, keys| BagLaw::EquiJoin { kind, keys }.transfer(l, Some(r));
        assert_eq!(
            number(join(JoinKind::Inner, BagJoinKeys::EqualNonNull)).minimum(),
            6
        );
        assert_eq!(
            number(join(JoinKind::Full, BagJoinKeys::EqualNonNull)).minimum(),
            6
        );
        assert_eq!(
            number(join(JoinKind::Full, BagJoinKeys::NeverMatch)).minimum(),
            5
        );
        assert_eq!(
            number(join(JoinKind::LeftAnti, BagJoinKeys::NeverMatch)).minimum(),
            3
        );
        assert!(matches!(
            join(JoinKind::Inner, BagJoinKeys::Unknown),
            BagCountProof::Residual { .. }
        ));
    }

    #[test]
    fn grouped_global_and_ranked_laws_are_distinct() {
        let empty = exact(0, BagScope::CompleteRelation);
        assert_eq!(
            number(BagLaw::GlobalAggregate.transfer(empty, None)).minimum(),
            1
        );
        let group = exact(0, BagScope::CandidateTuple);
        assert_eq!(number(BagLaw::GroupKey.transfer(group, None)).minimum(), 0);
        let partition = exact(7, BagScope::CompleteRelation);
        assert_eq!(
            number(
                BagLaw::RankedPrefix {
                    limit: 3,
                    strict_total_order: true
                }
                .transfer(partition, None)
            )
            .minimum(),
            3
        );
        assert!(matches!(
            BagLaw::RankedPrefix {
                limit: 3,
                strict_total_order: false
            }
            .transfer(partition, None),
            BagCountProof::Residual { .. }
        ));
    }

    #[test]
    fn impossible_and_underdetermined_mutations_are_not_constructive_proofs() {
        let initial = exact(2, BagScope::CompleteRelation);
        let too_many = exact(3, BagScope::CompleteRelation);
        assert_eq!(
            BagLaw::DeleteRows.transfer(initial, Some(too_many)),
            BagCountProof::Impossible
        );
        assert_eq!(
            BagLaw::UpdateRows.transfer(initial, Some(too_many)),
            BagCountProof::Impossible
        );
        let partial = evidence(0, Some(3), BagScope::CompleteRelation);
        assert!(matches!(
            BagLaw::DeleteRows.transfer(initial, Some(partial)),
            BagCountProof::Residual { .. }
        ));
        assert_eq!(
            number(BagLaw::AppendRows.transfer(initial, Some(too_many))).minimum(),
            5
        );
    }

    #[test]
    fn interval_bounds_never_claim_feasibility_from_overlapping_targets() {
        let left = evidence(1, Some(4), BagScope::CandidateTuple);
        let right = evidence(0, Some(2), BagScope::CandidateTuple);
        let count =
            BagLaw::SetTuple(SetMultiplicityRule::SaturatingDifference).transfer(left, Some(right));
        assert_eq!(number(count), CountBounds::new(0, Some(4)).expect("bounds"));
        assert_eq!(
            count.assess(CountBounds::new(2, Some(2)).expect("target")),
            BagCountTarget::Residual
        );
        assert_eq!(
            count.assess(CountBounds::new(5, Some(5)).expect("target")),
            BagCountTarget::Impossible
        );
    }
}
