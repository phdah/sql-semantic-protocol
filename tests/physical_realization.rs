//! TASK-68 physical leaf topology and the conservative single-row proof boundary.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_row_count_plan,
    physical_row_count_plan, physical_source_plan, AnalysisBundle, ConfiguredSqlInput,
    ConstraintValue, OutcomeGoal, OutcomeGoalStatus, OutcomeWitness, OutputDistribution,
    OutputValueCount, PhysicalPlanRef, PhysicalProofGap, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, WitnessDirection, WitnessFormula, WitnessObligation,
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
        assert!(plan.nodes()[0].operator_witnesses().is_empty());
        assert_eq!(plan.nodes()[1].operator_witnesses().len(), 1);
        assert_eq!(
            plan.nodes()[1].operator_witnesses()[0].origin_layer_id(),
            plan.target_layer_id()
        );
        let wire: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b))
                .expect("typed graph JSON");
        let node = &wire["graph"]["physical_nodes"]
            .as_array()
            .expect("physical nodes")[1];
        assert_eq!(node["operator_witnesses"][0]["operator"], "boolean",);

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
fn null_sensitive_filters_from_two_layers_are_jointly_solved_at_one_physical_leaf() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t WHERE a IS NOT NULL OR b IS NOT NULL",
                "SELECT a, b FROM stage WHERE a IS NULL OR b IS NOT NULL",
            ],
            dialect,
        );
        let plan = physical_source_plan(&b, b.layers()[1].id());
        assert_eq!(plan.gap(), None, "{dialect}: {plan:?}");
        assert_eq!(plan.sources(), &["t".to_string()]);
        for direction in [plan.qualifying(), plan.rejected()] {
            let WitnessDirection::Feasible(cases) = direction else {
                panic!("{dialect}: coupled physical filters must be jointly feasible: {plan:?}");
            };
            assert!(cases.iter().all(|case| case.obligations().iter().any(|o| {
                matches!(o, WitnessObligation::Predicate(WitnessFormula::RowTruth { row, .. })
                    if row.relation() == "t")
            })));
        }
    }

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (1,2),(NULL,3),(1,NULL),(NULL,NULL);
         CREATE TABLE stage AS SELECT a,b FROM t WHERE a IS NOT NULL OR b IS NOT NULL;",
    )
    .expect("joint rows");
    let included: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM stage WHERE a IS NULL OR b IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("terminal count");
    assert_eq!(included, 2);
    let rejected: i64 = conn
        .query_row("SELECT COUNT(*) FROM t WHERE ((a IS NOT NULL OR b IS NOT NULL) AND (a IS NULL OR b IS NOT NULL)) IS NOT TRUE", [], |r| r.get(0))
        .expect("rejections");
    assert_eq!(rejected, 2);
}

#[test]
fn contradictory_multilayer_filter_rejects_without_false_positive_realization() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a IS NULL AND b IS NOT NULL",
            "SELECT a FROM stage WHERE a IS NOT NULL OR b IS NULL",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[1].id());
    assert!(!matches!(plan.qualifying(), WitnessDirection::Feasible(_)));
    assert!(matches!(plan.rejected(), WitnessDirection::Feasible(_)));
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
    assert_eq!(plan.gap(), Some(PhysicalProofGap::PartialProducer));
    assert!(
        plan.nodes().iter().any(|node| {
            matches!(node.id(), PhysicalPlanRef::Layer(_)) && node.write_kind().is_some()
        }),
        "partial producer kind must survive as a reference node"
    );
    let effect_bundle: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("DML emitted");
    assert!(effect_bundle["graph"]["physical_nodes"]
        .as_array()
        .expect("graph nodes")
        .iter()
        .any(|n| n["write_kind"] == "append"));

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
fn graph_cycle_and_ambiguous_producers_are_distinct_typed_residuals() {
    let cyclic = bundle(
        &[
            "CREATE TABLE first AS SELECT a FROM second",
            "CREATE TABLE second AS SELECT a FROM first",
        ],
        "postgresql",
    );
    assert_eq!(
        physical_source_plan(&cyclic, cyclic.layers()[0].id()).gap(),
        Some(PhysicalProofGap::Cycle),
    );

    let ambiguous = bundle(
        &[
            "CREATE TABLE stage AS SELECT a FROM t",
            "CREATE TABLE stage AS SELECT a FROM r",
            "SELECT a FROM stage",
        ],
        "postgresql",
    );
    assert_eq!(
        physical_source_plan(&ambiguous, ambiguous.layers()[2].id()).gap(),
        Some(PhysicalProofGap::AmbiguousProducer),
    );
}

