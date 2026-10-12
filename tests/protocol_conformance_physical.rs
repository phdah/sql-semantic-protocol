//! Physical-source proofs against the protocol-owned versioned conformance suite.
//! The protocol owns the fixture cases, proof expectations and physical oracles.

mod common;
use common::DIALECTS;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_row_count_plan,
    physical_ordered_write_state_plan, physical_row_count_plan, physical_source_plan,
    AnalysisBundle, ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    WitnessDirection, WitnessObligation,
};

const ADVANCED: &str = include_str!("fixtures/conformance/v1/advanced_pipeline.sql");
const BOUNDARY: &str = include_str!("fixtures/conformance/v1/advanced_boundary.sql");
const SETS: &str = include_str!("fixtures/conformance/v1/set_operations.sql");
const WINDOW: &str = include_str!("fixtures/conformance/v1/raw_sql/window_ranked.sql");
const INSERT: &str = include_str!("fixtures/conformance/v1/raw_sql/insert_workflow.sql");
const EXISTS: &str = include_str!("fixtures/conformance/v1/raw_sql/unsupported_exists.sql");
const RANGE: &str = include_str!("fixtures/conformance/v1/raw_sql/range_sampling.sql");

fn fixture_bundle(sql: &str) -> AnalysisBundle {
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).expect("dialect");
    let columns = [
        (
            "raw_orders",
            &[
                ("order_id", "INTEGER"),
                ("customer_id", "INTEGER"),
                ("amount", "INTEGER"),
            ][..],
        ),
        (
            "raw_customers",
            &[("customer_id", "INTEGER"), ("active", "BOOLEAN")][..],
        ),
        ("raw_a", &[("value", "INTEGER")][..]),
        ("raw_b", &[("value", "INTEGER")][..]),
        ("raw_c", &[("value", "INTEGER")][..]),
        (
            "raw_window",
            &[("category", "VARCHAR"), ("score", "INTEGER")][..],
        ),
        ("raw_insert", &[("value", "INTEGER")][..]),
        ("insert_sink", &[("value", "INTEGER")][..]),
        (
            "raw_exists_orders",
            &[("customer_id", "INTEGER"), ("amount", "INTEGER")][..],
        ),
        ("raw_exists_customers", &[("customer_id", "INTEGER")][..]),
        ("raw_range", &[("value", "INTEGER")][..]),
    ];
    let schemas = columns
        .iter()
        .map(|(relation, columns)| {
            RelationSchema::new(
                *relation,
                columns
                    .iter()
                    .map(|(name, sql_type)| {
                        SchemaColumn::from_sql_type(*name, sql_type, dialect)
                            .expect("schema column")
                    })
                    .collect(),
            )
            .expect("relation schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let input = SqlInput::inline(sql);
    let configured =
        ConfiguredSqlInput::new("protocol-conformance", &input, dialect, parser.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).expect("fixture analysis")
}

fn layer_id<'a>(bundle: &'a AnalysisBundle, relation: &str) -> &'a str {
    bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|producer| producer.relation_name() == Some(relation))
        })
        .unwrap_or_else(|| panic!("missing fixture output {relation}"))
        .id()
}

#[test]
fn conformance_advanced_pipeline_constructs_joint_terminal_zero_from_raw_sources() {
    let bundle = fixture_bundle(ADVANCED);
    let summary = layer_id(&bundle, "mart_customer_summary");
    let stage = layer_id(&bundle, "stage_orders");
    let plan = physical_source_plan(&bundle, summary);
    assert_eq!(
        plan.sources(),
        &["raw_customers".to_string(), "raw_orders".to_string()]
    );
    assert!(
        matches!(plan.zero_output(), WitnessDirection::Feasible(_)),
        "conformance pipeline must be empty-preserving: {plan:?}; each layer: {:?}",
        bundle
            .layers()
            .iter()
            .map(|layer| (
                layer.id(),
                physical_source_plan(&bundle, layer.id())
                    .zero_output()
                    .clone()
            ))
            .collect::<Vec<_>>()
    );
    let joint = physical_joint_row_count_plan(&bundle, &[(stage, 0), (summary, 0)]);
    let WitnessDirection::Feasible(cases) = joint else {
        panic!("shared sources must prove both protocol conformance terminals empty: {joint:?}");
    };
    assert_eq!(
        cases[0]
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count(),
        2
    );
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_orders(order_id INTEGER, customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_customers(customer_id INTEGER, active BOOLEAN);",
    )
    .expect("source schemas");
    conn.execute_batch(ADVANCED)
        .expect("protocol conformance pipeline");
    for sql in [
        "SELECT COUNT(*) FROM stage_orders",
        "SELECT COUNT(*) FROM stage_customers",
        "SELECT COUNT(*) FROM core_enriched",
        "SELECT COUNT(*) FROM mart_customer_summary",
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("output");
        assert_eq!(count, 0, "{sql}");
    }
}

