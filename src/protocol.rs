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
/// Sections whose analysis has not been implemented are emitted conservatively as empty protocol
/// collections and accompanied by diagnostics. Predicates that are present but not understood are
/// retained explicitly through Predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryStatement {
    predicates: Predicates,
    diagnostics: Vec<Diagnostic>,
}

impl QueryStatement {
    pub(crate) fn new(predicates: Predicates, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            predicates,
            diagnostics,
        }
    }

    /// Return WHERE, HAVING, and QUALIFY semantics known for the query.
    pub fn predicates(&self) -> &Predicates {
        &self.predicates
    }

    /// Return diagnostics describing incomplete query semantics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
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

/// Predicate semantics currently known by the analyzer.
///
/// Concrete predicate forms are added by later semantic-analysis tasks. Until then, a parsed
/// predicate is retained as either unknown or explicitly unsupported rather than omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Predicate {
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the feature but does not support its semantics yet.
    Unsupported(UnsupportedSemantic),
}

/// Semantic value whose precise meaning cannot currently be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSemantic {
    reason: String,
}

impl UnknownSemantic {
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
