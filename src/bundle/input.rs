//! Source identities and per-input analyzed metadata.

use super::*;

/// Source identity retained for one SQL input unit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlInputSource {
    /// SQL supplied directly by the caller.
    Inline,
    /// SQL loaded from a file whose path identifies the source.
    File {
        /// Caller-visible path used to identify this input.
        path: String,
    },
}

/// One SQL text unit supplied to multi-input analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlInput {
    source: SqlInputSource,
    sql: String,
}

/// One SQL input with an explicit stable identity and caller-selected dialect.
///
/// This configured form is useful when input identity and dialect vary per unit, such as when an
/// analysis request is loaded from a manifest. The dialect implementation remains caller-owned.
pub struct ConfiguredSqlInput<'a> {
    id: &'a str,
    input: &'a SqlInput,
    dialect_name: &'a str,
    dialect: &'a dyn Dialect,
    relation_context: Option<&'a RelationContext>,
}

impl<'a> ConfiguredSqlInput<'a> {
    /// Construct one configured SQL input.
    pub fn new(
        id: &'a str,
        input: &'a SqlInput,
        dialect_name: &'a str,
        dialect: &'a dyn Dialect,
    ) -> Self {
        Self {
            id,
            input,
            dialect_name,
            dialect,
            relation_context: None,
        }
    }

    /// Return the stable input identifier supplied by the caller.
    pub fn id(&self) -> &str {
        self.id
    }

    /// Return the SQL input and source identity.
    pub fn input(&self) -> &SqlInput {
        self.input
    }

    /// Return the caller-visible dialect name.
    pub fn dialect_name(&self) -> &str {
        self.dialect_name
    }

    /// Return the sqlparser dialect implementation used at the parsing boundary.
    pub fn dialect(&self) -> &dyn Dialect {
        self.dialect
    }

    /// Attach default catalog and schema context to this input.
    pub fn with_relation_context(mut self, context: &'a RelationContext) -> Self {
        self.relation_context = Some(context);
        self
    }

    /// Return optional default catalog and schema context for this input.
    pub fn relation_context(&self) -> Option<&RelationContext> {
        self.relation_context
    }
}

impl SqlInput {
    /// Construct an inline SQL input.
    pub fn inline(sql: impl Into<String>) -> Self {
        Self {
            source: SqlInputSource::Inline,
            sql: sql.into(),
        }
    }

    /// Construct a file-backed SQL input from its source path and already-read SQL text.
    ///
    /// The library does not perform file I/O. Callers retain control over how file contents are
    /// loaded while the path remains available as stable source identity.
    pub fn file(path: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            source: SqlInputSource::File { path: path.into() },
            sql: sql.into(),
        }
    }

    /// Return the parser-independent source identity for this input.
    pub fn source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the SQL text supplied for this input.
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// One analyzed input in a multi-input protocol bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzedInput {
    id: String,
    source: SqlInputSource,
    dialect: String,
    relation_context: Option<RelationContext>,
    statements: Vec<ProtocolStatement>,
    relation_constraints: Vec<RelationConstraintSet>,
}

impl AnalyzedInput {
    /// Return the deterministic input identifier generated from caller order.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the source identity associated with this input.
    pub fn source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the normalized dialect name supplied by the caller.
    pub fn dialect(&self) -> &str {
        &self.dialect
    }

    /// Return optional relation-resolution context supplied for this input.
    pub fn relation_context(&self) -> Option<&RelationContext> {
        self.relation_context.as_ref()
    }

    /// Return analyzed statements in source statement order.
    pub fn statements(&self) -> &[ProtocolStatement] {
        &self.statements
    }

    /// Return relation constraints discovered in this input before bundle-level enrichment.
    pub fn relation_constraints(&self) -> &[RelationConstraintSet] {
        &self.relation_constraints
    }
}

/// Dataset produced by one transformation layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