#[test]
fn contradictory_source_domains_remain_explicitly_unrealized() {
    let b = bundle(&["SELECT a FROM t WHERE a > 10 AND a < 3"], "postgresql");
    let plan = physical_source_plan(&b, b.layers()[0].id());
    assert_eq!(plan.gap(), Some(PhysicalProofGap::ConflictingDomains));
    assert!(matches!(
        plan.qualifying(),
        WitnessDirection::Residual { .. }
    ));
    assert!(matches!(plan.zero_output(), WitnessDirection::Feasible(_)));
    let json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("canonical graph");
    assert_eq!(
        json["graph"]["physical_source_plans"][0]["gap"],
        "conflicting_domains"
    );
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
fn empty_grouped_join_dag_is_a_closed_world_zero_count_proof() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT l.a AS a, COUNT(*) AS n FROM l INNER JOIN r ON l.k = r.k GROUP BY l.a HAVING COUNT(*) > 1",
                "SELECT a FROM stage WHERE a > 0",
            ],
            dialect,
        );
        let target = b.layers()[1].id();
        let plan = physical_source_plan(&b, target);
        assert!(
            matches!(plan.zero_output(), WitnessDirection::Feasible(_)),
            "{dialect}: an ordinary GROUP BY cannot synthesize rows from empty joins: {plan:?}"
        );
        assert_eq!(plan.sources(), &["l".to_string(), "r".to_string()]);
        let WitnessDirection::Feasible(cases) = physical_row_count_plan(&b, target, 0) else {
            panic!("{dialect}: zero terminal rows must be constructible");
        };
        let obligations = cases[0].obligations();
        assert_eq!(
            obligations
                .iter()
                .filter(|obligation| matches!(obligation, WitnessObligation::ClosedWorld { .. }))
                .count(),
            2
        );
        assert!(obligations.iter().any(|obligation| matches!(
            obligation, WitnessObligation::OutputRows { layer_id, bounds }
                if layer_id == target && bounds.minimum() == 0 && bounds.maximum() == Some(0)
        )));
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         CREATE TABLE stage AS SELECT l.a AS a, COUNT(*) AS n
             FROM l INNER JOIN r ON l.k = r.k
             GROUP BY l.a HAVING COUNT(*) > 1;",
    )
    .expect("empty grouped join");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM stage WHERE a > 0", [], |row| {
            row.get(0)
        })
        .expect("grouped join count");
    assert_eq!(count, 0);
}

#[test]
fn empty_single_source_regular_grouping_is_zero_but_rollup_is_not() {
    let ordinary = bundle(&["SELECT a, COUNT(*) AS n FROM t GROUP BY a"], "postgresql");
    assert!(matches!(
        physical_source_plan(&ordinary, ordinary.layers()[0].id()).zero_output(),
        WitnessDirection::Feasible(_)
    ));

    let rollup = bundle(
        &["SELECT COUNT(*) AS n FROM t GROUP BY ROLLUP(a)"],
        "postgresql",
    );
    assert!(matches!(
        physical_source_plan(&rollup, rollup.layers()[0].id()).zero_output(),
        WitnessDirection::Residual { .. }
    ));

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE t(a INTEGER)")
        .expect("empty table");
    let ordinary_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) FROM t GROUP BY a)",
            [],
            |row| row.get(0),
        )
        .expect("empty ordinary grouping");
    let rollup_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT COUNT(*) FROM t GROUP BY ROLLUP(a))",
            [],
            |row| row.get(0),
        )
        .expect("empty rollup");
    assert_eq!((ordinary_rows, rollup_rows), (0, 1));
}

