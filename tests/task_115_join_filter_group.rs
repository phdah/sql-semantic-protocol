//! TASK-115: public closed-world proofs across WHERE, chained JOIN and GROUP.
mod common;
use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

fn bundle(sql: &[&str], dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r", "t", "stage"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "k", "k2", "a"]
                    .into_iter()
                    .map(|name| {
                        SchemaColumn::from_sql_type(
                            name,
                            if name == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("typed column")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let sources = sql
        .iter()
        .map(|statement| SqlInput::inline(*statement))
        .collect::<Vec<_>>();
    let selected = dialect_from_name(dialect).expect("dialect");
    let ids = (0..sql.len())
        .map(|index| format!("task-115-{index}"))
        .collect::<Vec<_>>();
    let configured = sources
        .iter()
        .zip(&ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, dialect, selected.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analysis")
}

#[test]
fn typed_nonkey_where_filter_is_on_the_same_three_way_physical_bags() {
    let sql = "SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k WHERE l.a > 0";
    for &dialect in DIALECTS {
        let analyzed = bundle(&[sql], dialect);
        let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[0].id(), 4)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: expected finite complete join construction: {plan:?}");
        };
        assert!(
            cases.iter().any(|case| {
                let obligations = case.obligations();
                obligations.iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinChainPopulation {
                            left_rows: 2,
                            middle_rows: 2,
                            right_rows: 1,
                            output_rows: 4,
                            closed_world: true,
                            ..
                        }
                    )
                }) && obligations
                    .iter()
                    .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                    .count()
                    == 3
                    && obligations.iter().any(|o| match o {
                        WitnessObligation::Rows {
                            boundary,
                            predicate,
                            ..
                        } if boundary.relation() == "l" => {
                            !format!("{predicate:?}").contains("All([])")
                        }
                        _ => false,
                    })
            }),
            "{dialect}: upstream TRUE condition must constrain original l source"
        );
    }
}

#[test]
fn composite_on_edges_share_one_complete_filtered_three_source_population() {
    let sql = "SELECT l.k FROM l JOIN r ON l.k=r.k AND l.k2=r.k2 JOIN t ON r.k=t.k AND r.k2=t.k2 WHERE l.a>0";
    for &dialect in DIALECTS {
        let analyzed = bundle(&[sql], dialect);
        let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[0].id(), 4)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: independently typed composite chain must construct: {plan:?}");
        };
        assert!(
            cases.iter().any(|case| {
                case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinChainPopulation {
                            left_rows: 2,
                            middle_rows: 2,
                            right_rows: 1,
                            output_rows: 4,
                            closed_world: true,
                            ..
                        }
                    )
                }) && case
                    .obligations()
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count()
                    == 3
                    && ["l", "r", "t"].iter().all(|source| {
                        case.obligations().iter().any(|obligation| {
                            matches!(
                                obligation,
                                WitnessObligation::Rows { boundary, predicate, .. }
                                    if boundary.relation() == *source
                                        && format!("{predicate:?}").contains("k2")
                            )
                        })
                    })
            }),
            "{dialect}: every composite equality must constrain the original source bag"
        );
    }
}

#[test]
fn composite_join_key_restriction_cannot_bypass_shared_key_reconciliation() {
    let sql = "SELECT l.k FROM l JOIN r ON l.k=r.k AND l.k2=r.k2 JOIN t ON r.k=t.k AND r.k2=t.k2 WHERE l.k2=2";
    let analyzed = bundle(&[sql], "postgresql");
    let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[0].id(), 4)]);
    if let WitnessDirection::Feasible(cases) = plan.outcome() {
        assert!(
            cases.iter().all(|case| {
                !case.obligations().iter().any(|obligation| {
                    matches!(obligation, WitnessObligation::JoinChainPopulation { .. })
                })
            }),
            "an independently restricted composite key cannot borrow the all-1 witness"
        );
    }
}

#[test]
fn two_hop_filtered_producer_uses_only_the_original_physical_bags() {
    let sql = [
        "CREATE TABLE stage AS SELECT id,k,k2,a FROM l WHERE a > 0",
        "SELECT stage.k FROM stage JOIN r ON stage.k=r.k JOIN t ON r.k=t.k",
    ];
    let analyzed = bundle(&sql, "postgresql");
    let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[1].id(), 4)]);
    assert_eq!(plan.sources(), &["l", "r", "t"].map(str::to_string));
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("filtered producer must use physical leaves: {plan:?}");
    };
    assert!(cases.iter().any(|case| {
        let obligations = case.obligations();
        obligations.iter().any(|obligation| matches!(
            obligation, WitnessObligation::JoinChainPopulation {
                left_rows: 2, middle_rows: 2, right_rows: 1, output_rows: 4, ..
            }
        )) && obligations.iter().any(|obligation| match obligation {
            WitnessObligation::Rows { boundary, predicate, .. } if boundary.relation() == "l" =>
                !matches!(predicate, sql_semantic_protocol::WitnessFormula::All(terms) if terms.is_empty()),
            _ => false,
        })
    }), "a filtered stage may not be seeded as a fourth physical source");
}

