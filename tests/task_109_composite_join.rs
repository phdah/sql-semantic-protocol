//! TASK-109: complete INNER join bags and two-key physical witness contracts.
//! Independent DuckDB input/output bags are executable oracles, not parser-only checks.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, JoinPopulationPattern, RelationCatalog, RelationSchema, SchemaColumn,
    SqlInput, WitnessDirection, WitnessObligation,
};

fn bundle(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["k", "a"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(column, "INTEGER", "postgresql")
                            .expect("typed INTEGER")
                    })
                    .collect(),
            )
            .expect("typed source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let source = SqlInput::inline(sql);
    let selected = dialect_from_name(dialect).expect("recognized dialect");
    let input = ConfiguredSqlInput::new("task-109", &source, dialect, selected.as_ref());
    analyze_configured_inputs_with_catalog(&[input], &catalog).expect("SQL analysis")
}

#[test]
fn composite_equality_controls_both_physical_keys_across_all_shared_dialects() {
    for &dialect in DIALECTS {
        let b = bundle(
            "SELECT l.k, l.a FROM l INNER JOIN r ON l.k = r.k AND l.a = r.a",
            dialect,
        );
        let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: compound INNER equality must construct a complete bag: {plan:?}");
        };
        assert_eq!(plan.sources(), &["l".to_string(), "r".to_string()]);
        assert!(
            cases.iter().any(|case| {
                case.obligations()
                    .iter()
                    .any(|obligation| match obligation {
                        WitnessObligation::JoinPopulation {
                            left_key,
                            right_key,
                            additional_keys,
                            join_kind: sql_semantic_protocol::JoinKind::Inner,
                            pattern: JoinPopulationPattern::CommonMatched,
                            left_rows: 2,
                            right_rows: 2,
                            output_rows: 4,
                            closed_world: true,
                            ..
                        } => {
                            left_key.name() == "k"
                                && right_key.name() == "k"
                                && additional_keys.len() == 1
                                && additional_keys[0].left().name() == "a"
                                && additional_keys[0].right().name() == "a"
                                && case
                                    .obligations()
                                    .iter()
                                    .filter(|other| {
                                        matches!(other, WitnessObligation::ClosedWorld { .. })
                                    })
                                    .count()
                                    == 2
                        }
                        _ => false,
                    })
            }),
            "{dialect}: proof must assign both equalities on the same closed source bags"
        );
    }
}

#[test]
fn duplicated_single_key_inner_join_has_exact_bag_multiplicity() {
    for &dialect in DIALECTS {
        let b = bundle("SELECT l.k FROM l JOIN r ON l.k = r.k", dialect);
        let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 6)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: duplicate physical join bag: {plan:?}");
        };
        assert!(
            cases.iter().any(|case| case.obligations().iter().any(|o| {
                matches!(o, WitnessObligation::JoinPopulation {
                pattern: JoinPopulationPattern::CommonMatched,
                left_rows: 2,
                right_rows: 3,
                output_rows: 6,
                additional_keys,
                closed_world: true,
                ..
            } if additional_keys.is_empty())
            })),
            "{dialect}: must preserve 2x3 duplicate matches"
        );
    }
}

fn oracle_pairs(db: &Connection, on: &str, rejected: bool) -> (i64, String) {
    let selection = if rejected {
        format!("FROM l CROSS JOIN r WHERE ({on}) IS NOT TRUE")
    } else {
        format!("FROM l JOIN r ON {on}")
    };
    db.query_row(
        &format!("SELECT COUNT(*), COALESCE(string_agg(l.rid || ':' || r.rid, ',' ORDER BY l.rid, r.rid), '') {selection}"),
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).expect("independent DuckDB complete source/terminal oracle")
}