#[test]
fn nested_aggregate_projections_cannot_claim_zero_from_empty_sources() {
    for query in [
        "SELECT COUNT(*) + 1 FROM t",
        "SELECT COALESCE(COUNT(*), 0) FROM t",
        "SELECT COUNT(*) + 1 FROM t WHERE a > 0",
        "SELECT COUNT(*) + 1 FROM l JOIN r ON l.k = r.k",
    ] {
        let b = bundle(&[query], "postgresql");
        let plan = physical_source_plan(&b, b.layers()[0].id());
        assert!(
            matches!(plan.zero_output(), WitnessDirection::Residual { .. }),
            "a global aggregate emits one row on empty input: {query}: {plan:?}"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE t(a INTEGER)")
        .expect("empty table");
    let value: i64 = conn
        .query_row("SELECT COUNT(*) + 1 FROM t", [], |row| row.get(0))
        .expect("global aggregate");
    assert_eq!(value, 1);
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

#[test]
fn all_empty_join_sources_prove_zero_output_through_downstream_filter() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT l.a, r.b FROM l FULL JOIN r ON l.k = r.k",
                "SELECT a FROM stage WHERE a IS NOT NULL",
            ],
            dialect,
        );
        let plan = physical_source_plan(&b, b.layers()[1].id());
        let WitnessDirection::Feasible(cases) = plan.zero_output() else {
            panic!("{dialect}: both fully empty join inputs must guarantee zero rows: {plan:?}");
        };
        assert_eq!(plan.sources(), &["l".to_string(), "r".to_string()]);
        assert_eq!(cases.len(), 1);
        let obligations = cases[0].obligations();
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                .count(),
            2
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::Rows { .. }))
                .count(),
            2
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER); CREATE TABLE r(b INTEGER, k INTEGER);
         CREATE TABLE stage AS SELECT l.a, r.b FROM l FULL JOIN r ON l.k=r.k;",
    )
    .expect("empty join sources");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM stage WHERE a IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("empty count");
    assert_eq!(count, 0);
}

#[test]
fn self_join_reuses_one_physical_empty_source_obligation() {
    let b = bundle(
        &["SELECT x.a FROM t AS x LEFT JOIN t AS y ON x.k = y.k"],
        "postgresql",
    );
    let plan = physical_source_plan(&b, b.layers()[0].id());
    assert_eq!(plan.sources(), &["t".to_string()]);
    let WitnessDirection::Feasible(cases) = plan.zero_output() else {
        panic!("self join of an empty controlled source must remain empty");
    };
    assert_eq!(
        cases[0]
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count(),
        1
    );
}

#[test]
fn exact_terminal_cardinality_is_constructive_through_shared_schema_backed_producers() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t",
                "CREATE TABLE mart AS SELECT a, b FROM stage",
                "SELECT a FROM mart",
            ],
            dialect,
        );
        let target = b.layers()[2].id();
        for rows in [0, 1, 3, 8] {
            let proof = physical_row_count_plan(&b, target, rows);
            let WitnessDirection::Feasible(cases) = proof else {
                panic!("{dialect}: {rows} must have source-backed construction: {proof:?}");
            };
            assert_eq!(cases.len(), 1);
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::Rows { boundary, bounds, closed_world: true, .. }
                    if boundary.relation() == "t"
                        && bounds.minimum() == rows
                        && bounds.maximum() == Some(rows)
            )));
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::OutputRows { layer_id, bounds }
                    if layer_id == target
                        && bounds.minimum() == rows
                        && bounds.maximum() == Some(rows)
            )));
        }
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL,NULL), (NULL,NULL), (1,1);
         CREATE TABLE stage AS SELECT a,b FROM t;
         CREATE TABLE mart AS SELECT a,b FROM stage;",
    )
    .expect("materialized chain");
    let output: i64 = conn
        .query_row("SELECT COUNT(*) FROM (SELECT a FROM mart)", [], |row| {
            row.get(0)
        })
        .expect("terminal rows");
    assert_eq!(output, 3);
}