#[test]
fn a_where_restriction_on_the_join_key_is_not_an_independent_proof() {
    let sql = "SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k WHERE l.k = 2";
    let analyzed = bundle(&[sql], "postgresql");
    let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[0].id(), 4)]);
    if let WitnessDirection::Feasible(cases) = plan.outcome() {
        assert!(
            cases.iter().all(|case| !case
                .obligations()
                .iter()
                .any(|o| matches!(o, WitnessObligation::JoinChainPopulation { .. }))),
            "an unreconciled join-key restriction cannot claim the common-key construction"
        );
    }
}

#[test]
fn downstream_count_having_reuses_exact_chain_bag() {
    let sql = [
        "CREATE TABLE stage AS SELECT l.k AS k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k WHERE l.a > 0",
        "SELECT stage.k, COUNT(*) AS n FROM stage GROUP BY stage.k HAVING COUNT(*) > 3",
    ];
    let analyzed = bundle(&sql, "postgresql");
    let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[1].id(), 1)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("grouped terminal must lift one complete source assignment: {plan:?}");
    };
    assert!(
        cases.iter().any(|case| {
            let obligations = case.obligations();
            obligations.iter().any(|o| {
                matches!(
                    o,
                    WitnessObligation::JoinChainPopulation { output_rows: 4, .. }
                )
            }) && obligations.iter().any(|o| {
                matches!(
                    o, WitnessObligation::Group { rows, .. }
                        if rows.minimum() == 4 && rows.maximum() == Some(4)
                )
            }) && obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                .count()
                == 3
        }),
        "COUNT(*)=4 must derive from the same original three closed leaves"
    );
}

#[test]
fn all_frozen_sql_variants_parse_in_all_thirteen_shared_dialects() {
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("frozen TASK-115 matrix");
    for case in inventory["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("fixture id");
        let sql = case["sql"].as_str().expect("SQL");
        let statements = sql.split(';').collect::<Vec<_>>();
        for &dialect in DIALECTS {
            let analyzed = bundle(&statements, dialect);
            assert_eq!(
                analyzed.layers().len(),
                statements.len(),
                "{id}: {dialect} must preserve every transformation layer"
            );
        }
    }
}

#[test]
fn duckdb_independently_checks_complete_source_and_terminal_identity_bags() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, k INTEGER, k2 INTEGER, a INTEGER);
         CREATE TABLE r(id VARCHAR, k INTEGER, k2 INTEGER);
         CREATE TABLE t(id VARCHAR, k INTEGER, k2 INTEGER);
         INSERT INTO l VALUES
           ('l1',1,10,1),('l2',1,10,2),('l3',2,20,-1),('l4',NULL,10,1);
         INSERT INTO r VALUES ('r1',1,10),('r2',1,10),('r3',2,20);
         INSERT INTO t VALUES ('t1',1,10),('t2',NULL,10);",
    )
    .expect("independent complete source bags");

    let joined = "FROM l JOIN r ON l.k=r.k AND l.k2=r.k2
                  JOIN t ON r.k=t.k AND r.k2=t.k2 WHERE l.a > 0";
    let output: String = db.query_row(
        &format!("SELECT string_agg(l.id || ':' || r.id || ':' || t.id, ',' ORDER BY l.id,r.id,t.id) {joined}"),
        [],
        |row| row.get(0),
    ).expect("terminal source-row identities");
    assert_eq!(output, "l1:r1:t1,l1:r2:t1,l2:r1:t1,l2:r2:t1");

    let not_true: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l CROSS JOIN r CROSS JOIN t
         WHERE (l.a>0 AND l.k=r.k AND l.k2=r.k2
                AND r.k=t.k AND r.k2=t.k2) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .expect("NOT TRUE cross-bag complement");
    assert_eq!(not_true, 20);
    let grouped: i64 = db.query_row(
        &format!("SELECT COUNT(*) FROM (SELECT l.k, COUNT(*) AS n {joined} GROUP BY l.k HAVING COUNT(*)>3) g"),
        [], |row| row.get(0),
    ).expect("group cardinality");
    assert_eq!(grouped, 1);
    let count_value: i64 = db
        .query_row(&format!("SELECT COUNT(*) {joined}"), [], |row| row.get(0))
        .expect("complete group contribution count");
    assert_eq!(count_value, 4);
}
