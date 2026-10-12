//! Functions, CASE, windows, and scalar operations.

use super::*;

/// One argument to an aggregate function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateArgument {
    /// A scalar expression argument.
    Expression(Expression),
    /// An unqualified wildcard such as COUNT(*).
    Wildcard,
    /// A qualified wildcard such as COUNT(table.*).
    QualifiedWildcard(String),
}

/// A normalized grouped aggregate function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateFunctionExpression {
    name: String,
    arguments: Vec<AggregateArgument>,
    distinct: bool,
    filter: Option<Box<Predicate>>,
}

impl AggregateFunctionExpression {
    pub(crate) fn new(
        name: String,
        arguments: Vec<AggregateArgument>,
        distinct: bool,
        filter: Option<Predicate>,
    ) -> Self {
        Self {
            name,
            arguments,
            distinct,
            filter: filter.map(Box::new),
        }
    }

    /// Return the aggregate function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return normalized aggregate arguments in SQL order.
    pub fn arguments(&self) -> &[AggregateArgument] {
        &self.arguments
    }

    /// Return whether the aggregate argument list uses DISTINCT.
    pub fn distinct(&self) -> bool {
        self.distinct
    }

    /// Return the aggregate FILTER predicate, if present.
    pub fn filter(&self) -> Option<&Predicate> {
        self.filter.as_deref()
    }
}

/// One normalized CASE expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseExpression {
    operand: Option<Box<Expression>>,
    branches: Vec<CaseBranch>,
    else_result: Option<Box<Expression>>,
    else_source_domains: CaseSourceDomains,
}

impl CaseExpression {
    pub(crate) fn new(
        operand: Option<Expression>,
        branches: Vec<CaseBranch>,
        else_result: Option<Expression>,
        else_source_domains: CaseSourceDomains,
    ) -> Self {
        Self {
            operand: operand.map(Box::new),
            branches,
            else_result: else_result.map(Box::new),
            else_source_domains,
        }
    }

    /// Return the optional simple-CASE operand.
    pub fn operand(&self) -> Option<&Expression> {
        self.operand.as_deref()
    }

    /// Return CASE branches in SQL order.
    pub fn branches(&self) -> &[CaseBranch] {
        &self.branches
    }

    /// Return the ELSE result, if present.
    pub fn else_result(&self) -> Option<&Expression> {
        self.else_result.as_deref()
    }

    /// Return source-column domains selecting the explicit or implicit ELSE branch.
    ///
    /// Local-relation copies preserve these domains, and composed layers remap them to physical
    /// sources only through proven plain-copy identity paths. Query-level filters are represented
    /// separately in column domains and are not folded into CASE branch reachability.
    pub fn else_source_domains(&self) -> &CaseSourceDomains {
        &self.else_source_domains
    }
}

/// One WHEN/THEN branch in a CASE expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseBranch {
    condition: Expression,
    result: Expression,
    source_domains: CaseSourceDomains,
}

impl CaseBranch {
    pub(crate) fn new(
        condition: Expression,
        result: Expression,
        source_domains: CaseSourceDomains,
    ) -> Self {
        Self {
            condition,
            result,
            source_domains,
        }
    }

    /// Return the WHEN condition or simple-CASE match value.
    pub fn condition(&self) -> &Expression {
        &self.condition
    }

    /// Return the THEN result expression.
    pub fn result(&self) -> &Expression {
        &self.result
    }

    /// Return source-column domains selecting this branch after earlier branches fail.
    ///
    /// Local-relation copies preserve these domains, and composed layers remap them to physical
    /// sources only through proven plain-copy identity paths. Query-level filters are represented
    /// separately in column domains and are not folded into CASE branch reachability.
    pub fn source_domains(&self) -> &CaseSourceDomains {
        &self.source_domains
    }
}

/// One conjunction of physical source-column domains that can select a CASE branch.
///
/// Alternatives inside CaseSourceDomains::Reachable are combined with logical OR. Domains inside
/// one alternative are combined with logical AND.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSourceDomainAlternative {
    column_domains: Vec<ColumnDomain>,
}

impl CaseSourceDomainAlternative {
    pub(crate) fn new(mut column_domains: Vec<ColumnDomain>) -> Self {
        column_domains.sort_by(|left, right| left.column().cmp(right.column()));
        Self { column_domains }
    }

