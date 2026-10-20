//! TASK-126: task-owned full physical and terminal navigation/ROWS oracles.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_window_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput,
    ValueDomain,
};
use sqlparser::parser::Parser;
use std::collections::{BTreeMap, BTreeSet};

const DIALECTS: &[&str] = &[
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-126-acceptance-freeze.json")).unwrap()
}

fn literal(v: &Value) -> LiteralValue {
    match v {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        Value::String(s) => LiteralValue::Text(s.to_string()),
        _ => panic!("unexpected physical scalar: {v}"),
    }
}

fn source(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        case["source_bag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                PhysicalWriteRow::new(
                    row.as_object()
                        .unwrap()
                        .iter()
                        .map(|(name, value)| (name.to_string(), literal(value)))
                        .collect(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn catalog() -> RelationCatalog {
    let schema = RelationSchema::new(
        "s",
        [("id", "INTEGER"), ("k", "INTEGER"), ("x", "INTEGER")]
            .into_iter()
            .map(|(name, kind)| SchemaColumn::from_sql_type(name, kind, "postgresql").unwrap())
            .collect(),
    )
    .unwrap();
    RelationCatalog::from_schemas(&[schema]).unwrap()
}

fn analyze(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let input = [ConfiguredSqlInput::new(
        case["id"].as_str().unwrap(),
        &sql,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&input, &catalog()).unwrap()
}

fn expected(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_object()
                .unwrap()
                .iter()
                .map(|(name, value)| (name.to_string(), literal(value)))
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

fn duckdb_oracle(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(id INTEGER, k INTEGER, x INTEGER);")
        .unwrap();
    for row in case["source_bag"].as_array().unwrap() {
        let sql_cell = |v: &Value| v.as_i64().map_or("NULL".to_string(), |n| n.to_string());
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {}, {});",
            sql_cell(&row["id"]),
            sql_cell(&row["k"]),
            sql_cell(&row["x"]),
        ))
        .unwrap();
    }
    let actual_source = db
        .prepare("SELECT id,k,x FROM s ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected_source = case["source_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["id"].as_i64(), row["k"].as_i64(), row["x"].as_i64()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual_source, expected_source,
        "full independent DuckDB physical bag"
    );

    let mut stmt = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    stmt.query_map([], |row| {
        let mut values = BTreeMap::new();
        for i in 0..row.as_ref().column_count() {
            let name = row.as_ref().column_name(i)?;
            let value = row
                .get::<_, Option<i64>>(i)?
                .map(|n| LiteralValue::Number(n.to_string()))
                .unwrap_or(LiteralValue::Null);
            values.insert(name.to_string(), value);
        }
        Ok(values)
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

#[test]
fn full_physical_and_terminal_oracles_are_exact_with_nulls_and_empty_frames() {
    let matrix = freeze();
    assert_eq!(matrix["version"], 1);
    assert_eq!(
        matrix["fixture_ids"],
        serde_json::json!(["74-C126-P", "74-C126-N"])
    );
    for case in matrix["cases"].as_array().unwrap() {
        if case["proof"] != "Exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case, "duckdb");
        let target = bundle.layers().last().unwrap().id();
        let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
        let PhysicalFiniteWindowProof::Exact(exact) = proof else {
            panic!("{id}: complete bag was not proven: {proof:?}");
        };
        assert_eq!(exact.source, "s", "{id}");
        assert_eq!(
            exact.source_row_ids,
            case["source_row_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            "{id}: no physical source identity lost"
        );
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected(case)),
            "{id}: canonical terminal"
        );
        assert_eq!(
            bag(&duckdb_oracle(case)),
            bag(&expected(case)),
            "{id}: independent DuckDB terminal"
        );
        for domain in case["typed_terminal_domains"].as_array().unwrap() {
            let column = domain["column"].as_str().unwrap();
            let Some(ValueDomain::Set(actual)) = exact.terminal_domains.get(column) else {
                panic!("{id}/{column}: exact finite domain missing");
            };
            assert_eq!(actual.mode(), SetMode::Include);
            let observed = actual
                .values()
                .iter()
                .map(|v| format!("{:?}", v.value()))
                .collect::<BTreeSet<_>>();
            let expected = domain["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(literal)
                .map(|v| format!("{v:?}"))
                .collect::<BTreeSet<_>>();
            assert_eq!(observed, expected, "{id}/{column}: exact domain");
            assert_eq!(observed.contains("Null"), domain["contains_null"] == true);
            let ints = actual
                .values()
                .iter()
                .filter_map(|v| match v.value() {
                    LiteralValue::Number(number) => number.parse::<i128>().ok(),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !ints.is_empty() {
                assert_eq!(
                    serde_json::json!({
                        "lower": ints.iter().min().unwrap(),
                        "upper": ints.iter().max().unwrap(),
                        "lower_inclusive": true,
                        "upper_inclusive": true
                    }),
                    domain["inclusive_bounds"],
                    "{id}/{column}: closed inclusive bounds"
                );
            }
        }
    }
}

#[test]
fn all_dialects_keep_the_same_exact_portable_results_or_a_named_parser_boundary() {
    let matrix = freeze();
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for case in matrix["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let sql = case["sql"].as_str().unwrap();
            let expected_parser = case["parser_status_by_dialect"][*dialect]
                .as_str()
                .unwrap_or_else(|| panic!("{dialect}/{id}: missing frozen parser status"));
            match (Parser::parse_sql(parser.as_ref(), sql), expected_parser) {
                (Ok(_), "parsed") => {}
                (Err(_), "parser_boundary") if case["proof"] == "Residual" => continue,
                (Err(error), "parsed") => {
                    panic!("{dialect}/{id}: expected parsed, got parser error: {error}")
                }
                (Ok(_), "parser_boundary") => {
                    panic!("{dialect}/{id}: parser success changes frozen disposition")
                }
                (_, status) => panic!("{dialect}/{id}: invalid parser disposition {status}"),
            }
            let bundle = analyze(case, dialect);
            let target = bundle.layers().last().unwrap().id();
            let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
            if case["proof"] == "Exact" {
                let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                    panic!("{dialect}/{id}: mandatory proof not Exact: {proof:?}");
                };
                assert_eq!(bag(&exact.terminal), bag(&expected(case)), "{dialect}/{id}");
            } else {
                assert!(
                    matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                    "{dialect}/{id}: conditional law was overclaimed: {proof:?}"
                );
            }
        }
    }
}

#[test]
fn invalid_complete_source_is_impossible_and_missing_source_is_residual() {
    let case = &freeze()["cases"][0];
    let bundle = analyze(case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[]),
        PhysicalFiniteWindowProof::Residual { .. }
    ));
    let invalid = PhysicalWriteState::new(
        "s",
        vec![PhysicalWriteRow::new(BTreeMap::from([
            (
                "id".to_string(),
                LiteralValue::Number("2147483648".to_string()),
            ),
            ("k".to_string(), LiteralValue::Number("0".to_string())),
            ("x".to_string(), LiteralValue::Number("1".to_string())),
        ]))
        .unwrap()],
    )
    .unwrap();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[invalid]),
        PhysicalFiniteWindowProof::Impossible { .. }
    ));
}

