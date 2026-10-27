//! TASK-146 public INSERT SELECT and CTE-backed complete state oracles.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    AnalysisBundle, CheckCompareOperator, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn analysis(sql: &str) -> AnalysisBundle {
    let schemas = ["source", "target"].map(|name| {
        RelationSchema::new(
            name,
            ["id", "score"]
                .into_iter()
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    });
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("task-146", &input, "duckdb", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn state(relation: &str, rows: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|(id, score)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), LiteralValue::Number(id.to_string())),
                    ("score".to_string(), LiteralValue::Number(score.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

type CteWitness<'a> = (&'a str, &'a [(i64, i64)], &'a str);

fn check_full_bags(
    sql: &str,
    source: &[(i64, i64)],
    prior: &[(i64, i64)],
    expected: &[(i64, i64)],
    cte_witness: Option<CteWitness<'_>>,
) {
    let bundle = analysis(sql);
    let initial = [state("source", source), state("target", prior)];
    let plan = physical_ordered_write_state_plan(&bundle, &initial);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &plan
    else {
        panic!("complete physical INSERT state must be exact: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    let transition = &transitions[0];
    if let Some((name, expected_rows, _)) = cte_witness {
        assert_eq!(transition.intermediate_states().len(), 1);
        let materialized = &transition.intermediate_states()[0];
        assert_eq!(materialized.relation(), name);
        assert_eq!(materialized.rows(), state(name, expected_rows).rows());
    } else {
        assert!(transition.intermediate_states().is_empty());
    }
    assert_eq!(transition.before().rows(), state("target", prior).rows());
    assert_eq!(
        transition.affected().inserted(),
        (expected.len() - prior.len()) as u64
    );
    let target = final_states
        .iter()
        .find(|state| state.relation() == "target")
        .unwrap();
    assert_eq!(transition.after(), target);
    assert_eq!(target.rows(), state("target", expected).rows());
    let source_state = final_states
        .iter()
        .find(|state| state.relation() == "source")
        .unwrap();
    assert_eq!(source_state.rows(), state("source", source).rows());
    assert_eq!(
        final_states.len(),
        2,
        "CTE must not become an independent physical source"
    );
    let target_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
    let domains = target.exact_domains(target_schema).unwrap();
    assert_eq!(domains.len(), 2);

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER, score INTEGER);
         CREATE TABLE target(id INTEGER, score INTEGER);",
    )
    .unwrap();
    for (id, score) in source {
        db.execute_batch(&format!("INSERT INTO source VALUES ({id}, {score})"))
            .unwrap();
    }
    for (id, score) in prior {
        db.execute_batch(&format!("INSERT INTO target VALUES ({id}, {score})"))
            .unwrap();
    }
    if let Some((name, expected_rows, stage_sql)) = cte_witness {
        let mut statement = db.prepare(stage_sql).unwrap();
        let actual = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected_rows,
            "independent complete DuckDB CTE bag: {name}"
        );
    }
    db.execute_batch(sql).unwrap();
    let mut statement = db
        .prepare("SELECT id,score FROM target ORDER BY id,score")
        .unwrap();
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap();
    let observed = rows.map(|row| row.unwrap()).collect::<Vec<_>>();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort();
    assert_eq!(
        observed, expected_sorted,
        "independent DuckDB full terminal bag"
    );
}

#[test]
fn insert_select_filters_and_reorders_into_existing_target() {
    check_full_bags(
        "INSERT INTO target(score,id) SELECT score,id FROM source WHERE id > 1",
        &[(1, 10), (2, 20), (3, 30)],
        &[(0, 0)],
        &[(0, 0), (2, 20), (3, 30)],
        None,
    );
}

#[test]
fn cte_filter_proves_the_entire_source_and_target_bags() {
    check_full_bags(
        "INSERT INTO target(score,id) WITH filtered AS
            (SELECT id,score FROM source WHERE id > 1)
            SELECT score,id FROM filtered",
        &[(1, 10), (2, 20), (3, 30)],
        &[],
        &[(2, 20), (3, 30)],
        Some((
            "filtered",
            &[(2, 20), (3, 30)],
            "WITH filtered AS (SELECT id,score FROM source WHERE id > 1)
             SELECT id,score FROM filtered ORDER BY id",
        )),
    );
}

#[test]
fn cte_outer_filter_and_duplicates_keep_original_row_multiplicity() {
    check_full_bags(
        "INSERT INTO target(id,score) WITH staged AS
            (SELECT id,score FROM source)
            SELECT id,score FROM staged WHERE id >= 2",
        &[(1, 10), (2, 20), (2, 20), (3, 30)],
        &[],
        &[(2, 20), (2, 20), (3, 30)],
        Some((
            "staged",
            &[(1, 10), (2, 20), (2, 20), (3, 30)],
            "WITH staged AS (SELECT id,score FROM source)
             SELECT id,score FROM staged ORDER BY id",
        )),
    );
}

#[test]
fn cte_never_makes_a_missing_physical_source_exact() {
    let bundle = analysis(
        "INSERT INTO target(id,score) WITH staged AS
            (SELECT id,score FROM source WHERE id > 1)
            SELECT id,score FROM staged",
    );
    assert!(matches!(
        physical_ordered_write_state_plan(&bundle, &[state("target", &[])]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}

#[test]
fn insert_select_duplicate_primary_key_is_atomically_impossible() {
    let mut bundle =
        analysis("INSERT INTO target(id,score) SELECT id,score FROM source WHERE id > 1");
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-146").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()],
    )
    .unwrap()]);
    assert!(matches!(
        physical_ordered_write_state_plan(
            &bundle,
            &[
                state("source", &[(1, 10), (2, 20)]),
                state("target", &[(2, 99)])
            ],
        ),
        PhysicalWriteStatePlan::Impossible { .. }
    ));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER,score INTEGER);
         CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);
         INSERT INTO source VALUES (1,10),(2,20);
         INSERT INTO target VALUES (2,99);",
    )
    .unwrap();
    assert!(db
        .execute_batch("INSERT INTO target SELECT * FROM source WHERE id > 1")
        .is_err());
    let prior: (i64, i64) = db
        .query_row("SELECT id,score FROM target", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(
        prior,
        (2, 99),
        "failed SQL must preserve entire prior target"
    );
}

