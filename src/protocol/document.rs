//! Root SQL Semantic Protocol document and statement metadata.

use super::*;

/// Root SQL Semantic Protocol document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protocol {
    protocol_version: &'static str,
    source: ProtocolSource,
    statements: Vec<ProtocolStatement>,
    relation_constraints: Vec<RelationConstraintSet>,
    comparison_declarations: Vec<ComparisonAssumption>,
}

impl Protocol {
    pub(crate) fn new(dialect: String, statements: Vec<ProtocolStatement>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            source: ProtocolSource { dialect },
            statements,
            relation_constraints: Vec::new(),
            comparison_declarations: Vec::new(),
        }
    }

    /// Declare warehouse comparison settings for single-input analysis.
    ///
    /// Callers must attest only settings known to hold for the target warehouse.
    pub fn declare_comparison_assumptions(&mut self, declared: &[ComparisonAssumption]) {
        self.comparison_declarations
            .extend(declared.iter().copied());
        self.comparison_declarations.sort();
        self.comparison_declarations.dedup();
        for statement in &mut self.statements {
            if let ProtocolStatement::Query(query) = statement {
                query.declare_comparison_assumptions(&self.comparison_declarations);
            }
        }
    }

    /// Return caller-declared comparison settings.
    pub fn comparison_declarations(&self) -> &[ComparisonAssumption] {
        &self.comparison_declarations
    }

    pub(crate) fn with_relation_constraints(
        mut self,
        relation_constraints: Vec<RelationConstraintSet>,
    ) -> Self {
        self.relation_constraints = relation_constraints;
        for statement in &mut self.statements {
            if let ProtocolStatement::Query(query) = statement {
                query.restrict_boolean_witness(&self.relation_constraints);
            }
        }
        self
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

    /// Return canonical relation constraints discovered from the analyzed SQL.
    pub fn relation_constraints(&self) -> &[RelationConstraintSet] {
        &self.relation_constraints
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
