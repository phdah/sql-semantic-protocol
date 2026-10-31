//! TASK-80 independent parent replay: a successful bounded idempotence witness
//! and a deliberately non-idempotent duplicate-key contradiction.
//!
//! SQL/dbt/ODCS source parity is owned by TASK-145..148; only independently
//! verified DuckDB semantics are claimed for these sequential statements.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};

fn replay_oracle(id: &str) -> Value {
    let freeze: Value = serde_json::from_str(include_str!(
        "../docs/task-80-parent-acceptance-freeze.json"
    ))
    .unwrap();
    assert_eq!(freeze["version"], 2);
    freeze["parent_replay_oracles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .expect("frozen parent replay case")
        .clone()
}

fn row(value: &Value) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        (
            "id".to_string(),
            LiteralValue::Number(value["id"].as_i64().unwrap().to_string()),
        ),
        (
            "score".to_string(),
            LiteralValue::Number(value["score"].as_i64().unwrap().to_string()),
        ),
    ]))
    .unwrap()
}

fn state(relation: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.as_array().unwrap().iter().map(row).collect(),
    )
    .unwrap()
}

fn analysis(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["target", "source"].map(|name| {
        RelationSchema::new(
            name,
            ["id", "score"]
                .into_iter()
                .map(|col| SchemaColumn::from_sql_type(col, "INTEGER", "duckdb").unwrap())
                .collect(),
        )
        .unwrap()
    });
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let queries = [SqlInput::inline(sql), SqlInput::inline(sql)];
    let configured = queries
        .iter()
        .enumerate()
        .map(|(index, query)| {
            ConfiguredSqlInput::new(
                if index == 0 { "replay-1" } else { "replay-2" },
                query,
                "duckdb",
                dialect.as_ref(),
            )
        })
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task80-parent")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()],
    )
    .unwrap()]);
    bundle
}

fn native_bag(db: &Connection, relation: &str) -> Vec<(i64, i64)> {
    let sql = format!("SELECT id,score FROM {relation} ORDER BY id,score");
    let mut statement = db.prepare(&sql).unwrap();
    statement
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn frozen_bag(rows: &Value) -> Vec<(i64, i64)> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| (row["id"].as_i64().unwrap(), row["score"].as_i64().unwrap()))
        .collect()
}

