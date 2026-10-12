//! Boolean predicates and SQL truth conditions.

use super::*;

/// Predicate semantics known by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Predicate {
    /// A comparison between two expressions.
    Comparison(ComparisonPredicate),
    /// A strictly normalized prefix LIKE predicate, retaining NULL-sensitive negation.
    LikePrefix(LikePrefixPredicate),
    /// Logical conjunction preserving SQL tree order.
    And(LogicalPredicate),
    /// Logical disjunction preserving SQL tree order.
    Or(LogicalPredicate),
    /// Logical negation.
    Not(NotPredicate),
    /// SQL IS NULL or IS NOT NULL.
    IsNull(IsNullPredicate),
    /// SQL IN or NOT IN with an explicit value list.
    In(InPredicate),
    /// SQL EXISTS or NOT EXISTS with nested query semantics.
    Exists(ExistsPredicate),
    /// SQL IN or NOT IN whose values come from a subquery.
    InSubquery(InSubqueryPredicate),
    /// SQL BETWEEN or NOT BETWEEN.
    Between(BetweenPredicate),
    /// An expression interpreted in boolean predicate context.
    BooleanExpression(Expression),
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the feature but does not support its semantics yet.
    Unsupported(UnsupportedSemantic),
}

/// A deliberately narrow SQL LIKE 'prefix%' predicate.
///
/// Only unescaped nonempty ASCII alphanumeric prefixes qualify for this
/// normalized type. Collation and padding attestations are evaluated separately
/// before any source-row witness direction can become exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LikePrefixPredicate {
    expression: Expression,
    prefix: String,
    negated: bool,
}

impl LikePrefixPredicate {
    pub(crate) fn new(expression: Expression, prefix: String, negated: bool) -> Self {
        debug_assert!(
            !prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        );
        Self {
            expression,
            prefix,
            negated,
        }
    }

    /// String-valued source expression tested by LIKE.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Literal prefix with the trailing wildcard removed.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// True for SQL NOT LIKE, with UNKNOWN preserved for NULL input.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// A comparison predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonPredicate {
    left: Expression,
    operator: ComparisonOperator,
    right: Expression,
}

impl ComparisonPredicate {
    pub(crate) fn new(left: Expression, operator: ComparisonOperator, right: Expression) -> Self {
        Self {
            left,
            operator,
            right,
        }
    }

    /// Return the left comparison expression.
    pub fn left(&self) -> &Expression {
        &self.left
    }

    /// Return the comparison operator.
    pub fn operator(&self) -> ComparisonOperator {
        self.operator
    }

    /// Return the right comparison expression.
    pub fn right(&self) -> &Expression {
        &self.right
    }
}

/// Comparison operators defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    /// Equal.
    Eq,
    /// Not equal.
    Neq,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// IS DISTINCT FROM.
    IsDistinctFrom,
    /// IS NOT DISTINCT FROM.
    IsNotDistinctFrom,
}

impl ComparisonOperator {
    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Eq => Self::Eq,
            Self::Neq => Self::Neq,
            Self::Lt => Self::Gt,
            Self::Lte => Self::Gte,
            Self::Gt => Self::Lt,
            Self::Gte => Self::Lte,
            Self::IsDistinctFrom => Self::IsDistinctFrom,
            Self::IsNotDistinctFrom => Self::IsNotDistinctFrom,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Neq => "neq",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::IsDistinctFrom => "is_distinct_from",
            Self::IsNotDistinctFrom => "is_not_distinct_from",
        }
    }
}

/// Operands for an AND or OR predicate.
///
/// Construction is crate-private so protocol values emitted by the analyzer always contain at
/// least the two operands required by protocol v0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalPredicate {
    operands: Vec<Predicate>,
}

impl LogicalPredicate {
    pub(crate) fn pair(left: Predicate, right: Predicate) -> Self {
        Self {
            operands: vec![left, right],
        }
    }

    /// Return logical operands in SQL evaluation-tree order.
    pub fn operands(&self) -> &[Predicate] {
        &self.operands
    }
}

/// A logical NOT predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotPredicate {
    operand: Box<Predicate>,
}

impl NotPredicate {
    pub(crate) fn new(operand: Predicate) -> Self {
        Self {
            operand: Box::new(operand),
        }
    }

    /// Return the predicate being negated.
    pub fn operand(&self) -> &Predicate {
        &self.operand
    }
}

/// An IS NULL predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsNullPredicate {
    expression: Expression,
    negated: bool,
}

impl IsNullPredicate {
    pub(crate) fn new(expression: Expression, negated: bool) -> Self {
        Self {
            expression,
            negated,
        }
    }

    /// Return the tested expression.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return whether the SQL form is IS NOT NULL.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// An IN-list predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InPredicate {
    expression: Expression,
    values: Vec<Expression>,
    negated: bool,
}

impl InPredicate {
    pub(crate) fn new(expression: Expression, values: Vec<Expression>, negated: bool) -> Self {
        Self {
            expression,
            values,
            negated,
        }
    }

    /// Return the expression tested for membership.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return IN-list values in SQL order.
    pub fn values(&self) -> &[Expression] {
        &self.values
    }

    /// Return whether the SQL form is NOT IN.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// An EXISTS or NOT EXISTS predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistsPredicate {
    subquery: SubquerySemantics,
    negated: bool,
}

impl ExistsPredicate {
    pub(crate) fn new(subquery: SubquerySemantics, negated: bool) -> Self {
        Self { subquery, negated }
    }

    /// Return the nested query semantics.
    pub fn subquery(&self) -> &SubquerySemantics {
        &self.subquery
    }

    /// Return whether the SQL form is NOT EXISTS.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// An IN or NOT IN predicate whose candidate values come from a subquery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InSubqueryPredicate {
    expression: Expression,
    subquery: SubquerySemantics,
    negated: bool,
}

impl InSubqueryPredicate {
    pub(crate) fn new(expression: Expression, subquery: SubquerySemantics, negated: bool) -> Self {
        Self {
            expression,
            subquery,
            negated,
        }
    }

    /// Return the expression tested for membership.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the nested query that supplies candidate values.
    pub fn subquery(&self) -> &SubquerySemantics {
        &self.subquery
    }

    /// Return whether the SQL form is NOT IN.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// A BETWEEN predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BetweenPredicate {
    expression: Expression,
    lower: Expression,
    upper: Expression,
    negated: bool,
}

impl BetweenPredicate {
    pub(crate) fn new(
        expression: Expression,
        lower: Expression,
        upper: Expression,
        negated: bool,
    ) -> Self {
        Self {
            expression,
            lower,
            upper,
            negated,
        }
    }

    /// Return the constrained expression.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the lower bound expression.
    pub fn lower(&self) -> &Expression {
        &self.lower
    }

    /// Return the upper bound expression.
    pub fn upper(&self) -> &Expression {
        &self.upper
    }

    /// Return whether the SQL form is NOT BETWEEN.
    pub fn negated(&self) -> bool {
        self.negated
    }
}
