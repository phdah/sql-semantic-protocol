//! Protocol serialization boundary.
//!
//! Serialization depends only on parser-independent protocol values. It deliberately contains no
//! sqlparser AST handling.

use serde_json::{json, Value};

use crate::protocol::{Diagnostic, Protocol, ProtocolStatement, UnsupportedStatement};

/// Serialize protocol domain values to JSON.
///
/// The current analyzer emits only the explicit unsupported-statement form. Later semantic tasks
/// can extend this module without changing the parsing or analysis boundaries.
pub fn to_json(protocol: &Protocol) -> String {
    protocol_to_value(protocol).to_string()
}

fn protocol_to_value(protocol: &Protocol) -> Value {
    let statements = protocol
        .statements()
        .iter()
        .map(statement_to_value)
        .collect::<Vec<_>>();

    json!({
        "protocol_version": protocol.protocol_version(),
        "source": {
            "dialect": protocol.source().dialect()
        },
        "statements": statements
    })
}

fn statement_to_value(statement: &ProtocolStatement) -> Value {
    match statement {
        ProtocolStatement::Unsupported(statement) => unsupported_statement_to_value(statement),
    }
}

fn unsupported_statement_to_value(statement: &UnsupportedStatement) -> Value {
    let diagnostics = statement
        .diagnostics()
        .iter()
        .map(diagnostic_to_value)
        .collect::<Vec<_>>();

    json!({
        "kind": "unsupported",
        "category": statement.category(),
        "diagnostics": diagnostics
    })
}

fn diagnostic_to_value(diagnostic: &Diagnostic) -> Value {
    json!({
        "severity": diagnostic.severity().as_str(),
        "code": diagnostic.code(),
        "area": diagnostic.area().as_str(),
        "message": diagnostic.message()
    })
}
