//! TASK-149 reviewed v2 frozen cell executable acceptance gates.
//! Every full target bag, SQL TRUE/IS NOT TRUE row ordinal and typed domain is
//! independently asserted against native DuckDB, not inferred from protocol.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name,
    physical_ordered_write_state_plan, ColumnValueKind, ColumnValueSource, ConfiguredSqlInput,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    ConstraintValue, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationConstraint, RelationConstraintSet, RelationSchema,
    SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-149-acceptance-freeze.json")).unwrap()
}

fn scalar(n: &Value) -> LiteralValue {
    if n.is_null() {
        LiteralValue::Null
    } else {
        LiteralValue::Number(n.as_i64().expect("INTEGER").to_string())
    }
}

fn row(value: &Value) -> PhysicalWriteRow {
    let m = value.as_object().expect("complete row");
    PhysicalWriteRow::new(m.iter().map(|(k, v)| (k.clone(), scalar(v))).collect()).unwrap()
}

fn state(rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "target",
        rows.as_array()
            .expect("complete bag")
            .iter()
            .map(row)
            .collect(),
    )
    .unwrap()
}

fn schema() -> RelationSchema {
    RelationSchema::new(
        "target",
        ["id", "score"]
            .into_iter()
            .map(|col| SchemaColumn::from_sql_type(col, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap()
}

fn enforced() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task149-freeze")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn bundle(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task149-freeze", &sql, "duckdb", dialect.as_ref());
    let catalog = RelationCatalog::from_schemas(&[schema()]).unwrap();
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap();
    match case["family"].as_str().unwrap() {
        "update_default" => {
            let source = ColumnValueSource::new(
                "score",
                ColumnValueKind::Default,
                Some(ConstraintValue::Integer(7)),
                enforced(),
            )
            .unwrap();
            bundle.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
                .unwrap()
                .with_column_value_sources(vec![source])]);
        }
        "enforced_primary_key_conflict" => {
            bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
                "target",
                vec![RelationConstraint::primary_key(vec!["id".into()], enforced()).unwrap()],
            )
            .unwrap()]);
        }
        _ => {}
    }
    bundle
}

fn native_bag(db: &Connection) -> PhysicalWriteState {
    let mut statement = db
        .prepare("SELECT id,score FROM target ORDER BY rowid")
        .unwrap();
    let rows = statement
        .query_map([], |r| {
            Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".into(),
                    id.map_or(LiteralValue::Null, |i| LiteralValue::Number(i.to_string())),
                ),
                ("score".into(), LiteralValue::Number(score.to_string())),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new("target", rows).unwrap()
}

fn native(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    let def = if case["family"] == "enforced_primary_key_conflict" {
        "CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER DEFAULT 7);"
    } else {
        "CREATE TABLE target(id INTEGER,score INTEGER DEFAULT 7);"
    };
    db.execute_batch(def).unwrap();
    for row in case["target_before"].as_array().unwrap() {
        db.execute(
            "INSERT INTO target(id,score) VALUES (?1,?2)",
            duckdb::params![row["id"].as_i64(), row["score"].as_i64().unwrap()],
        )
        .unwrap();
    }
    db
}

fn selected_ordinals(db: &Connection, selection: &str, reject: bool) -> Vec<String> {
    let filter = if reject {
        format!("({selection}) IS NOT TRUE")
    } else {
        format!("({selection}) IS TRUE")
    };
    let sql = format!("SELECT rowid FROM target WHERE {filter} ORDER BY rowid");
    let mut stmt = db.prepare(&sql).unwrap();
    let ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .map(|id| format!("target_before#{id}"))
        .collect();
    ids
}

fn returned(db: &Connection, case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let columns = case["returning"].as_array().expect("RETURNING rows");
    let sql = case["sql"].as_str().unwrap();
    let mut stmt = db.prepare(sql).unwrap();
    let mut cursor = stmt.query([]).unwrap();
    let mut values = Vec::new();
    while let Some(row) = cursor.next().unwrap() {
        let mut map = BTreeMap::new();
        // Every TASK-149 RETURNING fixture projects id,score in this order.
        let id = row.get::<_, Option<i64>>(0).unwrap();
        let score = row.get::<_, i64>(1).unwrap();
        map.insert(
            "id".into(),
            id.map_or(LiteralValue::Null, |i| LiteralValue::Number(i.to_string())),
        );
        map.insert("score".into(), LiteralValue::Number(score.to_string()));
        values.push(map);
    }
    assert_eq!(values.len(), columns.len());
    values
}

fn freq(rows: &[PhysicalWriteRow], column: &str) -> BTreeMap<Option<i64>, usize> {
    let mut values = BTreeMap::new();
    for row in rows {
        let lit = row.values().get(column).unwrap();
        let number = match lit {
            LiteralValue::Null => None,
            LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
            _ => panic!("non-integer"),
        };
        *values.entry(number).or_insert(0) += 1;
    }
    values
}

fn domain_from_freeze(case: &Value, column: &str) -> BTreeSet<Option<i64>> {
    case["typed_domains"][column]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::as_i64)
        .collect()
}

