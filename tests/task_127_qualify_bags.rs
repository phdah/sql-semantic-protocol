//! TASK-127: independent closed-world SQL and DuckDB QUALIFY conjunction oracles.
//! The frozen joined/grouped cases deliberately remain separate acceptance gates.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_window_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json")).unwrap()
}

fn source(case: &Value) -> PhysicalWriteState {
    let rows = case["physical_source_bags"]["s"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            PhysicalWriteRow::new(
                row.as_object()
                    .unwrap()
                    .iter()
                    .map(|(column, value)| {
                        (
                            column.clone(),
                            value
                                .as_i64()
                                .map(|number| LiteralValue::Number(number.to_string()))
                                .unwrap_or(LiteralValue::Null),
                        )
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new("s", rows).unwrap()
}

fn analyze(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        ["id", "k", "x"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap()])
    .unwrap();
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            case["id"].as_str().unwrap(),
            &sql,
            "duckdb",
            dialect.as_ref(),
        )],
        &catalog,
    )
    .unwrap()
}

fn expected(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["expected_full_terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_object()
                .unwrap()
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        value
                            .as_i64()
                            .map(|number| LiteralValue::Number(number.to_string()))
                            .unwrap_or(LiteralValue::Null),
                    )
                })
                .collect()
        })
        .collect()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut sorted = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    sorted.sort();
    sorted
}

fn duckdb(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(id INTEGER, k INTEGER, x INTEGER);")
        .unwrap();
    for row in case["physical_source_bags"]["s"].as_array().unwrap() {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {}, {});",
            row["id"], row["k"], row["x"]
        ))
        .unwrap();
    }
    let mut physical = db.prepare("SELECT id, k, x FROM s ORDER BY id").unwrap();
    let physical_rows = physical
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        physical_rows,
        case["physical_source_bags"]["s"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (
                row["id"].as_i64().unwrap(),
                row["k"].as_i64().unwrap(),
                row["x"].as_i64().unwrap()
            ))
            .collect::<Vec<_>>(),
        "the native engine must execute the exact frozen source bag"
    );
    let mut stmt = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    stmt.query_map([], |row| {
        let mut fields = BTreeMap::new();
        for index in 0..row.as_ref().column_count() {
            let name = row.as_ref().column_name(index)?;
            let value: i64 = row.get(index)?;
            fields.insert(name.to_string(), LiteralValue::Number(value.to_string()));
        }
        Ok(fields)
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

#[test]
fn selected_and_not_true_rank_conjunctions_match_complete_duckdb_bags() {
    let matrix = freeze();
    assert_eq!(matrix["version"], 1);
    assert_eq!(matrix["owner"], "TASK-127");
    let cases = matrix["cases"].as_array().unwrap();
    for case in cases.iter().filter(|case| {
        matches!(
            case["id"].as_str(),
            Some("74-C127-P1" | "74-C127-P2" | "74-C127-P3" | "74-C127-P6" | "74-C127-N1")
        )
    }) {
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case);
        let target = bundle.layers().last().unwrap().id();
        let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
        let PhysicalFiniteWindowProof::Exact(exact) = proof else {
            panic!("{id}: missing exact row membership proof: {proof:?}");
        };
        assert_eq!(exact.source, "s");
        let ids = |field: &str| {
            case[field]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(exact.source_row_ids, ids("physical_row_identities"), "{id}");
        assert_eq!(
            exact.terminal_row_ids,
            ids("selected_source_row_ids"),
            "{id}: TRUE identities"
        );
        assert_eq!(
            exact.rejected_row_ids,
            ids("rejected_source_row_ids"),
            "{id}: NOT TRUE identities"
        );
        assert_eq!(
            exact.source_row_ids.len(),
            exact.terminal_row_ids.len() + exact.rejected_row_ids.len(),
            "{id}: every source row must be classified"
        );
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected(case)),
            "{id}: protocol bag"
        );
        assert_eq!(bag(&exact.terminal), bag(&duckdb(case)), "{id}: DuckDB bag");
        for (column, domain) in case["typed_terminal_domains"].as_object().unwrap() {
            let bounds = &domain["inclusive_bounds"];
            let actual = exact.terminal_domains.get(column).unwrap();
            if bounds.is_null() {
                assert!(
                    matches!(actual, ValueDomain::Empty),
                    "{id}/{column}: empty domain"
                );
            } else {
                let expected_min = bounds[0].as_i64().unwrap();
                let expected_max = bounds[1].as_i64().unwrap();
                let ValueDomain::Set(values) = actual else {
                    panic!("{id}/{column}: exact finite column domain missing");
                };
                let mut integers = values
                    .values()
                    .iter()
                    .map(|v| match v.value() {
                        LiteralValue::Number(n) => n.parse::<i64>().unwrap(),
                        _ => panic!("{id}/{column}: unexpected non-integer domain"),
                    })
                    .collect::<Vec<_>>();
                integers.sort();
                assert_eq!(
                    integers.first().copied(),
                    Some(expected_min),
                    "{id}/{column}"
                );
                assert_eq!(
                    integers.last().copied(),
                    Some(expected_max),
                    "{id}/{column}"
                );
            }
        }
    }
}