fn assert_domains(
    state: &PhysicalWriteState,
    bundle: &sql_semantic_protocol::AnalysisBundle,
    frozen: &Value,
) {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
    let domains = state.exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    for domain in domains {
        let expected = &frozen[domain.column()];
        assert_eq!(expected["type"], "INTEGER");
        assert_eq!(expected["nullable"], false);
        assert_eq!(expected["lower_inclusive"], true);
        assert_eq!(expected["upper_inclusive"], true);
        let observed = domain
            .values()
            .iter()
            .map(|(value, count)| match value {
                LiteralValue::Number(n) => (n.clone(), *count),
                LiteralValue::Null => panic!("frozen nonnullable domain"),
                other => panic!("unexpected typed value {other:?}"),
            })
            .collect::<BTreeMap<_, _>>();
        let expected_frequencies = expected["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(observed, expected_frequencies, "{}", domain.column());
        let extrema = domain
            .values()
            .iter()
            .filter_map(|(value, _)| match value {
                LiteralValue::Number(n) => n.parse::<i64>().ok(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(extrema.iter().min().copied(), expected["lower"].as_i64());
        assert_eq!(extrema.iter().max().copied(), expected["upper"].as_i64());
    }
}

#[test]
fn conflict_do_nothing_has_exact_second_application_fixed_point() {
    let case = replay_oracle("80-P-REPLAY-CONFLICT");
    assert_eq!(case["proof_positive"].as_str().unwrap().contains("Exact"), true);
    let sql = case["sql"].as_str().unwrap();
    let bundle = analysis(sql);
    let source = state("source", &case["complete_source"]);
    let before = state("target", &case["initial_target"]);
    let plan = physical_ordered_write_state_plan(&bundle, &[source.clone(), before.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("two statements must be exact under enforced PK: {plan:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].before(), &before);
    assert_eq!(
        transitions[0].after(),
        &state("target", &case["target_after_first"])
    );
    assert_eq!(
        transitions[1].before(),
        &state("target", &case["target_after_first"])
    );
    assert_eq!(
        transitions[1].after(),
        &state("target", &case["target_after_second"])
    );
    for (transition, expected) in transitions.iter().zip([
        &case["first_affected"],
        &case["second_affected"],
    ]) {
        assert_eq!(transition.affected().inserted(), expected["inserted"].as_u64().unwrap());
        assert_eq!(transition.affected().updated(), expected["updated"].as_u64().unwrap());
        assert_eq!(transition.affected().deleted(), expected["deleted"].as_u64().unwrap());
    }
    let first = transitions[0].returning().unwrap();
    assert_eq!(first.rows().len(), 1);
    assert_eq!(
        first.rows()[0].values().get("id"),
        Some(&LiteralValue::Number("2".into()))
    );
    let second = transitions[1].returning().unwrap();
    assert!(second.rows().is_empty());
    let target_after = state("target", &case["target_after_second"]);
    assert_eq!(
        final_states.iter().find(|state| state.relation() == "target"),
        Some(&target_after)
    );
    assert_eq!(
        final_states.iter().find(|state| state.relation() == "source"),
        Some(&source)
    );
    assert_eq!(final_states.len(), 2);
    assert_domains(
        &target_after,
        &bundle,
        &case["typed_final_domains"],
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);
         CREATE TABLE source(id INTEGER,score INTEGER);
         INSERT INTO target VALUES (1,10);
         INSERT INTO source VALUES (1,20),(2,30);",
    )
    .unwrap();
    assert_eq!(native_bag(&db, "target"), frozen_bag(&case["initial_target"]));
    assert_eq!(native_bag(&db, "source"), frozen_bag(&case["complete_source"]));
    for (after, expected_returning) in [
        (&case["target_after_first"], &case["returning_first"]),
        (&case["target_after_second"], &case["returning_second"]),
    ] {
        let mut statement = db.prepare(sql).unwrap();
        let returned = statement
            .query_map([], |r| r.get::<_, i64>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        let expected = expected_returning
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(returned, expected, "complete DuckDB RETURNING output");
        drop(statement);
        assert_eq!(native_bag(&db, "target"), frozen_bag(after));
        assert_eq!(native_bag(&db, "source"), frozen_bag(&case["complete_source"]));
    }
}

#[test]
fn ordinary_duplicate_insert_is_impossible_not_idempotent() {
    let case = replay_oracle("80-N-REPLAY-PK");
    assert!(case["target_after_second"].is_null());
    assert_eq!(case["proof_not_true"].as_str().unwrap().contains("Impossible"), true);
    let sql = case["sql"].as_str().unwrap();
    let bundle = analysis(sql);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[state("target", &case["initial_target"])],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "a second insertion of the same PK must contradict the complete bag: {plan:?}"
    );

    // A failed second independent statement cannot be promoted to a
    // successful idempotent transition. DuckDB retains the first statement's
    // committed row, without proving any explicit transaction rollback law.
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);")
        .unwrap();
    assert!(native_bag(&db, "target").is_empty());
    db.execute_batch(sql).unwrap();
    assert_eq!(
        native_bag(&db, "target"),
        frozen_bag(&case["target_after_first"])
    );
    assert!(db.execute_batch(sql).is_err());
    assert_eq!(
        native_bag(&db, "target"),
        frozen_bag(&case["post_failure_database_state"])
    );
    let first = state("target", &case["post_failure_database_state"]);
    assert_domains(
        &first,
        &bundle,
        &case["typed_last_committed_domains"],
    );
}