    /// Return conjunctive physical source-column domains in deterministic column order.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.column_domains
    }
}

/// Source-column domains controlling CASE branch selection.
///
/// Reachability reflects CASE control flow, including prior branches, but does not intersect
/// query-level WHERE, HAVING, or QUALIFY domains. During composition, reachable domains are
/// rewritten to physical sources only through proven identity columns; unsafe hops become Unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CaseSourceDomains {
    /// The branch is reachable through one or more disjunctive domain alternatives.
    Reachable {
        /// Domain alternatives in deterministic derivation order.
        alternatives: Vec<CaseSourceDomainAlternative>,
    },
    /// Earlier CASE branches make this branch impossible to select.
    Unreachable,
    /// Branch selection cannot be reduced safely to physical source-column domains.
    Unknown(UnknownDomain),
}

impl CaseSourceDomains {
    pub(crate) fn reachable(alternatives: Vec<CaseSourceDomainAlternative>) -> Self {
        if alternatives.is_empty() {
            Self::Unreachable
        } else {
            Self::Reachable { alternatives }
        }
    }

    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        let reason = if reason.trim().is_empty() {
            "CASE branch source domains could not be derived safely".to_string()
        } else {
            reason
        };
        Self::Unknown(UnknownDomain { reason })
    }
}

/// A normalized function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionExpression {
    name: String,
    arguments: Vec<Expression>,
    distinct: bool,
}

impl FunctionExpression {
    pub(crate) fn new(name: String, arguments: Vec<Expression>, distinct: bool) -> Self {
        Self {
            name,
            arguments,
            distinct,
        }
    }

    /// Return the function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return normalized function arguments in SQL order.
    pub fn arguments(&self) -> &[Expression] {
        &self.arguments
    }

    /// Return whether the function argument list uses DISTINCT.
    pub fn distinct(&self) -> bool {
        self.distinct
    }
}

/// A normalized window function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFunctionExpression {
    function: FunctionExpression,
    window: WindowSpecification,
}

impl WindowFunctionExpression {
    pub(crate) fn new(function: FunctionExpression, window: WindowSpecification) -> Self {
        Self { function, window }
    }

    /// Return the normalized function call.
    pub fn function(&self) -> &FunctionExpression {
        &self.function
    }

    /// Return the resolved window specification.
    pub fn window(&self) -> &WindowSpecification {
        &self.window
    }
}

/// A resolved parser-independent window specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSpecification {
    name: Option<String>,
    partition_by: Vec<Expression>,
    order_by: Vec<WindowOrderExpression>,
    frame: Option<WindowFrame>,
}

impl WindowSpecification {
    pub(crate) fn new(
        name: Option<String>,
        partition_by: Vec<Expression>,
        order_by: Vec<WindowOrderExpression>,
        frame: Option<WindowFrame>,
    ) -> Self {
        Self {
            name,
            partition_by,
            order_by,
            frame,
        }
    }

    /// Return the referenced named window, if the function used one.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return normalized PARTITION BY expressions in SQL order.
    pub fn partition_by(&self) -> &[Expression] {
        &self.partition_by
    }

    /// Return normalized ORDER BY expressions in SQL order.
    pub fn order_by(&self) -> &[WindowOrderExpression] {
        &self.order_by
    }

    /// Return the explicit frame, if one was specified.
    pub fn frame(&self) -> Option<&WindowFrame> {
        self.frame.as_ref()
    }
}

/// One expression in a window ORDER BY clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOrderExpression {
    expression: Expression,
    ascending: Option<bool>,
    nulls_first: Option<bool>,
}

impl WindowOrderExpression {
    pub(crate) fn new(
        expression: Expression,
        ascending: Option<bool>,
        nulls_first: Option<bool>,
    ) -> Self {
        Self {
            expression,
            ascending,
            nulls_first,
        }
    }

    /// Return the ordering expression.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return explicit ascending or descending ordering, if specified.
    pub fn ascending(&self) -> Option<bool> {
        self.ascending
    }

    /// Return explicit NULLS FIRST or NULLS LAST ordering, if specified.
    pub fn nulls_first(&self) -> Option<bool> {
        self.nulls_first
    }
}