#[test]
fn ambiguous_peer_ties_fail_closed_and_bad_source_is_impossible() {
    let matrix = freeze();
    let peers = matrix["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "74-C127-N2")
        .unwrap();
    let bundle = analyze(peers);
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[source(peers)]),
        PhysicalFiniteWindowProof::Residual { .. }
    )); // Tied ROW_NUMBER identities cannot be chosen from source insertion order.

    let initial = &matrix["cases"][0];
    let bundle = analyze(initial);
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[]),
        PhysicalFiniteWindowProof::Residual { .. }
    ));
    let impossible = PhysicalWriteState::new(
        "s",
        vec![PhysicalWriteRow::new(BTreeMap::from([
            (
                "id".to_string(),
                LiteralValue::Number("2147483648".to_string()),
            ),
            ("k".to_string(), LiteralValue::Number("0".to_string())),
            ("x".to_string(), LiteralValue::Number("3".to_string())),
        ]))
        .unwrap()],
    )
    .unwrap();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[impossible]),
        PhysicalFiniteWindowProof::Impossible { .. }
    ));
}

fn composed(case: &Value) -> PhysicalFiniteWindowProof {
    let schemas = case["source_schema"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(relation, columns)| {
            RelationSchema::new(
                relation,
                columns
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(name, kind)| {
                        SchemaColumn::from_sql_type(name, kind.as_str().unwrap(), "duckdb").unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let parser = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(case["sql"].as_str().unwrap());
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            case["id"].as_str().unwrap(),
            &input,
            "duckdb",
            parser.as_ref(),
        )],
        &catalog,
    )
    .unwrap();
    let states = case["physical_source_bags"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(relation, rows)| {
            PhysicalWriteState::new(
                relation,
                rows.as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        PhysicalWriteRow::new(
                            row.as_object()
                                .unwrap()
                                .iter()
                                .map(|(name, value)| {
                                    (
                                        name.clone(),
                                        if value.is_null() {
                                            LiteralValue::Null
                                        } else {
                                            LiteralValue::Number(value.to_string())
                                        },
                                    )
                                })
                                .collect(),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    physical_finite_window_bag_plan(&bundle, bundle.layers().last().unwrap().id(), &states)
}

#[test]
fn joined_and_grouped_qualify_have_complete_physical_contributors() {
    let matrix = freeze();
    for id in ["74-C127-P4", "74-C127-P5"] {
        let case = matrix["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == id)
            .unwrap();
        let proof = composed(case);
        let PhysicalFiniteWindowProof::Exact(exact) = proof else {
            panic!("{id}: missing upstream source-backed exact proof: {proof:?}");
        };
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected(case)),
            "{id}: terminal bag"
        );
        assert_eq!(
            exact.source_row_ids.len(),
            exact.terminal_row_ids.len() + exact.rejected_row_ids.len(),
            "{id}: every materialized producer row classified",
        );
        for (name, domain) in case["typed_terminal_domains"].as_object().unwrap() {
            let ValueDomain::Set(values) = &exact.terminal_domains[name] else {
                panic!("{id}/{name}: missing finite output domain");
            };
            let mut numbers = values
                .values()
                .iter()
                .map(|value| match value.value() {
                    LiteralValue::Number(number) => number.parse::<i64>().unwrap(),
                    other => panic!("{id}/{name}: unexpected domain literal {other:?}"),
                })
                .collect::<Vec<_>>();
            numbers.sort();
            assert_eq!(
                numbers.first().copied(),
                domain["inclusive_bounds"][0].as_i64()
            );
            assert_eq!(
                numbers.last().copied(),
                domain["inclusive_bounds"][1].as_i64()
            );
        }
        if id == "74-C127-P4" {
            assert_eq!(exact.physical_sources, ["l", "r"]);
            assert_eq!(exact.source_row_ids, ["l#0|r#0", "l#0|r#1", "l#1|r#2"]);
            assert_eq!(exact.terminal_row_ids, ["l#0|r#0", "l#1|r#2"]);
            assert_eq!(exact.rejected_row_ids, ["l#0|r#1"]);
            assert_eq!(exact.rejected_upstream_row_ids.len(), 3);
            assert_eq!(exact.physical_source_row_ids["l"], ["l#0", "l#1"]);
            assert_eq!(exact.physical_source_row_ids["r"], ["r#0", "r#1", "r#2"]);
            assert_eq!(exact.producer_contributors["l#0|r#0"], ["l#0", "r#0"]);
        } else {
            assert_eq!(exact.physical_sources, ["s"]);
            assert_eq!(exact.source_row_ids, ["group(k=0)", "group(k=1)"]);
            assert_eq!(exact.terminal_row_ids, ["group(k=0)"]);
            assert_eq!(exact.rejected_row_ids, ["group(k=1)"]);
            assert!(exact.rejected_upstream_row_ids.is_empty());
            assert_eq!(exact.physical_source_row_ids["s"].len(), 6);
            assert_eq!(
                exact.producer_contributors["group(k=0)"],
                ["s#0", "s#1", "s#2"]
            );
            assert_eq!(
                exact.producer_contributors["group(k=1)"],
                ["s#3", "s#4", "s#5"]
            );
        }
    }
}

#[test]
fn composed_join_underordered_peers_are_not_promoted_to_exact() {
    let matrix = freeze();
    let mut case = matrix["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "74-C127-P4")
        .unwrap()
        .clone();
    case["physical_source_bags"]["r"][1]["x"] = serde_json::json!(3);
    assert!(matches!(
        composed(&case),
        PhysicalFiniteWindowProof::Residual { .. }
    ));
}
