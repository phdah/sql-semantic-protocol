//! graph regression tests.

use super::*;

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
        serde_json::from_str(include_str!("../../schema/protocol.schema.json"))
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
fn row_count_constructor_proves_attested_join_but_not_opaque_aggregate() {
    let joined = bundle(
        &["SELECT l.a FROM l INNER JOIN r ON l.k = r.k"],
        "postgresql",
    );
    let proof = physical_row_count_plan(&joined, joined.layers()[0].id(), 4);
    assert!(
        matches!(proof, WitnessDirection::Feasible(_)),
        "explicit complete typed join inputs now admit constructive cardinality"
    );

    let grouped = bundle(&["SELECT COUNT(*) AS c FROM t"], "postgresql");
    assert!(
        !matches!(
            physical_row_count_plan(&grouped, grouped.layers()[0].id(), 4),
            WitnessDirection::Feasible(_)
        ),
        "join completion cannot turn unproven aggregate shape feasible"
    );
}

#[test]
fn unconditional_delete_realizes_complete_before_after_physical_state() {
    for dialect in ["generic", "postgresql", "snowflake", "duckdb"] {
        let b = bundle(&["DELETE FROM t"], dialect);
        let id = b.layers()[0].id();
        for before in [0, 1, 3, 10] {
            let proof = physical_unconditional_delete_plan(&b, id, before);
            let WitnessDirection::Feasible(cases) = proof else {
                panic!("{dialect}: before {before} should be deletable: {proof:?}");
            };
            assert_eq!(cases.len(), 1);
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::StateRows { relation, before: initial, after }
                    if relation == "t"
                        && initial.minimum() == before
                        && initial.maximum() == Some(before)
                        && after.minimum() == 0
                        && after.maximum() == Some(0)
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
        "CREATE TABLE t(a INTEGER,b INTEGER,k INTEGER);
         INSERT INTO t VALUES (1,1,1),(NULL,NULL,NULL),(1,1,1);",
    )
    .expect("controlled initial target");
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("before");
    conn.execute_batch("DELETE FROM t").expect("delete");
    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("after");
    assert_eq!((before, after), (3, 0));
}

#[test]
fn state_realization_does_not_assume_filtered_mutations_or_prior_target_producers() {
    for sql in [
        "DELETE FROM t WHERE a IS NULL",
        "UPDATE t SET a = 3 WHERE b > 1",
        "INSERT INTO t (a,b,k) SELECT a,b,k FROM l",
    ] {
        let b = bundle(&[sql], "postgresql");
        let proof = physical_unconditional_delete_plan(&b, b.layers()[0].id(), 2);
        assert!(
            matches!(proof, WitnessDirection::Residual { .. }),
            "{sql}: partial mutation needs exact predicate and initial-state proof"
        );
    }
    let b = bundle(
        &["CREATE TABLE t AS SELECT a,b,k FROM l", "DELETE FROM t"],
        "postgresql",
    );
    assert!(matches!(
        physical_unconditional_delete_plan(&b, b.layers()[1].id(), 2),
        WitnessDirection::Residual { .. }
    ));
}
