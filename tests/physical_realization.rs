//! TASK-68 physical leaf topology and the conservative single-row proof boundary.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_source_plan,
    AnalysisBundle, ConfiguredSqlInput, PhysicalPlanRef, PhysicalProofGap, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessFormula, WitnessObligation,
};

fn bundle(queries: &[&str], dialect: &str) -> AnalysisBundle {
    let dialect_impl = dialect_from_name(dialect).expect("recognized dialect");
    let schemas = ["t", "l", "r", "stage", "mart"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["a", "b", "k"]
                    .into_iter()
                    .map(|name| {
                        SchemaColumn::from_sql_type(name, "INTEGER", "postgresql")
                            .expect("integer column")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let sources = queries
        .iter()
        .map(|q| SqlInput::inline(*q))
        .collect::<Vec<_>>();
    let ids = (0..queries.len())
        .map(|i| format!("q-{i}"))
        .collect::<Vec<_>>();
    let inputs = sources
        .iter()
        .zip(&ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, dialect, dialect_impl.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&inputs, &catalog).expect("analysis")
}

#[test]
fn simple_filter_has_both_physical_row_classifications_across_dialects() {
    for &dialect in DIALECTS {
        let b = bundle(&["SELECT a, b FROM t WHERE a > 2 OR b < 0"], dialect);
        let plan = physical_source_plan(&b, b.layers()[0].id());
        assert_eq!(plan.gap(), None, "{dialect}: {plan:?}");
        assert_eq!(plan.sources(), &["t".to_string()]);
        assert!(matches!(plan.qualifying(), WitnessDirection::Feasible(_)));
        assert!(matches!(plan.rejected(), WitnessDirection::Feasible(_)));
        assert_eq!(plan.nodes().len(), 2);
        assert_eq!(plan.nodes()[0].id(), &PhysicalPlanRef::Source("t".into()));
        let WitnessDirection::Feasible(cases) = plan.rejected() else {
            panic!("expected deliberate rejection");
        };
        assert!(cases
            .iter()
            .any(|case| case.obligations().iter().any(|obligation| {
                matches!(
                    obligation,
                    WitnessObligation::Predicate(WitnessFormula::RowTruth { row, .. })
                        if row.relation() == "t"
                )
            })));
    }
}

#[test]
fn transparent_producer_is_a_reference_not_a_second_physical_table() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "CREATE TABLE mart AS SELECT a, b FROM stage",
            "SELECT a FROM mart WHERE a IS NOT NULL OR b IS NULL",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[2].id());
    assert_eq!(plan.gap(), None, "{plan:?}");
    assert_eq!(plan.sources(), &["t".to_string()]);
    assert_eq!(plan.nodes().len(), 4);
    assert_eq!(plan.nodes()[0].id(), &PhysicalPlanRef::Source("t".into()));
    assert_eq!(
        plan.nodes()[1].id(),
        &PhysicalPlanRef::Layer("layer-0001".into())
    );
    assert_eq!(
        plan.nodes()[2].id(),
        &PhysicalPlanRef::Layer("layer-0002".into())
    );
    assert_eq!(
        plan.nodes()[3].id(),
        &PhysicalPlanRef::Layer("layer-0003".into())
    );
    assert_eq!(
        plan.nodes()[2].inputs(),
        &[PhysicalPlanRef::Layer("layer-0001".into())]
    );
    assert!(matches!(plan.rejected(), WitnessDirection::Feasible(_)));
}

#[test]
fn filtered_upstream_and_downstream_require_joint_satisfiability_proof() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a > 10 OR b < 0",
            "SELECT a FROM stage WHERE a < 20 OR b < 0",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[1].id());
    assert_eq!(plan.gap(), Some(PhysicalProofGap::MultipleWitnesses));
    assert!(matches!(
        plan.qualifying(),
        WitnessDirection::Residual { .. }
    ));
    assert!(matches!(plan.rejected(), WitnessDirection::Residual { .. }));
    assert_eq!(plan.sources(), &["t".to_string()]);
}

#[test]
fn computed_and_row_limited_projections_do_not_upgrade_local_evidence() {
    for query in [
        "SELECT a + 1 AS b FROM t WHERE a > 2 OR b < 0",
        "SELECT a FROM t WHERE a > 2 OR b < 0 LIMIT 1",
        "SELECT DISTINCT a FROM t WHERE a > 2 OR b < 0",
    ] {
        let b = bundle(&[query], "postgresql");
        let plan = physical_source_plan(&b, b.layers()[0].id());
        assert_eq!(
            plan.gap(),
            Some(PhysicalProofGap::NonInvertibleTransformation),
            "{query}"
        );
    }
}

