//! Parser-independent SQL Semantic Protocol domain values.
//!
//! The Rust model intentionally contains no sqlparser AST types. Unknown and unsupported
//! semantics remain explicit so consumers can distinguish incomplete analysis from known values.

/// Current protocol version emitted by this crate.
pub const PROTOCOL_VERSION: &str = "0.1.0";

/// Root SQL Semantic Protocol document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protocol {
    protocol_version: &'static str,
    source: ProtocolSource,
    statements: Vec<ProtocolStatement>,
}

impl Protocol {
    pub(crate) fn new(dialect: String, statements: Vec<ProtocolStatement>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            source: ProtocolSource { dialect },
            statements,
        }
    }

    /// Return the protocol contract version.
    pub fn protocol_version(&self) -> &str {
        self.protocol_version
    }

    /// Return metadata describing the analyzed SQL source.
    pub fn source(&self) -> &ProtocolSource {
        &self.source
    }

    /// Return statements in original SQL statement order.
    pub fn statements(&self) -> &[ProtocolStatement] {
        &self.statements
    }
}

/// Metadata about the SQL source that produced a protocol document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSource {
    dialect: String,
}

impl ProtocolSource {
    /// Return the caller-provided SQL dialect name.
    pub fn dialect(&self) -> &str {
        &self.dialect
    }
}

/// Protocol representation of one parsed SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProtocolStatement {
    /// A query for which known semantics and analysis gaps are preserved independently.
    Query(QueryStatement),
    /// A parsed statement whose statement-level semantics are not modeled.
    Unsupported(UnsupportedStatement),
}

/// Partially analyzed query semantics.
///
/// Sections whose analysis has not been implemented are emitted conservatively and accompanied by
/// diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryStatement {
    sources: Vec<SourceRelation>,
    dependencies: Vec<String>,
    joins: Vec<Join>,
    predicates: Box<Predicates>,
    output: Output,
    diagnostics: Vec<Diagnostic>,
}

impl QueryStatement {
    pub(crate) fn new(
        sources: Vec<SourceRelation>,
        dependencies: Vec<String>,
        joins: Vec<Join>,
        predicates: Predicates,
        output: Output,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            sources,
            dependencies,
            joins,
            predicates: Box::new(predicates),
            output,
            diagnostics,
        }
    }

    /// Return direct relational inputs in first semantic appearance order.
    pub fn sources(&self) -> &[SourceRelation] {
        &self.sources
    }

    /// Return normalized physical upstream dependencies in lexicographic order.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Return joins in SQL join order.
    pub fn joins(&self) -> &[Join] {
        &self.joins
    }

    /// Return WHERE, HAVING, and QUALIFY semantics known for the query.
    pub fn predicates(&self) -> &Predicates {
        &self.predicates
    }

    /// Return final query output columns in SELECT-list order.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Return diagnostics describing incomplete query semantics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Final columns produced by a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    columns: Vec<OutputColumn>,
}

impl Output {
    pub(crate) fn new(columns: Vec<OutputColumn>) -> Self {
        Self { columns }
    }

    /// Return final output columns in SELECT-list order.
    pub fn columns(&self) -> &[OutputColumn] {
        &self.columns
    }
}

/// One final query output column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputColumn {
    name: String,
    expression: Expression,
    lineage: Vec<LineageSource>,
}

impl OutputColumn {
    pub(crate) fn new(
        name: String,
        expression: Expression,
        mut lineage: Vec<LineageSource>,
    ) -> Self {
        lineage.sort();
        lineage.dedup();
        Self {
            name,
            expression,
            lineage,
        }
    }

    /// Return the final output column name or unresolved projection label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the semantic expression that produces this output column.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return physical source columns contributing to this output value.
    pub fn lineage(&self) -> &[LineageSource] {
        &self.lineage
    }
}

/// One physical source column contributing to an output value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineageSource {
    relation: String,
    column: String,
}

impl LineageSource {
    pub(crate) fn new(relation: String, column: String) -> Self {
        Self { relation, column }
    }

    /// Return the physical relation containing the source column.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the physical source column name.
    pub fn column(&self) -> &str {
        &self.column
    }
}

/// A direct relational input to a query.
///
/// Physical relations also appear in QueryStatement::dependencies. Local relations such as CTEs
/// and derived tables do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRelation {
    name: String,
    alias: Option<String>,
}

impl SourceRelation {
    pub(crate) fn new(name: String, alias: Option<String>) -> Self {
        Self { name, alias }
    }

