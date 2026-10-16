//! TASK-107 independent public-seam proof audit for the five frozen projected expressions.
//!
//! The SQL/dbt parity test cannot establish exactness: it compares two analyses.
//! These fixed closed-world physical observations instead check projection NULL
//! truth in both directions and reject an analyzer claim without a typed proof.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, BooleanWitnessDirection,
    ComposedSemantics, ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-107-acceptance-freeze.json");

fn ids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut statement = db
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{sql}: {error}"));
    statement
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|error| panic!("{sql}: {error}"))
        .map(|item| item.expect("physical row id"))
        .collect()
}

#[test]
fn five_projection_families_have_typed_domains_and_independent_two_direction_null_proofs() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("frozen TASK-107");
    let dialect = dialect_from_name("postgresql").expect("postgresql parser");
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t (id BIGINT PRIMARY KEY, a INTEGER);
         INSERT INTO t VALUES
         (1, NULL), (2, 0), (3, 1), (4, 2), (5, 3),
         (6, 4), (7, -1), (8, -2147483648), (9, 2147483647)",
    )
    .unwrap();

    // Independent oracle: complete input population with NULL and narrowing
    // failure rows. The SQL analyzer is never used to derive these expected IDs.
    let cases: [(&str, &str, &[i64]); 5] = [
        ("E107-V028", "TRY_CAST(a AS SMALLINT)", &[1, 8, 9]),
        ("E107-V029", "CASE WHEN a > 1 THEN a ELSE 0 END", &[]),
        ("E107-V030", "COALESCE(a, 0)", &[]),
        ("E107-V031", "NULLIF(a, 1)", &[1, 3]),
        ("E107-V033", "CAST(a AS BIGINT) * 3", &[1]),
    ];

    for (id, expression, expected_null) in cases {
        let fixture = freeze["inventory_cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|cell| cell["id"] == id)
            .expect("frozen projection");
        assert_eq!(fixture["classification"], "exact_required", "{id}");
        assert!(
            fixture["sql_shape"].as_str().unwrap().contains(expression),
            "{id}: frozen SQL no longer exercises the audited expression"
        );

        let projection = SqlInput::inline(format!("SELECT {expression} AS v FROM t"));
        let projection_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "project",
                &projection,
                "postgresql",
                dialect.as_ref(),
            )],
            &catalog,
        )
        .unwrap_or_else(|error| panic!("{id}: projection: {error}"));
        let ComposedSemantics::Resolved(projected) =
            projection_bundle.layers()[0].composed_semantics()
        else {
            panic!("{id}: missing typed projected semantics");
        };
        assert!(
            !matches!(
                projected.output().columns()[0].domain(),
                ValueDomain::Unknown(_)
            ),
            "{id}: claimed mandatory exact projected domain is unknown"
        );

        let predicate = format!("({expression}) IS NULL");
        let filter = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
        let filtered = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "filter",
                &filter,
                "postgresql",
                dialect.as_ref(),
            )],
            &catalog,
        )
        .unwrap_or_else(|error| panic!("{id}: {predicate}: {error}"));
        let ComposedSemantics::Resolved(semantics) = filtered.layers()[0].composed_semantics()
        else {
            panic!("{id}: unresolved truth proof");
        };
        let exact_directions = semantics.boolean_witnesses().iter().any(|item| {
            let witness = item.witness();
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(_) | BooleanWitnessDirection::Impossible
            ) && matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(_) | BooleanWitnessDirection::Impossible
            )
        });
        assert!(
            exact_directions || semantics.condition_exactness().is_exact(),
            "{id}: missing typed SQL TRUE/NOT TRUE proof; {predicate}"
        );

        let true_sql = format!("SELECT id FROM t WHERE {predicate} ORDER BY id");
        let not_true_sql = format!("SELECT id FROM t WHERE ({predicate}) IS NOT TRUE ORDER BY id");
        assert_eq!(ids(&db, &true_sql), expected_null, "{id}: TRUE bag");
        let true_set: BTreeSet<_> = expected_null.iter().copied().collect();
        let independently_rejected = (1..=9)
            .filter(|row| !true_set.contains(row))
            .collect::<Vec<_>>();
        assert_eq!(
            ids(&db, &not_true_sql),
            independently_rejected,
            "{id}: NOT TRUE bag (no UNKNOWN for IS NULL)"
        );
    }
}