#[test]
fn cte_insert_check_violation_preserves_prior_complete_target() {
    let sql = "INSERT INTO target(id,score) WITH staged AS
        (SELECT id,score FROM source WHERE id > 1)
        SELECT id,score FROM staged";
    let mut bundle = analysis(sql);
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-146").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::check_comparison(
            "score",
            CheckCompareOperator::Gte,
            ConstraintValue::Integer(0),
            evidence,
        )
        .unwrap()],
    )
    .unwrap()]);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[
            state("source", &[(1, 10), (2, -20), (3, 30)]),
            state("target", &[(0, 0)]),
        ],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "{plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER, score INTEGER);
         CREATE TABLE target(id INTEGER, score INTEGER CHECK(score >= 0));
         INSERT INTO source VALUES (1,10),(2,-20),(3,30);
         INSERT INTO target VALUES (0,0);",
    )
    .unwrap();
    assert!(db.execute_batch(sql).is_err());
    let existing: Vec<(i64, i64)> = {
        let mut query = db
            .prepare("SELECT id,score FROM target ORDER BY id")
            .unwrap();
        query
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect()
    };
    assert_eq!(existing, vec![(0, 0)]);
}

fn fk_bundle(sql: &str) -> AnalysisBundle {
    let mut bundle = analysis(sql);
    let evidence = || {
        vec![ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-146").unwrap(),
            ConstraintEnforcement::Enforced,
        )]
    };
    bundle.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::foreign_key(
                vec!["score".to_string()],
                "source",
                vec!["id".to_string()],
                evidence(),
            )
            .unwrap()],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "source",
            vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap()],
        )
        .unwrap(),
    ]);
    bundle
}

