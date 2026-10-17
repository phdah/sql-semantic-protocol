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
                ["k"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(column, "INTEGER", "postgresql")
                            .expect("typed integer")
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
        "CREATE TABLE stage AS SELECT k FROM l",
        "CREATE TABLE mart AS SELECT k FROM l",
        "SELECT a.k FROM stage a JOIN r b ON a.k=b.k JOIN mart c ON b.k=c.k",
    ];
    for &dialect in DIALECTS {
        let b = bundle(&queries, dialect);
        eprintln!("TASK110 graph: {:?}", b.inputs()[2].statements());
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