#[test]
fn conformance_boundary_fixture_retains_join_provenance_and_zero_rejection() {
    let bundle = fixture_bundle(BOUNDARY);
    let enriched = layer_id(&bundle, "core_enriched");
    let plan = physical_source_plan(&bundle, enriched);
    assert!(matches!(plan.zero_output(), WitnessDirection::Feasible(_)));
    assert!(matches!(
        physical_row_count_plan(&bundle, enriched, 1),
        WitnessDirection::Residual { .. }
    ));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_orders(order_id INTEGER, customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_customers(customer_id INTEGER, active BOOLEAN);",
    )
    .expect("source schemas");
    conn.execute_batch(BOUNDARY)
        .expect("protocol conformance boundary");
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM core_enriched", [], |row| row.get(0))
        .expect("enriched count");
    assert_eq!(rows, 0);
}

#[test]
fn conformance_set_fixtures_prove_empty_closed_world_and_respect_multiplicities() {
    let bundle = fixture_bundle(SETS);
    let wire: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&bundle))
            .expect("canonical JSON");
    let branches = wire["inputs"][0]["statements"][3]["set_operation"]["membership"]["branches"]
        .as_array()
        .expect("typed canonical set branches");
    assert_eq!(branches.len(), 2);
    assert!(branches
        .iter()
        .all(|branch| { branch["empty_input_preserving"] == serde_json::Value::Bool(true) }));
    for target in [
        "union_all_result",
        "union_result",
        "intersect_result",
        "except_result",
        "distinct_result",
        "limited_result",
    ] {
        let id = layer_id(&bundle, target);
        let proof = physical_row_count_plan(&bundle, id, 0);
        assert!(
            matches!(proof, WitnessDirection::Feasible(_)),
            "protocol conformance set fixture {target} must preserve empty inputs: {proof:?}; source plan: {:?}",
            physical_source_plan(&bundle, id)
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_a(value INTEGER);
         CREATE TABLE raw_b(value INTEGER);
         CREATE TABLE raw_c(value INTEGER);",
    )
    .expect("source schemas");
    conn.execute_batch(SETS)
        .expect("protocol conformance set pipeline");
    for target in [
        "union_all_result",
        "union_result",
        "intersect_result",
        "except_result",
        "distinct_result",
        "limited_result",
    ] {
        let sql = format!("SELECT COUNT(*) FROM {target}");
        let rows: i64 = conn
            .query_row(&sql, [], |row| row.get(0))
            .expect("set output count");
        assert_eq!(rows, 0, "{target}");
    }
}

#[test]
fn conformance_advanced_pipeline_oracle_exercises_positive_and_rejected_rows() {
    let bundle = fixture_bundle(ADVANCED);
    let summary = layer_id(&bundle, "mart_customer_summary");
    assert!(
        matches!(
            physical_row_count_plan(&bundle, summary, 1),
            WitnessDirection::Residual { .. }
        ),
        "a positive joined/grouped construction is not yet supported"
    );
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_orders(order_id INTEGER, customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_customers(customer_id INTEGER, active BOOLEAN);
         INSERT INTO raw_customers VALUES (1, true), (2, false);
         INSERT INTO raw_orders VALUES
            (11, 1, 100), (12, 2, 100), (13, 1, 90), (14, 999, 100);",
    )
    .expect("controlled source data");
    conn.execute_batch(ADVANCED)
        .expect("protocol conformance advanced SQL");
    let counts = [
        ("stage_orders", 3_i64),
        ("stage_customers", 1),
        ("core_enriched", 1),
        ("mart_customer_summary", 1),
    ];
    for (table, expected) in counts {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = conn.query_row(&sql, [], |row| row.get(0)).expect("count");
        assert_eq!(count, expected, "{table}");
    }
    let (category, orders, maximum): (String, i64, i64) = conn
        .query_row(
            "SELECT amount_bucket, order_count, max_amount FROM mart_customer_summary",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("summarized row");
    assert_eq!((category.as_str(), orders, maximum), ("high", 1, 100));
}

#[test]
fn conformance_set_oracles_preserve_duplicate_and_distinct_multiplicity() {
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_a(value INTEGER);
         CREATE TABLE raw_b(value INTEGER);
         CREATE TABLE raw_c(value INTEGER);
         INSERT INTO raw_a VALUES (10), (10), (NULL);
         INSERT INTO raw_b VALUES (20);
         INSERT INTO raw_c VALUES (30);",
    )
    .expect("controlled duplicate and NULL inputs");
    conn.execute_batch(SETS)
        .expect("protocol conformance set SQL");
    for (target, expected) in [
        ("set_a", 2_i64),
        ("set_b", 1),
        ("set_c", 1),
        ("union_all_result", 2),
        ("union_result", 1),
        ("intersect_result", 1),
        ("except_result", 1),
        ("distinct_result", 1),
        ("limited_result", 1),
    ] {
        let sql = format!("SELECT COUNT(*) FROM {target}");
        let rows: i64 = conn
            .query_row(&sql, [], |row| row.get(0))
            .expect("set cardinality");
        assert_eq!(rows, expected, "{target}");
    }
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM union_all_result WHERE marker = 2",
            [],
            |row| row.get(0),
        )
        .expect("limited rejection");
    assert_eq!(count, 0);
}

