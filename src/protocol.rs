//! Parser-independent SQL Semantic Protocol domain values.
//!
//! The Rust model intentionally contains no sqlparser AST types. Unknown and unsupported
//! semantics remain explicit so consumers can distinguish incomplete analysis from known values.

/// Current protocol version emitted by this crate.
pub const PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    column_domains: Vec<ColumnDomain>,
    output: Output,
    set_operation: Option<SetOperation>,
    produced_relation: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

impl QueryStatement {
    pub(crate) fn new(
        sources: Vec<SourceRelation>,
        dependencies: Vec<String>,
        joins: Vec<Join>,
        predicates: Predicates,
        mut column_domains: Vec<ColumnDomain>,
        output: Output,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        column_domains.sort_by(|left, right| left.column.cmp(&right.column));
        Self {
            sources,
            dependencies,
            joins,
            predicates: Box::new(predicates),
            column_domains,
            output,
            set_operation: None,
            produced_relation: None,
            diagnostics,
        }
    }

    pub(crate) fn with_set_operation(mut self, set_operation: Option<SetOperation>) -> Self {
        self.set_operation = set_operation;
        self
    }

    pub(crate) fn with_produced_relation(mut self, produced_relation: Option<String>) -> Self {
        self.produced_relation = produced_relation;
        self
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

    /// Return derived source-column value domains in deterministic column order.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.column_domains
    }

    /// Return final query output columns in SELECT-list order.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Return the set-operation tree when this query combines multiple query operands.
    pub fn set_operation(&self) -> Option<&SetOperation> {
        self.set_operation.as_ref()
    }

    /// Return the named relation produced by query-backed DDL, if any.
    ///
    /// Bare queries produce anonymous results and therefore return `None`.
    pub fn produced_relation(&self) -> Option<&str> {
        self.produced_relation.as_deref()
    }

    /// Return diagnostics describing incomplete query semantics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// A semantic SQL set operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOperator {
    /// UNION.
    Union,
    /// INTERSECT.
    Intersect,
    /// EXCEPT.
    Except,
}

impl SetOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Intersect => "intersect",
            Self::Except => "except",
        }
    }
}

/// Quantifier and alignment semantics for a SQL set operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetQuantifier {
    /// DISTINCT semantics, including an omitted SQL quantifier.
    Distinct,
    /// ALL semantics.
    All,
    /// BY NAME with the dialect-defined default duplicate treatment.
    ByName,
    /// ALL BY NAME.
    AllByName,
    /// DISTINCT BY NAME.
    DistinctByName,
}

impl SetQuantifier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Distinct => "distinct",
            Self::All => "all",
            Self::ByName => "by_name",
            Self::AllByName => "all_by_name",
            Self::DistinctByName => "distinct_by_name",
        }
    }

    pub(crate) fn uses_name_alignment(self) -> bool {
        matches!(self, Self::ByName | Self::AllByName | Self::DistinctByName)
    }
}

/// One operand in a recursive SQL set-operation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOperand {
    /// A query operand whose local semantics are represented by the surrounding query analysis.
    Query,
    /// A nested set operation.
    Operation(Box<SetOperation>),
}

/// One UNION, INTERSECT, or EXCEPT operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetOperation {
    operator: SetOperator,
    quantifier: SetQuantifier,
    left: SetOperand,
    right: SetOperand,
}

impl SetOperation {
    pub(crate) fn new(
        operator: SetOperator,
        quantifier: SetQuantifier,
        left: SetOperand,
        right: SetOperand,
    ) -> Self {
        Self {
            operator,
            quantifier,
            left,
            right,
        }
    }

    /// Return the set operator.
    pub fn operator(&self) -> SetOperator {
        self.operator
    }

    /// Return duplicate-treatment and alignment semantics.
    pub fn quantifier(&self) -> SetQuantifier {
        self.quantifier
    }

    /// Return the left operand.
    pub fn left(&self) -> &SetOperand {
        &self.left
    }

    /// Return the right operand.
    pub fn right(&self) -> &SetOperand {
        &self.right
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
    /// A window function call with a resolved parser-independent window specification.
    WindowFunction(WindowFunctionExpression),
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

/// Reference to a column whose scalar value domain was analyzed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColumnRef {
    relation: Option<String>,
    name: String,
}

impl ColumnRef {
    pub(crate) fn new(relation: Option<String>, name: String) -> Self {
        Self { relation, name }
    }

