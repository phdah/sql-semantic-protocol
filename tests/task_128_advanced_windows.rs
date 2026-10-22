//! TASK-128: frozen public physical-window proofs and independent DuckDB full-bag oracle.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_window_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput,
    ValueDomain,
};
use std::collections::{BTreeMap, BTreeSet};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-128-acceptance-freeze.json"))
        .expect("TASK-128 frozen acceptance inventory")
}

fn scalar(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        _ => panic!("fixture has unsupported scalar: {value}"),
    }
}

fn source(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        case["source_bag"].as_array().unwrap().iter().map(|row| {
            PhysicalWriteRow::new(row.as_object().unwrap().iter()
                .map(|(name, value)| (name.clone(), scalar(value))).collect()).unwrap()
        }).collect(),
    ).unwrap()
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "s", [("id", "INTEGER"), ("k", "INTEGER"), ("x", "INTEGER")]
            .into_iter().map(|(name, kind)| {
                SchemaColumn::from_sql_type(name, kind, "duckdb").unwrap()
            }).collect(),
    ).unwrap()]).unwrap()
}

fn analyze(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(sql);
    let inputs = [ConfiguredSqlInput::new(
        "task-128", &sql, "duckdb", dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&inputs, &catalog()).unwrap()
}

fn expected_rows(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"].as_array().unwrap().iter().map(|row| {
        row.as_object().unwrap().iter()
            .map(|(key, value)| (key.clone(), scalar(value))).collect()
    }).collect()
}

fn sorted_bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows.iter().map(|row| format!("{row:?}")).collect::<Vec<_>>();
    result.sort();
    result
}

fn duckdb_oracle(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(id INTEGER, k INTEGER, x INTEGER)").unwrap();
    for row in case["source_bag"].as_array().unwrap() {
        let sql = |value: &Value| value.as_i64().map_or("NULL".into(), |v| v.to_string());
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {}, {})",
            sql(&row["id"]), sql(&row["k"]), sql(&row["x"])
        )).unwrap();
    }
    let mut readback = db.prepare("SELECT id,k,x FROM s ORDER BY id").unwrap();
    let physical = readback.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<i64>>(2)?))
    }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    let original = case["source_bag"].as_array().unwrap().iter()
        .map(|row| (row["id"].as_i64().unwrap(), row["k"].as_i64().unwrap(), row["x"].as_i64()))
        .collect::<Vec<_>>();
    assert_eq!(physical, original, "full physical source bag must be read from native DuckDB");
    let mut query = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    query.query_map([], |row| {
        let mut values = BTreeMap::new();
        for index in 0..row.as_ref().column_count() {
            let name = row.as_ref().column_name(index)?.to_string();
            let value = row.get::<_, Option<i64>>(index)?
                .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()));
            values.insert(name, value);
        }
        Ok(values)
    }).unwrap().collect::<Result<Vec<_>, _>>().unwrap()
}

#[test]
fn exact_windows_match_frozen_independent_complete_bags_and_domains() {
    let freeze = frozen();
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["fixture_ids"], serde_json::json!(["74-C128-P", "74-C128-N"]));
    for case in freeze["cases"].as_array().unwrap() {
        if case["proof"] != "Exact" { continue; }
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case["sql"].as_str().unwrap());
        let target = bundle.layers().last().unwrap().id();
        let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
        let PhysicalFiniteWindowProof::Exact(result) = proof else {
            panic!("{id} expected complete Exact bag, got {proof:?}");
        };
        let expected_ids = case["source_row_ids"].as_array().unwrap().iter()
            .map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(result.source_row_ids, expected_ids, "{id}");
        assert_eq!(result.terminal_row_ids, expected_ids, "{id}");
        assert!(result.rejected_row_ids.is_empty(), "{id}");
        assert_eq!(sorted_bag(&result.terminal), sorted_bag(&expected_rows(case)), "{id}");
        assert_eq!(sorted_bag(&duckdb_oracle(case)), sorted_bag(&expected_rows(case)), "{id}");
        for domain in case["typed_terminal_domains"].as_array().unwrap() {
            let name = domain["column"].as_str().unwrap();
            let Some(ValueDomain::Set(observed)) = result.terminal_domains.get(name) else {
                panic!("{id}/{name}: expected exact finite domain");
            };
            assert_eq!(observed.mode(), SetMode::Include, "{id}/{name}");
            let actual = observed.values().iter().map(|v| format!("{:?}", v.value()))
                .collect::<BTreeSet<_>>();
            let expected = domain["values"].as_array().unwrap().iter()
                .map(|v| format!("{:?}", scalar(v))).collect::<BTreeSet<_>>();
            assert_eq!(actual, expected, "{id}/{name}: full inclusive finite set");
        }
    }
}

#[test]
fn unproved_order_and_invalid_ntile_fail_closed() {
    for case in frozen()["cases"].as_array().unwrap() {
        if case["proof"] != "Residual" { continue; }
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case["sql"].as_str().unwrap());
        let target = bundle.layers().last().unwrap().id();
        assert!(
            matches!(
                physical_finite_window_bag_plan(&bundle, target, &[source(case)]),
                PhysicalFiniteWindowProof::Residual { .. }
            ),
            "{id}: unsupported session/identity law must not claim Exact"
        );
    }
}

#[test]
fn frozen_window_coverage_has_individual_dialect_adapter_cells() {
    let freeze = frozen();
    let dialects = freeze["dialects"].as_array().unwrap();
    let adapters = freeze["adapters"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    assert_eq!(adapters.len(), 3);
    let mut ids = BTreeSet::new();
    for case in freeze["cases"].as_array().unwrap() {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        assert_eq!(case["dialect_cells"].as_object().unwrap().len(), dialects.len());
        assert_eq!(case["adapter_paths"].as_array().unwrap().len(), adapters.len());
        for dialect in dialects {
            let dialect = dialect.as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            // Parser availability is a boundary, not an attestation of native execution.
            let sql = SqlInput::inline(case["sql"].as_str().unwrap());
            let inputs = [ConfiguredSqlInput::new(
                "task-128-parser", &sql, dialect, parser.as_ref(),
            )];
            let _ = analyze_configured_inputs_with_catalog(&inputs, &catalog());
        }
    }
}
