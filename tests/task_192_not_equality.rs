//! TASK-192: direct ordinary integer equality under SQL NOT is a genuine
//! portable scalar gap. Only = / <> are inverted, with SQL NULL still UNKNOWN.
//! These tests use independent native bags, not an analyzer-derived expected
//! document, and leave ordered NOT and nullable IN conservative.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, ComposedSemantics,
    ConditionExactnessStatus, ConfiguredSqlInput, LiteralValue, RelationCatalog, SetMode, SqlInput,
    ValueDomain,
};

const DIALECTS: [&str; 13] = [
    "ansi",
    "bigquery",
    "clickhouse",
    "databricks",
    "duckdb",
    "generic",
    "hive",
    "mssql",
    "mysql",
    "postgresql",
    "redshift",
    "snowflake",
    "sqlite",
];

fn signed_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("DDL dialect");
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id INTEGER, amount INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("authoritative signed source schema");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("source schema")
}

fn check_domains(sql: &str, dialect_name: &str, should_include: bool) {
    let dialect = dialect_from_name(dialect_name).expect("public SQL dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-3vl-not-equality",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &signed_catalog())
        .expect("original scalar semantics");
    let ComposedSemantics::Resolved(semantics) = bundle
        .layers()
        .last()
        .expect("query layer")
        .composed_semantics()
    else {
        panic!("{dialect_name}: source lookup must resolve");
    };
    assert_eq!(
        semantics.condition_exactness().status(),
        ConditionExactnessStatus::Exact,
        "{dialect_name}: direct numeric equality NOT is three-valued exact"
    );
    let [column] = semantics.output().columns() else {
        panic!("{dialect_name}: one projected column");
    };
    let ValueDomain::Set(set) = column.domain() else {
        panic!("{dialect_name}: NOT equality must produce an exact finite set/exclusion");
    };
    assert_eq!(
        set.mode(),
        if should_include {
            SetMode::Include
        } else {
            SetMode::Exclude
        }
    );
    let values: Vec<_> = set.values().iter().map(|v| v.value().clone()).collect();
    let mut expected = vec![LiteralValue::Number("10".to_string())];
    // Exclusion must explicitly reject NULL. = retains only the matching
    // integer and therefore excludes NULL without a separate entry.
    if !should_include {
        expected.push(LiteralValue::Null);
    }
    assert_eq!(values.len(), expected.len());
    for value in expected {
        assert!(
            values.contains(&value),
            "{dialect_name}: missing expected scalar value {value:?}"
        );
    }
}

#[test]
fn direct_equality_not_exactly_inverts_scalar_domains_across_13_parsers() {
    for dialect in DIALECTS {
        for (sql, include) in [
            ("SELECT amount FROM orders WHERE NOT (amount = 10)", false),
            ("SELECT amount FROM orders WHERE NOT (amount <> 10)", true),
            ("SELECT amount FROM orders WHERE NOT (10 = amount)", false),
            ("SELECT amount FROM orders WHERE NOT (10 <> amount)", true),
        ] {
            check_domains(sql, dialect, include);
        }
    }
}

#[test]
fn native_full_null_duplicate_negative_bags_validate_inverted_truth() {
    let connection = Connection::open_in_memory().expect("DuckDB");
    connection
        .execute_batch(
            "CREATE TABLE orders (id INTEGER, amount INTEGER);
             INSERT INTO orders VALUES (1,-2), (2,10), (3,10), (4,NULL), (5,11);",
        )
        .expect("complete native source");
    let ids = |predicate: &str, truth: &str| {
        let query = format!("SELECT id FROM orders WHERE ({predicate}) {truth} ORDER BY id");
        let mut statement = connection.prepare(&query).expect("native SQL truth");
        statement
            .query_map([], |row| row.get::<_, i64>(0))
            .expect("complete identities")
            .map(|row| row.expect("signed row id"))
            .collect::<Vec<_>>()
    };
    for (predicate, selected, rejected, unknown) in [
        ("NOT (amount = 10)", vec![1, 5], vec![2, 3], vec![4]),
        ("NOT (amount <> 10)", vec![2, 3], vec![1, 5], vec![4]),
    ] {
        assert_eq!(ids(predicate, "IS TRUE"), selected);
        assert_eq!(ids(predicate, "IS FALSE"), rejected);
        assert_eq!(ids(predicate, "IS UNKNOWN"), unknown);
        let mut not_true = rejected.clone();
        not_true.extend(unknown);
        not_true.sort();
        assert_eq!(ids(predicate, "IS NOT TRUE"), not_true);
        assert_eq!(ids(predicate, ""), selected);
    }
}

#[test]
fn ordered_and_null_bearing_not_still_fail_closed() {
    let dialect = dialect_from_name("duckdb").expect("public parser");
    for sql in [
        "SELECT amount FROM orders WHERE NOT (amount > 10)",
        "SELECT amount FROM orders WHERE NOT (amount IN (10, NULL))",
        "SELECT amount FROM orders WHERE NOT NOT (amount = 10)",
    ] {
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "task192-still-residual",
            &input,
            "duckdb",
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &signed_catalog())
            .expect("canonical SQL");
        let ComposedSemantics::Resolved(composed) = bundle
            .layers()
            .last()
            .expect("query layer")
            .composed_semantics()
        else {
            panic!("expected single source");
        };
        assert_eq!(
            composed.condition_exactness().status(),
            ConditionExactnessStatus::Residual,
            "{sql}: no unsafe NOT generalization"
        );
    }
}
