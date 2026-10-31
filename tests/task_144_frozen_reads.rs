//! TASK-144 cross-child independent raw CTE, recursion and rejection oracles.
//! Every assertion is driven by the named, immutable task-owned matrix.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap()
}

fn number(value: &Value) -> LiteralValue {
    value.as_i64().map_or(LiteralValue::Null, |value| {
        LiteralValue::Number(value.to_string())
    })
}

fn input(case: &Value) -> (sql_semantic_protocol::AnalysisBundle, Vec<PhysicalWriteState>) {
    let mut schemas = Vec::new();
    let mut states = Vec::new();
    for (name, rows) in case["initial_bags"].as_object().unwrap() {
        schemas.push(RelationSchema::new(
            name.clone(),
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        ).unwrap());
        let physical = rows.as_array().unwrap().iter().map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), number(value))])).unwrap()
        }).collect();
        states.push(PhysicalWriteState::new(name.clone(), physical).unwrap());
    }
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let configured = [ConfiguredSqlInput::new("cte", &sql, "duckdb", dialect.as_ref())];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    (bundle, states)
}

fn expected_row(row: &Value, columns: &[String]) -> (BTreeMap<String, LiteralValue>, Vec<String>) {
    let values = columns.iter().map(|column| (column.clone(), number(&row[column]))).collect();
    let ids = row["source_ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    (values, ids)
}

fn bag(case: &Value) -> sql_semantic_protocol::PhysicalCteExact {
    let (bundle, states) = input(case);
    let target = bundle.layers().last().unwrap();
    let outcome = physical_finite_cte_bag_plan(&bundle, target.id(), &states, None);
    let PhysicalCteProof::Exact(proof) = outcome else {
        panic!("{}: missing exact CTE witness: {outcome:?}", case["id"]);
    };
    *proof
}

#[test]
fn every_frozen_raw_read_and_recursion_has_full_typed_oracle() {
    let freeze = frozen();
    let cases = freeze["cases"].as_array().unwrap();
    let mut verified = Vec::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        if !matches!(id,
            "78-C144-P-REUSED-FILTER" |
            "78-C144-N-NULL-DUP" |
            "78-C144-P-FINITE-RECURSION"
        ) {
            continue;
        }
        let proof = bag(case);
        let columns = case["domains"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        let mut observed = proof.terminal.iter().map(|row| (row.values.clone(), row.source_row_ids.clone())).collect::<Vec<_>>();
        let mut expected = case["expected_terminal"].as_array().unwrap().iter()
            .map(|row| expected_row(row, &columns)).collect::<Vec<_>>();
        observed.sort_by(|left, right| left.1.cmp(&right.1));
        expected.sort_by(|left, right| left.1.cmp(&right.1));
        assert_eq!(observed, expected, "{id}: complete terminal value/identity bag");
        for column in &columns {
            let actual = &proof.terminal_domains[column];
            let expected = &case["domains"][column];
            assert_eq!(actual.null_allowed, expected["null_allowed"].as_bool().unwrap(), "{id}/{column}: NULL");
            let lower = expected["lower"]["value"].as_i64().unwrap() as i128;
            let upper = expected["upper"]["value"].as_i64().unwrap() as i128;
            assert_eq!(actual.integer_bounds, Some((lower, upper)), "{id}/{column}: exact interval");
            assert_eq!(expected["lower"]["inclusive"], true, "{id}/{column}");
            assert_eq!(expected["upper"]["inclusive"], true, "{id}/{column}");
        }
        let (bundle, states) = input(case);
        let target = bundle.layers().last().unwrap();
        let wanted = case["true_tuple"].as_object().unwrap().iter()
            .map(|(key, val)| (key.clone(), number(val))).collect::<BTreeMap<_, _>>();
        let absent = case["not_true_tuple"].as_object().unwrap().iter()
            .map(|(key, val)| (key.clone(), number(val))).collect::<BTreeMap<_, _>>();
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle, target.id(), &states, Some(&wanted)),
            PhysicalCteProof::Exact(_)
        ), "{id}: positive membership");
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle, target.id(), &states, Some(&absent)),
            PhysicalCteProof::Impossible { .. }
        ), "{id}: closed-world NOT TRUE");
        if let Some(truth) = case.get("predicate_truth") {
            assert_eq!(proof.false_pairs["terminal/cte:0:stage"].iter()
                .map(|ids| ids[0].clone()).collect::<Vec<_>>(),
                truth["false_source_ids"].as_array().unwrap().iter()
                    .map(|id| id.as_str().unwrap().to_string()).collect::<Vec<_>>());
            assert_eq!(proof.unknown_pairs["terminal/cte:0:stage"].iter()
                .map(|ids| ids[0].clone()).collect::<Vec<_>>(),
                truth["unknown_source_ids"].as_array().unwrap().iter()
                    .map(|id| id.as_str().unwrap().to_string()).collect::<Vec<_>>());
        }
        let db = Connection::open_in_memory().unwrap();
        if let Some(raw) = case["initial_bags"]["raw"].as_array() {
            db.execute_batch("CREATE TABLE raw(k INTEGER)").unwrap();
            for value in raw {
                db.execute("INSERT INTO raw(k) VALUES (?1)", params![value.as_i64()]).unwrap();
            }
        }
        let sql = case["sql"].as_str().unwrap();
        let mut stmt = db.prepare(sql).unwrap();
        let mut native = stmt.query_map([], |row| {
            (0..columns.len()).map(|i| row.get::<_, Option<i64>>(i))
                .collect::<duckdb::Result<Vec<_>>>()
        }).unwrap().collect::<duckdb::Result<Vec<_>>>().unwrap();
        let mut canonical = proof.terminal.iter().map(|row| {
            columns.iter().map(|column| match &row.values[column] {
                LiteralValue::Null => None,
                LiteralValue::Number(raw) => Some(raw.parse::<i64>().unwrap()),
                _ => panic!("{}: unexpected output type", id),
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>();
        native.sort();
        canonical.sort();
        assert_eq!(native, canonical, "{id}: independent DuckDB complete bag");
        verified.push(id);
    }
    verified.sort();
    assert_eq!(verified, [
        "78-C144-N-NULL-DUP",
        "78-C144-P-FINITE-RECURSION",
        "78-C144-P-REUSED-FILTER",
    ]);
}

#[test]
fn unguarded_recursive_cycle_and_lateral_cannot_claim_exact() {
    let freeze = frozen();
    for id in ["78-C144-N-RECURSIVE-CYCLE", "78-C144-N-LATERAL-SESSION"] {
        let case = freeze["cases"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap();
        let (bundle, states) = input(case);
        let target = bundle.layers().last().unwrap();
        let proof = physical_finite_cte_bag_plan(&bundle, target.id(), &states, None);
        assert!(matches!(proof, PhysicalCteProof::Residual { .. }), "{id}: {proof:?}");
        assert_eq!(case["owner"], "TASK-144");
        assert!(case["required_evidence"].is_string());
        assert!(case["release_disposition"].is_string());
    }
}
