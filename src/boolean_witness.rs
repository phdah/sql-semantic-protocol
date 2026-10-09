//! Coupled source-row predicates that cannot be represented by independent column domains.
//!
//! Proof is deliberately narrow. An exact direction constrains a *single row* at the
//! originating relation boundary, retaining the logical tree and SQL's three-valued
//! WHERE semantics. No independent Cartesian sampling or SQL reparsing is implied.

use std::collections::BTreeSet;

use crate::domain::resolve_column;
use crate::protocol::{
    ColumnRef, ComparisonOperator, Expression, LiteralType, LiteralValue, Predicate, SourceRelation,
};

/// A generator-facing, typed source-row boolean expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanRowConstraint {
    /// Every child must hold on the same source row.
    All(Vec<BooleanRowConstraint>),
    /// At least one child must hold on the same source row.
    Any(Vec<BooleanRowConstraint>),
    /// Test a source value for NULL or NOT NULL.
    NullTest {
        /// Resolved source column.
        column: ColumnRef,
        /// Whether the operand is IS NOT NULL.
        negated: bool,
    },
    /// Compare an integer source value against a signed integer literal.
    IntegerComparison {
        /// Resolved source column.
        column: ColumnRef,
        /// SQL comparison operator.
        operator: ComparisonOperator,
        /// Integer literal, with source datatype verified separately.
        literal: i64,
    },
    /// A subtree that cannot be proved without SQL execution or assumptions.
    Residual {
        /// Why this subtree is not an exact source-row constraint.
        reason: String,
    },
}

impl BooleanRowConstraint {
    /// Whether this complete logical tree is within the proven source-row subset.
    pub fn is_exact(&self) -> bool {
        match self {
            Self::All(children) | Self::Any(children) => {
                !children.is_empty() && children.iter().all(Self::is_exact)
            }
            Self::NullTest { .. } | Self::IntegerComparison { .. } => true,
            Self::Residual { .. } => false,
        }
    }

    fn columns(&self, output: &mut Vec<ColumnRef>) {
        match self {
            Self::All(children) | Self::Any(children) => {
                for child in children {
                    child.columns(output);
                }
            }
            Self::NullTest { column, .. } | Self::IntegerComparison { column, .. } => {
                output.push(column.clone());
            }
            Self::Residual { .. } => {}
        }
    }
}

/// A truth obligation, evaluated with SQL's three-valued logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanTruthCase {
    /// Predicate evaluates to TRUE, and WHERE selects the row.
    True,
    /// Predicate evaluates to FALSE or UNKNOWN, and WHERE rejects the row.
    NotTrue,
}

impl BooleanTruthCase {
    /// Stable protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::True => "true",
            Self::NotTrue => "not_true",
        }
    }
}

/// Independent proof statuses for qualifying and rejected source rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanWitnessDirection {
    /// The complete typed boolean expression is a precise row obligation.
    Exact(BooleanTruthCase),
    /// Source-row constructibility cannot be proven.
    Residual {
        /// Explicit conservative diagnostic.
        reason: String,
    },
}

/// One coupled predicate at its original source boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanWitness {
    source_relation: String,
    condition: BooleanRowConstraint,
    qualifying: BooleanWitnessDirection,
    rejected: BooleanWitnessDirection,
}

impl BooleanWitness {
    /// The single source relation whose values must be solved jointly.
    pub fn source_relation(&self) -> &str {
        &self.source_relation
    }

    /// Typed predicate tree. The entire tree applies to one row.
    pub fn condition(&self) -> &BooleanRowConstraint {
        &self.condition
    }

    /// Obligation that makes the filtered row survive.
    pub fn qualifying(&self) -> &BooleanWitnessDirection {
        &self.qualifying
    }

    /// Obligation that makes the filtered row fail WHERE (FALSE or UNKNOWN).
    pub fn rejected(&self) -> &BooleanWitnessDirection {
        &self.rejected
    }
}