    /// Return the relation name or stable local relation identity.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the SQL alias when one is present.
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// A relation participating in a join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRef {
    relation: String,
    alias: Option<String>,
}

impl RelationRef {
    pub(crate) fn new(relation: String, alias: Option<String>) -> Self {
        Self { relation, alias }
    }

    /// Return the relation name or stable local relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the SQL alias when one is present.
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// Supported protocol join kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// INNER JOIN or JOIN.
    Inner,
    /// LEFT JOIN.
    Left,
    /// RIGHT JOIN.
    Right,
    /// FULL JOIN.
    Full,
    /// CROSS JOIN.
    Cross,
    /// LEFT SEMI JOIN.
    LeftSemi,
    /// RIGHT SEMI JOIN.
    RightSemi,
    /// LEFT ANTI JOIN.
    LeftAnti,
    /// RIGHT ANTI JOIN.
    RightAnti,
    /// The join exists but its exact kind is unsupported.
    Unknown,
}

impl JoinKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Inner => "inner",
            Self::Left => "left",
            Self::Right => "right",
            Self::Full => "full",
            Self::Cross => "cross",
            Self::LeftSemi => "left_semi",
            Self::RightSemi => "right_semi",
            Self::LeftAnti => "left_anti",
            Self::RightAnti => "right_anti",
            Self::Unknown => "unknown",
        }
    }
}

/// A normalized relationship between two query relations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    kind: JoinKind,
    left: RelationRef,
    right: RelationRef,
    condition: Option<Predicate>,
}

impl Join {
    pub(crate) fn new(
        kind: JoinKind,
        left: RelationRef,
        right: RelationRef,
        condition: Option<Predicate>,
    ) -> Self {
        Self {
            kind,
            left,
            right,
            condition,
        }
    }

    /// Return the normalized join kind.
    pub fn kind(&self) -> JoinKind {
        self.kind
    }

    /// Return the left relation participating in the join.
    pub fn left(&self) -> &RelationRef {
        &self.left
    }

    /// Return the right relation participating in the join.
    pub fn right(&self) -> &RelationRef {
        &self.right
    }

    /// Return the normalized join condition when it can be represented safely.
    pub fn condition(&self) -> Option<&Predicate> {
        self.condition.as_ref()
    }
}

/// Query predicates grouped by their SQL clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicates {
    where_predicate: Option<Predicate>,
    having_predicate: Option<Predicate>,
    qualify_predicate: Option<Predicate>,
}

impl Predicates {
    pub(crate) fn new(
        where_predicate: Option<Predicate>,
        having_predicate: Option<Predicate>,
        qualify_predicate: Option<Predicate>,
    ) -> Self {
        Self {
            where_predicate,
            having_predicate,
            qualify_predicate,
        }
    }

    /// Return the WHERE predicate when the query contains one.
    pub fn where_predicate(&self) -> Option<&Predicate> {
        self.where_predicate.as_ref()
    }

    /// Return the HAVING predicate when the query contains one.
    pub fn having_predicate(&self) -> Option<&Predicate> {
        self.having_predicate.as_ref()
    }

    /// Return the QUALIFY predicate when the query contains one.
    pub fn qualify_predicate(&self) -> Option<&Predicate> {
        self.qualify_predicate.as_ref()
    }
}

/// Parser-independent expression semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expression {
    /// A column reference.
    Column(ColumnExpression),
    /// A typed literal.
    Literal(LiteralExpression),
    /// A function call whose argument semantics are understood.
    Function(FunctionExpression),
    /// A supported unary operation.
    Unary(UnaryExpression),
    /// A supported binary operation.
    Binary(BinaryExpression),
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the expression but does not support its semantics.
    Unsupported(UnsupportedSemantic),
}

/// A column expression with an optional relation qualifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnExpression {
    relation: Option<String>,
    name: String,
}

impl ColumnExpression {
    pub(crate) fn new(relation: Option<String>, name: String) -> Self {
        Self { relation, name }
    }

    /// Return the relation qualifier when one is present.
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    /// Return the column name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A typed literal expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralExpression {
    literal_type: LiteralType,
    value: LiteralValue,
}

impl LiteralExpression {
    pub(crate) fn new(literal_type: LiteralType, value: LiteralValue) -> Self {
        Self {
            literal_type,
            value,
        }
    }

    /// Return the protocol literal type.
    pub fn literal_type(&self) -> LiteralType {
        self.literal_type
    }

    /// Return the literal value.
    pub fn value(&self) -> &LiteralValue {
        &self.value
    }
}

