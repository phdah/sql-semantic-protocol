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
fn opaque_and_null_bearing_not_still_fail_closed() {
    let dialect = dialect_from_name("duckdb").expect("public parser");
    for sql in [
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

#[test]
fn original_s011_has_exact_nullable_signed_interval_but_no_projected_id_bound() {
    const S011: &str = "SELECT id FROM orders WHERE NOT (amount > 1)";
    for name in DIALECTS {
        let dialect = dialect_from_name(name).expect("public parser");
        let input = SqlInput::inline(S011);
        let configured = [ConfiguredSqlInput::new(
            "task192-original-S011-signed",
            &input,
            name,
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &signed_catalog())
            .expect("original SQL under authoritative signed source type");
        let ComposedSemantics::Resolved(composed) = bundle
            .layers()
            .last()
            .expect("query layer")
            .composed_semantics()
        else {
            panic!("{name}: missing original signed source");
        };
        assert_eq!(
            composed.condition_exactness().status(),
            ConditionExactnessStatus::Exact,
            "S011/{name}: numeric NOT > is <= only for signed sources"
        );
        assert!(
            matches!(
                composed.output().columns()[0].domain(),
                ValueDomain::Unbounded
            ),
            "S011/{name}: filtering amount does not restrict projected id"
        );
        let amount = composed
            .column_domains()
            .iter()
            .find(|domain| {
                domain.column().relation() == Some("orders") && domain.column().name() == "amount"
            })
            .unwrap_or_else(|| panic!("S011/{name}: missing typed amount interval"));
        let ValueDomain::Ranges(ranges) = amount.domain() else {
            panic!("S011/{name}: signed amount predicate must be an interval");
        };
        let [range] = ranges.ranges() else {
            panic!("S011/{name}: one upper-bounded numeric interval");
        };
        assert!(
            range.lower().is_none(),
            "S011/{name}: no invented lower bound"
        );
        let bound = range.upper().expect("signed upper endpoint");
        assert_eq!(
            bound.value().value(),
            &LiteralValue::Number("1".to_string())
        );
        assert!(bound.inclusive(), "S011/{name}: complement of > is <=");
    }
}

#[test]
fn ordered_not_requires_authoritative_signed_type_and_never_assumes_nan_laws() {
    const SQL: &str = "SELECT amount FROM orders WHERE NOT (amount > 1)";
    let parser = dialect_from_name("duckdb").expect("public parser");
    let untyped =
        sql_semantic_protocol::analyze_sql(SQL, "duckdb", parser.as_ref()).expect("untyped SQL");
    let [sql_semantic_protocol::ProtocolStatement::Query(untyped)] = untyped.statements() else {
        panic!("one untyped query");
    };
    assert_eq!(
        untyped.condition_exactness().status(),
        ConditionExactnessStatus::Residual,
        "untyped ordered NOT remains conservative"
    );

    let ddl = SqlInput::inline("CREATE TABLE orders (id INTEGER, amount DOUBLE)");
    let schema = analyze_inputs(&[ddl], "duckdb", parser.as_ref()).expect("floating source type");
    let floating =
        RelationCatalog::from_schemas(schema.source_schemas()).expect("floating catalog");
    let input = SqlInput::inline(SQL);
    let configured = [ConfiguredSqlInput::new(
        "task192-float-not-negative",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let result =
        analyze_configured_inputs_with_catalog(&configured, &floating).expect("floating SQL");
    let ComposedSemantics::Resolved(composed) = result
        .layers()
        .last()
        .expect("floating query")
        .composed_semantics()
    else {
        panic!("one floating source");
    };
    assert_eq!(
        composed.condition_exactness().status(),
        ConditionExactnessStatus::Residual,
        "NaN and signed-zero comparison laws require separate evidence"
    );
}

#[test]
fn original_s011_native_three_valued_signed_truth_includes_nulls() {
    let connection = Connection::open_in_memory().expect("DuckDB");
    connection
        .execute_batch(
            "CREATE TABLE orders (id INTEGER, amount INTEGER);
             INSERT INTO orders VALUES (-1,-3), (0,1), (1,1), (2,2), (3,NULL), (4,100);",
        )
        .expect("complete nullable signed source");
    let ids = |truth: &str| {
        let query = format!("SELECT id FROM orders WHERE (NOT (amount > 1)) {truth} ORDER BY id");
        let mut statement = connection.prepare(&query).expect("native S011 SQL");
        statement
            .query_map([], |row| row.get::<_, i64>(0))
            .expect("native row identities")
            .map(|row| row.expect("physical ID"))
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("IS TRUE"), vec![-1, 0, 1]);
    assert_eq!(ids("IS FALSE"), vec![2, 4]);
    assert_eq!(ids("IS UNKNOWN"), vec![3]);
    assert_eq!(ids("IS NOT TRUE"), vec![2, 3, 4]);
}
