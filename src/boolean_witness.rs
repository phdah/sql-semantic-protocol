//! Coupled source-row predicates that cannot be represented by independent column domains.
//!
//! Proof is deliberately narrow. An exact direction constrains a *single row* at the
//! originating relation boundary, retaining the logical tree and SQL's three-valued
//! WHERE semantics. No independent Cartesian sampling or SQL reparsing is implied.

use std::collections::{BTreeMap, BTreeSet};

use crate::constraints::{
    ConstraintEnforcement, ConstraintValue, RelationConstraint, RelationConstraintSet,
};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SignedIntegerEvidence {
    pub(crate) minimum: i128,
    pub(crate) maximum: i128,
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

    fn mapped_columns(&self, mapped: &BTreeMap<ColumnRef, ColumnRef>) -> Option<Self> {
        match self {
            Self::All(items) => Some(Self::All(BooleanOperands::new(
                items
                    .iter()
                    .map(|item| item.mapped_columns(mapped))
                    .collect::<Option<Vec<_>>>()?,
            )?)),
            Self::Any(items) => Some(Self::Any(BooleanOperands::new(
                items
                    .iter()
                    .map(|item| item.mapped_columns(mapped))
                    .collect::<Option<Vec<_>>>()?,
            )?)),
            Self::NullTest { column, negated } => Some(Self::NullTest {
                column: mapped.get(column)?.clone(),
                negated: *negated,
            }),
            Self::IntegerComparison {
                column,
                operator,
                literal,
            } => Some(Self::IntegerComparison {
                column: mapped.get(column)?.clone(),
                operator: *operator,
                literal: *literal,
            }),
            Self::Residual { reason } => Some(Self::Residual {
                reason: reason.clone(),
            }),
        }
    }

    fn columns(&self, output: &mut Vec<ColumnRef>) {
        match self {
            Self::All(children) | Self::Any(children) => {
                for child in children.iter() {
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
    // Non-emitted source type bounds retained to recheck proofs when constraints arrive later.
    integer_bounds: BTreeMap<ColumnRef, SignedIntegerEvidence>,
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

    /// Replace intermediate references with proven identity-only physical source columns.
    ///
    /// Every column must reach the same physical relation without a cast,
    /// computation, ambiguous projection or lossy transformation.
    pub(crate) fn mapped_to_physical(
        &self,
        mut resolve: impl FnMut(&ColumnRef) -> Option<ColumnRef>,
    ) -> Option<Self> {
        let mut columns = Vec::new();
        self.condition.columns(&mut columns);
        let mut mapping = BTreeMap::new();
        for column in columns {
            if !mapping.contains_key(&column) {
                mapping.insert(column.clone(), resolve(&column)?);
            }
        }
        let relations = mapping
            .values()
            .filter_map(ColumnRef::relation)
            .collect::<BTreeSet<_>>();
        let mut relations = relations.into_iter();
        let relation = relations.next()?;
        if relations.next().is_some() {
            return None;
        }
        if mapping.values().any(|column| column.relation().is_none()) {
            return None;
        }
        let mut integer_bounds = BTreeMap::new();
        for (column, evidence) in &self.integer_bounds {
            let mapped = mapping.get(column)?.clone();
            if integer_bounds
                .insert(mapped, *evidence)
                .is_some_and(|old| old != *evidence)
            {
                return None;
            }
        }
        Some(Self {
            source_relation: relation.to_string(),
            condition: self.condition.mapped_columns(&mapping)?,
            qualifying: self.qualifying.clone(),
            rejected: self.rejected.clone(),
            integer_bounds,
        })
    }

    /// Recheck witness directions against enforced source constraints supplied later by adapters.
    ///
    /// This can only downgrade proofs. It never upgrades a previously residual direction,
    /// and unknown or unsupported constraint evidence cannot invent source rows.
    pub(crate) fn restrict_with_schema_constraints(&mut self, sets: &[RelationConstraintSet]) {
        let Some(set) = sets
            .iter()
            .find(|set| set.relation() == self.source_relation)
        else {
            return;
        };
        let mut restrictions = BTreeMap::<String, ColumnRestriction>::new();
        let mut unsupported = !set.diagnostics().is_empty();
        for constraint in set.constraints() {
            if !constraint
                .evidence()
                .iter()
                .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
            {
                // Unknown enforcement is not evidence that a declared restriction holds.
                // A generated row cannot be certified without resolving that uncertainty.
                unsupported |= constraint
                    .evidence()
                    .iter()
                    .any(|e| e.enforcement() == ConstraintEnforcement::Unknown);
                continue;
            }
            match constraint {
                RelationConstraint::PrimaryKey(key) => {
                    for column in key.columns() {
                        restrictions.entry(column.clone()).or_default().not_null = true;
                    }
                }
                RelationConstraint::NotNull(item) => {
                    restrictions
                        .entry(item.column().to_string())
                        .or_default()
                        .not_null = true;
                }
                RelationConstraint::AcceptedValues(item) => {
                    let restriction = restrictions.entry(item.column().to_string()).or_default();
                    let mut accepted = BTreeSet::new();
                    for value in item.values() {
                        match value {
                            ConstraintValue::Integer(value) => {
                                accepted.insert(i128::from(*value));
                            }
                            ConstraintValue::UnsignedInteger(value) => {
                                accepted.insert(i128::from(*value));
                            }
                            ConstraintValue::Null => {}
                            _ => unsupported = true,
                        }
                    }
                    restriction.accepted = Some(match restriction.accepted.take() {
                        Some(existing) => existing.intersection(&accepted).copied().collect(),
                        None => accepted,
                    });
                }
                RelationConstraint::UniqueKey(_) => {
                    // Uniqueness imposes no additional restriction on an individual row.
                }
                RelationConstraint::ForeignKey(_) => {
                    // A standalone row is not proven constructible without referenced rows.
                    unsupported = true;
                }
            }
        }
        // Even constraints on columns absent from the predicate can make the whole
        // relation unsatisfiable.
        unsupported |= restrictions
            .values()
            .any(|item| item.not_null && item.accepted.as_ref().is_some_and(BTreeSet::is_empty));
        let cases = if unsupported {
            BTreeSet::new()
        } else {
            possible_joint_truths(
                &self.condition,
                &|column| self.integer_bounds.get(column).copied(),
                Some(&restrictions),
            )
        };
        if matches!(self.qualifying, BooleanWitnessDirection::Exact(_))
            && !cases.contains(&SqlTruth::True)
        {
            self.qualifying = BooleanWitnessDirection::Residual {
                reason: "no provable qualifying assignment respects source constraints".to_string(),
            };
        }
        if matches!(self.rejected, BooleanWitnessDirection::Exact(_))
            && !cases.contains(&SqlTruth::False)
            && !cases.contains(&SqlTruth::Unknown)
        {
            self.rejected = BooleanWitnessDirection::Residual {
                reason: "no provable rejected assignment respects source constraints".to_string(),
            };
        }
    }
}

#[derive(Default)]
struct ColumnRestriction {
    not_null: bool,
    accepted: Option<BTreeSet<i128>>,
}

/// Establish an operator-local witness without changing whole-query exactness.
///
/// Signed integer comparisons require authoritative source datatype evidence, and
/// every supported leaf must bind unambiguously to the same relation. Repeated
/// references are evaluated together rather than independently.
pub(crate) fn analyze(
    predicate: Option<&Predicate>,
    sources: &[SourceRelation],
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<BooleanWitness> {
    let predicate = predicate?;
    if !has_logical_predicate(predicate) {
        return None;
    }
    let [source] = sources else {
        return None;
    };
    let condition = normalize(predicate, sources, &integer_evidence);
    let mut columns = Vec::new();
    condition.columns(&mut columns);
    let resolved_source = columns
        .iter()
        .all(|column| column.relation() == Some(source.name()));
    let candidate = condition.is_exact() && !columns.is_empty() && resolved_source;
    // A single truth-partition solver is used for every correlated expression.
    // Treating even different columns independently would be wrong once source
    // constraints or repeated appearances narrow the possible assignments.
    let cases = if candidate {
        possible_joint_truths(&condition, &integer_evidence, None)
    } else {
        BTreeSet::new()
    };
    let unsupported_reason = if columns.is_empty() {
        "predicate has no resolved source columns"
    } else if !resolved_source {
        "predicate columns do not resolve to the same source identity"
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
    let integer_bounds = columns
        .into_iter()
        .filter_map(|column| integer_evidence(&column).map(|bounds| (column, bounds)))
        .collect();
    Some(BooleanWitness {
        source_relation: source.name().to_string(),
        condition,
        qualifying: directions.0,
        rejected: directions.1,
        integer_bounds,
    })
}

fn has_logical_predicate(predicate: &Predicate) -> bool {
    matches!(predicate, Predicate::And(_) | Predicate::Or(_))
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
        }
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
            let (column, operator, literal) =
                if let Some(column) = invertible_integer_column(comparison.left(), sources, integer_evidence) {
                    (column, comparison.operator(), comparison.right())
                } else if let Some(column) = invertible_integer_column(comparison.right(), sources, integer_evidence) {
                    (column, comparison.operator().reversed(), comparison.left())
                } else {
                    return residual("comparison is noninvertible or correlates two source values");
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

// Ordinary CASTs to an equal or wider signed integer type preserve every
// source value, order, and NULL. A narrowing cast can truncate/overflow and
// must never be assumed invertible.
fn invertible_integer_column<'a>(
    expression: &'a Expression,
    sources: &[SourceRelation],
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<&'a crate::protocol::ColumnExpression> {
    match expression {
        Expression::SignedIntegerCast(cast) => {
            let column = identity_integer_column(cast.expression())?;
            let source = resolve_column(column, sources);
            let bounds = evidence(&source)?;
            let magnitude = 1_i128 << (u32::from(cast.target_bits()) - 1);
            (bounds.minimum >= -magnitude && bounds.maximum < magnitude).then_some(column)
        }
        other => identity_integer_column(other),
    }
}

// Unary plus and arithmetic with zero are identity transformations on
// signed integer columns, including SQL NULL. Other arithmetic can overflow
// or change source values and therefore must remain residual.
fn identity_integer_column(expression: &Expression) -> Option<&crate::protocol::ColumnExpression> {
    use crate::protocol::{BinaryOperator, UnaryOperator};
    match expression {
        Expression::Column(column) => Some(column),
        Expression::Unary(unary) if unary.operator() == UnaryOperator::Plus => {
            identity_integer_column(unary.operand())
        }
        Expression::Binary(binary) => match binary.operator() {
            BinaryOperator::Add if signed_integer_literal(binary.right()) == Some(0) => {
                identity_integer_column(binary.left())
            }
            BinaryOperator::Add if signed_integer_literal(binary.left()) == Some(0) => {
                identity_integer_column(binary.right())
            }
            BinaryOperator::Subtract if signed_integer_literal(binary.right()) == Some(0) => {
                identity_integer_column(binary.left())
            }
            _ => None,
        },
        _ => None,
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

fn collect_comparison_literals(
    condition: &BooleanRowConstraint,
    thresholds: &mut std::collections::BTreeMap<ColumnRef, BTreeSet<i128>>,
) {
    match condition {
        BooleanRowConstraint::All(operands) | BooleanRowConstraint::Any(operands) => {
            for operand in operands.iter() {
                collect_comparison_literals(operand, thresholds);
            }
        }
        BooleanRowConstraint::NullTest { column, .. } => {
            thresholds.entry(column.clone()).or_default();
        }
        BooleanRowConstraint::IntegerComparison {
            column, literal, ..
        } => {
            thresholds
                .entry(column.clone())
                .or_default()
                .insert(i128::from(*literal));
        }
        BooleanRowConstraint::Residual { .. } => {}
    }
}

fn possible_joint_truths(
    constraint: &BooleanRowConstraint,
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    restrictions: Option<&BTreeMap<String, ColumnRestriction>>,
) -> BTreeSet<SqlTruth> {
    const MAX_ASSIGNMENTS: usize = 4096;
    let mut thresholds = BTreeMap::new();
    collect_comparison_literals(constraint, &mut thresholds);
    let mut assignments = vec![BTreeMap::new()];
    for (column, literals) in thresholds {
        let restriction = restrictions.and_then(|items| items.get(column.name()));
        let bounds = evidence(&column);
        let mut values = BTreeSet::new();
        if let Some(allowed) = restriction.and_then(|item| item.accepted.as_ref()) {
            for value in allowed {
                if bounds.is_none_or(|bounds| bounds.minimum <= *value && *value <= bounds.maximum)
                {
                    values.insert(Some(*value));
                }
            }
        } else if literals.is_empty() {
            // A NULL test can be decided using any non-NULL witness.
            values.insert(Some(0_i128));
        } else {
            let Some(bounds) = bounds else {
                return BTreeSet::new();
            };
            values.extend([Some(bounds.minimum), Some(bounds.maximum)]);
            for literal in literals {
                for value in [literal - 1, literal, literal + 1] {
                    if bounds.minimum <= value && value <= bounds.maximum {
                        values.insert(Some(value));
                    }
                }
            }
        }
        // NULL remains possible unless enforcement explicitly excludes it.
        if !restriction.is_some_and(|item| item.not_null) {
            values.insert(None);
        }
        if assignments.len().saturating_mul(values.len()) > MAX_ASSIGNMENTS {
            return BTreeSet::new();
        }
        let mut expanded = Vec::new();
        for assignment in assignments {
            for value in &values {
                let mut candidate = assignment.clone();
                candidate.insert(column.clone(), *value);
                expanded.push(candidate);
            }
        }
        assignments = expanded;
    }
    assignments
        .iter()
        .filter_map(|assignment| eval_joint_truth(constraint, assignment))
        .collect()
}

fn eval_joint_truth(
    condition: &BooleanRowConstraint,
    assignment: &std::collections::BTreeMap<ColumnRef, Option<i128>>,
) -> Option<SqlTruth> {
    match condition {
        BooleanRowConstraint::All(operands) => {
            let mut truth = SqlTruth::True;
            for operand in operands.iter() {
                truth = truth.and(eval_joint_truth(operand, assignment)?);
            }
            Some(truth)
        }
        BooleanRowConstraint::Any(operands) => {
            let mut truth = SqlTruth::False;
            for operand in operands.iter() {
                truth = truth.or(eval_joint_truth(operand, assignment)?);
            }
            Some(truth)
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            let is_null = assignment.get(column)?.is_none();
            Some(if is_null != *negated {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => {
            use crate::protocol::ComparisonOperator as Op;
            let value = match assignment.get(column)? {
                Some(value) => *value,
                None => return Some(SqlTruth::Unknown),
            };
            let literal = i128::from(*literal);
            let result = match operator {
                Op::Eq => value == literal,
                Op::Neq => value != literal,
                Op::Lt => value < literal,
                Op::Lte => value <= literal,
                Op::Gt => value > literal,
                Op::Gte => value >= literal,
                Op::IsDistinctFrom | Op::IsNotDistinctFrom => return None,
            };
            Some(if result {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::Residual { .. } => None,
    }
}
