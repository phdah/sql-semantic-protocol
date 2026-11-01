//! TASK-158: every frozen acceptance row is replayed against the public
//! physical seam, not merely inventoried or parsed.

use serde_json::Value;
use sql_semantic_protocol::{
    physical_alter_column_snapshot, LiteralValue, PhysicalColumnAlterProof,
    PhysicalColumnAlterStep, PhysicalWriteRow, PhysicalWriteState, RelationSchema, SchemaColumn,
};
use std::collections::{BTreeMap, BTreeSet};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-158-acceptance-freeze.json")).unwrap()
}

fn schema(columns: &Value) -> RelationSchema {
    RelationSchema::new(
        "t",
        columns
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let declared = v.as_str().unwrap();
                let (name, data_type) = declared.split_once(' ').unwrap();
                SchemaColumn::from_sql_type(name, data_type, "duckdb").unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn state(rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "t",
        rows.as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                let values = entry
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(name, value)| {
                        let value = match value {
                            Value::Null => LiteralValue::Null,
                            Value::Number(n) => LiteralValue::Number(n.to_string()),
                            Value::String(s) => LiteralValue::Text(s.clone()),
                            Value::Bool(b) => LiteralValue::Boolean(*b),
                            other => panic!("unsupported literal {other:?}"),
                        };
                        (name.clone(), value)
                    })
                    .collect::<BTreeMap<_, _>>();
                PhysicalWriteRow::new(values).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn sorted_bag(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut result = rows
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[test]
fn every_frozen_case_has_a_complete_typed_exact_or_refutation() {
    let matrix = frozen();
    assert_eq!(matrix["version"], 2);
    assert_eq!(matrix["owner"], "TASK-158");
    assert_eq!(matrix["parent"], "TASK-83");
    let mut seen = BTreeSet::new();
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(seen.insert(id));
        let original_schema = schema(&case["initial_schema"]);
        let before = state(&case["prestate"]);
        let operation = match id {
            "83-C158-P-add-default" => PhysicalColumnAlterStep::Add {
                column: SchemaColumn::from_sql_type("b", "INTEGER", "duckdb").unwrap(),
                default: Some(LiteralValue::Number("2".into())),
            },
            "83-C158-P-add-null" => PhysicalColumnAlterStep::Add {
                column: SchemaColumn::from_sql_type("b", "INTEGER", "duckdb").unwrap(),
                default: None,
            },
            "83-C158-P-rename" => PhysicalColumnAlterStep::Rename {
                old: "a".into(),
                new: "c".into(),
            },
            "83-C158-P-drop" => PhysicalColumnAlterStep::Drop { column: "b".into() },
            "83-C158-N-duplicate" => PhysicalColumnAlterStep::Add {
                column: SchemaColumn::from_sql_type("a", "INTEGER", "duckdb").unwrap(),
                default: None,
            },
            "83-C158-N-missing" => PhysicalColumnAlterStep::Drop {
                column: "absent".into(),
            },
            "83-C158-N-type" => PhysicalColumnAlterStep::SetType {
                column: SchemaColumn::from_sql_type("a", "VARCHAR", "duckdb").unwrap(),
            },
            "83-C158-N-key" => PhysicalColumnAlterStep::Drop {
                column: "key".into(),
            },
            other => panic!("unreviewed frozen case {other}"),
        };
        let inventory = if id == "83-C158-N-key" {
            Some(vec![sql_semantic_protocol::RelationConstraintSet::new(
                "t",
                Vec::new(),
            )
            .unwrap()])
        } else {
            Some(Vec::new())
        };
        let proof = physical_alter_column_snapshot(
            &original_schema,
            &before,
            &[operation],
            inventory.as_deref(),
        );
        match case["expected"].as_str().unwrap() {
            "exact_row_projection_only" => {
                let PhysicalColumnAlterProof::Exact {
                    schema: after_schema,
                    state: after,
                } = proof
                else {
                    panic!("{id}: expected Exact, got {proof:?}");
                };
                let expected_schema = schema(&case["expected_schema"]);
                assert_eq!(after_schema.columns(), expected_schema.columns(), "{id}");
                assert_eq!(
                    after.rows().len(),
                    before.rows().len(),
                    "{id}: stable occurrence ids"
                );
                assert_eq!(
                    sorted_bag(after.rows()),
                    sorted_bag(state(&case["poststate"]).rows()),
                    "{id}"
                );
                let domains = after.exact_domains(&after_schema).unwrap();
                for (column, domain) in after_schema.columns().iter().zip(domains) {
                    let mut total = 0_u64;
                    let mut previous = BTreeSet::new();
                    for (value, frequency) in domain.values() {
                        assert!(*frequency > 0);
                        assert!(previous.insert(format!("{value:?}")));
                        total += frequency;
                    }
                    assert_eq!(total as usize, after.rows().len(), "{id}/{}", column.name());
                }
            }
            "impossible_duplicate_name" | "impossible_missing_column" => assert!(
                matches!(proof, PhysicalColumnAlterProof::Impossible { .. }),
                "{id}: {proof:?}"
            ),
            "residual_cast_rewrite_law" => match proof {
                PhysicalColumnAlterProof::Residual {
                    before_schema,
                    attempted_schema: Some(attempted),
                    ..
                } => {
                    assert_eq!(before_schema, original_schema);
                    assert_eq!(
                        attempted.columns()[0].data_type(),
                        SchemaColumn::from_sql_type("a", "VARCHAR", "duckdb")
                            .unwrap()
                            .data_type()
                    );
                }
                other => panic!("{id}: typed before/after schema required: {other:?}"),
            },
            "residual_constrained_schema_transition" => assert!(
                matches!(proof, PhysicalColumnAlterProof::Residual { .. }),
                "{id}: {proof:?}"
            ),
            other => panic!("{id}: unreviewed proof class {other}"),
        }
    }
}

#[test]
fn acceptance_dialect_matrix_is_complete_and_parent_stays_open() {
    let freeze = frozen();
    let cases = freeze["cases"].as_array().unwrap();
    let matrix = freeze["dialect_expected"].as_object().unwrap();
    assert_eq!(matrix.len(), 13);
    assert_eq!(
        matrix
            .values()
            .map(|v| v.as_array().unwrap().len())
            .sum::<usize>(),
        13 * cases.len()
    );
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let entry = &manifest["task_158_alter_column_transition_acceptance"];
    assert_eq!(entry["freeze_version"], 2);
    assert_eq!(entry["parent_signed_off"], false);
}