#[test]
fn frozen_acceptance_matrix_has_no_implicit_parser_or_adapter_exclusions() {
    let matrix = freeze();
    assert_eq!(matrix["owner"], "TASK-126");
    assert_eq!(
        matrix["original_parent_ac_map"],
        serde_json::json!([1, 2, 4, 5, 6])
    );
    assert_eq!(
        matrix["required_adapters"],
        serde_json::json!(["direct_sql", "dbt_compiled", "odcs_schema"])
    );
    assert_eq!(matrix["source_schema"]["s"]["id"], "INTEGER");
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    let mut ids = BTreeSet::new();
    let mut exact_count = 0;
    let mut residual_count = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate frozen case {id}");
        let parser = case["parser_status_by_dialect"].as_object().unwrap();
        assert_eq!(parser.len(), DIALECTS.len(), "{id}: parser matrix not bounded");
        for dialect in DIALECTS {
            let status = parser.get(*dialect).and_then(Value::as_str);
            assert!(
                matches!(status, Some("parsed" | "parser_boundary")),
                "{id}/{dialect}: no explicit parser disposition"
            );
            if case["proof"] == "Exact" {
                assert_eq!(status, Some("parsed"), "{id}/{dialect}: mandatory gap");
            }
        }
        for adapter in ["direct_sql", "dbt_compiled", "odcs_schema"] {
            let expected = case["expected_adapter_cells"][adapter].as_str();
            let required = if case["proof"] == "Exact" {
                "Exact_required"
            } else {
                "Residual_required"
            };
            assert_eq!(expected, Some(required), "{id}/{adapter}: missing adapter cell");
        }
        match case["proof"].as_str() {
            Some("Exact") => {
                exact_count += 1;
                assert_eq!(
                    case["source_bag"].as_array().unwrap().len(),
                    case["full_terminal_bag"].as_array().unwrap().len(),
                    "{id}: full source and terminal bag cardinalities differ"
                );
                assert_eq!(case["physical_positive_ids"], case["source_row_ids"]);
                assert_eq!(case["physical_not_true_ids"], serde_json::json!([]));
                assert!(!case["typed_terminal_domains"].as_array().unwrap().is_empty());
            }
            Some("Residual") => {
                residual_count += 1;
                assert!(!case["reason"].as_str().unwrap().is_empty());
                assert_eq!(case["typed_terminal_domains"], serde_json::json!([]));
            }
            _ => panic!("{id}: neither Exact nor conditional Residual"),
        }
    }
    assert_eq!((exact_count, residual_count), (3, 5));
}
