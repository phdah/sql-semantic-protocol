use sql_semantic_protocol::{analyze_sql, to_json, AnalysisError, Error, ProtocolStatement};
use sqlparser::dialect::{GenericDialect, SnowflakeDialect};

#[test]
fn valid_sql_returns_protocol_for_caller_selected_dialect() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT 1", "generic", &dialect)
        .expect("valid SQL should cross the public analysis boundary");

    assert_eq!(protocol.protocol_version(), "0.1.0");
    assert_eq!(protocol.source().dialect(), "generic");

    match protocol.statements().first() {
        Some(ProtocolStatement::Unsupported(statement)) => {
            assert_eq!(statement.category(), "query");
            assert_eq!(statement.diagnostics().len(), 1);
        }
        other => panic!("expected an explicit unsupported query statement, got {other:?}"),
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
    assert_eq!(value["statements"][0]["kind"], "unsupported");
}
