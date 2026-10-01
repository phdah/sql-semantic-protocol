use sql_semantic_protocol::{
    analyze_sql, to_json, AnalysisError, DiagnosticArea, Error, Predicate, ProtocolStatement,
};
use sqlparser::dialect::{GenericDialect, SnowflakeDialect};

#[test]
fn valid_sql_returns_partial_query_for_caller_selected_dialect() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT 1", "generic", &dialect)
        .expect("valid SQL should cross the public analysis boundary");

    assert_eq!(protocol.protocol_version(), "0.1.0");
    assert_eq!(protocol.source().dialect(), "generic");

    match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => {
            assert!(statement.predicates().where_predicate().is_none());
            assert!(statement
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.area() == DiagnosticArea::Output));
        }
        other => panic!("expected a partial query statement, got {other:?}"),
    }
}

#[test]
fn caller_can_supply_a_different_sqlparser_dialect() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql("SELECT 1", "snowflake", &dialect)
        .expect("Snowflake dialect should be supplied by the caller");

    assert_eq!(protocol.source().dialect(), "snowflake");
}

#[test]
fn parse_failures_are_distinct_library_errors() {
    let dialect = GenericDialect {};
    let error = analyze_sql("SELECT (", "generic", &dialect)
        .expect_err("malformed SQL should fail while parsing");

    assert!(matches!(error, Error::Parse(_)));
}

#[test]
fn analysis_failures_are_distinct_library_errors() {
    let dialect = GenericDialect {};
    let error =
        analyze_sql("", "generic", &dialect).expect_err("empty SQL should fail during analysis");

    assert!(matches!(
        error,
        Error::Analysis(AnalysisError::NoStatements)
    ));
}

#[test]
fn serialization_is_separate_from_analysis() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT 1", "generic", &dialect)
        .expect("valid SQL should produce protocol values");
    let json = to_json(&protocol);
    let value: serde_json::Value =
        serde_json::from_str(&json).expect("emitted protocol should be valid JSON");

    assert_eq!(value["protocol_version"], "0.1.0");
    assert_eq!(value["source"]["dialect"], "generic");
    assert_eq!(value["statements"][0]["kind"], "query");
}

#[test]
fn parsed_predicate_is_preserved_as_explicitly_unsupported() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT a FROM t WHERE a > 10", "generic", &dialect)
        .expect("query should preserve partial semantics");

    let statement = match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => statement,
        other => panic!("expected query statement, got {other:?}"),
    };

    match statement.predicates().where_predicate() {
        Some(Predicate::Unsupported(semantic)) => {
            assert_eq!(semantic.feature(), "where_predicate");
        }
        other => panic!("expected explicit unsupported WHERE predicate, got {other:?}"),
    }

    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Predicate
            && diagnostic.code() == "unsupported_where_predicate"
    }));
}

#[test]
fn unsupported_statement_remains_explicit() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("CREATE TABLE t (a INT)", "generic", &dialect)
        .expect("supported parser statement should return protocol output");

    match protocol.statements().first() {
        Some(ProtocolStatement::Unsupported(statement)) => {
            assert_eq!(statement.category(), "statement");
            assert_eq!(statement.diagnostics()[0].code(), "unsupported_statement");
        }
        other => panic!("expected unsupported statement, got {other:?}"),
    }
}

#[test]
fn unsupported_query_body_is_diagnosed_without_dropping_query() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("VALUES (1)", "generic", &dialect)
        .expect("VALUES query should parse and preserve its query envelope");

    match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => assert!(statement
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "unsupported_query_body")),
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn unsupported_table_factor_is_diagnosed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT * FROM (SELECT 1) AS derived", "generic", &dialect)
        .expect("derived table should parse");

    let statement = match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => statement,
        other => panic!("expected query statement, got {other:?}"),
    };

    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Source
            && diagnostic.code() == "unsupported_table_factor"
    }));
}

#[test]
fn unsupported_expression_is_diagnosed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE WHEN a > 0 THEN a ELSE 0 END FROM t",
        "generic",
        &dialect,
    )
    .expect("CASE expression should parse");

    let statement = match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => statement,
        other => panic!("expected query statement, got {other:?}"),
    };

    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Expression
            && diagnostic.code() == "unsupported_expression"
    }));
}

#[test]
fn unsupported_function_is_diagnosed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT COALESCE(a, 0) FROM t", "generic", &dialect)
        .expect("function expression should parse");

    let statement = match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => statement,
        other => panic!("expected query statement, got {other:?}"),
    };

    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Function
            && diagnostic.code() == "unsupported_function"
    }));
}