    /// Return the resolved relation name when one is known.
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    /// Return the referenced column name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A derived value domain for one referenced column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDomain {
    column: ColumnRef,
    domain: ValueDomain,
}

impl ColumnDomain {
    pub(crate) fn new(column: ColumnRef, domain: ValueDomain) -> Self {
        Self { column, domain }
    }

    /// Return the constrained column.
    pub fn column(&self) -> &ColumnRef {
        &self.column
    }

    /// Return the conservative scalar domain derived for the column.
    pub fn domain(&self) -> &ValueDomain {
        &self.domain
    }
}

/// Conservative scalar values that may satisfy the analyzed predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueDomain {
    /// No useful scalar restriction is known.
    Unbounded,
    /// One or more ordered ranges constrain the value.
    Ranges(RangesDomain),
    /// A finite inclusion or exclusion set constrains the value.
    Set(SetDomain),
    /// No value can satisfy the known constraints.
    Empty,
    /// A scalar domain cannot be derived safely from the known semantics.
    Unknown(UnknownDomain),
}

impl ValueDomain {
    pub(crate) fn ranges(ranges: Vec<ValueRange>) -> Self {
        if ranges.is_empty() {
            Self::Empty
        } else {
            Self::Ranges(RangesDomain { ranges })
        }
    }

    pub(crate) fn set(mode: SetMode, mut values: Vec<LiteralExpression>) -> Self {
        values.sort_by_key(literal_sort_key);
        values.dedup();

        if values.is_empty() {
            return match mode {
                SetMode::Include => Self::Empty,
                SetMode::Exclude => Self::Unbounded,
            };
        }

        Self::Set(SetDomain { mode, values })
    }

    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        let reason = if reason.trim().is_empty() {
            "column domain could not be derived safely".to_string()
        } else {
            reason
        };
        Self::Unknown(UnknownDomain { reason })
    }
}

/// Ordered ranges comprising a value domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangesDomain {
    ranges: Vec<ValueRange>,
}

impl RangesDomain {
    /// Return ranges in deterministic derivation order.
    pub fn ranges(&self) -> &[ValueRange] {
        &self.ranges
    }
}

/// One interval in an ordered value domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRange {
    lower: Option<Bound>,
    upper: Option<Bound>,
}

impl ValueRange {
    pub(crate) fn new(lower: Option<Bound>, upper: Option<Bound>) -> Self {
        Self { lower, upper }
    }

    /// Return the lower bound, or None when the range is unbounded below.
    pub fn lower(&self) -> Option<&Bound> {
        self.lower.as_ref()
    }

    /// Return the upper bound, or None when the range is unbounded above.
    pub fn upper(&self) -> Option<&Bound> {
        self.upper.as_ref()
    }
}

/// One inclusive or exclusive literal range bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    value: LiteralExpression,
    inclusive: bool,
}

impl Bound {
    pub(crate) fn new(value: LiteralExpression, inclusive: bool) -> Self {
        Self { value, inclusive }
    }

    /// Return the literal value used by this bound.
    pub fn value(&self) -> &LiteralExpression {
        &self.value
    }

    /// Return whether the bound includes its literal value.
    pub fn inclusive(&self) -> bool {
        self.inclusive
    }
}

/// Set-domain interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetMode {
    /// Only the listed values are allowed.
    Include,
    /// Every value except the listed values is allowed.
    Exclude,
}

impl SetMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Include => "include",
            Self::Exclude => "exclude",
        }
    }
}

/// A finite inclusion or exclusion domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetDomain {
    mode: SetMode,
    values: Vec<LiteralExpression>,
}

impl SetDomain {
    /// Return whether values are included or excluded.
    pub fn mode(&self) -> SetMode {
        self.mode
    }

    /// Return deterministically ordered literal values.
    pub fn values(&self) -> &[LiteralExpression] {
        &self.values
    }
}

/// Reason a safe scalar value domain could not be derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDomain {
    reason: String,
}

impl UnknownDomain {
    /// Return why domain derivation could not be completed safely.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

fn literal_sort_key(literal: &LiteralExpression) -> (&'static str, String) {
    let value = match literal.value() {
        LiteralValue::Null => "null".to_string(),
        LiteralValue::Boolean(value) => value.to_string(),
        LiteralValue::Number(value) | LiteralValue::Text(value) => value.clone(),
    };
    (literal.literal_type().as_str(), value)
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