#[test]
fn exact_singleton_cardinality_is_proved_without_physical_sources() {
    let b = bundle(&["SELECT 1 AS one"], "postgresql");
    let target = b.layers()[0].id();
    assert!(matches!(
        physical_row_count_plan(&b, target, 1),
        WitnessDirection::Feasible(_)
    ));
    for rows in [0, 2, 5] {
        assert!(matches!(
            physical_row_count_plan(&b, target, rows),
            WitnessDirection::Impossible
        ));
    }
}

#[test]
fn row_count_constructor_does_not_guess_after_joins_or_aggregates() {
    for query in [
        "SELECT l.a FROM l INNER JOIN r ON l.k = r.k",
        "SELECT COUNT(*) AS c FROM t",
    ] {
        let b = bundle(&[query], "postgresql");
        assert!(
            !matches!(
                physical_row_count_plan(&b, b.layers()[0].id(), 4),
                WitnessDirection::Feasible(_)
            ),
            "{query}"
        );
    }
}

#[test]
fn positive_filter_counts_require_closed_world_physical_qualifying_rows() {
    for &dialect in DIALECTS {
        let b = bundle(&["SELECT a, b FROM t WHERE a > 2 OR b < 0"], dialect);
        for rows in [1, 3, 8] {
            let target = b.layers()[0].id();
            let proof = physical_row_count_plan(&b, target, rows);
            let WitnessDirection::Feasible(cases) = proof else {
                panic!("{dialect}: expected proven filtered {rows}-row plan: {proof:?}");
            };
            assert_eq!(cases.len(), 1);
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::Rows {
                    boundary,
                    bounds,
                    predicate: WitnessFormula::RowTruth { .. },
                    closed_world: true,
                    ..
                } if boundary.relation() == "t"
                    && bounds.minimum() == rows
                    && bounds.maximum() == Some(rows)
            )));
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::ClosedWorld { boundary, .. }
                    if boundary.relation() == "t"
            )));
        }
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (3, NULL), (3, 3), (4, 4);
         CREATE TABLE stage AS SELECT a, b FROM t WHERE a > 2 OR b < 0;",
    )
    .expect("populate only qualifying rows");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM stage", [], |row| row.get(0))
        .expect("filtered count");
    assert_eq!(count, 3);
    conn.execute_batch("INSERT INTO t VALUES (1, 9), (NULL, 2);")
        .expect("add deliberately rejected rows");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a FROM t WHERE a > 2 OR b < 0)",
            [],
            |row| row.get(0),
        )
        .expect("rejected rows do not survive");
    assert_eq!(count, 3);
}

#[test]
fn nested_null_filters_prove_positive_counts_on_shared_source_rows() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t WHERE a IS NOT NULL OR b IS NOT NULL",
                "SELECT a FROM stage WHERE a IS NULL OR b IS NOT NULL",
            ],
            dialect,
        );
        let proof = physical_row_count_plan(&b, b.layers()[1].id(), 3);
        assert!(
            matches!(proof, WitnessDirection::Feasible(_)),
            "{dialect}: {proof:?}"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL, 1), (2, 2), (NULL, 3);
         CREATE TABLE stage AS SELECT a, b FROM t WHERE a IS NOT NULL OR b IS NOT NULL;",
    )
    .expect("populate matching physical rows");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a FROM stage WHERE a IS NULL OR b IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .expect("nested filter count");
    assert_eq!(count, 3);
}

