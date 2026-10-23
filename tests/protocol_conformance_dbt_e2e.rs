//! Executable TASK-99 integration gate against the byte-protocol conformance dbt corpus.
//! Run through `make dbt-e2e`, which materializes all eleven models before this test.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_finite_producer_exists_plan,
    physical_joint_row_count_plan, physical_row_count_plan, physical_source_plan, to_bundle_json,
    AnalysisBundle, ConfiguredSqlInput, LiteralValue, PhysicalProducerExistsProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, SqlInput, TimestampZone, WitnessDirection,
};

const PROJECT: &str = "protocol_conformance";
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance/v1/dbt_core_project")
}

fn model<'a>(manifest: &'a Value, name: &str) -> &'a Value {
    let id = format!("model.{PROJECT}.{name}");
    manifest["nodes"]
        .get(&id)
        .unwrap_or_else(|| panic!("missing conformance model {id}"))
}

fn layer_id<'a>(bundle: &'a AnalysisBundle, name: &str) -> &'a str {
    let id = format!("model.{PROJECT}.{name}");
    bundle
        .layers()
        .iter()
        .find(|layer| layer.input_id() == id)
        .unwrap_or_else(|| panic!("missing analyzed model {id}"))
        .id()
}

fn count(db: &Connection, model: &str) -> i64 {
    db.query_row(
        &format!("SELECT COUNT(*) FROM raw_analytics.\"{model}\""),
        [],
        |row| row.get(0),
    )
    .unwrap_or_else(|error| panic!("conformance dbt model {model} failed: {error}"))
}

