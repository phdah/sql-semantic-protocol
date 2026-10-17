//! TASK-110 public seams: a single physical assignment for linked joins and aliases.
mod common;
use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

fn bundle(sql: &[&str], dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r", "t", "stage", "mart"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "k"]
                    .into_iter()
                    .map(|column| {
                        let sql_type = if column == "id" { "VARCHAR" } else { "INTEGER" };
                        SchemaColumn::from_sql_type(column, sql_type, "postgresql")
                            .expect("typed source column")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let inputs = sql
        .iter()
        .enumerate()
        .map(|(i, statement)| {
            let selected = dialect_from_name(dialect).expect("dialect");
            let source = SqlInput::inline(*statement);
            // Keep the SQL input alive until analysis.
            (i, source, selected)
        })
        .collect::<Vec<_>>();
    let ids = (0..sql.len())
        .map(|i| format!("task-110-{i}"))
        .collect::<Vec<_>>();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|((_, source, dialect_impl), id)| {
            ConfiguredSqlInput::new(id, source, dialect, dialect_impl.as_ref())
        })
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analysis")
}

#[test]
fn three_way_inner_join_has_one_closed_population_per_physical_leaf() {
    let sql = "SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k";
    for &dialect in DIALECTS {
        let b = bundle(&[sql], dialect);
        let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4)]);
        assert_eq!(plan.sources(), &["l", "r", "t"].map(str::to_string));
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: expected complete chain proof, got {plan:?}");
        };
        assert!(
            cases.iter().any(|case| {
                let chain = case.obligations().iter().any(|o| {
                    matches!(
                        o,
                        WitnessObligation::JoinChainPopulation {
                            left_rows: 2,
                            middle_rows: 2,
                            right_rows: 1,
                            output_rows: 4,
                            closed_world: true,
                            ..
                        }
                    )
                });
                chain
                    && case
                        .obligations()
                        .iter()
                        .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                        .count()
                        == 3
            }),
            "{dialect}: must prove complete 2 x 2 x 1 bag"
        );
    }
}

#[test]
fn repeated_physical_source_is_assigned_once_for_both_aliases() {
    let queries = [
        "CREATE TABLE stage AS SELECT id,k FROM l",
        "CREATE TABLE mart AS SELECT id,k FROM l",
        "SELECT a.k FROM stage a JOIN r b ON a.k=b.k JOIN mart c ON b.k=c.k",
    ];
    for &dialect in DIALECTS {
        let b = bundle(&queries, dialect);
        let plan = physical_joint_source_plan(&b, &[(b.layers()[2].id(), 4)]);
        assert_eq!(plan.sources(), &["l", "r"].map(str::to_string));
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: shared-source chain should construct: {plan:?}");
        };
        assert!(
            cases
                .iter()
                .any(|case| case.obligations().iter().any(|o| matches!(
                    o,
                    WitnessObligation::JoinChainPopulation {
                        left_rows: 2,
                        middle_rows: 1,
                        right_rows: 2,
                        output_rows: 4,
                        closed_world: true,
                        ..
                    }
                ))),
            "{dialect}: aliases must share one physical two-row bag"
        );
        assert!(cases.iter().all(|case| case
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 2));
    }
}

#[test]
fn two_aliases_of_one_table_are_not_two_independent_physical_bags() {
    for &dialect in DIALECTS {
        let b = bundle(&["SELECT a.k FROM l a JOIN l b ON a.k=b.k"], dialect);
        let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4)]);
        assert_eq!(plan.sources(), &["l".to_string()]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: self-join must use same physical bag: {plan:?}");
        };
        assert!(cases.iter().any(|case| case.obligations().iter().any(|o| {
            matches!(
                o,
                WitnessObligation::JoinPopulation {
                    left_rows: 2,
                    right_rows: 2,
                    output_rows: 4,
                    closed_world: true,
                    ..
                }
            )
        })));
    }
}