/// Units used by an explicit window frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFrameUnits {
    /// ROWS frame semantics.
    Rows,
    /// RANGE frame semantics.
    Range,
    /// GROUPS frame semantics.
    Groups,
}

impl WindowFrameUnits {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Range => "range",
            Self::Groups => "groups",
        }
    }
}

/// One explicit window frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFrame {
    units: WindowFrameUnits,
    start_bound: WindowFrameBound,
    end_bound: WindowFrameBound,
}

impl WindowFrame {
    pub(crate) fn new(
        units: WindowFrameUnits,
        start_bound: WindowFrameBound,
        end_bound: WindowFrameBound,
    ) -> Self {
        Self {
            units,
            start_bound,
            end_bound,
        }
    }

    /// Return the frame units.
    pub fn units(&self) -> WindowFrameUnits {
        self.units
    }

    /// Return the starting frame bound.
    pub fn start_bound(&self) -> &WindowFrameBound {
        &self.start_bound
    }

    /// Return the ending frame bound.
    pub fn end_bound(&self) -> &WindowFrameBound {
        &self.end_bound
    }
}

/// One normalized window-frame boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowFrameBound {
    /// CURRENT ROW.
    CurrentRow,
    /// UNBOUNDED PRECEDING.
    UnboundedPreceding,
    /// A bounded PRECEDING offset.
    Preceding(Box<Expression>),
    /// UNBOUNDED FOLLOWING.
    UnboundedFollowing,
    /// A bounded FOLLOWING offset.
    Following(Box<Expression>),
}

/// A statically typed integer cast recorded in a predicate expression.
///
/// Only explicit signed widths (16, 32, 64) are constructible. The source
/// datatype must independently establish lossless widening before a witness
/// can treat this as an invertible expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIntegerCastExpression {
    expression: Box<Expression>,
    target_bits: u16,
}

impl SignedIntegerCastExpression {
    pub(crate) fn new(expression: Expression, target_bits: u16) -> Self {
        debug_assert!(matches!(target_bits, 16 | 32 | 64));
        Self {
            expression: Box::new(expression),
            target_bits,
        }
    }

    /// Expression being cast.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Signed target width in bits.
    pub fn target_bits(&self) -> u16 {
        self.target_bits
    }
}

/// A normalized unary operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpression {
    operator: UnaryOperator,
    operand: Box<Expression>,
}

impl UnaryExpression {
    pub(crate) fn new(operator: UnaryOperator, operand: Expression) -> Self {
        Self {
            operator,
            operand: Box::new(operand),
        }
    }

    /// Return the unary operator.
    pub fn operator(&self) -> UnaryOperator {
        self.operator
    }

    /// Return the unary operand.
    pub fn operand(&self) -> &Expression {
        &self.operand
    }
}

/// Unary expression operators defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Unary plus.
    Plus,
    /// Unary minus.
    Minus,
    /// Bitwise NOT.
    BitwiseNot,
}

impl UnaryOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::BitwiseNot => "bitwise_not",
        }
    }
}

/// A normalized binary operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpression {
    operator: BinaryOperator,
    left: Box<Expression>,
    right: Box<Expression>,
}

impl BinaryExpression {
    pub(crate) fn new(operator: BinaryOperator, left: Expression, right: Expression) -> Self {
        Self {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Return the binary operator.
    pub fn operator(&self) -> BinaryOperator {
        self.operator
    }

    /// Return the left operand.
    pub fn left(&self) -> &Expression {
        &self.left
    }

    /// Return the right operand.
    pub fn right(&self) -> &Expression {
        &self.right
    }
}

/// Binary expression operators defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    /// Addition.
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
    /// Modulo.
    Modulo,
    /// String concatenation.
    StringConcat,
    /// Bitwise AND.
    BitwiseAnd,
    /// Bitwise OR.
    BitwiseOr,
    /// Bitwise XOR.
    BitwiseXor,
}

impl BinaryOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Modulo => "modulo",
            Self::StringConcat => "string_concat",
            Self::BitwiseAnd => "bitwise_and",
            Self::BitwiseOr => "bitwise_or",
            Self::BitwiseXor => "bitwise_xor",
        }
    }
}
