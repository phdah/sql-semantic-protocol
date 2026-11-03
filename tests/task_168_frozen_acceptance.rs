//! TASK-168 immutable acceptance inventory with independent native DuckDB bags.
//! Run all frozen cells through the public API, not private interpreter helpers.
mod common;
use common::DIALECTS;
use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_terminal_bag_plan,
    physical_joint_tuple_histogram_plan, physical_single_tuple_histogram_plan,
    to_physical_joint_terminal_json, AnalysisBundle, ConfiguredSqlInput, ConstraintValue,
    LiteralValue, OutcomeGoal, OutputDistribution, OutputValueCount, PhysicalJointOtherExact,
    PhysicalJointTerminalProof, PhysicalSingleTupleExact, PhysicalSingleTupleProof,
    PhysicalTupleBucket, PhysicalTupleHistogramGoal, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-168-acceptance-freeze.json")).unwrap()
}
fn catalog() -> RelationCatalog {
    let schemas = ["l", "r"]
        .into_iter()
        .map(|name| {
            RelationSchema::new(
                name,
                [("id", "VARCHAR"), ("k", "INTEGER"), ("v", "INTEGER")]
                    .into_iter()
                    .map(|(column, kind)| {
                        SchemaColumn::from_sql_type(column, kind, "postgresql").unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    RelationCatalog::from_schemas(&schemas).unwrap()
}
fn analyze(case: &Value, dialect: &str) -> Result<AnalysisBundle, String> {
    let sql = case["sql"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| SqlInput::inline(value.as_str().unwrap()))
        .collect::<Vec<_>>();
    let parser = dialect_from_name(dialect).unwrap();
    let configured = sql
        .iter()
        .enumerate()
        .map(|(n, source)| {
            ConfiguredSqlInput::new(
                if n == 0 && sql.len() == 2 {
                    "task168-group"
                } else {
                    "task168-second"
                },
                source,
                dialect,
                parser.as_ref(),
            )
        })
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog())
        .map_err(|err| format!("{err:?}"))
}
fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    case["source_bags"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(relation, rows)| {
            let rows = rows
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let fields = row
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(col, value)| {
                            let scalar = match col.as_str() {
                                "id" => LiteralValue::Text(value.as_str().unwrap().into()),
                                _ => value.as_i64().map_or(LiteralValue::Null, |n| {
                                    LiteralValue::Number(n.to_string())
                                }),
                            };
                            (col.to_string(), scalar)
                        })
                        .collect();
                    PhysicalWriteRow::new(fields).unwrap()
                })
                .collect();
            PhysicalWriteState::new(relation.as_str(), rows).unwrap()
        })
        .collect()
}
fn typed(value: &Value) -> ConstraintValue {
    if value.is_null() {
        ConstraintValue::Null
    } else if let Some(i) = value.as_i64() {
        ConstraintValue::Integer(i)
    } else {
        ConstraintValue::String(value.as_str().unwrap().into())
    }
}
fn tuple_goal(case: &Value, layer: &str) -> PhysicalTupleHistogramGoal {
    PhysicalTupleHistogramGoal::new(
        layer,
        case["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect(),
        case["tuple_histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                PhysicalTupleBucket::new(
                    b["values"].as_array().unwrap().iter().map(typed).collect(),
                    b["rows"].as_u64().unwrap(),
                )
            })
            .collect(),
    )
    .unwrap()
}
fn scalar_histogram(name: &str, values: &[(i64, u64)]) -> OutputDistribution {
    OutputDistribution::new(
        name,
        values
            .iter()
            .map(|(v, c)| OutputValueCount::new(ConstraintValue::Integer(*v), *c))
            .collect(),
    )
    .unwrap()
}
fn joint_goals(bundle: &AnalysisBundle, case: &Value) -> Vec<OutcomeGoal> {
    let family = case["family"].as_str().unwrap();
    let second = if family == "window" {
        OutcomeGoal::new(
            bundle.layers()[1].id(),
            Some(3),
            None,
            vec![scalar_histogram("copies", &[(1, 1), (2, 2)])],
        )
        .unwrap()
    } else {
        OutcomeGoal::new(
            bundle.layers()[1].id(),
            Some(3),
            None,
            vec![scalar_histogram("v", &[(10, 2), (20, 1)])],
        )
        .unwrap()
    };
    vec![
        OutcomeGoal::new(
            bundle.layers()[0].id(),
            Some(1),
            Some(1),
            vec![scalar_histogram("n", &[(2, 1)])],
        )
        .unwrap(),
        second,
    ]
}
fn scalar(row: &LiteralValue) -> Value {
    match row {
        LiteralValue::Null => Value::Null,
        LiteralValue::Number(n) => json!(n.parse::<i64>().unwrap()),
        LiteralValue::Text(v) => json!(v),
        LiteralValue::Boolean(v) => json!(v),
        _ => panic!("an uncertified literal cannot be a native numeric tuple"),
    }
}
fn normalized(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut bag = rows
        .iter()
        .map(|row| {
            let json = row
                .iter()
                .map(|(k, v)| (k.to_owned(), scalar(v)))
                .collect::<serde_json::Map<_, _>>();
            Value::Object(json).to_string()
        })
        .collect::<Vec<_>>();
    bag.sort();
    bag
}
fn expected(case: &Value, key: &str) -> Vec<String> {
    let mut bag = case[key]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>();
    bag.sort();
    bag
}
fn assert_verdict(case: &Value, result: &str, dialect: &str) {
    assert_eq!(
        result,
        case["expectation"].as_str().unwrap(),
        "{} / {dialect}: frozen proof strength diverged",
        case["id"]
    );
}
fn check_exact_value_domain(domain: &ValueDomain, expected: &Value, case_id: &str, column: &str) {
    let ValueDomain::Set(set) = domain else {
        panic!("{case_id}/{column}: no exact finite typed domain: {domain:?}");
    };
    assert_eq!(set.mode(), SetMode::Include);
    let actual = set
        .values()
        .iter()
        .filter_map(|entry| match entry.value() {
            LiteralValue::Number(v) => Some(v.parse::<i64>().unwrap()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let frozen = expected["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual, frozen,
        "{case_id}/{column}: strongest exact integer domain"
    );
    assert_eq!(
        domain.admits_null(),
        Some(expected["null_allowed"].as_bool().unwrap())
    );
    let min = actual.iter().next().copied();
    let max = actual.iter().next_back().copied();
    assert_eq!(min.map_or(Value::Null, |v| json!(v)), expected["lower"]);
    assert_eq!(max.map_or(Value::Null, |v| json!(v)), expected["upper"]);
    assert_eq!(expected["lower_inclusive"], true);
    assert_eq!(expected["upper_inclusive"], true);
}
fn check_domains(domains: &BTreeMap<String, ValueDomain>, expected: &Value, id: &str) {
    assert_eq!(domains.len(), expected.as_object().unwrap().len());
    for (name, frozen) in expected.as_object().unwrap() {
        check_exact_value_domain(&domains[name], frozen, id, name);
    }
}
fn check_set_domains(
    exact: &sql_semantic_protocol::PhysicalFiniteSetTreeExact,
    expected: &Value,
    id: &str,
) {
    assert_eq!(exact.terminal_domains.len(), 2);
    for (idx, name) in ["k", "v"].into_iter().enumerate() {
        let actual = &exact.terminal_domains[idx];
        let frozen = &expected[name];
        let observed = actual
            .values
            .iter()
            .map(|v| v.to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let wanted = frozen["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(observed, wanted, "{id}/{name}: exact finite set values");
        assert_eq!(
            actual.contains_null,
            frozen["null_allowed"].as_bool().unwrap()
        );
        let bounds = actual.bounds();
        assert_eq!(bounds.map(|(a, _)| json!(a)), Some(frozen["lower"].clone()));
        assert_eq!(bounds.map(|(_, b)| json!(b)), Some(frozen["upper"].clone()));
        assert_eq!(frozen["lower_inclusive"], true);
        assert_eq!(frozen["upper_inclusive"], true);
    }
}
fn joint_case(case: &Value, dialect: &str, bundle: &AnalysisBundle) {
    let states = snapshots(case);
    let goals = joint_goals(bundle, case);
    let request = tuple_goal(case, bundle.layers()[1].id());
    let result = physical_joint_tuple_histogram_plan(bundle, &goals, &states, &[request]);
    let verdict = match &result {
        PhysicalJointTerminalProof::Exact(_) => "Exact",
        PhysicalJointTerminalProof::Impossible { .. } => "Impossible",
        PhysicalJointTerminalProof::Residual { .. } => "Residual",
    };
    assert_verdict(case, verdict, dialect);
    let claim: Value =
        serde_json::from_str(&to_physical_joint_terminal_json(bundle, &result)).unwrap();
    assert_eq!(claim["evidence"]["status"], verdict.to_ascii_lowercase());
    if verdict != "Exact" {
        assert!(
            claim["evidence"].get("sources").is_none(),
            "unproved tuple goal must not emit a fabricated exact physical population"
        );
    }

    // Independent complete source and terminal fixtures remain checked even
    // when an impossible/requested tuple cannot be emitted as an exact proof.
    let original = physical_joint_terminal_bag_plan(bundle, &goals, &states);
    let PhysicalJointTerminalProof::Exact(exact) = &original else {
        panic!(
            "{} / {dialect}: frozen source must have exact base bag: {original:?}",
            case["id"]
        );
    };
    assert_eq!(
        normalized(&exact.grouped.terminal),
        expected(case, "grouped_terminal")
    );
    check_domains(
        &exact.grouped.terminal_domains,
        &case["grouped_terminal_domains"],
        case["id"].as_str().unwrap(),
    );
    assert_eq!(
        json!(exact.grouped.selected),
        case["physical_truth_partitions"]["join_true_pairs"]
    );
    assert_eq!(
        json!(exact.grouped.rejected),
        case["physical_truth_partitions"]["join_not_true_pairs"]
    );
    let second = match &exact.other {
        PhysicalJointOtherExact::Window(window) => {
            assert_eq!(
                window.source_row_ids.len(),
                case["source_bags"]["l"].as_array().unwrap().len()
            );
            check_domains(
                &window.terminal_domains,
                &case["native_terminal_domains"],
                case["id"].as_str().unwrap(),
            );
            &window.terminal
        }
        PhysicalJointOtherExact::Set(set) => {
            check_set_domains(
                set,
                &case["native_terminal_domains"],
                case["id"].as_str().unwrap(),
            );
            &set.terminal
        }
    };
    assert_eq!(normalized(second), expected(case, "second_terminal"));
    let sidecar: Value =
        serde_json::from_str(&to_physical_joint_terminal_json(bundle, &original)).unwrap();
    assert_eq!(sidecar["evidence"]["status"], "exact");
    assert_eq!(sidecar["evidence"]["sources"].as_array().unwrap().len(), 2);
    assert_eq!(
        sidecar["evidence"]["grouped"]["selected"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(sidecar["evidence"]["grouped"]["domains"]["n"].is_object());
    assert_eq!(
        sidecar["evidence"]["secondary"]["proof"]["terminal"]
            .as_array()
            .unwrap()
            .len(),
        second.len()
    );
}
fn single_case(case: &Value, dialect: &str, bundle: &AnalysisBundle) {
    let goal = tuple_goal(case, bundle.layers().last().unwrap().id());
    let states = snapshots(case);
    let result = physical_single_tuple_histogram_plan(bundle, &states, &goal);
    let verdict = match &result {
        PhysicalSingleTupleProof::Exact(_) => "Exact",
        PhysicalSingleTupleProof::Impossible { .. } => "Impossible",
        PhysicalSingleTupleProof::Residual { .. } => "Residual",
    };
    assert_verdict(case, verdict, dialect);
    if let PhysicalSingleTupleProof::Exact(exact) = result {
        match exact {
            PhysicalSingleTupleExact::Page(page) => {
                check_domains(
                    &page.terminal_domains,
                    &case["native_terminal_domains"],
                    case["id"].as_str().unwrap(),
                );
                assert_eq!(normalized(&page.terminal), expected(case, "terminal"));
                assert_eq!(page.rows.len(), 3);
                assert_eq!(page.rows.iter().filter(|r| r.survives).count(), 2);
                assert_eq!(page.terminal_contributors.len(), 2);
                assert!(format!("{:?}", page.terminal_domains["v"]).contains("10"));
            }
            other => panic!("frozen LIMIT should be page, not {other:?}"),
        }
    }
}

#[test]
fn versioned_inventory_covers_every_frozen_family_and_named_dialect() {
    let matrix = freeze();
    assert_eq!(matrix["owner"], "TASK-168");
    assert_eq!(matrix["version"], 2);
    assert_eq!(
        matrix["freeze_history"],
        json!(["docs/task-168-acceptance-freeze-v1.json"])
    );
    let v1: Value =
        serde_json::from_str(include_str!("../docs/task-168-acceptance-freeze-v1.json")).unwrap();
    assert!(v1["frozen_before_semantic_changes"].as_bool().unwrap());
    assert!(!matrix["frozen_before_semantic_changes"].as_bool().unwrap());
    assert_eq!(matrix["fixtures"], json!(["85-C168-P", "85-C168-N"]));
    let cases = matrix["matrix"].as_array().unwrap();
    assert_eq!(cases.len(), 7);
    assert_eq!(cases.iter().filter(|v| v["required"] == true).count(), 6);
    let families = cases
        .iter()
        .map(|v| v["family"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        families,
        [
            "set",
            "window",
            "order_limit_fanout",
            "qualify_window_fanout",
            "unproved_comparison_law"
        ]
        .into_iter()
        .collect()
    );
    for case in cases {
        assert!(case["id"].as_str().unwrap().starts_with("85-C168-"));
        assert_eq!(case["external_boundary"]["owner"],"TASK-168");
        assert!(case["external_boundary"]["needed"].as_str().unwrap().contains("bag"));
        assert!(case["external_boundary"]["disposition"].as_str().unwrap().contains("TASK-91"));
        assert!(case["source_bags"].is_object());
        assert_eq!(case["columns"].as_array().unwrap().len(), 2);
        assert!(case["tuple_histogram"].is_array());
        assert_eq!(case["dialect_cells"].as_array().unwrap().len(), 13);
        let ids = case["dialect_cells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["dialect"].as_str().unwrap())
            .collect::<Vec<_>>();
        let canonical = DIALECTS
            .iter()
            .copied()
            .filter(|v| *v != "postgres")
            .collect::<Vec<_>>();
        assert_eq!(ids, canonical);
        for cell in case["dialect_cells"].as_array().unwrap() {
            assert_eq!(cell["owner"],"TASK-168");
            assert_eq!(cell["classification"],"canonical_parser_equivalent");
            assert!(cell["release_disposition"].as_str().unwrap().contains("TASK-91"));
            assert!(cell["required_external_evidence"].as_str().unwrap().contains("bag"));
        }
        for cell in case["adapter_cells"].as_array().unwrap() {
            assert!(["direct_sql", "compiled_dbt_catalog", "odcs"]
                .contains(&cell["adapter"].as_str().unwrap()));
            assert_eq!(cell["owner"],"TASK-168");
            assert!(!cell["required_fact"].as_str().unwrap().is_empty());
            assert!(!cell["evidence_test"].as_str().unwrap().is_empty());
        }
    }
}

#[test]
fn canonical_tuple_proof_follows_frozen_seven_case_dialect_matrix() {
    let matrix = freeze();
    for case in matrix["matrix"].as_array().unwrap() {
        for cell in case["dialect_cells"].as_array().unwrap() {
            let dialect = cell["dialect"].as_str().unwrap();
            match analyze(case, dialect) {
                Ok(bundle) => {
                    assert_eq!(
                        cell["classification"], "canonical_parser_equivalent",
                        "{dialect}: frozen parser classification"
                    );
                    if case["sql"].as_array().unwrap().len() == 2 {
                        joint_case(case, dialect, &bundle);
                    } else {
                        single_case(case, dialect, &bundle);
                    }
                }
                Err(err) => {
                    assert_eq!(
                        cell["classification"], "parser_boundary",
                        "{dialect} {}: {err}",
                        case["id"]
                    );
                }
            }
        }
    }
}

fn native_json_rows(db: &Connection, sql: &str) -> Vec<String> {
    let mut statement = db.prepare(sql).unwrap();
    let mut rows = statement
        .query_map([], |row| {
            let mut map = serde_json::Map::new();
            for index in 0..row.as_ref().column_count() {
                let name = row.as_ref().column_name(index)?.to_owned();
                let value: Option<i64> = row.get(index)?;
                map.insert(name, value.map_or(Value::Null, |v| json!(v)));
            }
            Ok(Value::Object(map).to_string())
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    rows.sort();
    rows
}

#[test]
fn independent_duckdb_executes_each_frozen_physical_source_and_output_bag() {
    let matrix = freeze();
    for case in matrix["matrix"].as_array().unwrap() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE l(id VARCHAR,k INTEGER,v INTEGER); CREATE TABLE r(id VARCHAR,k INTEGER,v INTEGER);").unwrap();
        for (relation, rows) in case["source_bags"].as_object().unwrap() {
            for source in rows.as_array().unwrap() {
                let k = source["k"]
                    .as_i64()
                    .map_or("NULL".to_owned(), |v| v.to_string());
                let v = source["v"].as_i64().unwrap();
                let id = source["id"].as_str().unwrap();
                db.execute_batch(&format!("INSERT INTO {relation} VALUES ('{id}',{k},{v});"))
                    .unwrap();
            }
            let observed = db
                .prepare(&format!("SELECT id,k,v FROM {relation} ORDER BY id"))
                .unwrap()
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let k: Option<i64> = row.get(1)?;
                    let v: i64 = row.get(2)?;
                    Ok(json!({"id":id,"k":k,"v":v}).to_string())
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let mut frozen = rows
                .as_array()
                .unwrap()
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>();
            frozen.sort();
            assert_eq!(
                observed, frozen,
                "{}: native source rows must match the frozen physical bag",
                case["id"]
            );
        }
        let queries = case["sql"].as_array().unwrap();
        if queries.len() == 2 {
            assert_eq!(
                native_json_rows(&db, queries[0].as_str().unwrap()),
                expected(case, "grouped_terminal")
            );
            assert_eq!(
                native_json_rows(&db, queries[1].as_str().unwrap()),
                expected(case, "second_terminal")
            );
        } else if case["family"] == "order_limit_fanout" {
            assert_eq!(
                native_json_rows(&db, queries[0].as_str().unwrap()),
                expected(case, "terminal")
            );
            let selected = db
                .prepare("SELECT id FROM l ORDER BY v LIMIT 2")
                .unwrap()
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(selected, vec!["l0".to_string(), "l1".to_string()]);
            assert_eq!(
                case["physical_truth_partitions"]["selected_source_row_ids"],
                json!(["l#0", "l#1"])
            );
            assert_eq!(
                case["physical_truth_partitions"]["not_true_source_row_ids"],
                json!(["l#2"])
            );
        } else {
            // Native DuckDB gives an independently fixed output even if the
            // portable protocol must return Residual for implicit tie/NULL laws.
            assert_eq!(
                native_json_rows(&db, queries[0].as_str().unwrap()),
                vec![
                    r#"{"k":1,"v":10}"#.to_string(),
                    r#"{"k":null,"v":30}"#.to_string()
                ]
            );
            let native_ranks = db.prepare(
                "SELECT id, ROW_NUMBER() OVER (PARTITION BY k ORDER BY v) AS rn FROM l ORDER BY id"
            ).unwrap().query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?)))
                .unwrap().collect::<Result<Vec<_>,_>>().unwrap();
            let selected = native_ranks
                .iter()
                .filter(|(_, rank)| *rank == 1)
                .map(|(id, _)| id.to_owned())
                .collect::<Vec<_>>();
            let rejected = native_ranks
                .iter()
                .filter(|(_, rank)| *rank != 1)
                .map(|(id, _)| id.to_owned())
                .collect::<Vec<_>>();
            assert_eq!(selected, vec!["l0".to_string(), "l2".to_string()]);
            assert_eq!(rejected, vec!["l1".to_string()]);
            assert_eq!(
                case["physical_truth_partitions"]["duckdb_selected_source_row_ids"],
                json!(selected)
            );
            assert_eq!(
                case["physical_truth_partitions"]["duckdb_rejected_source_row_ids"],
                json!(rejected)
            );
        }
    }
}