#[test]
fn joint_counts_keep_filter_truth_and_fail_closed_on_distinct_conditions() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a IS NOT NULL OR b IS NOT NULL",
                "SELECT b FROM r WHERE a IS NULL OR b IS NULL",
                "SELECT a FROM t WHERE a IS NULL OR b IS NULL",
            ],
            dialect,
        );
        let raw = b.layers()[0].id();
        let positive = b.layers()[1].id();
        let independent = b.layers()[2].id();
        let incompatible = b.layers()[3].id();
        let proof = physical_joint_row_count_plan(&b, &[(raw, 3), (positive, 3), (independent, 2)]);
        let WitnessDirection::Feasible(cases) = proof else {
            panic!("{dialect}: independently constructible filters: {proof:?}");
        };
        assert_eq!(cases.len(), 1);
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|obligation| matches!(
                    obligation,
                    WitnessObligation::Rows {
                        predicate: WitnessFormula::RowTruth { .. },
                        ..
                    }
                ))
                .count(),
            2,
            "{dialect}: do not discard physical row truth while merging sources"
        );
        assert!(matches!(
            physical_joint_row_count_plan(&b, &[(positive, 3), (incompatible, 3)]),
            WitnessDirection::Residual { .. }
        ));
        assert!(matches!(
            physical_joint_row_count_plan(&b, &[(raw, 4), (positive, 3)]),
            WitnessDirection::Residual { .. }
        ));
    }
}

#[test]
fn simple_integer_comparison_and_null_predicates_support_complete_positive_counts() {
    for &dialect in DIALECTS {
        for query in [
            "SELECT a FROM t WHERE a > 2",
            "SELECT a FROM t WHERE a IS NULL",
            "SELECT a FROM t WHERE a IS NOT NULL",
        ] {
            let b = bundle(&[query], dialect);
            let id = b.layers()[0].id();
            assert!(
                matches!(physical_row_count_plan(&b, id, 3), WitnessDirection::Feasible(_)),
                "{dialect}: {query}"
            );
        }
    }
}

#[test]
fn positive_counts_do_not_assume_arithmetic_or_row_limits_preserve_counts() {
    for query in [
        "SELECT a FROM t WHERE a + 1 > 2 LIMIT 1",
        "SELECT a FROM t WHERE a IS NOT NULL LIMIT 1",
        "SELECT a FROM t WHERE a * 2 > 5",
    ] {
        let b = bundle(&[query], "postgresql");
        assert!(
            !matches!(
                physical_row_count_plan(&b, b.layers()[0].id(), 3),
                WitnessDirection::Feasible(_)
            ),
            "{query}: a noninvertible/limited predicate cannot prove arbitrary physical counts"
        );
    }
}

#[test]
fn joint_terminal_goals_share_physical_rows_once_and_detect_conflicting_counts() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t",
                "CREATE TABLE mart AS SELECT a, b FROM stage",
                "SELECT a FROM stage",
                "SELECT b FROM mart",
            ],
            dialect,
        );
        let outputs = [(b.layers()[2].id(), 3), (b.layers()[3].id(), 3)];
        let witness = physical_joint_row_count_plan(&b, &outputs);
        let WitnessDirection::Feasible(cases) = witness else {
            panic!("{dialect}: jointly consistent physical source goals: {witness:?}");
        };
        assert_eq!(cases.len(), 1);
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|o| matches!(
                    o, WitnessObligation::Rows { boundary, .. } if boundary.relation() == "t"
                ))
                .count(),
            1,
            "shared physical input should be constructed once",
        );
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                .count(),
            2,
        );
        assert!(matches!(
            physical_joint_row_count_plan(&b, &[(b.layers()[2].id(), 3), (b.layers()[3].id(), 4),]),
            WitnessDirection::Impossible,
        ));
    }
}

#[test]
fn independent_physical_sources_can_satisfy_joint_row_targets() {
    let b = bundle(&["SELECT a FROM t", "SELECT a FROM r"], "postgresql");
    let WitnessDirection::Feasible(cases) =
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 2), (b.layers()[1].id(), 7)])
    else {
        panic!("two independent, unconstrained physical source counts should be satisfiable");
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(
        cases[0]
            .obligations()
            .iter()
            .filter(|obligation| matches!(obligation, WitnessObligation::ClosedWorld { .. }))
            .count(),
        2,
    );
}

#[test]
fn joint_terminal_zero_from_filter_is_not_conflated_with_positive_source_rows() {
    let b = bundle(
        &["SELECT a FROM t WHERE a > 1", "SELECT a FROM t"],
        "postgresql",
    );
    assert!(matches!(
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 0), (b.layers()[1].id(), 2)]),
        WitnessDirection::Residual { .. },
    ));
}