#[test]
fn source_free_set_leaf_cannot_be_erased_by_emptying_the_other_source() {
    let bundle = fixture_bundle("SELECT 1 AS marker UNION ALL SELECT value AS marker FROM raw_a");
    let target = bundle.layers()[0].id();
    assert!(matches!(
        physical_source_plan(&bundle, target).zero_output(),
        WitnessDirection::Residual { .. }
    ));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE raw_a(value INTEGER)")
        .expect("empty source schema");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT 1 AS marker UNION ALL SELECT value AS marker FROM raw_a)",
            [],
            |row| row.get(0),
        )
        .expect("set count");
    assert_eq!(count, 1);
}

#[test]
fn conformance_corpus_has_all_seven_raw_and_eleven_dbt_model_shapes() {
    let suite: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/conformance/v1/suite.json"))
            .expect("protocol-owned suite manifest");
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance/v1");
    let raw = suite["raw_sql"].as_array().expect("raw SQL cases");
    assert_eq!(raw.len(), 7);
    for case in raw {
        let sql = std::fs::read_to_string(root.join(case["path"].as_str().unwrap()))
            .expect("protocol-owned SQL case");
        assert!(!sql.trim().is_empty());
        // The portable manifest is authoritative for certified physical count
        // classes; compare them against the actual typed protocol planner.
        if let Some(goals) = case["row_count_goals"].as_array() {
            let bundle = fixture_bundle(&sql);
            for goal in goals {
                let target = goal["target"].as_str().expect("target relation");
                let rows = goal["rows"].as_u64().expect("nonnegative count") as usize;
                let proof = physical_row_count_plan(&bundle, layer_id(&bundle, target), rows);
                let expected = goal["outcome"].as_str().expect("proof outcome");
                let agrees = match expected {
                    "feasible" => matches!(proof, WitnessDirection::Feasible(_)),
                    "residual" => matches!(proof, WitnessDirection::Residual { .. }),
                    "impossible" => matches!(proof, WitnessDirection::Impossible),
                    _ => panic!("unknown conformance classification: {expected}"),
                };
                assert!(agrees, "{target}/{rows}: expected {expected}, got {proof:?}");
            }
        }
    }
    let models = suite["dbt"]["models"].as_array().expect("dbt models");
    assert_eq!(models.len(), 11);
    for model in models {
        let sql = std::fs::read_to_string(root.join(model["path"].as_str().unwrap()))
            .expect("protocol-owned model");
        assert!(sql.contains("select") || sql.contains("SELECT"));
    }
    let seeds = suite["dbt"]["seeds"].as_array().expect("dbt seeds");
    assert_eq!(seeds.len(), 4);
    for seed in seeds {
        let csv = std::fs::read_to_string(
            root.join("dbt_core_project/seeds")
                .join(seed.as_str().unwrap()),
        )
        .expect("protocol-owned seed");
        assert!(csv.lines().count() >= 3, "source has header and data");
    }
}

#[test]
fn pinned_range_fixture_has_exact_positive_and_rejected_source_rows() {
    let bundle = fixture_bundle(RANGE);
    let id = layer_id(&bundle, "range_result");
    let first = physical_row_count_plan(&bundle, id, 2);
    assert!(
        matches!(first, WitnessDirection::Feasible(_)),
        "a single filtered physical source must produce a complete positive count: {first:?}"
    );
    assert_eq!(first, physical_row_count_plan(&bundle, id, 2));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw_range(value INTEGER);
         INSERT INTO raw_range VALUES (10), (10), (NULL), (9), (1001);
    ",
    )
    .unwrap();
    db.execute_batch(RANGE).unwrap();
    let (total, nulls, rejected): (i64, i64, i64) = db
        .query_row(
            "SELECT (SELECT COUNT(*) FROM range_result),
                    (SELECT COUNT(*) FROM range_result WHERE value IS NULL),
                    (SELECT COUNT(*) FROM raw_range WHERE value IS NULL OR value < 10 OR value > 1000)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((total, nulls, rejected), (2, 0, 3));
}

