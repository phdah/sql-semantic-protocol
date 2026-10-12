//! Scalar expressions, literal values and value domains.

use super::*;

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
    /// A grouped aggregate function call.
    AggregateFunction(AggregateFunctionExpression),
    /// A window function call with a resolved parser-independent window specification.
    WindowFunction(WindowFunctionExpression),
    /// A CASE expression preserving branch semantics.
    Case(CaseExpression),
    /// A predicate used as a boolean-valued scalar expression.
    BooleanPredicate(Box<Predicate>),
    /// A signed integer cast with an explicit canonical target width.
    SignedIntegerCast(SignedIntegerCastExpression),
    /// A supported unary operation.
    Unary(UnaryExpression),
    /// A supported binary operation.
    Binary(BinaryExpression),
    /// A scalar subquery evaluated as one expression value.
    ScalarSubquery(Box<ScalarSubqueryExpression>),
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the expression but does not support its semantics.
    Unsupported(UnsupportedSemantic),
}

/// Query semantics retained when a subquery appears inside an expression or predicate.
///
/// The summary is intentionally parser-independent and local to the nested query. Physical
/// dependencies remain explicit, correlated outer references are resolved to physical lineage
/// where possible, and nested diagnostics stay attached instead of disappearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubquerySemantics {
    sources: Vec<SourceRelation>,
    dependencies: Vec<String>,
    correlations: Vec<LineageSource>,
    joins: Vec<Join>,
    output: Output,
    row_conditions: RowConditions,
    row_shape_preserves_candidates: bool,
    diagnostics: Vec<Diagnostic>,
}

impl SubquerySemantics {
    pub(crate) fn new(
        sources: Vec<SourceRelation>,
        mut dependencies: Vec<String>,
        mut correlations: Vec<LineageSource>,
        joins: Vec<Join>,
        output: Output,
        row_conditions: RowConditions,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        dependencies.sort();
        dependencies.dedup();
        correlations.sort();
        correlations.dedup();
        Self {
            sources,
            dependencies,
            correlations,
            joins,
            output,
            row_conditions,
            row_shape_preserves_candidates: false,
            diagnostics,
        }
    }

    pub(crate) fn with_row_shape_preserves_candidates(mut self, preserves: bool) -> Self {
        self.row_shape_preserves_candidates = preserves;
        self
    }

    /// Return whether the nested SELECT preserves candidate-row existence without row-shaping.
    pub fn row_shape_preserves_candidates(&self) -> bool {
        self.row_shape_preserves_candidates
    }

    /// Return nested source relations and their original SQL instance aliases.
    pub fn sources(&self) -> &[SourceRelation] {
        &self.sources
    }

    /// Return physical relations read by the nested query.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Return physical outer-scope columns referenced by the nested query.
    pub fn correlations(&self) -> &[LineageSource] {
        &self.correlations
    }

    /// Return joins whose row-membership equalities belong to the nested query.
    pub fn joins(&self) -> &[Join] {
        &self.joins
    }

    /// Return projected nested-query output semantics.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Return WHERE, HAVING, and QUALIFY semantics inside the nested query.
    pub fn predicates(&self) -> &Predicates {
        &self.row_conditions.predicates
    }

    /// Return source-column domains derived inside the nested query.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.row_conditions.column_domains
    }

    /// Return row-condition exactness for the nested query scope.
    pub fn condition_exactness(&self) -> &ConditionExactness {
        &self.row_conditions.exactness
    }

    /// Return diagnostics scoped to the nested query.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// A scalar subquery used as an expression value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarSubqueryExpression {
    subquery: SubquerySemantics,
}

impl ScalarSubqueryExpression {
    pub(crate) fn new(subquery: SubquerySemantics) -> Self {
        Self { subquery }
    }

    /// Return the nested query semantics.
    pub fn subquery(&self) -> &SubquerySemantics {
        &self.subquery
    }
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

    /// Return whether SQL NULL is admitted by this domain.
    ///
    /// A None result means the domain itself is unknown, so NULL membership cannot be proven.
    pub fn admits_null(&self) -> Option<bool> {
        match self {
            Self::Unbounded => Some(true),
            Self::Ranges(_) => Some(false),
            Self::Set(domain) => {
                let contains_null = domain
                    .values
                    .iter()
                    .any(|literal| matches!(literal.value(), LiteralValue::Null));
                Some(match domain.mode {
                    SetMode::Include => contains_null,
                    SetMode::Exclude => !contains_null,
                })
            }
            Self::Empty => Some(false),
            Self::Unknown(_) => None,
        }
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
