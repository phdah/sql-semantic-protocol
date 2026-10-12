//! Executable TASK-99 integration gate against the byte-pinned sql-tdg dbt corpus.
//! Run through `make dbt-e2e`, which materializes all eleven models before this test.

use std::fs;
use std::path::PathBuf;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_joint_row_count_plan, physical_row_count_plan,
    physical_source_plan, to_bundle_json, AnalysisBundle, ConfiguredSqlInput, RelationCatalog,
    SqlInput, WitnessDirection,
};

const PROJECT: &str = "sql_tdg_e2e";
const MODELS: [&str; 11] = [
    "aggregate_summary",
    "boundary_final_orders",
    "derived_orders",
    "enriched_orders",
    "final_orders",
    "independent_return_summary",
    "ranked_orders",
    "stg_customers",
    "stg_orders",
    "subquery_orders",
    "unioned_orders",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sql_tdg/dbt_core_project")
}

fn model<'a>(manifest: &'a Value, name: &str) -> &'a Value {
    let id = format!("model.{PROJECT}.{name}");
    manifest["nodes"]
        .get(&id)
        .unwrap_or_else(|| panic!("missing pinned model {id}"))
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
    .unwrap_or_else(|error| panic!("pinned dbt model {model} failed: {error}"))
}

#[test]
#[ignore = "requires compiled pinned dbt artifacts; run make dbt-e2e"]
fn pinned_sql_tdg_dbt_project_certifies_all_models_and_fail_closed_plans() {
    let root = root();
    let manifest_text = fs::read_to_string(root.join("target/manifest.json"))
        .expect("make dbt-e2e must compile the pinned sql-tdg manifest");
    let catalog_text = fs::read_to_string(root.join("target/catalog.json"))
        .expect("make dbt-e2e must introspect the pinned sql-tdg catalog");
    let json: Value = serde_json::from_str(&manifest_text).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let manifest = parse_dbt_manifest(&manifest_text).unwrap();
    let catalog = parse_dbt_catalog(&catalog_text).unwrap();
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref())
        .expect("pinned direct dbt model graph is analyzable");
    assert_eq!(
        to_bundle_json(&bundle),
        to_bundle_json(
            &analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap()
        ),
        "canonical dbt analysis is not byte deterministic"
    );
    for &name in &MODELS {
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
    let db_path =
        std::env::var("DBT_E2E_DATABASE").expect("Makefile must select the pinned dbt database");
    let db = Connection::open(db_path).expect("dbt-generated DuckDB fixture");
    for (name, expected) in [
        ("aggregate_summary", 1),
        ("boundary_final_orders", 1),
        ("derived_orders", 1),
        ("enriched_orders", 1),
        ("final_orders", 1),
        ("independent_return_summary", 1),
        ("ranked_orders", 1),
        ("stg_customers", 1),
        ("stg_orders", 1),
        ("subquery_orders", 1),
        ("unioned_orders", 2),
    ] {
        assert_eq!(
            count(&db, name),
            expected,
            "{name}: whole-project target changed"
        );
    }
    // Every terminal sees the same physical seed bag. Check output values,
    // including duplicate multiplicity, rather than counts alone.
    let snapshots: [(&str, &str, &[&str]); 11] = [
        ("stg_orders", "order_id::VARCHAR || ':' || amount::VARCHAR || ':' || status || ':' || region", &["1:42:paid:north"]),
        ("stg_customers", "customer_id::VARCHAR || ':' || score::VARCHAR", &["10:7"]),
        ("enriched_orders", "order_id::VARCHAR || ':' || amount::VARCHAR || ':' || score::VARCHAR", &["1:42:7"]),
        ("aggregate_summary", "region || ':' || order_count::VARCHAR || ':' || max_amount::VARCHAR", &["north:1:42"]),
        ("boundary_final_orders", "amount::VARCHAR", &["42"]),
        ("derived_orders", "amount_bucket || ':' || paid_flag::VARCHAR", &["medium:1"]),
        ("final_orders", "amount::VARCHAR || ':' || status || ':' || region", &["42:paid:north"]),
        ("independent_return_summary", "reason || ':' || return_count::VARCHAR || ':' || max_refund::VARCHAR", &["full:1:70"]),
        ("ranked_orders", "region || ':' || amount::VARCHAR || ':' || rn::VARCHAR", &["north:42:1"]),
        ("subquery_orders", "amount::VARCHAR || ':' || region", &["42:north"]),
        ("unioned_orders", "amount::VARCHAR || ':' || region", &["42:north", "42:north"]),
    ];
    for (name, expr, expected) in snapshots {
        let query = format!("SELECT {expr} FROM raw_analytics.{name} ORDER BY 1");
        let mut statement = db.prepare(&query).expect("snapshot query");
        let observed = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("typed snapshot")
            .map(|row| row.expect("snapshot text"))
            .collect::<Vec<_>>();
        let expected = expected.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        assert_eq!(observed, expected, "{name}: complete physical output changed");
    }
    for relation in ["orders", "customers", "legacy_orders", "returns"] {
        let query = format!("SELECT COUNT(*) FROM raw.{relation}");
        let rows: i64 = db.query_row(&query, [], |row| row.get(0)).unwrap();
        assert_eq!(rows, 2, "{relation}: pinned source was not materialized");
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
        .expect("direct pinned compiled SQL");
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
