//! Parser-independent SQL Semantic Protocol domain values.
//!
//! The Rust model intentionally contains no sqlparser AST types. The v0 model is expanded as
//! semantic-analysis tasks are implemented; unsupported parsed statements remain explicit.

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
    /// A parsed statement whose semantics are not yet modeled.
    Unsupported(UnsupportedStatement),
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
