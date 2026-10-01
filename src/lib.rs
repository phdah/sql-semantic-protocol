//! Public library entry points for SQL Semantic Protocol.
//!
//! - analyze_sql parses SQL with a caller-supplied dialect and returns protocol domain values.
//! - to_json serializes protocol domain values without exposing parser AST types.
//! - protocol contains the parser-independent public protocol model, including explicit unknown
//!   and unsupported semantic values.

mod analysis;
mod emission;
mod parser;
pub mod protocol;

use std::fmt;

use sqlparser::dialect::Dialect;

pub use analysis::AnalysisError;
pub use emission::to_json;
pub use parser::ParseError;
pub use protocol::{
    Diagnostic, DiagnosticArea, DiagnosticSeverity, Predicate, Predicates, Protocol,
    ProtocolSource, ProtocolStatement, QueryStatement, UnknownSemantic, UnsupportedSemantic,
    UnsupportedStatement,
};

/// Error returned when SQL cannot be converted into protocol domain values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// SQL parsing failed before semantic analysis could begin.
    Parse(ParseError),
    /// Parsing succeeded but semantic analysis could not produce a protocol value.
    Analysis(AnalysisError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "{error}"),
            Self::Analysis(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Analysis(error) => Some(error),
        }
    }
}

/// Parse and analyze SQL using the dialect selected by the caller.
///
/// The dialect name is copied into the protocol source metadata. The dialect implementation is
/// used only at the parsing boundary; sqlparser AST values never appear in the returned protocol.
pub fn analyze_sql(
    sql: &str,
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<Protocol, Error> {
    let parsed = parser::parse_sql(sql, dialect).map_err(Error::Parse)?;
    analysis::analyze(parsed, dialect_name).map_err(Error::Analysis)
}