#[test]
fn duckdb_oracle_checks_every_positive_and_not_true_pair_including_nulls() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(rid VARCHAR, k INTEGER, a INTEGER);
         CREATE TABLE r(rid VARCHAR, k INTEGER, a INTEGER);
         INSERT INTO l VALUES ('l1',1,10),('l2',1,20),('l3',NULL,30);
         INSERT INTO r VALUES ('r1',1,10),('r2',1,50),('r3',NULL,70);",
    )
    .expect("complete source input bags");
    assert_eq!(
        oracle_pairs(&db, "l.k = r.k", false),
        (4, "l1:r1,l1:r2,l2:r1,l2:r2".into())
    );
    assert_eq!(
        oracle_pairs(&db, "l.k = r.k", true),
        (5, "l1:r3,l2:r3,l3:r1,l3:r2,l3:r3".into())
    );
    assert_eq!(
        oracle_pairs(&db, "l.k = r.k AND l.a = r.a", false),
        (1, "l1:r1".into())
    );
    assert_eq!(
        oracle_pairs(&db, "l.k = r.k AND l.a = r.a", true),
        (8, "l1:r2,l1:r3,l2:r1,l2:r2,l2:r3,l3:r1,l3:r2,l3:r3".into())
    );
    let null_matching: i64 = db
        .query_row(
            "SELECT count(*) FROM l JOIN r ON l.k = r.k WHERE l.k IS NULL",
            [],
            |row| row.get(0),
        )
        .expect("NULL is not an ordinary equality match");
    assert_eq!(null_matching, 0);
    let expected_integer_domain: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = db
        .query_row(
            "SELECT MIN(l.k), MAX(l.k), MIN(l.a), MAX(l.a)
             FROM l JOIN r ON l.k = r.k AND l.a = r.a",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("exact positive output domain extrema");
    assert_eq!(
        expected_integer_domain,
        (Some(1), Some(1), Some(10), Some(10))
    );
}

#[test]
fn duckdb_oracle_checks_one_to_many_closed_world_bag() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(rid VARCHAR, k INTEGER);
         CREATE TABLE r(rid VARCHAR, k INTEGER);
         INSERT INTO l VALUES ('l1',1),('l2',1);
         INSERT INTO r VALUES ('r1',1),('r2',1),('r3',1);",
    )
    .expect("duplicate physical sources");
    assert_eq!(
        oracle_pairs(&db, "l.k = r.k", false),
        (6, "l1:r1,l1:r2,l1:r3,l2:r1,l2:r2,l2:r3".into())
    );
    assert_eq!(oracle_pairs(&db, "l.k = r.k", true), (0, String::new()));
}

#[test]
fn unsupported_computed_join_key_is_not_promoted_to_exact() {
    let b = bundle("SELECT l.k FROM l JOIN r ON l.k + 1 = r.k", "postgresql");
    let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4)]);
    assert!(matches!(plan.outcome(), WitnessDirection::Residual { .. }));
}

#[test]
fn composite_keys_remain_identical_under_direct_dbt_and_odcs_schema_evidence() {
    use sql_semantic_protocol::SchemaSourceKind;

    let sql = "SELECT l.k, l.a FROM l INNER JOIN r ON l.k = r.k AND l.a = r.a";
    let mut baseline = None;
    for kind in [
        SchemaSourceKind::DbtCatalog,
        SchemaSourceKind::DbtManifest,
        SchemaSourceKind::ExternalMetadata,
    ] {
        let schemas = ["l", "r"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["k", "a"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "postgresql")
                                .expect("INTEGER key evidence")
                        })
                        .collect(),
                )
                .expect("schema")
                .with_source_kind(kind)
            })
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
        let source = SqlInput::inline(sql);
        let dialect = dialect_from_name("postgresql").expect("PostgreSQL");
        let configured =
            ConfiguredSqlInput::new("task-109-adapters", &source, "postgresql", dialect.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog)
            .expect("source-independent analysis");
        let proof = physical_joint_source_plan(&bundle, &[(bundle.layers()[0].id(), 4)]);
        assert!(
            matches!(proof.outcome(), WitnessDirection::Feasible(_)),
            "{kind:?}: full typed composite join proof required"
        );
        if let Some(ref expected) = baseline {
            assert_eq!(&proof, expected, "{kind:?}: metadata provenance parity");
        } else {
            baseline = Some(proof);
        }
    }
}

#[test]
fn emitted_composite_population_contains_the_full_key_tuple() {
    use sql_semantic_protocol::{to_bundle_json, OutcomeGoal};

    let mut b = bundle(
        "SELECT l.k, l.a FROM l JOIN r ON l.k = r.k AND l.a = r.a",
        "postgresql",
    );
    let goal = OutcomeGoal::new(b.layers()[0].id(), Some(4), None, vec![])
        .expect("bounded composite output goal");
    b.set_outcome_goals(&[goal]).expect("goal analysis");
    let emitted: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&b)).expect("canonical bundle JSON");
    let cases = emitted["graph"]["physical_joint_count_plan"]["outcome"]["cases"]
        .as_array()
        .expect("canonical cases");
    assert!(cases.iter().any(|case| {
        case["obligations"]
            .as_array()
            .expect("canonical obligations")
            .iter()
            .any(|obligation| {
                obligation["kind"] == "join_population"
                    && obligation["join_kind"] == "inner"
                    && obligation["additional_keys"][0]["left"]["name"] == "a"
                    && obligation["additional_keys"][0]["right"]["name"] == "a"
                    && obligation["closed_world"] == true
            })
    }));
}
