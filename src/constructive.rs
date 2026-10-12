//! Typed, source-independent constructive obligations.
//!
//! These obligations are *local* operator evidence, not a claim that an entire
//! dependency graph is writable at a physical leaf. TASK-68 owns that proof.
//! Intermediate boundaries must be realized through their producer before use.

use crate::boolean_witness::{BooleanRowConstraint, BooleanTruthCase, BooleanWitnessDirection};
use crate::bundle::{GroupBoundaryKind, ResolvedComposedSemantics};
use crate::group_witness::{GroupAggregate, GroupValueTest, GroupWitnessDirection};
use crate::join_witness::{JoinSide, JoinWitnessDirection, JoinWitnessShape};
use crate::protocol::{ColumnDomain, ColumnRef, ComparisonOperator, SetWitnessDirection};
use crate::subquery_witness::{
    SubqueryCorrelation, SubqueryMembershipCase, SubqueryMembershipDirection,
};
use crate::window_witness::{WindowOrderKey, WindowWitnessDirection};

// Source-independent proof contracts are kept separate from their validation
// and from local operator-witness construction.
mod cases;
mod direction;
mod local;
mod types;
mod witness;

pub use cases::*;
pub use direction::*;
pub use local::*;
pub use types::*;
pub use witness::*;

// Local tests remain close to the public contract, while implementation is modular.
#[cfg(test)]
mod tests {
    use super::*;

    fn bound(min: u64, max: Option<u64>) -> CountBounds {
        CountBounds::new(min, max).expect("valid test bounds")
    }

    fn counted(rows: u64) -> WitnessDirection {
        WitnessDirection::feasible(vec![WitnessCase::new(
            vec![WitnessObligation::OutputRows {
                layer_id: "terminal".to_owned(),
                bounds: bound(rows, Some(rows)),
            }],
            ProofStrength::Equivalent,
        )
        .expect("valid proof")])
        .expect("feasible")
    }

