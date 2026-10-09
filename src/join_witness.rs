//! Typed matched and unmatched join row witnesses.
//!
//! Witnesses describe source obligations. They never assume key uniqueness, so multi-matches
//! may produce multiple result rows, and a non-match means the absence of *any* matching partner.

use crate::bundle::ComposedJoinColumn;
use crate::protocol::{ComparisonOperator, JoinKind};

/// Which input contributes a source row to one witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinWitnessShape {
    /// One row from each input satisfies the comparison.
    Matched,
    /// A left input row has no right partner satisfying the comparison.
    LeftUnmatched,
    /// A right input row has no left partner satisfying the comparison.
    RightUnmatched,
}

impl JoinWitnessShape {
    /// Stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::LeftUnmatched => "left_unmatched",
            Self::RightUnmatched => "right_unmatched",
        }
    }

    /// Minimum number of matching partners required for the selected source row.
    pub const fn min_matches(self) -> usize {
        match self {
            Self::Matched => 1,
            Self::LeftUnmatched | Self::RightUnmatched => 0,
        }
    }

    /// Maximum matching partners required, or none when unbounded.
    pub const fn max_matches(self) -> Option<usize> {
        match self {
            Self::Matched => None,
            Self::LeftUnmatched | Self::RightUnmatched => Some(0),
        }
    }
}

/// A source row may be absent from the output only on the indicated null-extended side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinSide {
    /// The absent input is the left side.
    Left,
    /// The absent input is the right side.
    Right,
}

impl JoinSide {
    /// Stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// One qualifying or rejected source-row witness, including optional null extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinWitnessCase {
    shape: JoinWitnessShape,
    null_extended_side: Option<JoinSide>,
}

impl JoinWitnessCase {
    pub(crate) fn new(shape: JoinWitnessShape, null_extended_side: Option<JoinSide>) -> Self {
        Self { shape, null_extended_side }
    }

    /// Matching or unmatched relation-instance obligation.
    pub fn shape(&self) -> JoinWitnessShape {
        self.shape
    }

    /// The side emitted with all NULL columns for a preserved unmatched row, if any.
    pub fn null_extended_side(&self) -> Option<JoinSide> {
        self.null_extended_side
    }
}

/// A witness direction is exact, impossible, or deliberately residual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinWitnessDirection {
    /// Every listed case is provable under SQL's ordinary three-valued ON semantics.
    Exact(Vec<JoinWitnessCase>),
    /// No row of this classification can arise from this join in isolation.
    Impossible,
    /// The analyzer cannot prove a consumer-generatable obligation.
    Residual { reason: String },
}

/// Witnesses for a single join at a particular transformation layer.
///
/// The two endpoints identify physical leaf columns and distinct source instances even for
/// self-joins. The comparison is evaluated using SQL three-valued logic: UNKNOWN does not
/// match. A matched witness must therefore use non-NULL values for ordinary comparisons.
/// Unmatched cases require zero TRUE comparisons, not necessarily empty inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinWitness {
    kind: JoinKind,
    left: Option<ComposedJoinColumn>,
    right: Option<ComposedJoinColumn>,
    comparison: Option<ComparisonOperator>,
    qualifying: JoinWitnessDirection,
    rejected: JoinWitnessDirection,
    origin_layer_id: String,
}

impl JoinWitness {
    pub(crate) fn residual(kind: JoinKind, origin_layer_id: String, reason: &str) -> Self {
        let residual = || JoinWitnessDirection::Residual { reason: reason.to_string() };
        Self {
            kind, left: None, right: None, comparison: None,
            qualifying: residual(), rejected: residual(), origin_layer_id,
        }
    }

    pub(crate) fn exact(
        kind: JoinKind,
        left: ComposedJoinColumn,
        right: ComposedJoinColumn,
        comparison: ComparisonOperator,
        origin_layer_id: String,
    ) -> Self {
        use JoinSide::{Left, Right};
        use JoinWitnessShape::{LeftUnmatched, Matched, RightUnmatched};
        let exact = |entries: &[(JoinWitnessShape, Option<JoinSide>)]| {
            JoinWitnessDirection::Exact(entries.iter().map(|(shape, side)| {
                JoinWitnessCase::new(*shape, *side)
            }).collect())
        };
        let matched = (Matched, None);
        let left_unmatched = (LeftUnmatched, None);
        let right_unmatched = (RightUnmatched, None);
        let (qualifying, rejected) = match kind {
            JoinKind::Inner => (
                exact(&[matched]),
                exact(&[left_unmatched, right_unmatched]),
            ),
            JoinKind::Left => (
                exact(&[matched, (LeftUnmatched, Some(Right))]),
                exact(&[right_unmatched]),
            ),
            JoinKind::Right => (
                exact(&[matched, (RightUnmatched, Some(Left))]),
                exact(&[left_unmatched]),
            ),
            JoinKind::Full => (
                exact(&[
                    matched,
                    (LeftUnmatched, Some(Right)),
                    (RightUnmatched, Some(Left)),
                ]),
                JoinWitnessDirection::Impossible,
            ),
            JoinKind::LeftSemi => (
                exact(&[matched]), exact(&[left_unmatched]),
            ),
            JoinKind::RightSemi => (
                exact(&[matched]), exact(&[right_unmatched]),
            ),
            JoinKind::LeftAnti => (
                exact(&[left_unmatched]), exact(&[matched]),
            ),
            JoinKind::RightAnti => (
                exact(&[right_unmatched]), exact(&[matched]),
            ),
            JoinKind::Cross | JoinKind::Unknown => {
                return Self::residual(kind, origin_layer_id, "unsupported_join_kind");
            }
        };
        Self {
            kind, left: Some(left), right: Some(right), comparison: Some(comparison),
            qualifying, rejected, origin_layer_id,
        }
    }

    /// SQL join kind.
    pub fn kind(&self) -> JoinKind { self.kind }
    /// Physical left comparison endpoint, if proven.
    pub fn left(&self) -> Option<&ComposedJoinColumn> { self.left.as_ref() }
    /// Physical right comparison endpoint, if proven.
    pub fn right(&self) -> Option<&ComposedJoinColumn> { self.right.as_ref() }
    /// Canonical SQL comparison, if proven.
    pub fn comparison(&self) -> Option<ComparisonOperator> { self.comparison }
    /// Qualifying row obligations.
    pub fn qualifying(&self) -> &JoinWitnessDirection { &self.qualifying }
    /// Rejected row obligations.
    pub fn rejected(&self) -> &JoinWitnessDirection { &self.rejected }
    /// Originating transformation layer.
    pub fn origin_layer_id(&self) -> &str { &self.origin_layer_id }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outer_join_has_proven_null_extension_and_unbounded_matches() {
        let endpoint = |instance: &str| ComposedJoinColumn::new(
            "orders".to_owned(), "id".to_owned(), instance.to_owned()
        );
        let witness = JoinWitness::exact(
            JoinKind::Left, endpoint("a"), endpoint("b"), ComparisonOperator::Eq,
            "layer".to_owned()
        );
        assert_ne!(witness.left(), witness.right());
        let JoinWitnessDirection::Exact(cases) = witness.qualifying() else {
            panic!("expected exact witness");
        };
        assert_eq!(cases[0].shape().min_matches(), 1);
        assert_eq!(cases[0].shape().max_matches(), None);
        assert_eq!(cases[1].null_extended_side(), Some(JoinSide::Right));
        assert_eq!(cases[1].shape().max_matches(), Some(0));
    }
}
