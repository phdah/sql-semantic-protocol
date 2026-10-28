//! TASK-148: independently certify complete ordered INSERT source and target bags.
//! The acceptance inventory is frozen in docs/task-148-acceptance-freeze.json.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_ordered_write_state_plan, ColumnValueKind, ColumnValueSource,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-148-acceptance-freeze.json")).unwrap()
}

fn rows(value: &serde_json::Value) -> Vec<(i64, i64)> {
    value.as_array().unwrap().iter().map(|row| {
        (row["id"].as_i64().unwrap(), row["score"].as_i64().unwrap())
    }).collect()
}

fn state(relation: &str, values: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, values.iter().map(|(id, score)| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number(id.to_string())),
            ("score".to_string(), LiteralValue::Number(score.to_string())),
        ])).unwrap()
    }).collect()).unwrap()
}

fn enforced() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task148").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn analysis(case: &serde_json::Value) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["source", "target", "report"].map(|relation| {
        RelationSchema::new(relation, ["id", "score"].into_iter().map(|column| {
            SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap()
        }).collect()).unwrap()
    });
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let sqls = case["statements"].as_array().unwrap().iter()
        .map(|sql| SqlInput::inline(sql.as_str().unwrap())).collect::<Vec<_>>();
    let ids = (0..sqls.len()).map(|i| format!("task148-{i}")).collect::<Vec<_>>();
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = sqls.iter().zip(&ids).map(|(sql, id)| {
        ConfiguredSqlInput::new(id, sql, "duckdb", dialect.as_ref())
    }).collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    if case["id"] == "80-C148-P-default-then-ctas" {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
            .unwrap().with_column_value_sources([
                ("id", 5), ("score", 7)
            ].into_iter().map(|(column, value)| {
                ColumnValueSource::new(
                    column, ColumnValueKind::Default,
                    Some(ConstraintValue::Integer(value)), enforced(),
                ).unwrap()
            }).collect())]);
    }
    if case["id"] == "80-C148-N-duplicate-pk" {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::primary_key(vec!["id".into()], enforced()).unwrap()],
        ).unwrap()]);
    }
    bundle
}

fn sorted(mut values: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    values.sort();
    values
}

fn native_rows(db: &Connection, relation: &str) -> Vec<(i64, i64)> {
    let mut stmt = db.prepare(&format!("SELECT id,score FROM {relation} ORDER BY id,score"))
        .unwrap();
    stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap().map(|r| r.unwrap()).collect()
}

fn duckdb(case: &serde_json::Value) {
    let db = Connection::open_in_memory().unwrap();
    let primary = if case["id"] == "80-C148-N-duplicate-pk" {
        "PRIMARY KEY"
    } else {
        ""
    };
    db.execute_batch(&format!(
        "CREATE TABLE source(id INTEGER,score INTEGER);
         CREATE TABLE target(id INTEGER {primary} DEFAULT 5,score INTEGER DEFAULT 7);"
    )).unwrap();
    for (relation, key) in [("source", "source"), ("target", "prestate")] {
        for (id, score) in rows(&case[key]) {
            db.execute_batch(&format!(
                "INSERT INTO {relation}(id,score) VALUES ({id},{score})"
            )).unwrap();
        }
    }
    let original_source = native_rows(&db, "source");
    let mut insert_count = 0;
    for sql in case["statements"].as_array().unwrap() {
        let sql = sql.as_str().unwrap();
        if sql == "SELECT COUNT(*) FROM target" {
            let observed: i64 = db.query_row(sql, [], |row| row.get(0)).unwrap();
            assert_eq!(observed, 2, "SELECT must observe preceding INSERT");
        } else if case["id"] == "80-C148-N-duplicate-pk" && insert_count == 1 {
            assert!(db.execute_batch(sql).is_err(), "duplicate key must fail");
            break;
        } else {
            db.execute_batch(sql).unwrap();
        }
        if sql.starts_with("INSERT INTO ") {
            insert_count += 1;
        }
    }
    if case["id"] == "80-C148-N-duplicate-pk" {
        assert_eq!(native_rows(&db, "target"), vec![(1, 10)]);
    } else {
        assert_eq!(native_rows(&db, "target"), sorted(rows(&case["target_after"])));
        assert_eq!(native_rows(&db, "report"), sorted(rows(&case["report_after"])));
    }
    assert_eq!(native_rows(&db, "source"), original_source, "source remains immutable");
}