/// Establish an operator-local witness without changing whole-query exactness.
///
/// Signed integer comparisons require authoritative source datatype evidence, and
/// every supported leaf must bind unambiguously to the same relation. Repeated
/// references remain residual until their joint feasibility can be proved.
pub(crate) fn analyze(
    predicate: Option<&Predicate>,
    sources: &[SourceRelation],
    integer_type_accepts: impl Fn(&ColumnRef, i64) -> bool,
) -> Option<BooleanWitness> {
    let predicate = predicate?;
    if !has_or(predicate) {
        return None;
    }
    let [source] = sources else {
        return None;
    };
    let condition = normalize(predicate, sources, &integer_type_accepts);
    let mut columns = Vec::new();
    condition.columns(&mut columns);
    // There must actually be a cross-column logical relation to preserve.
    if columns.iter().collect::<BTreeSet<_>>().len() < 2 {
        return None;
    }
    let unique_columns = columns.iter().collect::<BTreeSet<_>>().len() == columns.len();
    let resolved_source = columns
        .iter()
        .all(|column| column.relation() == Some(source.name()));
    let directions = if condition.is_exact() && unique_columns && resolved_source {
        (
            BooleanWitnessDirection::Exact(BooleanTruthCase::True),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue),
        )
    } else {
        let reason = if !resolved_source {
            "predicate columns do not resolve to the same source identity"
        } else if !unique_columns {
            "repeated column conditions require joint feasibility analysis"
        } else {
            "a boolean branch lacks proven source datatype or supported semantics"
        };
        let direction = BooleanWitnessDirection::Residual {
            reason: reason.to_string(),
        };
        (direction.clone(), direction)
    };
    Some(BooleanWitness {
        source_relation: source.name().to_string(),
        condition,
        qualifying: directions.0,
        rejected: directions.1,
    })
}

fn has_or(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Or(_) => true,
        Predicate::And(logical) => logical.operands().iter().any(has_or),
        _ => false,
    }
}

fn normalize(
    predicate: &Predicate,
    sources: &[SourceRelation],
    integer_type_accepts: &impl Fn(&ColumnRef, i64) -> bool,
) -> BooleanRowConstraint {
    match predicate {
        Predicate::And(logical) => BooleanRowConstraint::All(
            logical
                .operands()
                .iter()
                .map(|item| normalize(item, sources, integer_type_accepts))
                .collect(),
        ),
        Predicate::Or(logical) => BooleanRowConstraint::Any(
            logical
                .operands()
                .iter()
                .map(|item| normalize(item, sources, integer_type_accepts))
                .collect(),
        ),
        Predicate::IsNull(test) => {
            if let Expression::Column(column) = test.expression() {
                BooleanRowConstraint::NullTest {
                    column: resolve_column(column, sources),
                    negated: test.negated(),
                }
            } else {
                residual("NULL test involves a computed expression")
            }
        }
        Predicate::Comparison(comparison) => {
            let (column, operator, literal) = match (comparison.left(), comparison.right()) {
                (Expression::Column(column), Expression::Literal(literal)) => {
                    (column, comparison.operator(), literal)
                }
                (Expression::Literal(literal), Expression::Column(column)) => {
                    (column, comparison.operator().reversed(), literal)
                }
                _ => return residual("comparison is computed or correlates two source values"),
            };
            if matches!(
                operator,
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ) {
                return residual("null-safe comparison is not part of the integer witness subset");
            }
            let (LiteralType::Integer, LiteralValue::Number(value)) =
                (literal.literal_type(), literal.value())
            else {
                return residual("comparison literal is not a signed integer");
            };
            let Ok(value) = value.parse::<i64>() else {
                return residual("comparison integer is outside the supported signed range");
            };
            let column = resolve_column(column, sources);
            if !integer_type_accepts(&column, value) {
                return residual("source datatype does not prove exact signed integer comparison");
            }
            BooleanRowConstraint::IntegerComparison {
                column,
                operator,
                literal: value,
            }
        }
        Predicate::Not(_) => residual("logical NOT needs explicit three-valued inversion"),
        _ => residual("computed, function, collation, cast or other predicate is not invertible"),
    }
}

fn residual(reason: &str) -> BooleanRowConstraint {
    BooleanRowConstraint::Residual {
        reason: reason.to_string(),
    }
}
