//! Exact SQL bag-key histograms over complete physical relations.
//!
//! Join cardinality needs per-key frequency, not just the total row count.
//! These proofs handle mixed matching/unmatched keys and repeated aliases,
//! with SQL NULL = NULL remaining UNKNOWN for joins. Candidate lookup of an
//! absent key never implies that the entire source relation is empty.

use std::collections::{BTreeMap, BTreeSet};

use crate::bag_semantics::BagSourceIdentity;
use crate::constraints::ConstraintValue;
use crate::protocol::JoinKind;

/// Canonical identity of a physical key expression, independent of SQL alias.
///
/// Equal identities assert the same expression over the same physical rows;
/// two different columns of one table must carry different identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BagKeyExpressionIdentity(String);

impl BagKeyExpressionIdentity {
    /// Construct a nonempty physical expression identity established upstream.
    pub fn new(value: &str) -> Option<Self> {
        if value.is_empty() {
            return None;
        }
        Some(Self(value.to_string()))
    }
}

/// Complete, typed frequency distribution of one SQL join-key expression.
///
/// Rows with SQL NULL are counted but do not join to other NULL rows. Only
/// integer and boolean exact value comparisons are admitted by the join
/// evaluator; collation, coercion and vendor-specific numeric equalities
/// remain residual until the caller supplies authoritative comparison law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BagKeyHistogram {
    source: BagSourceIdentity,
    key_expression: BagKeyExpressionIdentity,
    frequencies: BTreeMap<ConstraintValue, u64>,
    total: u64,
}

impl BagKeyHistogram {
    /// Construct an entire source-key histogram, rejecting duplicate keys,
    /// zero-count entries and total overflow. An empty map proves an empty
    /// relation, not merely an absent candidate tuple.
    pub fn new(
        source: BagSourceIdentity,
        key_expression: BagKeyExpressionIdentity,
        frequencies: Vec<(ConstraintValue, u64)>,
    ) -> Option<Self> {
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        for (key, count) in frequencies {
            if count == 0 || entries.contains_key(&key) {
                return None;
            }
            total = total.checked_add(count)?;
            entries.insert(key, count);
        }
        Some(Self {
            source,
            key_expression,
            frequencies: entries,
            total,
        })
    }

    /// Identity of the underlying physical source and its SQL relation instance.
    pub fn source(&self) -> &BagSourceIdentity {
        &self.source
    }

    /// Canonical physical expression used to compute this frequency distribution.
    pub fn key_expression(&self) -> &BagKeyExpressionIdentity {
        &self.key_expression
    }

    /// Number of all physical rows, including SQL NULL keys.
    pub fn total_rows(&self) -> u64 {
        self.total
    }

    /// Complete candidate key multiplicity, or zero for a missing key.
    pub fn count(&self, key: &ConstraintValue) -> u64 {
        self.frequencies.get(key).copied().unwrap_or(0)
    }

    /// Deterministically sorted complete key classes.
    pub fn entries(&self) -> &BTreeMap<ConstraintValue, u64> {
        &self.frequencies
    }
}

/// Whether a whole-source histogram join is arithmetically proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BagHistogramProof {
    /// Complete output histogram for the coalesced equality join-key.
    Exact(BTreeMap<ConstraintValue, u64>),
    /// Contradictory complete histograms for aliases of one physical source.
    Impossible,
    /// Unsupported comparison law, join kind or count overflow.
    Residual { reason: &'static str },
}