/// Literal types defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralType {
    /// SQL NULL.
    Null,
    /// Boolean literal.
    Boolean,
    /// Integer numeric literal.
    Integer,
    /// Decimal or exponent numeric literal.
    Decimal,
    /// Text string literal.
    String,
    /// Date literal.
    Date,
    /// Time literal.
    Time,
    /// Timestamp literal.
    Timestamp,
    /// Interval literal.
    Interval,
}

impl LiteralType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::String => "string",
            Self::Date => "date",
            Self::Time => "time",
            Self::Timestamp => "timestamp",
            Self::Interval => "interval",
        }
    }
}

/// Literal payload used by LiteralExpression.
///
/// Number values are canonical JSON-number text validated before protocol construction.
/// Text carries string-like SQL literals, including date/time literals whose type is retained by
/// LiteralExpression::literal_type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LiteralValue {
    /// SQL NULL.
    Null,
    /// Boolean value.
    Boolean(bool),
    /// Canonical JSON-number text.
    Number(String),
    /// String-like literal value.
    Text(String),
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

/// Predicate semantics known by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Predicate {
    /// A comparison between two expressions.
    Comparison(ComparisonPredicate),
    /// Logical conjunction preserving SQL tree order.
    And(LogicalPredicate),
    /// Logical disjunction preserving SQL tree order.
    Or(LogicalPredicate),
    /// Logical negation.
    Not(NotPredicate),
    /// SQL IS NULL or IS NOT NULL.
    IsNull(IsNullPredicate),
    /// SQL IN or NOT IN.
    In(InPredicate),
    /// SQL BETWEEN or NOT BETWEEN.
    Between(BetweenPredicate),
    /// An expression interpreted in boolean predicate context.
    BooleanExpression(Expression),
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the feature but does not support its semantics yet.
    Unsupported(UnsupportedSemantic),
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

/// Semantic value whose precise meaning cannot currently be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSemantic {
    reason: String,
}

impl UnknownSemantic {
    pub(crate) fn new(reason: String) -> Self {
        Self { reason }
    }

    /// Return the reason the semantic value could not be resolved.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Semantic feature recognized by the parser but not supported by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSemantic {
    feature: String,
    reason: Option<String>,
}

impl UnsupportedSemantic {
    pub(crate) fn new(feature: String, reason: Option<String>) -> Self {
        Self { feature, reason }
    }

    /// Return the stable name of the unsupported semantic feature.
    pub fn feature(&self) -> &str {
        &self.feature
    }

    /// Return an optional human-readable reason for the unsupported feature.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// A parsed statement that cannot yet be represented semantically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedStatement {
    category: String,
    diagnostics: Vec<Diagnostic>,
}

impl UnsupportedStatement {
    pub(crate) fn new(category: String, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            category,
            diagnostics,
        }
    }

    /// Return the broad statement category reported by the analyzer.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return diagnostics explaining why the statement is unsupported.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Analyzer diagnostic attached to protocol semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    severity: DiagnosticSeverity,
    code: String,
    area: DiagnosticArea,
    message: String,
}

impl Diagnostic {
    pub(crate) fn new(
        severity: DiagnosticSeverity,
        code: String,
        area: DiagnosticArea,
        message: String,
    ) -> Self {
        Self {
            severity,
            code,
            area,
            message,
        }
    }

    /// Return the diagnostic severity.
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Return the stable diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Return the semantic area affected by the diagnostic.
    pub fn area(&self) -> DiagnosticArea {
        self.area
    }

    /// Return the human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Severity of a protocol diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// Informational diagnostic.
    Info,
    /// Incomplete semantics that consumers should account for.
    Warning,
    /// Semantic problem that prevents reliable interpretation of the affected area.
    Error,
}

impl DiagnosticSeverity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Semantic area affected by a protocol diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticArea {
    /// Whole-statement semantics.
    Statement,
    /// Source-relation semantics.
    Source,
    /// Join semantics.
    Join,
    /// Predicate semantics.
    Predicate,
    /// Column-domain semantics.
    Domain,
    /// Output-column semantics.
    Output,
    /// Expression semantics.
    Expression,
    /// Function semantics.
    Function,
    /// Semantics that do not fit a more specific area.
    Other,
}

impl DiagnosticArea {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Statement => "statement",
            Self::Source => "source",
            Self::Join => "join",
            Self::Predicate => "predicate",
            Self::Domain => "domain",
            Self::Output => "output",
            Self::Expression => "expression",
            Self::Function => "function",
            Self::Other => "other",
        }
    }
}
