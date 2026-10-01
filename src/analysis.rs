//! Semantic-analysis boundary.
//!
//! This module is the only layer that converts sqlparser AST statements into public protocol
//! values. Task 2 establishes the boundary without claiming semantics that later tasks implement.

use std::fmt;

use sqlparser::ast::Statement as SqlStatement;

use crate::parser::ParsedSql;
use crate::protocol::{
    Diagnostic, DiagnosticArea, DiagnosticSeverity, Protocol, ProtocolStatement,
    UnsupportedStatement,
};

/// Error produced after parsing succeeds but protocol analysis cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalysisError {
    /// The caller did not provide a non-empty dialect name for protocol metadata.
    EmptyDialectName,
    /// Parsing succeeded but produced no SQL statements to analyze.
    NoStatements,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDialectName => {
                write!(formatter, "analysis requires a non-empty dialect name")
            }
            Self::NoStatements => write!(formatter, "analysis requires at least one SQL statement"),
        }
    }
}

impl std::error::Error for AnalysisError {}

pub(crate) fn analyze(parsed: ParsedSql, dialect_name: &str) -> Result<Protocol, AnalysisError> {
    if dialect_name.trim().is_empty() {
        return Err(AnalysisError::EmptyDialectName);
    }

    if parsed.statements.is_empty() {
        return Err(AnalysisError::NoStatements);
    }

    let statements = parsed
        .statements
        .iter()
        .map(unsupported_statement)
        .collect();

    Ok(Protocol::new(dialect_name.to_string(), statements))
}

fn unsupported_statement(statement: &SqlStatement) -> ProtocolStatement {
    let category = match statement {
        SqlStatement::Query(_) => "query",
        _ => "statement",
    };

    let diagnostic = Diagnostic::new(
        DiagnosticSeverity::Warning,
        "semantic_analysis_pending".to_string(),
        DiagnosticArea::Statement,
        format!("semantic analysis for parsed {category} statements is not implemented yet"),
    );

    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        category.to_string(),
        vec![diagnostic],
    ))
}