#[test]
fn pinned_window_qualify_fixture_obeys_partition_and_null_rejection() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw_window(category VARCHAR, score INTEGER);
         INSERT INTO raw_window VALUES
           ('keep',10),('keep',10),('keep',NULL),('discard',10),('keep',11);",
    )
    .unwrap();
    db.execute_batch(WINDOW).unwrap();
    let values: (i64, i64, i64) = db
        .query_row(
            "SELECT COUNT(*), MIN(marker), MAX(rn) FROM ranked_result",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(values, (1, 1, 1));
    // Rank-one is a single terminal row even with duplicate eligible sources.
    let source_matches: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM raw_window WHERE category='keep' AND score=10",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(source_matches, 2);
}

#[test]
fn pinned_exists_fixture_requires_a_matching_source_row_and_rejects_null() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw_exists_orders(customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_exists_customers(customer_id INTEGER);
         INSERT INTO raw_exists_orders VALUES (1,100),(2,100),(NULL,100),(1,90);
         INSERT INTO raw_exists_customers VALUES (1),(NULL);",
    )
    .unwrap();
    db.execute_batch(EXISTS).unwrap();
    let matches: i64 = db
        .query_row("SELECT COUNT(*) FROM exists_result", [], |r| r.get(0))
        .unwrap();
    assert_eq!(matches, 1);
    let bundle = fixture_bundle(EXISTS);
    let id = layer_id(&bundle, "exists_result");
    let plan = physical_source_plan(&bundle, id);
    assert_eq!(
        plan.sources(),
        &[
            "raw_exists_customers".to_string(),
            "raw_exists_orders".to_string()
        ]
    );
    // Do not infer that executing an example proves arbitrary correlated populations.
    let proof = physical_row_count_plan(&bundle, id, 1);
    assert!(!matches!(proof, WitnessDirection::Impossible), "{proof:?}");
}

#[test]
fn pinned_insert_select_realizes_a_closed_world_state_with_duplicates() {
    use std::collections::BTreeMap;
    let bundle = fixture_bundle(INSERT);
    let row = |value: Option<i64>| {
        PhysicalWriteRow::new(BTreeMap::from([(
            "value".to_string(),
            value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
        )]))
        .unwrap()
    };
    let initial = [
        PhysicalWriteState::new(
            "raw_insert",
            vec![row(Some(10)), row(Some(10)), row(None), row(Some(11))],
        )
        .unwrap(),
        PhysicalWriteState::new("insert_sink", Vec::new()).unwrap(),
    ];
    let plan = physical_ordered_write_state_plan(&bundle, &initial);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("conformance INSERT has a complete typed source state: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].affected().inserted(), 2);
    let target = final_states
        .iter()
        .find(|state| state.relation() == "insert_sink")
        .unwrap();
    assert_eq!(target.rows(), &[row(Some(10)), row(Some(10))]);

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw_insert(value INTEGER);
         CREATE TABLE insert_sink(value INTEGER);
         INSERT INTO raw_insert VALUES (10),(10),(NULL),(11);",
    )
    .unwrap();
    db.execute_batch(INSERT).unwrap();
    let snapshot: (i64, i64, i64) = db.query_row(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE value=10), COUNT(*) FILTER (WHERE value IS NULL) FROM insert_sink",
        [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).unwrap();
    assert_eq!(snapshot, (2, 2, 0));
}

#[test]
fn raw_sql_and_dbt_semantics_are_deterministic_across_pinned_catalog_inputs() {
    let sql = "CREATE VIEW range_result AS SELECT value FROM raw_range WHERE value >= 10 AND value <= 1000";
    let reference = fixture_bundle(sql);
    let serialized = sql_semantic_protocol::to_bundle_json(&reference);
    assert_eq!(
        serialized,
        sql_semantic_protocol::to_bundle_json(&fixture_bundle(sql))
    );
    let range = fixture_bundle(RANGE);
    let bound = physical_row_count_plan(&range, layer_id(&range, "range_result"), 2);
    assert_eq!(
        bound,
        physical_row_count_plan(&range, layer_id(&range, "range_result"), 2)
    );
    for &dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        let input =
            SqlInput::inline("SELECT value FROM raw_range WHERE value >= 10 AND value <= 1000");
        let configured = ConfiguredSqlInput::new("range", &input, dialect, parser.as_ref());
        let schema = RelationSchema::new(
            "raw_range",
            vec![SchemaColumn::from_sql_type("value", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap();
        let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
        let b = analyze_configured_inputs_with_catalog(&[configured], &catalog)
            .unwrap_or_else(|e| panic!("{dialect} must parse shared SQL: {e}"));
        let proof = physical_row_count_plan(&b, b.layers()[0].id(), 2);
        assert!(
            matches!(proof, WitnessDirection::Feasible(_)),
            "{dialect}: {proof:?}"
        );
    }
}
