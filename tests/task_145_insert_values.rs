//! TASK-145 public INSERT VALUES/DEFAULT VALUES closed-state execution proofs.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    physical_ordered_write_state_plan_with_identities, AnalysisBundle, ColumnValueKind,
    ColumnValueSource, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, GeneratedValueExpression,
    LiteralValue, PhysicalIdentitySequence, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput, WriteEffectAction,
};

fn analysis(sql: &str, dialect: &str) -> AnalysisBundle {
    let schema = RelationSchema::new(
        "target",
        ["id", "score"]
            .into_iter()
            .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect_impl = dialect_from_name(dialect).unwrap();
    let input = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("task-145", &input, dialect, dialect_impl.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn state(rows: &[(Option<i64>, Option<i64>)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "target",
        rows.iter()
            .map(|(id, score)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".to_string(),
                        id.map_or(LiteralValue::Null, |value| {
                            LiteralValue::Number(value.to_string())
                        }),
                    ),
                    (
                        "score".to_string(),
                        score.map_or(LiteralValue::Null, |value| {
                            LiteralValue::Number(value.to_string())
                        }),
                    ),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn exact_after(
    bundle: &AnalysisBundle,
    before: &[(Option<i64>, Option<i64>)],
) -> PhysicalWriteState {
    let plan = physical_ordered_write_state_plan(bundle, &[state(before)]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &plan
    else {
        panic!("expected exact feasible INSERT, got {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].before().rows(), state(before).rows());
    assert_eq!(transitions[0].after().rows(), final_states[0].rows());
    final_states[0].clone()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-145").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn defaults(bundle: &mut AnalysisBundle, columns: &[(&str, i64)]) {
    let sources = columns
        .iter()
        .map(|(column, value)| {
            ColumnValueSource::new(
                *column,
                ColumnValueKind::Default,
                Some(ConstraintValue::Integer(*value)),
                evidence(),
            )
            .unwrap()
        })
        .collect();
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(sources)]);
}

#[test]
fn literal_multirow_reordered_and_duplicate_insert_preserve_complete_bags() {
    for (sql, initial, expected) in [
        (
            "INSERT INTO target (id, score) VALUES (1, 10), (2, 20)",
            vec![],
            vec![(Some(1), Some(10)), (Some(2), Some(20))],
        ),
        (
            "INSERT INTO target (score, id) VALUES (10, 1), (20, 2)",
            vec![],
            vec![(Some(1), Some(10)), (Some(2), Some(20))],
        ),
        (
            "INSERT INTO target VALUES (1, 10), (2, 20)",
            vec![],
            vec![(Some(1), Some(10)), (Some(2), Some(20))],
        ),
        (
            "INSERT INTO target (id, score) VALUES (1, 10), (1, 10)",
            vec![(Some(0), Some(0))],
            vec![(Some(0), Some(0)), (Some(1), Some(10)), (Some(1), Some(10))],
        ),
        (
            "INSERT INTO target (id, score) VALUES (NULL, 10)",
            vec![],
            vec![(None, Some(10))],
        ),
    ] {
        let bundle = analysis(sql, "duckdb");
        let actual = exact_after(&bundle, &initial);
        assert_eq!(actual.rows(), state(&expected).rows(), "{sql}");
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation() == "target")
            .unwrap();
        let domains = actual.exact_domains(schema).unwrap();
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].column(), "id");
        assert_eq!(domains[1].column(), "score");
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE target(id INTEGER, score INTEGER);")
            .unwrap();
        for (id, score) in &initial {
            let as_sql =
                |number: &Option<i64>| number.map_or("NULL".to_string(), |n| n.to_string());
            db.execute_batch(&format!(
                "INSERT INTO target VALUES ({}, {})",
                as_sql(id),
                as_sql(score)
            ))
            .unwrap();
        }
        db.execute_batch(sql).unwrap();
        let mut stmt = db
            .prepare("SELECT id, score FROM target ORDER BY id NULLS FIRST, score")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
            })
            .unwrap();
        let mut observed = rows.map(|row| row.unwrap()).collect::<Vec<_>>();
        let mut expected_sorted = expected;
        observed.sort();
        expected_sorted.sort();
        assert_eq!(observed, expected_sorted, "DuckDB full target bag: {sql}");
    }
}

#[test]
fn default_values_and_omitted_columns_require_authoritative_evidence() {
    let mut omitted = analysis("INSERT INTO target (id) VALUES (1), (2)", "duckdb");
    defaults(&mut omitted, &[("score", 7)]);
    assert_eq!(
        exact_after(&omitted, &[]).rows(),
        state(&[(Some(1), Some(7)), (Some(2), Some(7))]).rows()
    );

    let mut full_default = analysis("INSERT INTO target DEFAULT VALUES", "duckdb");
    defaults(&mut full_default, &[("id", 5), ("score", 7)]);
    let effect = full_default.write_state_effects();
    assert!(matches!(
        effect[0].effect().branches()[0].action(),
        WriteEffectAction::InsertDefaultValues
    ));
    assert_eq!(effect[0].effect().affected_rows().minimum(), 1);
    assert_eq!(effect[0].effect().affected_rows().maximum(), Some(1));
    assert_eq!(
        exact_after(&full_default, &[]).rows(),
        state(&[(Some(5), Some(7))]).rows()
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER DEFAULT 5, score INTEGER DEFAULT 7);
         INSERT INTO target (id) VALUES (1),(2);
         INSERT INTO target DEFAULT VALUES;",
    )
    .unwrap();
    let mut stmt = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap();
    assert_eq!(
        rows.map(|x| x.unwrap()).collect::<Vec<_>>(),
        vec![(1, 7), (2, 7), (5, 7)]
    );

    let without = analysis("INSERT INTO target DEFAULT VALUES", "duckdb");
    assert!(matches!(
        physical_ordered_write_state_plan(&without, &[state(&[])]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}

#[test]
fn exact_constraints_reject_impossible_duplicates_without_emitting_poststate() {
    let mut bundle = analysis(
        "INSERT INTO target(id,score) VALUES (1,10),(1,20)",
        "duckdb",
    );
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap()],
    )
    .unwrap()]);
    assert!(matches!(
        physical_ordered_write_state_plan(&bundle, &[state(&[])]),
        PhysicalWriteStatePlan::Impossible { .. }
    ));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);")
        .unwrap();
    assert!(db
        .execute_batch("INSERT INTO target VALUES (1,10),(1,20)")
        .is_err());
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM target", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn opaque_insert_literal_is_explicitly_unsupported() {
    let bundle = analysis("INSERT INTO target(id,score) VALUES (1, 1 + 2)", "duckdb");
    let plan = physical_ordered_write_state_plan(&bundle, &[state(&[])]);
    assert!(!matches!(plan, PhysicalWriteStatePlan::Feasible { .. }));
}