#[test]
fn all_frozen_positive_insert_families_compose_through_later_ddl_and_reads() {
    let matrix = freeze();
    assert_eq!(matrix["fixture_ids"], serde_json::json!(["80-C148-P", "80-C148-N"]));
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(cases.iter().filter(|c| c["classification"] == "mandatory_exact").count(), 4);
    for case in cases.iter().filter(|c| c["classification"] == "mandatory_exact") {
        let id = case["id"].as_str().unwrap();
        let inputs = [
            state("source", &rows(&case["source"])),
            state("target", &rows(&case["prestate"])),
        ];
        let bundle = analysis(case);
        let plan = physical_ordered_write_state_plan(&bundle, &inputs);
        let PhysicalWriteStatePlan::Feasible { transitions, final_states } = &plan else {
            panic!("{id}: complete ordered state expected: {plan:?}");
        };
        assert_eq!(transitions.len(), 2, "{id}: INSERT and CTAS, not a read-only SELECT");
        assert_eq!(transitions[0].before().rows(), inputs[1].rows());
        assert_eq!(transitions[0].after().rows(),
            state("target", &rows(&case["target_after"])).rows(), "{id}: full target bag");
        assert_eq!(transitions[1].after().rows(),
            state("report", &rows(&case["report_after"])).rows(), "{id}: full CTAS bag");
        assert_eq!(transitions[1].before().rows().len(), 0, "{id}: CREATE has no prestate");
        assert_eq!(transitions[0].affected().inserted(),
            case["affected"][0]["inserted"].as_u64().unwrap(), "{id}");
        assert_eq!(transitions[1].affected().inserted(),
            case["affected"][1]["inserted"].as_u64().unwrap(), "{id}");
        assert_eq!(final_states.len(), 3, "{id}: CTE cannot be a physical source");
        assert_eq!(final_states.iter().find(|s| s.relation() == "source"), Some(&inputs[0]));
        assert_eq!(final_states.iter().find(|s| s.relation() == "target"),
            Some(transitions[0].after()));
        assert_eq!(final_states.iter().find(|s| s.relation() == "report"),
            Some(transitions[1].after()));
        let intermediates = case["intermediate"].as_array().unwrap();
        assert_eq!(transitions[0].intermediate_states().len(), intermediates.len(), "{id}");
        for (actual, expected) in transitions[0].intermediate_states().iter().zip(intermediates) {
            assert_eq!(actual.rows(), state(expected["relation"].as_str().unwrap(),
                &rows(&expected["rows"])).rows(), "{id}: complete CTE producer");
        }
        let schema = bundle.source_schemas().iter()
            .find(|schema| schema.relation() == "target").unwrap();
        let domains = transitions[0].after().exact_domains(schema).unwrap();
        for domain in domains {
            let expected = &case["typed_domain"][domain.column()];
            assert_eq!(expected["nullable"], false, "{id}");
            let values = domain.values().iter().map(|(v, count)| {
                (v.clone(), *count)
            }).collect::<Vec<_>>();
            let expected_values = expected["values"].as_array().unwrap().iter().map(|v| {
                (LiteralValue::Number(v.as_i64().unwrap().to_string()), 1)
            }).collect::<Vec<_>>();
            assert_eq!(values, expected_values, "{id}: exact typed value frequencies");
            let signed = values.iter().map(|(v, _)| match v {
                LiteralValue::Number(v) => v.parse::<i64>().unwrap(),
                _ => panic!("{id}: expected INTEGER domain"),
            }).collect::<Vec<_>>();
            assert_eq!(signed.iter().min().copied(), expected["lower"].as_i64(), "{id}");
            assert_eq!(signed.iter().max().copied(), expected["upper"].as_i64(), "{id}");
            assert_eq!(expected["lower_inclusive"], true);
            assert_eq!(expected["upper_inclusive"], true);
        }
        duckdb(case);
    }
}

#[test]
fn duplicate_key_is_impossible_after_prior_insert_and_sql_oracle_rejects_it() {
    let matrix = freeze();
    let case = matrix["cases"].as_array().unwrap().iter()
        .find(|c| c["id"] == "80-C148-N-duplicate-pk").unwrap();
    let result = physical_ordered_write_state_plan(
        &analysis(case), &[state("source", &[]), state("target", &[])],
    );
    assert!(matches!(result, PhysicalWriteStatePlan::Impossible { .. }), "{result:?}");
    duckdb(case);
}

#[test]
fn unsupported_transaction_does_not_disappear_from_ordered_program() {
    let matrix = freeze();
    let case = matrix["cases"].as_array().unwrap().iter()
        .find(|c| c["id"] == "80-C148-N-unproved-transaction").unwrap();
    let result = physical_ordered_write_state_plan(
        &analysis(case), &[state("source", &[]), state("target", &[])],
    );
    assert!(matches!(
        result,
        PhysicalWriteStatePlan::Residual { .. }
    ), "{result:?}");
}
