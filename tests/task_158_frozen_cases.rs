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
    assert_eq!(matrix["version"], 3);
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
                let true_ordinals = case["source_truth"]["true_ordinals"].as_array().unwrap();
                let not_true_ordinals = case["source_truth"]["not_true_ordinals"]
                    .as_array()
                    .unwrap();
                assert_eq!(true_ordinals.len(), before.rows().len());
                assert!(not_true_ordinals.is_empty());
                assert_eq!(case["after_count"], after.rows().len());
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
                    let frozen_domain = &case["typed_domains"][column.name()];
                    let mut actual = BTreeMap::<String, u64>::new();
                    let mut numbers = Vec::<i64>::new();
                    for (value, n) in domain.values() {
                        let key = match value {
                            LiteralValue::Null => "NULL".to_string(),
                            LiteralValue::Number(number) => {
                                numbers.push(number.parse().unwrap());
                                number.clone()
                            }
                            other => panic!("{id}: unsupported integer domain {other:?}"),
                        };
                        actual.insert(key, *n);
                    }
                    let expected = frozen_domain["frequencies"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_u64().unwrap()))
                        .collect::<BTreeMap<_, _>>();
                    assert_eq!(
                        actual,
                        expected,
                        "{id}/{}: exact frozen typed frequency",
                        column.name()
                    );
                    assert_eq!(
                        actual.contains_key("NULL"),
                        frozen_domain["nullable"].as_bool().unwrap()
                    );
                    assert_eq!(
                        numbers.iter().min().copied(),
                        frozen_domain["lower"].as_i64()
                    );
                    assert_eq!(
                        numbers.iter().max().copied(),
                        frozen_domain["upper"].as_i64()
                    );
                    assert_eq!(frozen_domain["lower_inclusive"], !numbers.is_empty());
                    assert_eq!(frozen_domain["upper_inclusive"], !numbers.is_empty());
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
    assert_eq!(entry["freeze_version"], 3);
    assert_eq!(entry["parent_signed_off"], false);
}

#[test]
fn duckdb_executes_every_frozen_exact_case_and_rejects_schema_contradictions() {
    use duckdb::Connection;
    for case in frozen()["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if matches!(id, "83-C158-N-type" | "83-C158-N-key") {
            // Vendor cast and constraint enforcement is a conditional engine law,
            // not an Exact/Impossible inference from the SQL parser.
            continue;
        }
        let db = Connection::open_in_memory().unwrap();
        let before = case["initial_schema"]
            .as_array()
            .unwrap()
            .iter()
            .map(|column| column.as_str().unwrap())
            .collect::<Vec<_>>();
        db.execute_batch(&format!("CREATE TABLE t ({})", before.join(", ")))
            .unwrap();
        for original_row in case["prestate"].as_array().unwrap() {
            let row = original_row.as_object().unwrap();
            let columns = before
                .iter()
                .map(|declared| declared.split_once(' ').unwrap().0)
                .collect::<Vec<_>>();
            let values = columns
                .iter()
                .map(|name| match &row[*name] {
                    Value::Null => "NULL".to_string(),
                    Value::Number(number) => number.to_string(),
                    other => panic!("{id}: unsupported native SQL value {other:?}"),
                })
                .collect::<Vec<_>>();
            db.execute_batch(&format!(
                "INSERT INTO t ({}) VALUES ({})",
                columns.join(", "),
                values.join(", ")
            ))
            .unwrap();
        }
        let operation = case["sql"].as_str().unwrap();
        if id.starts_with("83-C158-N-") {
            assert!(
                db.execute_batch(operation).is_err(),
                "{id}: native contradiction"
            );
            continue;
        }
        db.execute_batch(operation).unwrap();
        let after_cols = case["expected_schema"]
            .as_array()
            .unwrap()
            .iter()
            .map(|declared| declared.as_str().unwrap().split_once(' ').unwrap().0)
            .collect::<Vec<_>>();
        let mut statement = db
            .prepare(&format!("SELECT {} FROM t", after_cols.join(", ")))
            .unwrap();
        let native = statement
            .query_map([], |row| {
                let mut values = BTreeMap::new();
                for (index, column) in after_cols.iter().enumerate() {
                    let n: Option<i64> = row.get(index)?;
                    let canonical = n.map_or(LiteralValue::Null, |number| {
                        LiteralValue::Number(number.to_string())
                    });
                    values.insert((*column).to_string(), canonical);
                }
                Ok(PhysicalWriteRow::new(values).unwrap())
            })
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(
            sorted_bag(&native),
            sorted_bag(state(&case["poststate"]).rows()),
            "{id}: independently executed native source/target bag"
        );
        let mut columns = db.prepare(
            "SELECT column_name FROM information_schema.columns WHERE table_name='t' ORDER BY ordinal_position"
        ).unwrap();
        let actual_names = columns
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(
            actual_names, after_cols,
            "{id}: native post-schema ordinal identity"
        );
    }
}