#[test]
fn simultaneous_empty_filtered_and_joined_terminals_reuse_sources_once() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10",
                "CREATE TABLE joined AS SELECT l.a FROM l JOIN r ON l.k = r.k",
                "SELECT a FROM filtered WHERE a < 0",
                "SELECT a FROM joined WHERE a > 0",
            ],
            dialect,
        );
        let targets = [(b.layers()[2].id(), 0), (b.layers()[3].id(), 0)];
        let WitnessDirection::Feasible(cases) = physical_joint_row_count_plan(&b, &targets) else {
            panic!("{dialect}: all-empty physical leaves jointly prove both zero outputs");
        };
        assert_eq!(cases.len(), 1);
        let obligations = cases[0].obligations();
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                .count(),
            3
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                .count(),
            2
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER); CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10;
         CREATE TABLE joined AS SELECT l.a FROM l JOIN r ON l.k = r.k;",
    )
    .expect("empty source materialization");
    for sql in [
        "SELECT COUNT(*) FROM filtered WHERE a < 0",
        "SELECT COUNT(*) FROM joined WHERE a > 0",
    ] {
        let count: i64 = conn
            .query_row(sql, [], |row| row.get(0))
            .expect("terminal row count");
        assert_eq!(count, 0);
    }
}

#[test]
fn conflicting_filtered_zero_and_positive_transparent_path_is_residual_not_impossible() {
    let b = bundle(
        &[
            "CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10",
            "CREATE TABLE other AS SELECT a FROM t",
        ],
        "postgresql",
    );
    let actual =
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 0), (b.layers()[1].id(), 2)]);
    assert!(
        matches!(actual, WitnessDirection::Residual { .. }),
        "two rows at t can both fail a > 10 while satisfying other: {actual:?}"
    );
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (2);
         CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10;
         CREATE TABLE other AS SELECT a FROM t;",
    )
    .expect("nullable filtered source");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM filtered", [], |row| row.get(0))
        .expect("filtered");
    let other: i64 = conn
        .query_row("SELECT COUNT(*) FROM other", [], |row| row.get(0))
        .expect("other");
    assert_eq!((count, other), (0, 2));
}

#[test]
fn shared_transparent_zero_and_positive_targets_are_impossible() {
    let b = bundle(
        &[
            "CREATE TABLE first AS SELECT a FROM t",
            "CREATE TABLE second AS SELECT a FROM t",
        ],
        "postgresql",
    );
    assert!(matches!(
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 0), (b.layers()[1].id(), 1)]),
        WitnessDirection::Impossible
    ));
}

#[test]
fn zero_rows_prove_complete_empty_histograms_and_group_count() {
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a FROM t WHERE a IS NOT NULL",
            "SELECT a, COUNT(*) AS n FROM stage GROUP BY a HAVING COUNT(*) > 0",
        ],
        "postgresql",
    );
    let target = b.layers()[1].id().to_string();
    let histogram = OutputDistribution::new(
        "a",
        vec![OutputValueCount::new(
            sql_semantic_protocol::ConstraintValue::Integer(5),
            0,
        )],
    )
    .expect("empty frequency");
    b.set_outcome_goals(
        &[OutcomeGoal::new(&target, Some(0), Some(0), vec![histogram])
            .expect("complete empty output")],
    )
    .expect("goal");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
    let json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("JSON");
    assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "empty_sources");
    assert_eq!(
        json["outcome_goals"][0]["witness"]["relations"],
        serde_json::json!(["t"])
    );

    let mut groups_only = bundle(&["SELECT a, COUNT(*) AS n FROM t GROUP BY a"], "postgresql");
    let id = groups_only.layers()[0].id().to_string();
    groups_only
        .set_outcome_goals(&[
            OutcomeGoal::new(&id, None, Some(0), vec![]).expect("zero surviving groups")
        ])
        .expect("group goal");
    assert_eq!(
        groups_only.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE t(a INTEGER)")
        .expect("empty source");
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) FROM t GROUP BY a)",
            [],
            |row| row.get(0),
        )
        .expect("grouped query");
    assert_eq!(rows, 0);
}

