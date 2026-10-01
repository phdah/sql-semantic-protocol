//! SQL parsing boundary.
//!
//! This module contains sqlparser AST values so the rest of the crate can depend on the
//! parser-independent protocol model.

use std::fmt;

use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::parser::Parser;

/// Error produced while parsing SQL text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    /// Return the parser-provided failure message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SQL parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

pub(crate) struct ParsedSql {
    pub(crate) statements: Vec<Statement>,
}

pub(crate) fn parse_sql(sql: &str, dialect: &dyn Dialect) -> Result<ParsedSql, ParseError> {
    Parser::parse_sql(dialect, sql)
        .map(|statements| ParsedSql { statements })
        .map_err(|error| ParseError {
            message: error.to_string(),
        })
}