impl BagHistogramProof {
    /// Total output rows when all key classes are proved.
    pub fn total_rows(&self) -> Option<u64> {
        match self {
            Self::Exact(entries) => entries
                .values()
                .try_fold(0u64, |sum, value| sum.checked_add(*value)),
            Self::Impossible | Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ComparableKeyType {
    Boolean,
    SignedInteger,
    UnsignedInteger,
}

fn comparable_type(key: &ConstraintValue) -> Option<ComparableKeyType> {
    match key {
        ConstraintValue::Boolean(_) => Some(ComparableKeyType::Boolean),
        ConstraintValue::Integer(_) => Some(ComparableKeyType::SignedInteger),
        ConstraintValue::UnsignedInteger(_) => Some(ComparableKeyType::UnsignedInteger),
        _ => None,
    }
}

fn exact_comparable(key: &ConstraintValue) -> bool {
    matches!(
        key,
        ConstraintValue::Null
            | ConstraintValue::Boolean(_)
            | ConstraintValue::Integer(_)
            | ConstraintValue::UnsignedInteger(_)
    )
}

fn matching_frequency(kind: JoinKind, left: u64, right: u64, is_null: bool) -> Option<u64> {
    let pairs = if is_null { 0 } else { left.checked_mul(right)? };
    let left_matched = !is_null && right > 0;
    let right_matched = !is_null && left > 0;
    match kind {
        JoinKind::Inner => Some(pairs),
        JoinKind::Left => Some(if left_matched { pairs } else { left }),
        JoinKind::Right => Some(if right_matched { pairs } else { right }),
        JoinKind::Full => {
            if left_matched && right_matched {
                Some(pairs)
            } else {
                left.checked_add(right)
            }
        }
        JoinKind::LeftSemi => Some(if left_matched { left } else { 0 }),
        JoinKind::RightSemi => Some(if right_matched { right } else { 0 }),
        JoinKind::LeftAnti => Some(if left_matched { 0 } else { left }),
        JoinKind::RightAnti => Some(if right_matched { 0 } else { right }),
        JoinKind::Cross | JoinKind::Unknown => None,
    }
}

/// Prove per-key bag multiplicities for an equality join with complete keys.
///
/// Inputs are complete histograms of their respective compared key expressions.
/// Only aliases evaluating the *same* physical key expression over the same
/// complete source must have identical histograms. Different expressions of the
/// same table can legitimately have different distributions.
/// Unlike a single total-count product, mixed key matches, NULL/nonmatches,
/// duplicate pairs, outer rows and semi/anti membership are all evaluated.
///
/// Output is grouped by COALESCE(left_key, right_key) for FULL/RIGHT joins;
/// it is grouped by the preserved key for other join kinds. The output does
/// not prove the multiplicity of arbitrary other projected columns.
pub fn equijoin_key_histogram(
    kind: JoinKind,
    left: &BagKeyHistogram,
    right: &BagKeyHistogram,
) -> BagHistogramProof {
    if left.source().physical_relation() == right.source().physical_relation()
        && left.key_expression() == right.key_expression()
        && left.entries() != right.entries()
    {
        return BagHistogramProof::Impossible;
    }
    if matches!(kind, JoinKind::Cross | JoinKind::Unknown) {
        return BagHistogramProof::Residual {
            reason: "unsupported_join_key_law",
        };
    }
    let keys = left
        .entries()
        .keys()
        .chain(right.entries().keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    if keys.iter().any(|key| !exact_comparable(key)) {
        return BagHistogramProof::Residual {
            reason: "unproved_key_comparison_law",
        };
    }
    let key_types = keys
        .iter()
        .filter_map(comparable_type)
        .collect::<BTreeSet<_>>();
    if key_types.len() > 1 {
        return BagHistogramProof::Residual {
            reason: "mixed_key_type_comparison_unknown",
        };
    }

    let mut result = BTreeMap::new();
    for key in keys {
        let left_rows = left.count(&key);
        let right_rows = right.count(&key);
        let Some(rows) = matching_frequency(
            kind,
            left_rows,
            right_rows,
            matches!(key, ConstraintValue::Null),
        ) else {
            return BagHistogramProof::Residual {
                reason: "join_multiplicity_overflow",
            };
        };
        if rows > 0 {
            result.insert(key, rows);
        }
    }
    BagHistogramProof::Exact(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(relation: &str, instance: &str) -> BagSourceIdentity {
        BagSourceIdentity::new(relation, instance).expect("valid physical relation")
    }

    fn histogram_for_key(
        relation: &str,
        instance: &str,
        key_expression: &str,
        entries: &[(Option<i64>, u64)],
    ) -> BagKeyHistogram {
        BagKeyHistogram::new(
            source(relation, instance),
            BagKeyExpressionIdentity::new(key_expression).expect("key expression"),
            entries
                .iter()
                .map(|(value, rows)| {
                    (
                        value.map_or(ConstraintValue::Null, ConstraintValue::Integer),
                        *rows,
                    )
                })
                .collect(),
        )
        .expect("complete histogram")
    }

    fn histogram(
        relation: &str,
        instance: &str,
        entries: &[(Option<i64>, u64)],
    ) -> BagKeyHistogram {
        histogram_for_key(relation, instance, "k", entries)
    }

    #[test]
    fn mixed_duplicate_and_null_keys_preserve_full_bag_counts() {
        let l = histogram("l", "l", &[(Some(1), 3), (Some(2), 1), (None, 2)]);
        let r = histogram("r", "r", &[(Some(1), 2), (Some(3), 4), (None, 1)]);
        for (kind, total) in [
            (JoinKind::Inner, 6),
            (JoinKind::Left, 9),
            (JoinKind::Right, 11),
            (JoinKind::Full, 14),
            (JoinKind::LeftSemi, 3),
            (JoinKind::LeftAnti, 3),
            (JoinKind::RightSemi, 2),
            (JoinKind::RightAnti, 5),
        ] {
            let proof = equijoin_key_histogram(kind, &l, &r);
            assert_eq!(proof.total_rows(), Some(total), "{kind:?}");
        }
    }

    #[test]
    fn absent_candidate_and_completely_empty_source_are_distinct() {
        let nonempty = histogram("l", "l", &[(Some(1), 2)]);
        assert_eq!(nonempty.count(&ConstraintValue::Integer(9)), 0);
        assert_eq!(nonempty.total_rows(), 2);
        let empty = histogram("r", "r", &[]);
        assert_eq!(empty.total_rows(), 0);
        assert_eq!(
            equijoin_key_histogram(JoinKind::Left, &nonempty, &empty).total_rows(),
            Some(2)
        );
    }

    #[test]
    fn aliases_cannot_supply_conflicting_histograms_for_one_source() {
        let first = histogram("physical.t", "a", &[(Some(1), 2)]);
        let second = histogram("physical.t", "b", &[(Some(1), 3)]);
        assert_eq!(
            equijoin_key_histogram(JoinKind::Inner, &first, &second),
            BagHistogramProof::Impossible
        );
        let same = histogram("physical.t", "b", &[(Some(1), 2)]);
        assert_eq!(
            equijoin_key_histogram(JoinKind::Inner, &first, &same).total_rows(),
            Some(4)
        );
    }

    #[test]
    fn self_join_distinct_physical_key_expressions_may_have_different_histograms() {
        // Three physical rows: (id, manager_id) = (1, 1), (2, 1), (3, 2).
        let managers = histogram_for_key(
            "employees",
            "a",
            "manager_id",
            &[(Some(1), 2), (Some(2), 1)],
        );
        let ids = histogram_for_key(
            "employees",
            "b",
            "id",
            &[(Some(1), 1), (Some(2), 1), (Some(3), 1)],
        );
        assert_eq!(
            equijoin_key_histogram(JoinKind::Inner, &managers, &ids).total_rows(),
            Some(3)
        );
        // Contradictory complete evidence for one and the same expression
        // remains impossible, even across different relation aliases.
        let other_managers = histogram_for_key("employees", "b", "manager_id", &[(Some(1), 3)]);
        assert_eq!(
            equijoin_key_histogram(JoinKind::Inner, &managers, &other_managers),
            BagHistogramProof::Impossible
        );
    }

    #[test]
    fn implicit_cross_type_comparisons_must_not_be_assumed() {
        let signed = histogram("l", "l", &[(Some(1), 2)]);
        let unsigned = BagKeyHistogram::new(
            source("r", "r"),
            BagKeyExpressionIdentity::new("k").expect("key"),
            vec![(ConstraintValue::UnsignedInteger(1), 3)],
        )
        .expect("input");
        assert!(matches!(
            equijoin_key_histogram(JoinKind::Inner, &signed, &unsigned),
            BagHistogramProof::Residual {
                reason: "mixed_key_type_comparison_unknown"
            }
        ));
    }

    #[test]
    fn unknown_collation_and_arithmetic_overflow_remain_residual() {
        let strings = BagKeyHistogram::new(
            source("l", "l"),
            BagKeyExpressionIdentity::new("k").expect("key"),
            vec![(ConstraintValue::String("a".to_string()), 1)],
        )
        .expect("input");
        let other = histogram("r", "r", &[(Some(1), 1)]);
        assert!(matches!(
            equijoin_key_histogram(JoinKind::Inner, &strings, &other),
            BagHistogramProof::Residual { .. }
        ));
        let huge = histogram("l", "l", &[(Some(1), u64::MAX)]);
        let two = histogram("r", "r", &[(Some(1), 2)]);
        assert!(matches!(
            equijoin_key_histogram(JoinKind::Inner, &huge, &two),
            BagHistogramProof::Residual { .. }
        ));
    }
}