#[test]
fn duckdb_proves_complete_join_and_not_true_source_identity_bags() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, k INTEGER);
        CREATE TABLE r(id VARCHAR, k INTEGER);
        CREATE TABLE t(id VARCHAR, k INTEGER);
        INSERT INTO l VALUES ('l1',1),('l2',1);
        INSERT INTO r VALUES ('r1',1),('r2',1);
        INSERT INTO t VALUES ('t1',1);",
    )
    .expect("positive physical bags");
    let terminal: String = db
        .query_row(
            "SELECT string_agg(l.id || ':' || r.id || ':' || t.id, ',' ORDER BY l.id,r.id,t.id)
         FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k",
            [],
            |row| row.get(0),
        )
        .expect("complete terminal rows");
    assert_eq!(terminal, "l1:r1:t1,l1:r2:t1,l2:r1:t1,l2:r2:t1");
    let self_rows: String = db
        .query_row(
            "SELECT string_agg(a.id || ':' || b.id, ',' ORDER BY a.id,b.id)
         FROM l a JOIN l b ON a.k=b.k",
            [],
            |row| row.get(0),
        )
        .expect("self join identities");
    assert_eq!(self_rows, "l1:l1,l1:l2,l2:l1,l2:l2");
    db.execute_batch(
        "DELETE FROM l; DELETE FROM r; DELETE FROM t;
        INSERT INTO l VALUES ('l1',1),('l2',NULL);
        INSERT INTO r VALUES ('r1',1),('r2',2);
        INSERT INTO t VALUES ('t1',2);",
    )
    .expect("NULL and rejecting input");
    let rejected: String = db
        .query_row(
            "SELECT string_agg(l.id || ':' || r.id || ':' || t.id, ',' ORDER BY l.id,r.id,t.id)
         FROM l CROSS JOIN r CROSS JOIN t
         WHERE ((l.k=r.k) AND (r.k=t.k)) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .expect("complete NOT TRUE bag");
    assert_eq!(rejected, "l1:r1:t1,l1:r2:t1,l2:r1:t1,l2:r2:t1");
    let output_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k",
            [],
            |row| row.get(0),
        )
        .expect("empty result");
    assert_eq!(output_count, 0);
}

#[test]
fn frozen_join_chain_matrix_has_stable_closed_world_cases_and_parser_coverage() {
    let frozen: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-110-acceptance-freeze.json"))
            .expect("versioned TASK-110 inventory");
    let cases = frozen["cases"].as_array().expect("mandatory cases");
    assert_eq!(cases.len(), 4);
    for case in cases {
        let id = case["id"].as_str().expect("stable fixture identity");
        assert!(id.starts_with("71-C110-"), "{id}: TASK-110 namespace");
        assert_eq!(case["classification"], "exact_required");
        assert_eq!(case["source_column_type"], "INTEGER");
        assert_eq!(
            case["join_comparison"],
            "ordinary_SQL_equality_null_is_not_true"
        );
        let matched = case["matches"].as_array().expect("row identity bag");
        assert_eq!(
            matched.len() as u64,
            case["expected_output_count"].as_u64().expect("count"),
            "{id}: complete matched identity bag"
        );
        for &dialect in DIALECTS {
            let normalized = if dialect == "postgres" {
                "postgresql"
            } else {
                dialect
            };
            assert!(
                case["dialects"][normalized].is_object(),
                "{id}/{dialect}: missing mandatory parser matrix cell"
            );
            let statements = case["statements"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.as_str().expect("SQL"))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![case["sql"].as_str().expect("SQL")]);
            let analyzed = bundle(&statements, dialect);
            assert_eq!(analyzed.layers().len(), statements.len(), "{id}/{dialect}");
        }
    }
}