#[test]
#[ignore = "requires compiled conformance dbt artifacts; run make dbt-e2e"]
fn conformance_conformance_dbt_project_certifies_all_models_and_fail_closed_plans() {
    let root = root();
    let suite: Value = serde_json::from_str(include_str!("fixtures/conformance/v1/suite.json"))
        .expect("protocol-owned conformance suite");
    let expectations = suite["dbt"]["models"].as_array().expect("dbt expectations");
    assert_eq!(expectations.len(), 11, "the v1 conformance inventory");
    let manifest_text = fs::read_to_string(root.join("target/manifest.json"))
        .expect("make dbt-e2e must compile the protocol conformance manifest");
    let catalog_text = fs::read_to_string(root.join("target/catalog.json"))
        .expect("make dbt-e2e must introspect the protocol conformance catalog");
    let json: Value = serde_json::from_str(&manifest_text).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let manifest = parse_dbt_manifest(&manifest_text).unwrap();
    let catalog = parse_dbt_catalog(&catalog_text).unwrap();
    let mut bundle = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref())
        .expect("conformance direct dbt model graph is analyzable");
    assert_eq!(
        to_bundle_json(&bundle),
        to_bundle_json(
            &analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap()
        ),
        "canonical dbt analysis is not byte deterministic"
    );
    for expectation in expectations {
        let name = expectation["name"].as_str().expect("model name");
        let node = model(&json, name);
        let compiled = node["compiled_code"]
            .as_str()
            .unwrap_or_else(|| panic!("{name} is not compiled"));
        let original = fs::read_to_string(root.join("models").join(format!("{name}.sql"))).unwrap();
        assert_eq!(node["raw_code"].as_str().unwrap().trim(), original.trim());
        assert!(!compiled.contains("{{"), "Jinja leaked into {name}");
        analyze_sql(compiled, "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("compiled {name} is invalid SQL: {error}"));
        let _ = layer_id(&bundle, name);
    }
    let db_path = std::env::var("DBT_E2E_DATABASE")
        .expect("Makefile must select the conformance dbt database");
    let db = Connection::open(db_path).expect("dbt-generated DuckDB fixture");
    // Shared versioned suite expectations are the only output oracle source.
    for expectation in expectations {
        let name = expectation["name"].as_str().expect("model name");
        let expected = expectation["row_count"].as_i64().expect("row count");
        assert_eq!(
            count(&db, name),
            expected,
            "{name}: whole-project target changed"
        );
        let expression = expectation["snapshot_expression"]
            .as_str()
            .expect("snapshot projection");
        let query = format!("SELECT {expression} FROM raw_analytics.\"{name}\" ORDER BY 1");
        let mut statement = db.prepare(&query).expect("snapshot query");
        let observed = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("typed snapshot")
            .map(|row| row.expect("snapshot text"))
            .collect::<Vec<_>>();
        let expected_rows = expectation["expected_rows"]
            .as_array()
            .expect("expected rows")
            .iter()
            .map(|row| row.as_str().expect("expected value").to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            observed, expected_rows,
            "{name}: complete physical output changed"
        );
    }
    for seed in suite["dbt"]["seeds"]
        .as_array()
        .expect("canonical seed names")
    {
        let relation = seed
            .as_str()
            .expect("seed file")
            .strip_suffix(".csv")
            .unwrap();
        let query = format!("SELECT COUNT(*) FROM raw.{relation}");
        let rows: i64 = db.query_row(&query, [], |row| row.get(0)).unwrap();
        assert_eq!(
            rows,
            suite["dbt"]["source_row_count"]
                .as_i64()
                .expect("source row count"),
            "{relation}: conformance source was not materialized"
        );
    }
    let joined: (String, i64, i64) = db
        .query_row(
            "SELECT region, order_count, max_amount FROM raw_analytics.aggregate_summary",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(joined, ("north".to_string(), 1, 42));
    let derived: (String, i64) = db
        .query_row(
            "SELECT amount_bucket, paid_flag FROM raw_analytics.derived_orders",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(derived, ("medium".to_string(), 1));
    let rank: i64 = db
        .query_row("SELECT rn FROM raw_analytics.ranked_orders", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(rank, 1);
    let duplicate_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM raw_analytics.unioned_orders WHERE amount=42 AND region='north'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(duplicate_count, 2);
    for name in ["final_orders", "boundary_final_orders", "subquery_orders"] {
        let rejections: i64 = db.query_row(
            &format!("SELECT COUNT(*) FROM raw_analytics.\"{name}\" WHERE amount=5 OR amount IS NULL"),
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(rejections, 0, "{name}: deliberately rejected rows leaked");
    }
    // TASK-132: evaluate the canonical producer DAG from independent complete
    // physical leaf bags, not from materialized dbt intermediate snapshots.
    let proof_graph = physical_source_plan(&bundle, layer_id(&bundle, "subquery_orders"));
    assert_eq!(proof_graph.sources().len(), 2);
    let mut physical = Vec::new();
    for name in proof_graph.sources() {
        let unquoted = name.replace(['"', '`'], "");
        if unquoted.ends_with(".orders") {
            let mut q = db.prepare(
                "SELECT id,customer_id,amount,status,CAST(created_at AS VARCHAR),region FROM raw.orders ORDER BY id"
            ).unwrap();
            let rows = q
                .query_map([], |r| {
                    Ok(PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Number(r.get::<_, i64>(0)?.to_string()),
                        ),
                        (
                            "customer_id".to_string(),
                            LiteralValue::Number(r.get::<_, i64>(1)?.to_string()),
                        ),
                        (
                            "amount".to_string(),
                            LiteralValue::Number(r.get::<_, i64>(2)?.to_string()),
                        ),
                        (
                            "status".to_string(),
                            LiteralValue::Text(r.get::<_, String>(3)?),
                        ),
                        (
                            "created_at".to_string(),
                            LiteralValue::Text(r.get::<_, String>(4)?),
                        ),
                        (
                            "region".to_string(),
                            LiteralValue::Text(r.get::<_, String>(5)?),
                        ),
                    ]))
                    .unwrap())
                })
                .unwrap()
                .collect::<duckdb::Result<Vec<_>>>()
                .unwrap();
            physical.push(PhysicalWriteState::new(name, rows).unwrap());
        } else if unquoted.ends_with(".customers") {
            let mut q = db
                .prepare("SELECT id,score,active FROM raw.customers ORDER BY id")
                .unwrap();
            let rows = q
                .query_map([], |r| {
                    Ok(PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Number(r.get::<_, i64>(0)?.to_string()),
                        ),
                        (
                            "score".to_string(),
                            LiteralValue::Number(r.get::<_, i64>(1)?.to_string()),
                        ),
                        (
                            "active".to_string(),
                            LiteralValue::Boolean(r.get::<_, bool>(2)?),
                        ),
                    ]))
                    .unwrap())
                })
                .unwrap()
                .collect::<duckdb::Result<Vec<_>>>()
                .unwrap();
            physical.push(PhysicalWriteState::new(name, rows).unwrap());
        } else {
            panic!("unexpected TASK-132 physical leaf: {name}");
        }
    }
    let timestamp_type: String = db.query_row(
        "SELECT typeof(created_at) FROM raw.orders LIMIT 1",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(timestamp_type, "TIMESTAMP");
    let orders_name = physical.iter().find(|state| {
        state.relation().replace(['"', '`'], "").ends_with(".orders")
    }).unwrap().relation().to_string();
    bundle.attest_source_timestamp_zone(
        &orders_name, "created_at", TimestampZone::WithoutTimeZone,
    ).expect("DuckDB introspected unzoned TIMESTAMP physical source");

    let final_id = layer_id(&bundle, "subquery_orders");
    let proof = physical_finite_producer_exists_plan(&bundle, final_id, &physical, true);
    let PhysicalProducerExistsProof::Exact(exact) = proof else {
        panic!("source-complete canonical producer proof is not Exact: {proof:?}");
    };
    for name in [
        "stg_orders",
        "stg_customers",
        "enriched_orders",
        "subquery_orders",
    ] {
        assert_eq!(
            exact.layer_bags[layer_id(&bundle, name)].len(),
            1,
            "{name}: complete producer bag"
        );
    }
    assert_eq!(exact.terminal.len(), 1);
    assert_eq!(
        exact.terminal[0].values,
        BTreeMap::from([
            ("amount".to_string(), LiteralValue::Number("42".to_string())),
            (
                "region".to_string(),
                LiteralValue::Text("north".to_string())
            ),
        ])
    );
    assert_eq!(exact.terminal[0].source_row_ids.len(), 2);
    assert_eq!(
        exact.terminal_domains["amount"].integer_bounds,
        Some((42, 42))
    );
    assert_eq!(
        exact.terminal_domains["region"].values,
        vec![LiteralValue::Text("north".to_string())]
    );
    assert!(!exact.terminal_domains["amount"].null_allowed);
    assert!(!exact.terminal_domains["region"].null_allowed);
    assert_eq!(exact.rejected[layer_id(&bundle, "stg_orders")].len(), 1);
    assert_eq!(exact.rejected[layer_id(&bundle, "stg_customers")].len(), 1);
    assert!(matches!(
        physical_finite_producer_exists_plan(&bundle, final_id, &physical, false),
        PhysicalProducerExistsProof::Residual { .. }
    ));
    let mut duplicate = physical.clone();
    let index = duplicate
        .iter()
        .position(|state| state.relation().contains("orders"))
        .unwrap();
    let mut rows = duplicate[index].rows().to_vec();
    rows.push(rows[0].clone());
    duplicate[index] = PhysicalWriteState::new(duplicate[index].relation(), rows).unwrap();
    assert!(matches!(
        physical_finite_producer_exists_plan(&bundle, final_id, &duplicate, true),
        PhysicalProducerExistsProof::Impossible { .. }
    ));

    // TASK-132: independent complete raw source, intermediary producer, and
    // correlated final terminal snapshots for the actual compiled dbt model.
    let task_132: Value =
        serde_json::from_str(include_str!("../docs/task-132-acceptance-freeze.json"))
            .expect("versioned task 132 acceptance freeze");
    let case = task_132["cases"]
        .as_array()
        .expect("task 132 cases")
        .iter()
        .find(|case| case["variant"] == "dbt_subquery_orders")
        .expect("named subquery_orders.sql matrix cell");
    assert_eq!(case["expected_selected_source_ids"][0], "orders:1");
    assert_eq!(case["expected_rejected_not_true_source_ids"][0], "orders:2");
    let mut raw_order_query = db
        .prepare("SELECT id,customer_id,amount,status,region FROM raw.orders ORDER BY id")
        .expect("physical raw orders");
    let raw_orders = raw_order_query
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        raw_orders,
        vec![
            (1, 10, 42, "paid".to_string(), "north".to_string()),
            (2, 20, 5, "pending".to_string(), "south".to_string()),
        ]
    );
    let mut raw_customer_query = db
        .prepare("SELECT id,score,active FROM raw.customers ORDER BY id")
        .expect("physical raw customers");
    let raw_customers = raw_customer_query
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(raw_customers, vec![(10, 7, true), (20, -1, false)]);

    for (name, sql, expected) in [
        (
            "stg_orders",
            "SELECT order_id FROM raw_analytics.stg_orders ORDER BY order_id",
            vec![1_i64],
        ),
        (
            "stg_customers",
            "SELECT customer_id FROM raw_analytics.stg_customers ORDER BY customer_id",
            vec![10_i64],
        ),
        (
            "enriched_orders",
            "SELECT order_id FROM raw_analytics.enriched_orders ORDER BY order_id",
            vec![1_i64],
        ),
    ] {
        let mut stmt = db.prepare(sql).expect("materialized dbt intermediary");
        let actual = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            actual, expected,
            "{name}: complete physical producer identity"
        );
    }
    let mut terminal_query = db
        .prepare("SELECT amount,region FROM raw_analytics.subquery_orders ORDER BY amount,region")
        .expect("compiled subquery terminal");
    let actual = terminal_query
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(actual, vec![(42, "north".to_string())]);
    assert_eq!(case["expected_terminal_bag"][0]["amount"], 42);
    assert_eq!(case["expected_terminal_bag"][0]["region"], "north");

    let rejected: Vec<i64> = db
        .prepare("SELECT id FROM raw.orders WHERE status <> 'paid' OR amount < 20 ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(rejected, vec![2_i64], "negative raw source identity");

    // The positive joined/grouped example is an engine oracle, not an automatic
    // proof that every possible complete physical-source bag is constructible.
    let final_id = layer_id(&bundle, "aggregate_summary");
    let final_proof = physical_row_count_plan(&bundle, final_id, 1);
    assert!(
        matches!(final_proof, WitnessDirection::Residual { .. }),
        "positive multi-parent aggregate cannot claim a source-level proof without complete joint populations: {final_proof:?}"
    );
    let stage = layer_id(&bundle, "stg_orders");
    let contradiction = physical_joint_row_count_plan(&bundle, &[(stage, 0), (stage, 1)]);
    assert!(
        matches!(contradiction, WitnessDirection::Impossible),
        "two different exact counts for one canonical terminal must contradict: {contradiction:?}"
    );
    let graph = physical_source_plan(&bundle, final_id);
    assert!(
        graph
            .sources()
            .iter()
            .any(|source| source.contains("\"raw\".\"orders\"")),
        "the canonical graph lost the physical source: {graph:?}"
    );
    let zero = physical_joint_row_count_plan(
        &bundle,
        &[
            (layer_id(&bundle, "stg_orders"), 0),
            (layer_id(&bundle, "aggregate_summary"), 0),
        ],
    );
    assert!(
        matches!(zero, WitnessDirection::Feasible(_)),
        "the source-complete jointly empty dbt DAG must be realizable: {zero:?}"
    );

    // Compare schema-backed direct SQL and dbt adapter for a source-local
    // terminal. Both derive the same physical source, count proof, and SQL.
    let stage_sql = model(&json, "stg_orders")["compiled_code"]
        .as_str()
        .unwrap();
    let stage_input = SqlInput::inline(stage_sql);
    let configured = ConfiguredSqlInput::new("direct", &stage_input, "duckdb", dialect.as_ref());
    let source_catalog = RelationCatalog::from_schemas(bundle.source_schemas()).unwrap();
    let direct = analyze_configured_inputs_with_catalog(&[configured], &source_catalog)
        .expect("direct conformance compiled SQL");
    let direct_graph = physical_source_plan(&direct, direct.layers()[0].id());
    let dbt_graph = physical_source_plan(&bundle, layer_id(&bundle, "stg_orders"));
    assert_eq!(direct_graph.sources(), dbt_graph.sources());
    let direct_zero = physical_row_count_plan(&direct, direct.layers()[0].id(), 0);
    let dbt_zero = physical_row_count_plan(&bundle, layer_id(&bundle, "stg_orders"), 0);
    assert!(
        matches!(direct_zero, WitnessDirection::Feasible(_))
            && matches!(dbt_zero, WitnessDirection::Feasible(_)),
        "both adapters must prove empty physical sources: direct={direct_zero:?}, dbt={dbt_zero:?}"
    );
}
