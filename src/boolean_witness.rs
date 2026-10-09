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

/// A logical operand sequence with at least two children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanOperands {
    operands: Vec<BooleanRowConstraint>,
}

impl BooleanOperands {
    fn new(operands: Vec<BooleanRowConstraint>) -> Option<Self> {
        (operands.len() >= 2).then_some(Self { operands })
    }

    /// Return the operands in SQL evaluation-tree order.
    pub fn as_slice(&self) -> &[BooleanRowConstraint] {
        &self.operands
    }
}

impl std::ops::Deref for BooleanOperands {
    type Target = [BooleanRowConstraint];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Catalog-backed signed integer bounds used to prove feasible truth outcomes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SignedIntegerEvidence {
    pub(crate) minimum: i128,
    pub(crate) maximum: i128,
    pub(crate) explicitly_nullable: bool,
}

/// A generator-facing, typed source-row boolean expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanRowConstraint {
    /// Every child must hold on the same source row.
    All(BooleanOperands),
    /// At least one child must hold on the same source row.
    Any(BooleanOperands),
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
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<BooleanWitness> {
    let predicate = predicate?;
    if !has_or(predicate) {
        return None;
    }
    let [source] = sources else {
        return None;
    };
    let condition = normalize(predicate, sources, &integer_evidence);
    let mut columns = Vec::new();
    condition.columns(&mut columns);
    let distinct_columns = columns.iter().collect::<BTreeSet<_>>().len();
    let unique_columns = distinct_columns == columns.len();
    let resolved_source = columns
        .iter()
        .all(|column| column.relation() == Some(source.name()));
    let candidate = condition.is_exact()
        && distinct_columns >= 2
        && unique_columns
        && resolved_source;
    let cases = if candidate {
        possible_truths(&condition, &integer_evidence)
    } else {
        BTreeSet::new()
    };
    let unsupported_reason = if distinct_columns < 2 {
        "source columns cannot be proven to form a supported cross-column predicate"
    } else if !resolved_source {
        "predicate columns do not resolve to the same source identity"
    } else if !unique_columns {
        "repeated column conditions require joint feasibility analysis"
    } else {
        "a boolean branch lacks proven source datatype or supported semantics"
    };
    let direction = |truth: BooleanTruthCase, feasible: bool| {
        if candidate && feasible {
            BooleanWitnessDirection::Exact(truth)
        } else {
            BooleanWitnessDirection::Residual {
                reason: if candidate {
                    "no feasible source-row assignment proves the requested truth direction"
                        .to_string()
                } else {
                    unsupported_reason.to_string()
                },
            }
        }
    };
    let directions = (
        direction(BooleanTruthCase::True, cases.contains(&SqlTruth::True)),
        direction(
            BooleanTruthCase::NotTrue,
            cases.contains(&SqlTruth::False) || cases.contains(&SqlTruth::Unknown),
        ),
    );
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
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> BooleanRowConstraint {
    match predicate {
        Predicate::And(logical) | Predicate::Or(logical) => {
            let operands = logical
                .operands()
                .iter()
                .map(|item| normalize(item, sources, integer_evidence))
                .collect();
            match BooleanOperands::new(operands) {
                Some(operands) if matches!(predicate, Predicate::And(_)) => {
                    BooleanRowConstraint::All(operands)
                }
                Some(operands) => BooleanRowConstraint::Any(operands),
                None => residual("logical predicate has fewer than two operands"),
            }
        },
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
                (Expression::Column(column), right) if signed_integer_literal(right).is_some() => {
                    (column, comparison.operator(), right)
                }
                (left, Expression::Column(column)) if signed_integer_literal(left).is_some() => {
                    (column, comparison.operator().reversed(), left)
                }
                _ => return residual("comparison is computed or correlates two source values"),
            };
            if matches!(
                operator,
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ) {
                return residual("null-safe comparison is not part of the integer witness subset");
            }
            let Some(value) = signed_integer_literal(literal) else {
                return residual("comparison literal is not a supported signed integer");
            };
            let column = resolve_column(column, sources);
            if integer_evidence(&column).is_none() {
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

fn signed_integer_literal(expression: &Expression) -> Option<i64> {
    use crate::protocol::UnaryOperator;
    fn number(expression: &Expression) -> Option<i128> {
        match expression {
            Expression::Literal(literal) if literal.literal_type() == LiteralType::Integer => {
                let LiteralValue::Number(value) = literal.value() else {
                    return None;
                };
                value.parse::<i128>().ok()
            }
            Expression::Unary(unary) => match unary.operator() {
                UnaryOperator::Plus => number(unary.operand()),
                UnaryOperator::Minus => number(unary.operand())?.checked_neg(),
                UnaryOperator::BitwiseNot => None,
            },
            _ => None,
        }
    }
    i64::try_from(number(expression)?).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SqlTruth {
    True,
    False,
    Unknown,
}

impl SqlTruth {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::True,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::False,
        }
    }
}

fn possible_truths(
    constraint: &BooleanRowConstraint,
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> BTreeSet<SqlTruth> {
    match constraint {
        BooleanRowConstraint::All(operands) | BooleanRowConstraint::Any(operands) => {
            let mut possible = BTreeSet::from([if matches!(constraint, BooleanRowConstraint::All(_)) {
                SqlTruth::True
            } else {
                SqlTruth::False
            }]);
            for operand in operands.iter() {
                let next = possible_truths(operand, integer_evidence);
                possible = possible
                    .iter()
                    .flat_map(|lhs| {
                        next.iter().map(move |rhs| {
                            if matches!(constraint, BooleanRowConstraint::All(_)) {
                                lhs.and(*rhs)
                            } else {
                                lhs.or(*rhs)
                            }
                        })
                    })
                    .collect();
            }
            possible
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            // Without source nullability evidence both NULL and non-NULL remain possible.
            let may_be_null = integer_evidence(column)
                .is_none_or(|evidence| evidence.explicitly_nullable);
            if may_be_null {
                BTreeSet::from([SqlTruth::True, SqlTruth::False])
            } else {
                BTreeSet::from([if *negated { SqlTruth::True } else { SqlTruth::False }])
            }
        }
        BooleanRowConstraint::IntegerComparison { column, operator, literal } => {
            use crate::protocol::ComparisonOperator as Op;
            let Some(bounds) = integer_evidence(column) else {
                return BTreeSet::new();
            };
            let value = i128::from(*literal);
            let (true_possible, false_possible) = match operator {
                Op::Eq => (bounds.minimum <= value && value <= bounds.maximum,
                           bounds.minimum < value || value < bounds.maximum),
                Op::Neq => (bounds.minimum < value || value < bounds.maximum,
                            bounds.minimum <= value && value <= bounds.maximum),
                Op::Lt => (bounds.minimum < value, bounds.maximum >= value),
                Op::Lte => (bounds.minimum <= value, bounds.maximum > value),
                Op::Gt => (bounds.maximum > value, bounds.minimum <= value),
                Op::Gte => (bounds.maximum >= value, bounds.minimum < value),
                Op::IsDistinctFrom | Op::IsNotDistinctFrom => return BTreeSet::new(),
            };
            let mut possible = BTreeSet::new();
            if true_possible { possible.insert(SqlTruth::True); }
            if false_possible { possible.insert(SqlTruth::False); }
            if bounds.explicitly_nullable { possible.insert(SqlTruth::Unknown); }
            possible
        }
        BooleanRowConstraint::Residual { .. } => BTreeSet::new(),
    }
}