#[test]
fn emitted_chain_obligation_has_all_instances_and_exact_source_bag_counts() {
    use sql_semantic_protocol::{to_bundle_json, OutcomeGoal};

    let mut b = bundle(
        &["SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k"],
        "postgresql",
    );
    let goal = OutcomeGoal::new(b.layers()[0].id(), Some(4), None, vec![])
        .expect("bounded three-input goal");
    b.set_outcome_goals(&[goal]).expect("goal analysis");
    let wire: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&b)).expect("canonical bundle JSON");
    let cases = wire["graph"]["physical_joint_count_plan"]["outcome"]["cases"]
        .as_array()
        .expect("canonical cases");
    assert!(
        cases.iter().any(|case| {
            case["obligations"]
                .as_array()
                .expect("typed obligations")
                .iter()
                .any(|obligation| {
                    obligation["kind"] == "join_chain_population"
                        && obligation["left_row"]["relation"] == "l"
                        && obligation["middle_row"]["relation"] == "r"
                        && obligation["right_row"]["relation"] == "t"
                        && obligation["left_key"]["name"] == "k"
                        && obligation["middle_left_key"]["name"] == "k"
                        && obligation["middle_right_key"]["name"] == "k"
                        && obligation["right_key"]["name"] == "k"
                        && obligation["matched_value"] == 1
                        && obligation["left_rows"] == 2
                        && obligation["middle_rows"] == 2
                        && obligation["right_rows"] == 1
                        && obligation["output_rows"] == 4
                        && obligation["closed_world"] == true
                })
        }),
        "the public wire contract must serialize one joint key assignment"
    );
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json"))
            .expect("protocol JSON schema");
    assert!(schema["$defs"]["constructiveObligation"]["oneOf"]
        .as_array()
        .expect("known variants")
        .iter()
        .any(|variant| variant["properties"]["kind"]["const"] == "join_chain_population"));
}

#[test]
fn physical_chain_is_invariant_to_attested_schema_metadata_provenance() {
    use sql_semantic_protocol::SchemaSourceKind;

    let sql = "SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k";
    let mut baseline = None;
    for kind in [
        SchemaSourceKind::DbtCatalog,
        SchemaSourceKind::DbtManifest,
        SchemaSourceKind::ExternalMetadata,
    ] {
        let schemas = ["l", "r", "t"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                        .expect("attested key")],
                )
                .expect("source schema")
                .with_source_kind(kind)
            })
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
        let source = SqlInput::inline(sql);
        let dialect = dialect_from_name("postgresql").expect("dialect");
        let input =
            ConfiguredSqlInput::new("task-110-metadata", &source, "postgresql", dialect.as_ref());
        let b = analyze_configured_inputs_with_catalog(&[input], &catalog)
            .expect("metadata-equivalent analysis");
        let proof = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4)]);
        assert!(matches!(proof.outcome(), WitnessDirection::Feasible(_)));
        if let Some(expected) = &baseline {
            assert_eq!(
                &proof, expected,
                "{kind:?}: one provenance-independent proof"
            );
        } else {
            baseline = Some(proof);
        }
    }
}

#[test]
fn duckdb_shared_producers_reuse_one_complete_original_source_bag() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, k INTEGER);
         CREATE TABLE r(id VARCHAR, k INTEGER);
         INSERT INTO l VALUES ('l1',1),('l2',1);
         INSERT INTO r VALUES ('r1',1);
         CREATE TABLE stage AS SELECT id,k FROM l;
         CREATE TABLE mart AS SELECT id,k FROM l;",
    )
    .expect("complete source and materialized producer population");
    let identities: String = db
        .query_row(
            "SELECT string_agg(a.id || ':' || b.id || ':' || c.id, ',' ORDER BY a.id,b.id,c.id)
             FROM stage a JOIN r b ON a.k=b.k JOIN mart c ON b.k=c.k",
            [],
            |row| row.get(0),
        )
        .expect("independent shared materialization oracle");
    assert_eq!(identities, "l1:r1:l1,l1:r1:l2,l2:r1:l1,l2:r1:l2");
    let extent: (Option<i64>, Option<i64>, i64) = db
        .query_row(
            "SELECT MIN(a.k),MAX(c.k),COUNT(*) FROM stage a
             JOIN r b ON a.k=b.k JOIN mart c ON b.k=c.k",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("actual positive domain and count");
    assert_eq!(extent, (Some(1), Some(1), 4));
}
