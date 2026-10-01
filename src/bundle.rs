//! Multi-input analysis orchestration.
//!
//! This module owns parser-independent input identities and bundles. It deliberately reuses the
//! existing single-input analyzer for each unit and does not build cross-input graph semantics.

use std::fmt;

use sqlparser::dialect::Dialect;

use crate::protocol::ProtocolStatement;
use crate::{analyze_sql, Error};

/// Protocol version used by multi-input analysis bundles.
pub const MULTI_INPUT_PROTOCOL_VERSION: &str = "0.2.0";

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
    statements: Vec<ProtocolStatement>,
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

    /// Return analyzed statements in source statement order.
    pub fn statements(&self) -> &[ProtocolStatement] {
        &self.statements
    }
}

/// Multi-input analysis result before cross-input graph construction is implemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisBundle {
    protocol_version: &'static str,
    inputs: Vec<AnalyzedInput>,
}

impl AnalysisBundle {
    /// Return the multi-input protocol contract version.
    pub fn protocol_version(&self) -> &str {
        self.protocol_version
    }

    /// Return analyzed inputs in caller-provided order.
    pub fn inputs(&self) -> &[AnalyzedInput] {
        &self.inputs
    }
}

/// Error produced while parsing or analyzing one input in a multi-input invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputAnalysisError {
    input_id: String,
    source: SqlInputSource,
    error: Error,
}

impl InputAnalysisError {
    /// Return the deterministic identifier of the input that failed.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Return the source identity of the input that failed.
    pub fn input_source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the underlying parse or analysis error.
    pub fn error(&self) -> &Error {
        &self.error
    }
}

impl fmt::Display for InputAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            SqlInputSource::Inline => {
                write!(formatter, "input {} (inline): {}", self.input_id, self.error)
            }
            SqlInputSource::File { path } => write!(
                formatter,
                "input {} (file '{}'): {}",
                self.input_id, path, self.error
            ),
        }
    }
}

impl std::error::Error for InputAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Parse and analyze an arbitrary number of SQL inputs using one caller-selected dialect.
///
/// Inputs are analyzed in caller order. Generated IDs start at `input-0001`; the numeric width
/// expands when necessary rather than imposing a maximum input count.
pub fn analyze_inputs(
    inputs: &[SqlInput],
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<AnalysisBundle, InputAnalysisError> {
    let width = inputs.len().max(1).to_string().len().max(4);
    let mut analyzed_inputs = Vec::with_capacity(inputs.len());

    for (index, input) in inputs.iter().enumerate() {
        let input_id = format!("input-{:0width$}", index + 1, width = width);
        let protocol = analyze_sql(input.sql(), dialect_name, dialect).map_err(|error| {
            InputAnalysisError {
                input_id: input_id.clone(),
                source: input.source().clone(),
                error,
            }
        })?;

        analyzed_inputs.push(AnalyzedInput {
            id: input_id,
            source: input.source().clone(),
            dialect: dialect_name.to_string(),
            statements: protocol.statements().to_vec(),
        });
    }

    Ok(AnalysisBundle {
        protocol_version: MULTI_INPUT_PROTOCOL_VERSION,
        inputs: analyzed_inputs,
    })
}
