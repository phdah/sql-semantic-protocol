use sql_semantic_protocol::{
    analyze_sql, to_json, DiagnosticArea, Expression, Protocol, ProtocolStatement, QueryStatement,
    SetOperand, SetOperator, SetQuantifier, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

const DIALECTS: &[&str] = &[
    "generic",
    "mysql",
    "postgresql",
    "postgres",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn union_all_merges_positional_lineage_and_keeps_left_output_names() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT id FROM left_table UNION ALL SELECT user_id AS renamed FROM right_table",
        "generic",
        &dialect,
    )
    .expect("UNION ALL should analyze");

    let query = first_query(&protocol);
    let operation = query.set_operation().expect("set operation should be present");
    assert_eq!(operation.operator(), SetOperator::Union);
    assert_eq!(operation.quantifier(), SetQuantifier::All);

    let columns = query.output().columns();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name(), "id");
    assert!(matches!(columns[0].expression(), Expression::Unknown(_)));
    assert_eq!(columns[0].lineage().len(), 2);
    assert_eq!(columns[0].lineage()[0].relation(), "left_table");
    assert_eq!(columns[0].lineage()[0].column(), "id");
    assert_eq!(columns[0].lineage()[1].relation(), "right_table");
    assert_eq!(columns[0].lineage()[1].column(), "user_id");

    assert_eq!(
        query
            .dependencies()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["left_table", "right_table"]
    );

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(
        json["inputs"][0]["statements"][0]["set_operation"]["operator"],
        "union"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["set_operation"]["quantifier"],
        "all"
    );
}

#[test]
fn intersect_and_except_are_typed_semantics() {
    let dialect = GenericDialect {};

    for (sql, expected) in [
        (
            "SELECT id FROM left_table INTERSECT SELECT id FROM right_table",
            SetOperator::Intersect,
        ),
        (
            "SELECT id FROM left_table EXCEPT SELECT id FROM right_table",
            SetOperator::Except,
        ),
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).expect("set operation should analyze");
        let operation = first_query(&protocol)
            .set_operation()
            .expect("set operation should be present");

        assert_eq!(operation.operator(), expected);
        assert_eq!(operation.quantifier(), SetQuantifier::Distinct);
        assert_eq!(first_query(&protocol).output().columns().len(), 1);
    }
}

#[test]
fn nested_and_chained_set_operations_preserve_the_operation_tree() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "(SELECT id FROM a UNION ALL SELECT id FROM b) INTERSECT SELECT id FROM c",
        "generic",
        &dialect,
    )
    .expect("nested set operations should analyze");

    let operation = first_query(&protocol)
        .set_operation()
        .expect("outer set operation should be present");
    assert_eq!(operation.operator(), SetOperator::Intersect);
    assert!(matches!(
        operation.left(),
        SetOperand::Operation(inner) if inner.operator() == SetOperator::Union
            && inner.quantifier() == SetQuantifier::All
    ));
    assert!(matches!(operation.right(), SetOperand::Query));
    assert_eq!(first_query(&protocol).output().columns().len(), 1);
}

#[test]
fn arity_mismatch_is_explicit_and_does_not_invent_output() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a, b FROM left_table UNION SELECT a FROM right_table",
        "generic",
        &dialect,
    )
    .expect("parser should preserve an arity mismatch for semantic diagnostics");

    let query = first_query(&protocol);
    assert!(query.output().columns().is_empty());
    assert!(query.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Output
            && diagnostic.code() == "set_operation_arity_mismatch"
    }));
}

#[test]
fn conflicting_branch_domains_degrade_to_unknown() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE a > 10 UNION ALL SELECT a FROM t WHERE a < 0",
        "generic",
        &dialect,
    )
    .expect("branch domains should analyze conservatively");

    let domains = first_query(&protocol).column_domains();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].column().relation(), Some("t"));
    assert_eq!(domains[0].column().name(), "a");
    assert!(matches!(domains[0].domain(), ValueDomain::Unknown(_)));
}

#[test]
fn set_level_order_and_limit_remain_attached_to_the_result() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT id FROM a UNION ALL SELECT id FROM b ORDER BY id LIMIT 5",
        "generic",
        &dialect,
    )
    .expect("set-level clauses should remain attached to the query");

    let query = first_query(&protocol);
    assert_eq!(query.output().columns().len(), 1);
    assert!(query
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_order_by"));
    assert!(query
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_limit"));
}

#[test]
fn standard_union_all_is_analyzed_across_all_exposed_dialects() {
    let sql = "SELECT id FROM left_table UNION ALL SELECT id FROM right_table";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} failed UNION ALL: {error}"));

        let operation = first_query(&protocol)
            .set_operation()
            .expect("UNION ALL should be represented");
        assert_eq!(
            operation.operator(),
            SetOperator::Union,
            "dialect {dialect_name}"
        );
        assert_eq!(
            operation.quantifier(),
            SetQuantifier::All,
            "dialect {dialect_name}"
        );
    }
}

#[test]
fn snowflake_minus_is_normalized_to_except() {
    let dialect = dialect_from_str("snowflake").expect("snowflake dialect should exist");
    let protocol = analyze_sql(
        "SELECT id FROM left_table MINUS SELECT id FROM right_table",
        "snowflake",
        dialect.as_ref(),
    )
    .expect("Snowflake MINUS should parse and analyze");

    let operation = first_query(&protocol)
        .set_operation()
        .expect("MINUS should be represented as a set operation");
    assert_eq!(operation.operator(), SetOperator::Except);
    assert_eq!(operation.quantifier(), SetQuantifier::Distinct);
}
