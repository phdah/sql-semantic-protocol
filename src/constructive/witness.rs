//! Local operator identity and paired witness directions.

use super::*;

/// Origin of one typed operator proof in the resolved dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessOperator {
    /// Coupled WHERE predicate.
    Boolean,
    /// Join matched/unmatched row membership.
    Join,
    /// Grouped HAVING.
    Group,
    /// Window ranking.
    Window,
    /// Subquery membership.
    Subquery,
    /// SQL set operation.
    Set,
}

impl WitnessOperator {
    /// Stable schema tag.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Join => "join",
            Self::Group => "group",
            Self::Window => "window",
            Self::Subquery => "subquery",
            Self::Set => "set",
        }
    }
}

/// Canonical typed local witness with two independent proof directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructiveWitness {
    pub(super) operator: WitnessOperator,
    pub(super) origin_layer_id: String,
    pub(super) qualifying: WitnessDirection,
    pub(super) rejected: WitnessDirection,
}

impl ConstructiveWitness {
    /// Operator that supplied the local proof.
    pub fn operator(&self) -> WitnessOperator {
        self.operator
    }
    /// Layer supplying provenance.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }
    /// Matching classification, independently proven.
    pub fn qualifying(&self) -> &WitnessDirection {
        &self.qualifying
    }
    /// Rejected classification, independently proven.
    pub fn rejected(&self) -> &WitnessDirection {
        &self.rejected
    }

    /// SQL AND of two classifications introduced at the same operator boundary.
    ///
    /// TRUE requires both inputs TRUE; FALSE or UNKNOWN requires at least
    /// one input NOT TRUE. Both directions retain sufficient witness cases.
    pub fn logical_and(&self, other: &Self) -> Option<Self> {
        if self.origin_layer_id != other.origin_layer_id {
            return None;
        }
        Some(Self {
            operator: WitnessOperator::Boolean,
            origin_layer_id: self.origin_layer_id.clone(),
            qualifying: self.qualifying.all(&other.qualifying),
            rejected: self.rejected.any(&other.rejected),
        })
    }

    /// SQL OR of two classifications introduced at the same operator boundary.
    pub fn logical_or(&self, other: &Self) -> Option<Self> {
        if self.origin_layer_id != other.origin_layer_id {
            return None;
        }
        Some(Self {
            operator: WitnessOperator::Boolean,
            origin_layer_id: self.origin_layer_id.clone(),
            qualifying: self.qualifying.any(&other.qualifying),
            rejected: self.rejected.all(&other.rejected),
        })
    }

    /// SQL NOT at this boundary. A nullable expression's NOT TRUE is not
    /// equivalent to FALSE: an UNKNOWN input remains UNKNOWN under SQL NOT.
    ///
    /// An explicitly proven two-valued expression permits swapping directions.
    /// Otherwise qualifying is residual and known TRUE cases remain sufficient
    /// for a rejected NOT, but an impossible TRUE does not prove NOT impossible.
    pub fn logical_not(&self, proven_two_valued: bool) -> Self {
        let (qualifying, rejected) = if proven_two_valued {
            (self.rejected.clone(), self.qualifying.clone())
        } else {
            (
                WitnessDirection::residual("sql_not_requires_false_vs_unknown_proof"),
                match &self.qualifying {
                    WitnessDirection::Feasible(_) => self.qualifying.clone(),
                    _ => WitnessDirection::residual("nullable_not_rejection_unproven"),
                },
            )
        };
        Self {
            operator: WitnessOperator::Boolean,
            origin_layer_id: self.origin_layer_id.clone(),
            qualifying,
            rejected,
        }
    }
}