#[test]
fn generated_insert_fields_use_attested_row_local_dependencies() {
    let mut bundle = analysis("INSERT INTO target(id) VALUES (2),(3)", "duckdb");
    let generated = GeneratedValueExpression::Add(
        Box::new(GeneratedValueExpression::Column("id".to_string())),
        Box::new(GeneratedValueExpression::Literal(ConstraintValue::Integer(
            1,
        ))),
    );
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![ColumnValueSource::new(
            "score",
            ColumnValueKind::Generated,
            None,
            evidence(),
        )
        .unwrap()
        .with_generated_expression(generated)
        .unwrap()])]);
    assert_eq!(
        exact_after(&bundle, &[]).rows(),
        state(&[(Some(2), Some(3)), (Some(3), Some(4))]).rows()
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER GENERATED ALWAYS AS (id + 1));
         INSERT INTO target(id) VALUES (2),(3);",
    )
    .unwrap();
    let mut stmt = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let observed = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(observed, vec![(2, 3), (3, 4)]);
}

#[test]
fn identity_insert_requires_caller_attested_sequence_and_preserves_row_order() {
    let mut bundle = analysis("INSERT INTO target(score) VALUES (10),(20)", "duckdb");
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![ColumnValueSource::new(
            "id",
            ColumnValueKind::Identity,
            None,
            evidence(),
        )
        .unwrap()])]);
    assert!(matches!(
        physical_ordered_write_state_plan(&bundle, &[state(&[])]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
    let sequence = PhysicalIdentitySequence::new("target", "id", 100, 5).unwrap();
    let exact =
        physical_ordered_write_state_plan_with_identities(&bundle, &[state(&[])], &[sequence]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = exact
    else {
        panic!("attested identity must be constructible: {exact:?}");
    };
    assert_eq!(transitions[0].affected().inserted(), 2);
    assert_eq!(
        final_states[0].rows(),
        state(&[(Some(100), Some(10)), (Some(105), Some(20))]).rows()
    );
}

#[test]
fn inserting_an_unproved_default_expression_is_not_an_exact_write() {
    for sql in [
        "INSERT INTO target(id,score) VALUES (1,DEFAULT)",
        "INSERT INTO target(id,score) VALUES (1,1+2)",
        "INSERT INTO target(id,score) VALUES (1,'not-an-integer')",
    ] {
        let bundle = analysis(sql, "duckdb");
        assert!(
            !matches!(
                physical_ordered_write_state_plan(&bundle, &[state(&[])]),
                PhysicalWriteStatePlan::Feasible { .. }
            ),
            "{sql}"
        );
    }
}

#[test]
fn emitted_protocol_carries_source_free_write_actions_and_exact_literal_rows() {
    let literal = analysis(
        "INSERT INTO target (id,score) VALUES (1,10),(2,NULL)", "duckdb"
    );
    let encoded = sql_semantic_protocol::to_bundle_json(&literal);
    assert_eq!(encoded, sql_semantic_protocol::to_bundle_json(&literal));
    let document: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let statement = &document["inputs"][0]["statements"][0];
    assert_eq!(statement["write"]["kind"],"append");
    assert_eq!(statement["write"]["default_values"],false);
    assert_eq!(statement["write"]["state_effect"]["branches"][0]["action"]["kind"],"insert_query");
    assert_eq!(statement["finite_table"]["rows"].as_array().unwrap().len(),2);

    let defaults = analysis("INSERT INTO target DEFAULT VALUES","duckdb");
    let document:serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&defaults)).unwrap();
    let statement=&document["inputs"][0]["statements"][0];
    assert_eq!(statement["write"]["default_values"],true);
    assert_eq!(statement["write"]["target"],"target");
    assert_eq!(statement["write"]["state_effect"]["branches"][0]["action"]["kind"],
        "insert_default_values");
    assert_eq!(statement["write"]["state_effect"]["affected_rows"]["minimum"],1);
    assert_eq!(statement["write"]["state_effect"]["affected_rows"]["maximum"],1);
}
