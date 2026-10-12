//! Proof-case validation and cross-obligation consistency.

use super::*;

/// A jointly enforced conjunction of typed witness obligations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessCase {
    obligations: Vec<WitnessObligation>,
    strength: ProofStrength,
}

impl WitnessCase {
    /// Construct a nonempty case; reject directly conflicting cardinality and NULL facts.
    ///
    /// This local check is deliberately incomplete: success does not prove cross-row
    /// satisfiability, type compatibility, or producer realizability.
    pub fn new(obligations: Vec<WitnessObligation>, strength: ProofStrength) -> Option<Self> {
        let mut unique = Vec::new();
        for obligation in obligations {
            if !unique.contains(&obligation) {
                unique.push(obligation);
            }
        }
        let obligations = unique;
        if obligations.is_empty()
            || obligations.iter().any(invalid_obligation)
            || directly_conflicts(&obligations)
        {
            return None;
        }
        Some(Self {
            obligations,
            strength,
        })
    }
    /// Requirements that must hold together.
    pub fn obligations(&self) -> &[WitnessObligation] {
        &self.obligations
    }
    /// Strength of the proof relative to the classified result.
    pub fn strength(&self) -> ProofStrength {
        self.strength
    }
}

pub(super) fn invalid_formula(formula: &WitnessFormula) -> bool {
    match formula {
        WitnessFormula::All(children) | WitnessFormula::Any(children) => {
            children.is_empty() || children.iter().any(invalid_formula)
        }
        WitnessFormula::Not(child) => invalid_formula(child),
        WitnessFormula::RowTruth { predicate, .. } => !predicate.is_exact(),
        WitnessFormula::TupleComparison { left, right, .. } => {
            left.is_empty() || left.len() != right.len()
        }
        WitnessFormula::Comparison { .. }
        | WitnessFormula::IsNull { .. }
        | WitnessFormula::StringPrefix { .. } => false,
    }
}

pub(super) fn invalid_obligation(obligation: &WitnessObligation) -> bool {
    match obligation {
        WitnessObligation::Predicate(formula) => invalid_formula(formula),
        WitnessObligation::Rows {
            quantifier,
            bounds,
            predicate,
            closed_world,
            ..
        } => {
            invalid_formula(predicate)
                || (matches!(quantifier, RowQuantifier::Exists) && bounds.minimum() == 0)
                || (matches!(quantifier, RowQuantifier::ForAll) && !closed_world)
        }
        WitnessObligation::NoMatchingPartner { closed_world, .. } => !closed_world,
        WitnessObligation::JoinPair { null_extended, .. } => null_extended.is_some(),
        WitnessObligation::JoinPopulation {
            left_row,
            right_row,
            left_key,
            right_key,
            join_kind,
            pattern,
            left_rows,
            right_rows,
            output_rows,
            closed_world,
        } => {
            !closed_world
                || left_row.instance() == right_row.instance()
                || left_row.relation() != left_key.relation().unwrap_or("")
                || right_row.relation() != right_key.relation().unwrap_or("")
                || pattern.output_rows(*join_kind, *left_rows, *right_rows) != Some(*output_rows)
                || (left_row.relation() == right_row.relation() && left_rows != right_rows)
        }
        WitnessObligation::OperatorPopulation { witness } => !matches!(
            witness,
            crate::outcome_proofs::OutcomeWitness::Groups {
                groups: 1..,
                rows_per_group: 1..,
                ..
            } | crate::outcome_proofs::OutcomeWitness::Ranked { rows: 1.., .. }
                | crate::outcome_proofs::OutcomeWitness::SetTuples { tuples: 1.., .. }
        ),
        WitnessObligation::Group { rows, non_null, .. } => {
            non_null.minimum() > rows.maximum().unwrap_or(u64::MAX)
        }
        WitnessObligation::Ranked {
            strict_unique,
            closed_world,
            order_by,
            ..
        } => !strict_unique || !closed_world || order_by.is_empty(),
        WitnessObligation::Membership { closed_world, .. } => !closed_world,
        WitnessObligation::SetTuple {
            branch_identity,
            tuple_columns,
            matching_rows,
            closed_world,
            ..
        } => {
            branch_identity.is_empty()
                || tuple_columns.is_empty()
                || (*matching_rows == 0 && !closed_world)
        }
        WitnessObligation::SetResultTuple { nulls_equal, .. } => !nulls_equal,
        WitnessObligation::OutputRows { layer_id, .. } => layer_id.is_empty(),
        WitnessObligation::StateRows { relation, .. } => relation.is_empty(),
        WitnessObligation::ClosedWorld { boundary, coverage } => {
            boundary.kind() != GroupBoundaryKind::Physical
                || match coverage {
                    ClosedWorldCoverage::EntireRelation => false,
                    ClosedWorldCoverage::CandidateTuple {
                        branch_identity,
                        columns,
                    } => {
                        branch_identity.is_empty()
                            || columns.is_empty()
                            || columns.iter().any(String::is_empty)
                    }
                }
        }
        WitnessObligation::Producer {
            boundary,
            physical_sources,
        } => {
            boundary.kind() != GroupBoundaryKind::Intermediate
                || physical_sources.iter().any(String::is_empty)
        }
    }
}

pub(super) fn directly_conflicts(obligations: &[WitnessObligation]) -> bool {
    for (index, first) in obligations.iter().enumerate() {
        for second in obligations.iter().skip(index + 1) {
            match (first, second) {
                (
                    WitnessObligation::OutputRows {
                        layer_id: l,
                        bounds: a,
                    },
                    WitnessObligation::OutputRows {
                        layer_id: r,
                        bounds: b,
                    },
                ) if l == r => {
                    if disjoint(*a, *b) {
                        return true;
                    }
                }
                (
                    WitnessObligation::StateRows {
                        relation: l,
                        before: ab,
                        after: aa,
                    },
                    WitnessObligation::StateRows {
                        relation: r,
                        before: bb,
                        after: ba,
                    },
                ) if l == r => {
                    if disjoint(*ab, *bb) || disjoint(*aa, *ba) {
                        return true;
                    }
                }
                (
                    WitnessObligation::Predicate(WitnessFormula::RowTruth {
                        row: a,
                        predicate: ap,
                        truth: at,
                    }),
                    WitnessObligation::Predicate(WitnessFormula::RowTruth {
                        row: b,
                        predicate: bp,
                        truth: bt,
                    }),
                ) if a == b && ap == bp && at != bt => return true,
                _ => {}
            }
        }
    }
    false
}

pub(super) fn disjoint(left: CountBounds, right: CountBounds) -> bool {
    left.maximum.is_some_and(|max| max < right.minimum)
        || right.maximum.is_some_and(|max| max < left.minimum)
}