#[test]
fn every_frozen_physical_cell_matches_native_complete_state_and_truth_partition() {
    let freeze = freeze();
    assert_eq!(freeze["version"], 2);
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 13);
    let mut verified = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let b = bundle(case);
        let before = state(&case["target_before"]);
        let plan = physical_ordered_write_state_plan(&b, std::slice::from_ref(&before));
        let db = native(case);
        assert_eq!(native_bag(&db), before, "{id}: independent full input bag");
        if let Some(selection) = case["selection_sql"].as_str() {
            if case["sql_true_row_ids"].is_array() {
                let trues = selected_ordinals(&db, selection, false);
                let rejects = selected_ordinals(&db, selection, true);
                let from_freeze = |key: &str| {
                    case[key]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    trues,
                    from_freeze("sql_true_row_ids"),
                    "{id}: SQL TRUE identities"
                );
                assert_eq!(
                    rejects,
                    from_freeze("sql_not_true_row_ids"),
                    "{id}: SQL NOT TRUE identities"
                );
                assert_eq!(
                    trues.len() + rejects.len(),
                    before.rows().len(),
                    "{id}: complete partition"
                );
            }
        }
        match case["proof"].as_str().unwrap() {
            "residual_missing_attestation" => {
                assert!(
                    matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
                    "{id}: {plan:?}"
                );
                assert!(case["conditional_disposition"]
                    .as_str()
                    .unwrap()
                    .contains("TASK-149"));
            }
            "impossible" => {
                assert!(
                    matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
                    "{id}: {plan:?}"
                );
                assert!(
                    db.execute_batch(case["sql"].as_str().unwrap()).is_err(),
                    "{id}: native contradiction"
                );
                assert_eq!(
                    native_bag(&db),
                    before,
                    "{id}: atomic native failure must not mutate bag"
                );
            }
            "exact" | "exact_not_true" | "exact_with_attested_default" => {
                let PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                } = plan
                else {
                    panic!("{id}: {plan:?}")
                };
                assert_eq!(transitions.len(), 1, "{id}: one mutation layer");
                let transition = &transitions[0];
                let expected = state(&case["target_after"]);
                assert_eq!(transition.before(), &before, "{id}: physical before");
                assert_eq!(
                    transition.after(),
                    &expected,
                    "{id}: complete ordered physical after"
                );
                assert_eq!(
                    final_states.as_slice(),
                    std::slice::from_ref(&expected),
                    "{id}: final target"
                );
                assert!(
                    transition.intermediate_states().is_empty(),
                    "{id}: no invented producers"
                );
                assert_eq!(
                    transition.affected().updated(),
                    case["affected"]["updated"].as_u64().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    transition.affected().deleted(),
                    case["affected"]["deleted"].as_u64().unwrap(),
                    "{id}"
                );
                assert_eq!(transition.affected().inserted(), 0, "{id}");
                let after_rows = transition.after().rows();
                if case["typed_domains"].is_object() {
                    let domains = transition.after().exact_domains(&schema()).unwrap();
                    assert_eq!(domains.len(), 2);
                    for domain in &domains {
                        let name = domain.column();
                        let actual = freq(after_rows, name);
                        assert_eq!(
                            actual.keys().copied().collect::<BTreeSet<_>>(),
                            domain_from_freeze(case, name),
                            "{id} {name}: typed set"
                        );
                        assert_eq!(
                            domain.values().iter().map(|(_, count)| *count).sum::<u64>(),
                            after_rows.len() as u64,
                            "{id} {name}: multiplicity"
                        );
                        let proof_counts: BTreeMap<Option<i64>, usize> = domain.values().iter()
                            .map(|(lit, count)| {
                                let number = match lit {
                                    LiteralValue::Null => None,
                                    LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
                                    _ => panic!("{id}: unexpected noninteger domain")
                                };
                                (number,usize::try_from(*count).unwrap())
                            }).collect();
                        assert_eq!(proof_counts,actual,"{id} {name}: exact protocol value frequencies");
                        let frozen_counts: BTreeMap<Option<i64>, usize> = case["typed_domains"][name]["frequencies"]
                            .as_array().unwrap().iter()
                            .map(|pair|(pair["value"].as_i64(),usize::try_from(pair["count"].as_u64().unwrap()).unwrap()))
                            .collect();
                        assert_eq!(frozen_counts,actual,"{id} {name}: independent expected frequencies");
                        let nonnull = actual.keys().flatten().copied().collect::<Vec<_>>();
                        if !nonnull.is_empty() {
                            let lower = *nonnull.iter().min().unwrap();
                            let upper = *nonnull.iter().max().unwrap();
                            assert_eq!(
                                Some(lower),
                                case["typed_domains"][name]["lower"].as_i64(),
                                "{id} {name}: inclusive lower"
                            );
                            assert_eq!(
                                Some(upper),
                                case["typed_domains"][name]["upper"].as_i64(),
                                "{id} {name}: inclusive upper"
                            );
                            assert_eq!(case["typed_domains"][name]["lower_inclusive"], true);
                            assert_eq!(case["typed_domains"][name]["upper_inclusive"], true);
                        }
                        assert_eq!(
                            actual.contains_key(&None),
                            case["typed_domains"][name]["nullable"].as_bool().unwrap(),
                            "{id} {name}: NULL"
                        );
                    }
                }
                if case["returning"].is_array() {
                    let actual = transition.returning().expect("RETURNING is present");
                    let expect = case["returning"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(row)
                        .collect::<Vec<_>>();
                    assert_eq!(actual.rows(), expect, "{id}: complete RETURNING bag");
                    let domains = transition.returning_exact_domains(&schema()).unwrap();
                    assert_eq!(domains.len(), 2, "{id}: exact returned-column types");
                    let native_returned = returned(&db, case);
                    let actual_values = actual
                        .rows()
                        .iter()
                        .map(|r| r.values().clone())
                        .collect::<Vec<_>>();
                    assert_eq!(
                        actual_values, native_returned,
                        "{id}: native RETURNING rows"
                    );
                } else {
                    assert!(
                        transition.returning().is_none(),
                        "{id}: no silent RETURNING"
                    );
                    db.execute_batch(case["sql"].as_str().unwrap()).unwrap();
                }
                assert_eq!(
                    native_bag(&db),
                    expected,
                    "{id}: independent full native final bag"
                );
                let affected_expected = case["affected_row_images"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(row)
                    .collect::<Vec<_>>();
                let selected = case["sql_true_row_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .unwrap()
                            .split('#')
                            .next_back()
                            .unwrap()
                            .parse::<usize>()
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let actual_affected = selected
                    .iter()
                    .map(|i| {
                        if case["family"].as_str().unwrap().starts_with("delete")
                            || case["family"] == "empty_returning"
                        {
                            before.rows()[*i].clone()
                        } else {
                            expected.rows()[*i].clone()
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual_affected, affected_expected,
                    "{id}: canonical physical row identity / affected image"
                );
                verified += 1;
            }
            other => panic!("{id}: unknown proof {other}"),
        }
    }
    assert_eq!(verified, 11);
}

#[test]
fn every_frozen_parser_cell_requires_a_verified_explicit_classification() {
    let freeze = freeze();
    let mut observed = Vec::new();
    let mut unverified = Vec::new();
    let mut cells = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let sql = case["sql"].as_str().unwrap();
        let mut row = BTreeMap::new();
        for dialect in freeze["dialects"].as_array().unwrap() {
            let dialect = dialect.as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            let status = match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
                Err(_) => "parser_boundary",
                Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Query(query) if query.write().is_some() => "canonical",
                    ProtocolStatement::Unsupported(_) => "unsupported",
                    _ => "invalid_no_mutation",
                },
            };
            row.insert(dialect.to_owned(), status.to_owned());
            let expected = case["parser_cells"][dialect].as_str().unwrap();
            if expected != status {
                unverified.push(format!(
                    "{}:{dialect} expected={expected} actual={status}",
                    case["id"].as_str().unwrap()
                ));
            }
            cells += 1;
        }
        observed.push(serde_json::json!({"id":case["id"],"parser_cells":row}));
    }
    assert_eq!(cells, 169);
    assert!(
        unverified.is_empty(),
        "TASK149_ACTUAL_PARSER_MATRIX={} mismatches={:?}",
        serde_json::to_string(&observed).unwrap(),
        unverified
    );
}