#[test]
fn direct_join_group_and_set_counts_survive_materialized_copy_chains() {
    for &dialect in DIALECTS {
        for (producer, expected) in [
            (
                "CREATE TABLE stage AS SELECT l.a AS a FROM l INNER JOIN r ON l.k = r.k",
                "join_pairs",
            ),
            (
                "CREATE TABLE stage AS SELECT k, COUNT(*) AS n FROM t GROUP BY k HAVING COUNT(*) >= 2",
                "groups",
            ),
            (
                "CREATE TABLE stage AS SELECT a FROM l UNION ALL SELECT a FROM r",
                "set_tuples",
            ),
        ] {
            let mut b = bundle(
                &[producer, "CREATE TABLE mart AS SELECT a FROM stage", "SELECT a FROM mart"],
                dialect,
            );
            // A group produces k/n rather than a, so use its actual
            // preserved grouping key in the downstream projections.
            if expected == "groups" {
                b = bundle(
                    &[
                        producer,
                        "CREATE TABLE mart AS SELECT k FROM stage",
                        "SELECT k FROM mart",
                    ],
                    dialect,
                );
            }
            let id = b.layers()[2].id().to_string();
            b.set_outcome_goals(&[
                OutcomeGoal::new(&id, Some(2), None, vec![]).expect("goal")
            ])
            .expect("attach");
            assert_eq!(
                b.outcome_goals()[0].status(),
                OutcomeGoalStatus::Feasible,
                "{dialect}: {producer}: {:?}",
                b.outcome_goals()
            );
            let kind = match b.outcome_goals()[0].witness() {
                Some(OutcomeWitness::JoinPairs { .. }) => "join_pairs",
                Some(OutcomeWitness::Groups { .. }) => "groups",
                Some(OutcomeWitness::SetTuples { .. }) => "set_tuples",
                other => panic!("{dialect}: unexpected witness {other:?}"),
            };
            assert_eq!(kind, expected);
        }
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         INSERT INTO l VALUES (10,1),(20,2);
         INSERT INTO r VALUES (30,1),(40,2);
         CREATE TABLE stage AS SELECT l.a AS a FROM l INNER JOIN r ON l.k = r.k;
         CREATE TABLE mart AS SELECT a FROM stage;",
    )
    .expect("joined stage");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM (SELECT a FROM mart)", [], |row| {
            row.get(0)
        })
        .expect("joined output");
    assert_eq!(count, 2);
}

#[test]
fn downstream_filters_do_not_inherit_unqualified_join_constructions() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT l.a AS a FROM l INNER JOIN r ON l.k = r.k",
            "SELECT a FROM stage WHERE a > 100 OR a IS NULL",
        ],
        "postgresql",
    );
    let mut b = b;
    let id = b.layers()[1].id().to_string();
    b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(2), None, vec![]).expect("goal")])
        .expect("attach");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
}

#[test]
fn transitive_value_histograms_preserve_renamed_source_columns() {
    for &dialect in DIALECTS {
        let mut b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a AS v, b FROM t",
                "CREATE TABLE mart AS SELECT v AS final_a, b FROM stage",
                "SELECT final_a AS total FROM mart",
            ],
            dialect,
        );
        let id = b.layers()[2].id().to_string();
        let histogram = OutputDistribution::new(
            "total",
            vec![
                OutputValueCount::new(ConstraintValue::Integer(4), 2),
                OutputValueCount::new(ConstraintValue::Null, 1),
            ],
        )
        .expect("valid histogram");
        b.set_outcome_goals(
            &[OutcomeGoal::new(&id, Some(3), None, vec![histogram]).expect("goal")],
        )
        .expect("attach");
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Feasible,
            "{dialect}: {:?}",
            b.outcome_goals()
        );
        let json: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("wire");
        assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "source_rows");
        assert_eq!(json["outcome_goals"][0]["witness"]["relation"], "t");
        assert_eq!(
            json["outcome_goals"][0]["witness"]["columns"][0]["column"],
            "a"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (4, 1), (NULL, 2), (4, 3);
         CREATE TABLE stage AS SELECT a AS v, b FROM t;
         CREATE TABLE mart AS SELECT v AS final_a, b FROM stage;",
    )
    .expect("physical fixture");
    let (total, fours, nulls): (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE total = 4),
             COUNT(*) FILTER (WHERE total IS NULL)
             FROM (SELECT final_a AS total FROM mart)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("histogram oracle");
    assert_eq!((total, fours, nulls), (3, 2, 1));
}

