//! Unsupported statement classification and diagnostic ordering.

use super::*;

pub(super) fn unsupported_queryless_create_table() -> ProtocolStatement {
    let diagnostic = warning(
        "unsupported_queryless_create_table",
        DiagnosticArea::Statement,
        "CREATE TABLE without an AS query does not define transformation semantics",
    );

    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        "create_table".to_string(),
        vec![diagnostic],
    ))
}

pub(super) fn unsupported_write_statement(category: &str, code: &str, message: &str) -> ProtocolStatement {
    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        category.to_string(),
        vec![warning(code, DiagnosticArea::Statement, message)],
    ))
}

pub(super) fn unsupported_statement() -> ProtocolStatement {
    let diagnostic = warning(
        "unsupported_statement",
        DiagnosticArea::Statement,
        "the parsed SQL statement is not supported by semantic analysis",
    );

    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        "statement".to_string(),
        vec![diagnostic],
    ))
}

pub(super) fn warning(code: &str, area: DiagnosticArea, message: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        code.to_string(),
        area,
        message.to_string(),
    )
}

pub(super) fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        left.area()
            .as_str()
            .cmp(right.area().as_str())
            .then_with(|| left.code().cmp(right.code()))
            .then_with(|| left.message().cmp(right.message()))
    });
}