#[test]
fn cte_insert_enforces_referenced_primary_key_and_complete_fk_bag() {
    let sql = "INSERT INTO target(score,id) WITH staged AS
        (SELECT id,score FROM source WHERE id > 1)
        SELECT score,id FROM staged";
    let valid = fk_bundle(sql);
    let source = state("source", &[(1, 1), (2, 2), (3, 3)]);
    let target = state("target", &[(0, 1)]);
    let plan = physical_ordered_write_state_plan(&valid, &[source.clone(), target.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("attested FK and PK with full population must be exact: {plan:?}");
    };
    assert_eq!(transitions[0].affected().inserted(), 2);
    assert_eq!(transitions[0].before(), &target);
    assert_eq!(
        transitions[0].intermediate_states()[0].rows(),
        state("staged", &[(2, 2), (3, 3)]).rows()
    );
    assert_eq!(
        final_states
            .iter()
            .find(|s| s.relation() == "target")
            .unwrap()
            .rows(),
        state("target", &[(0, 1), (2, 2), (3, 3)]).rows()
    );

    let violation = physical_ordered_write_state_plan(
        &valid,
        &[
            state("source", &[(1, 1), (2, 42), (3, 3)]),
            state("target", &[]),
        ],
    );
    assert!(
        matches!(violation, PhysicalWriteStatePlan::Residual { .. }),
        "{violation:?}"
    );
    let missing_ref = physical_ordered_write_state_plan(&valid, &[state("target", &[])]);
    assert!(
        matches!(missing_ref, PhysicalWriteStatePlan::Residual { .. }),
        "{missing_ref:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER PRIMARY KEY,score INTEGER);
         CREATE TABLE target(id INTEGER,score INTEGER REFERENCES source(id));
         INSERT INTO source VALUES (1,1),(2,2),(3,3);
         INSERT INTO target VALUES (0,1);",
    )
    .unwrap();
    db.execute_batch(sql).unwrap();
    let mut q = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let matched: Vec<(i64, i64)> = q
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(matched, vec![(0, 1), (2, 2), (3, 3)]);
    drop(q);

    let bad = Connection::open_in_memory().unwrap();
    bad.execute_batch(
        "CREATE TABLE source(id INTEGER PRIMARY KEY,score INTEGER);
         CREATE TABLE target(id INTEGER,score INTEGER REFERENCES source(id));
         INSERT INTO source VALUES (1,1),(2,42),(3,3);",
    )
    .unwrap();
    assert!(bad.execute_batch(sql).is_err());
    let count: i64 = bad
        .query_row("SELECT COUNT(*) FROM target", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "FK violation cannot partially insert");
}

#[test]
fn ordered_insert_cte_then_insert_from_target_reads_prior_committed_state() {
    let sql = "INSERT INTO target(id,score) WITH staged AS
        (SELECT id,score FROM source WHERE id > 1)
        SELECT id,score FROM staged;
        INSERT INTO target(id,score) SELECT id + 10,score FROM target WHERE id = 2;";
    let bundle = analysis(sql);
    let source = state("source", &[(1, 10), (2, 20), (3, 30)]);
    let target = state("target", &[(0, 0)]);
    let plan = physical_ordered_write_state_plan(&bundle, &[source.clone(), target.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("ordered CTE and subsequent target read must be exact: {plan:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].affected().inserted(), 2);
    assert_eq!(transitions[1].affected().inserted(), 1);
    assert_eq!(
        transitions[1].before().rows(),
        state("target", &[(0, 0), (2, 20), (3, 30)]).rows()
    );
    assert_eq!(
        final_states
            .iter()
            .find(|s| s.relation() == "target")
            .unwrap()
            .rows(),
        state("target", &[(0, 0), (2, 20), (3, 30), (12, 20)]).rows()
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER,score INTEGER);
         CREATE TABLE target(id INTEGER,score INTEGER);
         INSERT INTO source VALUES (1,10),(2,20),(3,30);
         INSERT INTO target VALUES (0,0);",
    )
    .unwrap();
    db.execute_batch(sql).unwrap();
    let mut q = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let rows = q
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows, vec![(0, 0), (2, 20), (3, 30), (12, 20)]);
}

#[test]
fn insert_cte_rejects_unproved_scope_shaping_and_never_serializes_it_as_exact() {
    for sql in [
        "INSERT INTO target(id,score) WITH RECURSIVE staged AS
         (SELECT id,score FROM source) SELECT id,score FROM staged",
        "INSERT INTO target(id,score) WITH a AS
         (SELECT id,score FROM source), b AS (SELECT id,score FROM a)
         SELECT id,score FROM b",
        "INSERT INTO target(id,score) WITH staged AS
         (SELECT id,score FROM source LIMIT 1)
         SELECT id,score FROM staged",
    ] {
        let bundle = analysis(sql);
        let plan = physical_ordered_write_state_plan(
            &bundle,
            &[state("source", &[(1, 10)]), state("target", &[])],
        );
        assert!(
            !matches!(plan, PhysicalWriteStatePlan::Feasible { .. }),
            "{sql}"
        );
    }
}

#[test]
fn cte_insert_wire_emits_both_parser_independent_stages() {
    let bundle = analysis(
        "INSERT INTO target(score,id) WITH filtered AS
         (SELECT id,score FROM source WHERE id > 1)
         SELECT score,id FROM filtered",
    );
    let wire = sql_semantic_protocol::to_bundle_json(&bundle);
    assert_eq!(wire, sql_semantic_protocol::to_bundle_json(&bundle));
    let json: serde_json::Value = serde_json::from_str(&wire).unwrap();
    let statement = &json["inputs"][0]["statements"][0];
    let stage = &statement["write"]["cte_source"];
    assert_eq!(stage["name"], "filtered");
    assert_eq!(stage["producer"]["kind"], "query");
    assert_eq!(stage["consumer"]["kind"], "query");
    assert_eq!(stage["producer"]["sources"][0]["name"], "source");
    assert_eq!(stage["consumer"]["sources"][0]["name"], "filtered");
    assert_eq!(
        statement["write"]["target_columns"],
        serde_json::json!(["score", "id"])
    );
}

#[test]
fn insert_ordered_source_requires_order_execution_law_before_claiming_exact_rows() {
    let bundle = analysis(
        "INSERT INTO target(id,score) SELECT id,score FROM source ORDER BY id DESC",
    );
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[state("source",&[(1,10),(2,20)]),state("target",&[])],
    );
    assert!(!matches!(plan,PhysicalWriteStatePlan::Feasible { .. }),"{plan:?}");
}