#[test]
fn transitive_histograms_fail_closed_on_noninvertible_or_incompatible_values() {
    for &dialect in DIALECTS {
        let queries = [
            "CREATE TABLE stage AS SELECT a + 1 AS v FROM t",
            "SELECT v AS total FROM stage",
        ];
        let mut b = bundle(&queries, dialect);
        let id = b.layers()[1].id().to_string();
        let histogram = OutputDistribution::new(
            "total",
            vec![OutputValueCount::new(ConstraintValue::Integer(4), 3)],
        )
        .expect("histogram");
        b.set_outcome_goals(
            &[OutcomeGoal::new(&id, Some(3), None, vec![histogram]).expect("goal")],
        )
        .expect("attach");
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Residual,
            "{dialect}: computed source values cannot be inverted"
        );
    }
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a AS v FROM t",
            "SELECT v AS total FROM stage",
        ],
        "postgresql",
    );
    let id = b.layers()[1].id().to_string();
    let invalid_type = OutputDistribution::new(
        "total",
        vec![OutputValueCount::new(
            ConstraintValue::Integer(3_000_000_000),
            1,
        )],
    )
    .expect("histogram");
    b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(1), None, vec![invalid_type]).expect("goal")])
        .expect("attach");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
}

#[test]
fn outcome_goal_adapter_emits_derived_physical_source_count_proofs() {
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "CREATE TABLE mart AS SELECT a FROM stage",
        ],
        "postgresql",
    );
    let terminal_id = b.layers()[1].id().to_string();
    b.set_outcome_goals(&[
        OutcomeGoal::new(&terminal_id, Some(4), None, vec![]).expect("valid terminal row count")
    ])
    .expect("goal attachment");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
    let json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("outcomes JSON");
    assert_eq!(json["outcome_goals"][0]["assessment"]["status"], "feasible");
    assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "source_rows");
    assert_eq!(json["outcome_goals"][0]["witness"]["relation"], "t");
    assert_eq!(json["outcome_goals"][0]["witness"]["rows"], 4);

    let mut filtered = bundle(&["SELECT a FROM t WHERE a > 2"], "postgresql");
    let filtered_id = filtered.layers()[0].id().to_string();
    filtered
        .set_outcome_goals(&[
            OutcomeGoal::new(&filtered_id, Some(0), None, vec![]).expect("zero count goal")
        ])
        .expect("goal attachment");
    assert_eq!(
        filtered.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible,
    );
    let data: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&filtered))
            .expect("filtered JSON");
    assert_eq!(data["outcome_goals"][0]["witness"]["kind"], "empty_sources");
    assert_eq!(
        data["outcome_goals"][0]["witness"]["relations"],
        serde_json::json!(["t"])
    );

    // The legacy SourceRows witness cannot express positive RowTruth
    // obligations. A typed physical count must not become unfiltered data.
    let mut positive = bundle(&["SELECT a, b FROM t WHERE a > 2 OR b < 0"], "postgresql");
    let positive_id = positive.layers()[0].id().to_string();
    assert!(matches!(
        physical_row_count_plan(&positive, &positive_id, 3),
        WitnessDirection::Feasible(_)
    ));
    positive
        .set_outcome_goals(&[
            OutcomeGoal::new(&positive_id, Some(3), None, vec![]).expect("positive filter goal")
        ])
        .expect("goal attachment");
    assert_eq!(
        positive.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );
}
