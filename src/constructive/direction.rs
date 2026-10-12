//! Independent qualifying/rejected proof directions.

use super::*;

/// Classification is independent in the qualifying and rejected directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessDirection {
    /// One of these separately sufficient constructions is available.
    Feasible(Vec<WitnessCase>),
    /// The specified classification is impossible at this local operator.
    Impossible,
    /// No constructive proof exists; never assume this means impossible.
    Residual { reason: String },
}

impl WitnessDirection {
    /// Validate that every feasible direction has at least one sufficient case.
    pub fn feasible(cases: Vec<WitnessCase>) -> Option<Self> {
        if cases.is_empty()
            || cases.iter().any(|case| {
                !matches!(
                    case.strength(),
                    ProofStrength::Sufficient | ProofStrength::Equivalent
                )
            })
        {
            return None;
        }
        Some(Self::Feasible(cases))
    }

    /// Conjoin sufficient cases at the same proof boundary, preserving shared row variables.
    ///
    /// A finite 256-pair bound prevents explosive proof enumeration. Unknown
    /// satisfiability remains residual, while proven direct contradictions are impossible.
    pub fn all(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Impossible, _) | (_, Self::Impossible) => Self::Impossible,
            (Self::Residual { .. }, _) | (_, Self::Residual { .. }) => {
                Self::residual("conjunctive_operand_not_proven")
            }
            (Self::Feasible(left), Self::Feasible(right)) => {
                if left.len().checked_mul(right.len()).is_none_or(|n| n > 256) {
                    return Self::residual("conjunctive_case_limit");
                }
                let fully_equivalent = left.len() == 1
                    && right.len() == 1
                    && left[0].strength() == ProofStrength::Equivalent
                    && right[0].strength() == ProofStrength::Equivalent;
                let mut cases = Vec::new();
                let mut unproved = false;
                for a in left {
                    for b in right {
                        // Sufficient constructions are examples, not exhaustive
                        // model sets. Two different examples cannot be merged
                        // without a proof of joint satisfiability.
                        if a.obligations() != b.obligations() {
                            unproved = true;
                            continue;
                        }
                        let strength = if a.strength() == ProofStrength::Equivalent
                            && b.strength() == ProofStrength::Equivalent
                        {
                            ProofStrength::Equivalent
                        } else {
                            ProofStrength::Sufficient
                        };
                        if let Some(case) = WitnessCase::new(a.obligations().to_vec(), strength) {
                            if !cases.contains(&case) {
                                cases.push(case);
                            }
                        }
                    }
                }
                if !cases.is_empty() {
                    Self::Feasible(cases)
                } else if fully_equivalent
                    && directly_conflicts(
                        &left[0]
                            .obligations()
                            .iter()
                            .cloned()
                            .chain(right[0].obligations().iter().cloned())
                            .collect::<Vec<_>>(),
                    )
                {
                    Self::Impossible
                } else if unproved {
                    Self::residual("conjunctive_realization_unproven")
                } else {
                    Self::residual("conjunctive_case_satisfiability_unproven")
                }
            }
        }
    }

    /// Disjoin sufficient alternatives; one proven branch suffices even when the other is residual.
    pub fn any(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Feasible(a), Self::Feasible(b)) => {
                let mut cases = a.clone();
                for case in b {
                    if !cases.contains(case) {
                        cases.push(case.clone());
                    }
                }
                Self::Feasible(cases)
            }
            (Self::Feasible(_), _) => self.clone(),
            (_, Self::Feasible(_)) => other.clone(),
            (Self::Impossible, Self::Impossible) => Self::Impossible,
            _ => Self::residual("disjunctive_operand_not_proven"),
        }
    }

    fn residual(reason: &str) -> Self {
        Self::Residual {
            reason: reason.to_string(),
        }
    }
}
