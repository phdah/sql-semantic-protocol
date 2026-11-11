//! TASK-192 original S013: prove correlated EXISTS on physical source-row
//! occurrences, including duplicates, NULL and unmatched rows. This is a
//! source-local membership witness, not scalar-domain or vendor signoff.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, ComposedSemantics,
    ConditionExactnessStatus, ConfiguredSqlInput, RelationCatalog, SqlInput,
    SubqueryMembershipCase, SubqueryMembershipDirection, SubqueryMembershipKind, ValueDomain,
};

const MATRIX: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn native_ids(db: &Connection, query: &str) -> Vec<i64> {
    let mut statement = db.prepare(query).expect("native original SQL");
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("native complete identity rows");
    let mut ids = rows.map(|row| row.expect("native source occurrence")).collect::<Vec<_>>();
    ids.sort();
    ids
}

fn original_exists() -> (Value, String) {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen original inventory");
    let sql = matrix["cases"]
        .as_array()
        .expect("43 original queries")
        .iter()
        .find(|case| case["id"] == "88-C177-S013")
        .expect("S013")["sql"]
        .as_str()
        .expect("original SQL")
        .to_string();
    (matrix, sql)
}

#[test]
fn original_correlated_exists_has_complete_duplicate_preserving_true_false_bags() {
    let (_, sql) = original_exists();
    let db = Connection::open_in_memory().expect("DuckDB");
    // Two outer occurrences of id=1 and two inner matches of id=1. SQL
    // EXISTS is a SEMIJOIN: inner multiplicity does not multiply outer rows.
    db.execute_batch(
        "CREATE TABLE orders (id INTEGER, amount INTEGER);
         CREATE TABLE customers (id INTEGER, name VARCHAR);
         INSERT INTO orders VALUES
           (-1,10), (0,10), (1,10), (1,NULL), (2,19), (3,20), (NULL,30);
         INSERT INTO customers VALUES
           (1,'one'), (1,'duplicate'), (3,'three'), (NULL,'null');",
    )
    .expect("independently authored complete physical source bags");
    assert_eq!(
        native_ids(&db, "SELECT rowid FROM orders"),
        vec![0, 1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        native_ids(&db, "SELECT rowid FROM customers"),
        vec![0, 1, 2, 3]
    );
    assert_eq!(native_ids(&db, &sql), vec![1, 1, 3]);

    let condition = "EXISTS (SELECT 1 FROM customers c WHERE c.id = o.id)";
    let not_condition = "NOT EXISTS (SELECT 1 FROM customers c WHERE c.id = o.id)";
    for (test, selected) in [
        (format!("({condition}) IS TRUE"), vec![2, 3, 5]),
        (format!("({condition}) IS FALSE"), vec![0, 1, 4, 6]),
        (format!("({condition}) IS UNKNOWN"), vec![]),
        (format!("({condition}) IS NOT TRUE"), vec![0, 1, 4, 6]),
        (not_condition.to_string(), vec![0, 1, 4, 6]),
    ] {
        assert_eq!(
            native_ids(&db, &format!("SELECT o.rowid FROM orders o WHERE {test}")),
            selected,
            "complete source occurrence truth: {test}"
        );
    }
    assert_eq!(
        native_ids(&db, "SELECT o.id FROM orders o WHERE NOT EXISTS (SELECT 1 FROM customers c WHERE c.id = o.id) AND o.id IS NOT NULL"),
        vec![-1, 0, 2],
        "NOT EXISTS preserves outer duplicate and nullable row identities"
    );
}

#[test]
fn s013_has_separate_physical_source_correlations_and_exact_both_direction_witness() {
    let (matrix, sql) = original_exists();
    let ddl = SqlInput::inline(
        "CREATE TABLE orders (id INTEGER, amount INTEGER);
         CREATE TABLE customers (id INTEGER, name VARCHAR)",
    );
    let parser = dialect_from_name("duckdb").expect("DDL parser");
    let source = analyze_inputs(&[ddl], "duckdb", parser.as_ref())
        .expect("authoritative physical source schemas");
    let catalog = RelationCatalog::from_schemas(source.source_schemas())
        .expect("source catalog");

    for name in matrix["dialects"].as_array().expect("13 parser dialects") {
        let name = name.as_str().expect("dialect");
        let dialect = dialect_from_name(name).expect("public parser");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "task192-original-s013",
            &input,
            name,
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .unwrap_or_else(|error| panic!("S013/{name}: {error}"));
        let layer = bundle.layers().last().expect("original query layer");
        let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
            panic!("S013/{name}: missing composed source");
        };
        assert_eq!(
            composed.condition_exactness().status(),
            ConditionExactnessStatus::Residual,
            "S013/{name}: subquery witness is source-local, not whole-query scalar Exact"
        );
        assert!(
            matches!(composed.output().columns()[0].domain(), ValueDomain::Unbounded | ValueDomain::Unknown(_)),
            "S013/{name}: correlated EXISTS cannot infer a finite projected id set"
        );

        let [entry] = composed.subquery_witnesses() else {
            panic!("S013/{name}: require one source-local EXISTS witness");
        };
        let witness = entry.witness();
        assert_eq!(witness.kind(), SubqueryMembershipKind::Exists);
        assert_eq!(witness.outer_relation(), "o");
        assert_eq!(witness.outer_source_relation(), Some("orders"));
        assert_eq!(witness.inner_relation(), Some("customers"));
        let [equality] = witness.correlations() else {
            panic!("S013/{name}: require exactly one source correlation");
        };
        assert_eq!(equality.outer().relation(), Some("orders"));
        assert_eq!(equality.outer().name(), "id");
        assert_eq!(equality.inner().relation(), Some("customers"));
        assert_eq!(equality.inner().name(), "id");
        assert_eq!(
            witness.qualifying(),
            &SubqueryMembershipDirection::Exact(vec![SubqueryMembershipCase::MatchingRow]),
            "S013/{name}: complete matching candidate witness"
        );
        assert_eq!(
            witness.rejected(),
            &SubqueryMembershipDirection::Exact(vec![SubqueryMembershipCase::NoCandidates]),
            "S013/{name}: NULL key or missing match is NOT TRUE"
        );
    }
}