    #[test]
    fn conjunction_detects_shared_conflicting_counts_but_disjunction_preserves_cases() {
        let a = counted(1);
        let b = counted(2);
        assert_eq!(a.all(&b), WitnessDirection::Impossible);
        let WitnessDirection::Feasible(options) = a.any(&b) else {
            panic!("disjoint cases are alternatives");
        };
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].strength(), ProofStrength::Equivalent);
        assert_eq!(counted(1).all(&counted(1)), counted(1).all(&counted(1)));
    }

    #[test]
    fn distinct_sufficient_examples_are_not_a_joint_satisfiability_proof() {
        let sufficient = |count| {
            WitnessDirection::feasible(vec![WitnessCase::new(
                vec![WitnessObligation::OutputRows {
                    layer_id: "terminal".to_string(),
                    bounds: bound(count, Some(count)),
                }],
                ProofStrength::Sufficient,
            )
            .expect("case")])
            .expect("direction")
        };
        assert!(matches!(
            sufficient(1).all(&sufficient(2)),
            WitnessDirection::Residual { .. }
        ));
        assert!(matches!(
            sufficient(1).all(&sufficient(1)),
            WitnessDirection::Feasible(_)
        ));
    }

    #[test]
    fn residual_is_not_impossible_and_sufficient_or_branch_survives_it() {
        let residual = WitnessDirection::Residual {
            reason: "unknown".into(),
        };
        assert_eq!(
            residual.all(&counted(1)),
            WitnessDirection::Residual {
                reason: "conjunctive_operand_not_proven".to_owned(),
            }
        );
        assert_eq!(residual.any(&counted(1)), counted(1));
        assert_eq!(
            residual.all(&WitnessDirection::Impossible),
            WitnessDirection::Impossible
        );
    }

    #[test]
    fn malformed_quantifiers_tuples_and_closed_world_absence_are_unrepresentable() {
        let boundary = WitnessBoundary::new("raw.orders", GroupBoundaryKind::Physical, "layer")
            .expect("boundary");
        let row = RowVariable::new("raw.orders", "o", "candidate").expect("row");
        let value = WitnessTerm::Integer(1);
        let tautology = WitnessFormula::IsNull {
            term: value.clone(),
            negated: true,
        };
        let case = |obligation| WitnessCase::new(vec![obligation], ProofStrength::Sufficient);
        assert!(case(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: bound(0, Some(1)),
            predicate: tautology.clone(),
            closed_world: false,
        })
        .is_none());
        assert!(case(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::Exists,
            bounds: bound(0, Some(1)),
            predicate: tautology.clone(),
            closed_world: true,
        })
        .is_none());
        assert!(case(WitnessObligation::Predicate(
            WitnessFormula::TupleComparison {
                left: vec![value.clone()],
                equal: true,
                right: vec![value.clone(), value.clone()],
            }
        ))
        .is_none());
        assert!(case(WitnessObligation::NoMatchingPartner {
            candidate: row.clone(),
            partner: row.clone(),
            comparison: ComparisonOperator::Eq,
            left: ColumnRef::new(Some("raw.orders".into()), "id".into()),
            right: ColumnRef::new(Some("raw.orders".into()), "id".into()),
            null_extended: None,
            closed_world: false,
        })
        .is_none());
        assert!(case(WitnessObligation::Producer {
            boundary,
            physical_sources: vec!["raw.orders".into()],
        })
        .is_none());
    }

    #[test]
    fn sql_not_does_not_turn_unknown_into_false() {
        let w = ConstructiveWitness {
            operator: WitnessOperator::Boolean,
            origin_layer_id: "layer".into(),
            qualifying: counted(1),
            rejected: counted(2),
        };
        let nullable = w.logical_not(false);
        assert!(matches!(
            nullable.qualifying(),
            WitnessDirection::Residual { .. }
        ));
        assert_eq!(nullable.rejected(), w.qualifying());
        let two_valued = w.logical_not(true);
        assert_eq!(two_valued.qualifying(), w.rejected());
        assert_eq!(two_valued.rejected(), w.qualifying());
    }

    #[test]
    fn and_or_propagate_classifications_independently() {
        let w = ConstructiveWitness {
            operator: WitnessOperator::Boolean,
            origin_layer_id: "layer".into(),
            qualifying: counted(1),
            rejected: counted(2),
        };
        let both = w.logical_and(&w).expect("same layer");
        assert!(matches!(both.qualifying(), WitnessDirection::Feasible(_)));
        assert!(matches!(both.rejected(), WitnessDirection::Feasible(_)));
        let either = w.logical_or(&w).expect("same layer");
        assert_eq!(either.qualifying(), w.qualifying());
        assert_eq!(either.rejected(), w.rejected());
        let other = ConstructiveWitness {
            origin_layer_id: "different".into(),
            ..w.clone()
        };
        assert!(w.logical_or(&other).is_none());
    }

    #[test]
    fn invalid_bounds_and_empty_names_cannot_be_created() {
        assert!(CountBounds::new(3, Some(2)).is_none());
        assert!(RowVariable::new("orders", "", "candidate").is_none());
        assert!(WitnessBoundary::new("x", GroupBoundaryKind::Physical, "").is_none());
        assert!(WitnessCase::new(Vec::new(), ProofStrength::Sufficient).is_none());
        assert!(WitnessDirection::feasible(Vec::new()).is_none());
    }

    #[test]
    fn contradictory_output_and_state_counts_are_rejected() {
        assert!(WitnessCase::new(
            vec![
                WitnessObligation::OutputRows {
                    layer_id: "l".into(),
                    bounds: bound(3, Some(3))
                },
                WitnessObligation::OutputRows {
                    layer_id: "l".into(),
                    bounds: bound(0, Some(2))
                },
            ],
            ProofStrength::Sufficient
        )
        .is_none());
        assert!(WitnessCase::new(
            vec![
                WitnessObligation::StateRows {
                    relation: "t".into(),
                    before: bound(0, Some(0)),
                    after: bound(2, Some(3))
                },
                WitnessObligation::StateRows {
                    relation: "t".into(),
                    before: bound(0, Some(0)),
                    after: bound(4, Some(5))
                },
            ],
            ProofStrength::Sufficient
        )
        .is_none());
    }

    #[test]
    fn unknown_is_not_equivalent_to_boolean_false() {
        let row = RowVariable::new("orders", "o", "candidate").expect("valid row");
        let predicate = BooleanRowConstraint::NullTest {
            column: ColumnRef::new(Some("orders".to_string()), "amount".to_string()),
            negated: false,
        };
        let truth = |truth| {
            WitnessObligation::Predicate(WitnessFormula::RowTruth {
                row: row.clone(),
                predicate: predicate.clone(),
                truth,
            })
        };
        assert!(WitnessCase::new(
            vec![
                truth(BooleanTruthCase::True),
                truth(BooleanTruthCase::NotTrue),
            ],
            ProofStrength::Sufficient
        )
        .is_none());
    }

    #[test]
    fn necessity_does_not_claim_sufficient_constructibility() {
        let case = WitnessCase::new(
            vec![WitnessObligation::OutputRows {
                layer_id: "x".into(),
                bounds: bound(1, Some(1)),
            }],
            ProofStrength::Necessary,
        )
        .expect("valid necessary condition");
        assert!(WitnessDirection::feasible(vec![case]).is_none());
    }
}
