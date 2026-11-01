//! Independent TASK-81 parent AC #4 source-complete INSERT/MERGE/UPDATE replay.
//! Child freezes remain immutable; the parent freeze owns these replay expectations.

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

fn frozen() -> Value {
    serde_json::from_str(include_str!(
        "../docs/task-81-parent-acceptance-freeze.json"
    ))
    .unwrap()
}

fn row(value: &Value) -> PhysicalWriteRow {
    let fields = value
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            let item = value
                .as_i64()
                .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()));
            (name.clone(), item)
        })
        .collect();
    PhysicalWriteRow::new(fields).unwrap()
}

fn state(relation: &str, value: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        value.as_array().unwrap().iter().map(row).collect(),
    )
    .unwrap()
}

fn analysis(sql: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let target = RelationSchema::new(
        "target",
        ["id", "score"]
            .into_iter()
            .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    let source = RelationSchema::new(
        "source",
        ["id", "score"]
            .into_iter()
            .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[target, source]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let owned = sql
        .iter()
        .map(|text| SqlInput::inline(*text))
        .collect::<Vec<_>>();
    let ids = (0..owned.len())
        .map(|index| format!("parent81-{index}"))
        .collect::<Vec<_>>();
    let inputs = owned
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let attested = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task81-parent").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".to_owned()], attested).unwrap()],
    )
    .unwrap()]);
    bundle
}

fn domains(rows: &PhysicalWriteState, schema: &RelationSchema, expected: &Value) {
    let observed = rows.exact_domains(schema).unwrap();
    assert_eq!(observed.len(), expected.as_object().unwrap().len());
    for column in observed {
        let fixture = &expected[column.column()];
        let freq = column
            .values()
            .iter()
            .map(|(value, count)| {
                let label = match value {
                    LiteralValue::Number(n) => n.clone(),
                    LiteralValue::Null => "NULL".to_string(),
                    unexpected => panic!("unexpected typed value {unexpected:?}"),
                };
                (label, *count)
            })
            .collect::<BTreeMap<_, _>>();
        let specified = fixture["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(value, count)| (value.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(freq, specified, "{}: exact frequency", column.column());
        let numeric = freq
            .keys()
            .filter(|key| key.as_str() != "NULL")
            .map(|key| key.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(numeric.iter().min().copied(), fixture["lower"].as_i64());
        assert_eq!(numeric.iter().max().copied(), fixture["upper"].as_i64());
        assert_eq!(
            freq.contains_key("NULL"),
            fixture["nullable"].as_bool().unwrap()
        );
        assert_eq!(
            !numeric.is_empty(),
            fixture["lower_inclusive"].as_bool().unwrap()
        );
        assert_eq!(
            !numeric.is_empty(),
            fixture["upper_inclusive"].as_bool().unwrap()
        );
        assert_eq!(fixture["type"], "INTEGER");
    }
}

fn native(db: &Connection) -> PhysicalWriteState {
    let mut stmt = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".to_owned(), LiteralValue::Number(id.to_string())),
                ("score".to_owned(), LiteralValue::Number(score.to_string())),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new("target", rows).unwrap()
}

#[test]
fn ordered_insert_merge_update_preserves_exact_native_bags_and_returning_domains() {
    let freeze = frozen();
    let fixture = &freeze["parent_replay_oracles"][0];
    assert_eq!(freeze["version"], 2);
    assert_eq!(fixture["id"], "81-P-REPLAY-MERGE");
    let steps = fixture["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    let sql = steps
        .iter()
        .map(|step| step["sql"].as_str().unwrap())
        .collect::<Vec<_>>();
    let bundle = analysis(&sql);
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    let source = state("source", &fixture["source"]);
    let initial = state("target", &fixture["target_initial"]);
    let plan = physical_ordered_write_state_plan(&bundle, &[initial, source.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("complete uniquely matched MERGE must be feasible: {plan:?}");
    };
    assert_eq!(transitions.len(), 3);
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER); CREATE TABLE source(id INTEGER PRIMARY KEY,score INTEGER); INSERT INTO source VALUES (1,50),(3,30);").unwrap();
    for (index, (transition, step)) in transitions.iter().zip(steps).enumerate() {
        let before = state("target", &step["before"]);
        let expected_after = state("target", &step["after"]);
        assert_eq!(
            transition.before(),
            &before,
            "step {index}: canonical prestate"
        );
        assert_eq!(native(&db), before, "step {index}: native prestate");
        let actual_counts = transition.affected();
        assert_eq!(
            actual_counts.inserted(),
            step["affected"]["inserted"].as_u64().unwrap()
        );
        assert_eq!(
            actual_counts.updated(),
            step["affected"]["updated"].as_u64().unwrap()
        );
        assert_eq!(
            actual_counts.deleted(),
            step["affected"]["deleted"].as_u64().unwrap()
        );
        if index == 2 {
            let mut stmt = db.prepare(sql[index]).unwrap();
            let returned = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            assert_eq!(returned, vec![(3, 35)]);
            let canonical = transition.returning().expect("bounded RETURNING supported");
            assert_eq!(canonical.rows(), state("target", &step["returning"]).rows());
            let columns = transition.returning_exact_domains(schema).unwrap();
            assert_eq!(columns.len(), 2);
            let returned_schema = RelationSchema::new("target", schema.columns().to_vec()).unwrap();
            domains(canonical, &returned_schema, &step["returning_domains"]);
        } else {
            db.execute_batch(sql[index]).unwrap();
            assert!(transition.returning().is_none());
        }
        assert_eq!(
            transition.after(),
            &expected_after,
            "step {index}: canonical poststate"
        );
        let materialized = native(&db);
        assert_eq!(
            materialized, expected_after,
            "step {index}: independent native poststate"
        );
        domains(transition.after(), schema, &step["target_domains"]);
        domains(&materialized, schema, &step["target_domains"]);
    }
    assert_eq!(
        state("target", &fixture["expected_final"]),
        transitions.last().unwrap().after().clone()
    );
    assert_eq!(
        final_states
            .iter()
            .find(|state| state.relation() == "source"),
        Some(&source)
    );
    let mut stmt = db
        .prepare("SELECT id,score FROM source ORDER BY id")
        .unwrap();
    let native_source = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(native_source, vec![(1, 50), (3, 30)]);
}

#[test]
fn duplicate_merge_source_matches_are_residual_not_a_selected_winner() {
    let freeze = frozen();
    let negative = &freeze["parent_replay_oracles"][1];
    assert_eq!(negative["id"], "81-N-REPLAY-MULTIMATCH");
    let bundle = analysis(&[negative["sql"].as_str().unwrap()]);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[
            state("target", &negative["target_before"]),
            state("source", &negative["source"]),
        ],
    );
    let PhysicalWriteStatePlan::Residual { gap, .. } = plan else {
        panic!("ambiguous matching must fail closed: {plan:?}");
    };
    assert_eq!(
        format!("{gap:?}"),
        negative["expected_gap"].as_str().unwrap()
    );
}