#[test]
fn shared_producer_is_visited_once_and_unsupported_join_is_residual() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "SELECT x.a FROM stage x JOIN stage y ON x.a = y.a",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[1].id());
    assert_eq!(plan.sources(), &["t".to_string()]);
    assert_eq!(plan.nodes().len(), 3);
    assert!(plan.gap().is_some());
    assert!(matches!(
        plan.qualifying(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn partial_writes_and_missing_targets_fail_closed() {
    let b = bundle(
        &[
            "INSERT INTO stage SELECT a, b FROM t",
            "SELECT a FROM stage WHERE a IS NOT NULL OR b IS NULL",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[1].id());
    assert_eq!(plan.gap(), Some(PhysicalProofGap::UnresolvedSemantics));
    let missing = physical_source_plan(&b, "nonexistent");
    assert_eq!(missing.gap(), Some(PhysicalProofGap::UnknownTarget));
    assert!(missing.sources().is_empty());
}

#[test]
fn duckdb_terminal_rows_agree_with_physical_source_membership_classification() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "SELECT a FROM stage WHERE a IS NOT NULL OR b IS NULL",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[1].id());
    assert_eq!(plan.gap(), None, "{plan:?}");
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (1, 5), (4, 6), (NULL, 7);
         CREATE TABLE stage AS SELECT a, b FROM t;",
    )
    .expect("fixture");
    let mut stmt = conn
        .prepare("SELECT a FROM stage WHERE a IS NOT NULL OR b IS NULL")
        .expect("query");
    let output = stmt
        .query_map([], |row| row.get::<_, i32>(0))
        .expect("rows")
        .map(|row| row.expect("integer result"))
        .collect::<Vec<_>>();
    assert_eq!(output, vec![1, 4]);
    // The NULL candidate is deliberately rejected by the NULL-aware predicate.
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM t WHERE (a IS NOT NULL OR b IS NULL) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .expect("rejected rows");
    assert_eq!(count, 1);
}

#[test]
fn zero_output_is_a_closed_world_empty_physical_source_obligation() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "SELECT a + 1 FROM stage WHERE a > 2",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[1].id());
    let WitnessDirection::Feasible(cases) = plan.zero_output() else {
        panic!("single-source filter cannot create a row from an empty input");
    };
    assert_eq!(cases.len(), 1);
    assert!(cases[0].obligations().iter().any(|obligation| matches!(
        obligation,
        WitnessObligation::Rows { boundary, bounds, closed_world: true, .. }
            if boundary.relation() == "t"
                && bounds.minimum() == 0
                && bounds.maximum() == Some(0)
    )));
    assert!(cases[0].obligations().iter().any(|obligation| matches!(
        obligation,
        WitnessObligation::ClosedWorld { boundary, .. }
            if boundary.relation() == "t"
    )));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
        CREATE TABLE stage AS SELECT a, b FROM t;",
    )
    .expect("empty source");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a + 1 FROM stage WHERE a > 2)",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 0);
}

#[test]
fn zero_output_does_not_mistake_global_aggregate_for_empty_result() {
    let b = bundle(&["SELECT COUNT(*) FROM t"], "postgresql");
    let plan = physical_source_plan(&b, b.layers()[0].id());
    assert!(matches!(
        plan.zero_output(),
        WitnessDirection::Residual { .. }
    ));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE t(a INTEGER, b INTEGER)")
        .expect("empty source");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("global count");
    assert_eq!(count, 0);
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM (SELECT COUNT(*) FROM t)", [], |row| {
            row.get(0)
        })
        .expect("aggregate row");
    assert_eq!(rows, 1);
}

#[test]
fn canonical_wire_graph_deduplicates_producer_nodes_and_references() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "CREATE TABLE mart AS SELECT a, b FROM stage",
            "SELECT a FROM stage WHERE a > 2 OR b < 0",
        ],
        "postgresql",
    );
    let raw: serde_json::Value = serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b))
        .expect("valid canonical protocol");
    let graph = &raw["graph"];
    let nodes = graph["physical_nodes"].as_array().expect("physical nodes");
    let plans = graph["physical_source_plans"]
        .as_array()
        .expect("physical plans");
    assert_eq!(
        nodes.len(),
        4,
        "one unique node per physical source or layer"
    );
    assert_eq!(plans.len(), b.layers().len());
    assert_eq!(
        nodes
            .iter()
            .filter(|n| n["ref"] == serde_json::json!({"kind":"source","id":"t"}))
            .count(),
        1,
    );
    let mart = plans
        .iter()
        .find(|p| p["layer_id"] == "layer-0002")
        .expect("mart");
    assert_eq!(mart["node_refs"].as_array().expect("refs").len(), 3);
    assert_eq!(mart["physical_sources"], serde_json::json!(["t"]));
    assert_eq!(mart["zero_output"]["status"], "feasible");
    assert_eq!(mart["qualifying"]["status"], "residual");
    assert_eq!(mart["gap"], "no_witness");

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json"))
            .expect("schema parses");
    assert_eq!(
        schema["$defs"]["graph"]["properties"]["physical_source_plans"]["items"]["$ref"],
        "#/$defs/physicalSourcePlan",
    );
    assert_eq!(
        schema["$defs"]["physicalSourcePlan"]["properties"]["zero_output"]["$ref"],
        "#/$defs/constructiveDirection",
    );
    assert_eq!(
        sql_semantic_protocol::to_bundle_json(&b),
        sql_semantic_protocol::to_bundle_json(&b),
        "protocol must remain deterministic",
    );
}
