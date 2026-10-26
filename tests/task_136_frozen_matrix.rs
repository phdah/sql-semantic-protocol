//! Independent TASK-136 acceptance matrix: native complete bags and physical proofs.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    physical_finite_page_membership_plan, to_bundle_json, ConfiguredSqlInput, LiteralValue,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-136-acceptance-freeze.json")).unwrap()
}
fn value(v: &Value) -> LiteralValue {
    v.as_i64()
        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
}
fn state(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        case["source"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), value(v))])).unwrap())
            .collect(),
    )
    .unwrap()
}
fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap()
}
fn bundle(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let input = SqlInput::inline(case["sql"].as_str().unwrap());
    let configured = [ConfiguredSqlInput::new(
        case["id"].as_str().unwrap(),
        &input,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap()
}
fn native_bag(case: &Value) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s (k INTEGER)").unwrap();
    for entry in case["source"].as_array().unwrap() {
        let scalar = entry.as_i64().map_or("NULL".to_string(), |v| v.to_string());
        db.execute_batch(&format!("INSERT INTO s VALUES ({scalar})"))
            .unwrap();
    }
    let observed = db
        .prepare("SELECT k FROM s")
        .unwrap()
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut observed = observed;
    let mut expected = case["source"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::as_i64)
        .collect::<Vec<_>>();
    observed.sort();
    expected.sort();
    assert_eq!(
        observed, expected,
        "{}: complete physical source",
        case["id"]
    );

    // SQL Server TOP is compared with a verified DuckDB-equivalent ordered LIMIT.
    let sql = case["native_oracle_sql"]
        .as_str()
        .unwrap_or(case["sql"].as_str().unwrap());
    let terminal = db
        .prepare(sql)
        .unwrap()
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = case["output"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::as_i64)
        .collect::<Vec<_>>();
    assert_eq!(
        terminal, expected,
        "{}: native complete terminal",
        case["id"]
    );
}
fn ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_string())
        .collect()
}
fn emitted_bag(exact: &sql_semantic_protocol::PhysicalFinitePageExact) -> Vec<LiteralValue> {
    exact.terminal.iter().map(|row| row["k"].clone()).collect()
}

#[test]
fn mandatory_frozen_cases_prove_full_physical_bags_domains_and_both_membership_directions() {
    let frozen = freeze();
    assert_eq!(
        frozen["fixture_ids"],
        serde_json::json!(["76-C136-P", "76-C136-N"])
    );
    let mut checked = 0;
    for case in frozen["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory" {
            continue;
        }
        checked += 1;
        native_bag(case);
        let dialect = if case["id"] == "76-C136-P-top" {
            "mssql"
        } else {
            "duckdb"
        };
        let analyzed = bundle(case, dialect);
        let target = analyzed.layers().last().unwrap().id();
        let physical = state(case);
        let actual =
            physical_finite_page_bag_plan(&analyzed, target, std::slice::from_ref(&physical));
        let PhysicalFinitePageProof::Exact(exact) = actual else {
            panic!(
                "{}: expected exact physical outcome, got {actual:?}",
                case["id"]
            );
        };
        assert_eq!(exact.source, "s");
        assert_eq!(exact.rows.len(), case["source"].as_array().unwrap().len());
        assert_eq!(
            exact
                .rows
                .iter()
                .map(|row| row.source_row_id.clone())
                .collect::<Vec<_>>(),
            ids(&case["physical_source_row_ids"])
        );
        assert_eq!(
            exact
                .rows
                .iter()
                .filter(|row| row.survives)
                .map(|row| row.source_row_id.clone())
                .collect::<Vec<_>>(),
            ids(&case["selected"])
        );
        assert_eq!(
            exact
                .rows
                .iter()
                .filter(|row| !row.survives)
                .map(|row| row.source_row_id.clone())
                .collect::<Vec<_>>(),
            ids(&case["rejected"])
        );
        assert_eq!(
            emitted_bag(&exact),
            case["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(value)
                .collect::<Vec<_>>()
        );
        let domain = &exact.terminal_domains["k"];
        let values = &case["domain"]["k"];
        if case["output"].as_array().unwrap().is_empty() {
            assert!(matches!(domain, ValueDomain::Empty), "{domain:?}");
        } else {
            let ValueDomain::Set(set) = domain else {
                panic!("not finite strongest domain: {domain:?}");
            };
            assert_eq!(set.mode(), SetMode::Include);
            let mut got = set
                .values()
                .iter()
                .map(|item| format!("{:?}", item.value()))
                .collect::<Vec<_>>();
            let mut want = case["output"]
                .as_array()
                .unwrap()
                .iter()
                .map(value)
                .map(|x| format!("{x:?}"))
                .collect::<Vec<_>>();
            got.sort();
            want.sort();
            want.dedup();
            assert_eq!(got, want, "typed exact output set");
            assert_eq!(domain.admits_null(), values["nullable"].as_bool());
            let nonnull = case["output"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_i64)
                .collect::<Vec<_>>();
            let min = *nonnull.iter().min().unwrap();
            let max = *nonnull.iter().max().unwrap();
            assert_eq!(values["lower"], min);
            assert_eq!(values["upper"], max);
            assert_eq!(values["inclusive"], true);
        }
        let missing = BTreeMap::from([("k".to_string(), LiteralValue::Number("999".to_string()))]);
        assert!(
            matches!(
                physical_finite_page_membership_plan(
                    &analyzed,
                    target,
                    std::slice::from_ref(&physical),
                    &missing
                ),
                PhysicalFinitePageProof::Impossible { .. }
            ),
            "{}: negative membership must be impossible",
            case["id"]
        );
        if let Some(first) = exact.terminal.first() {
            assert!(
                matches!(
                    physical_finite_page_membership_plan(
                        &analyzed,
                        target,
                        std::slice::from_ref(&physical),
                        first
                    ),
                    PhysicalFinitePageProof::Exact(_)
                ),
                "{}: positive membership should be exact",
                case["id"]
            );
        }
        let wire: Value = serde_json::from_str(&to_bundle_json(&analyzed)).unwrap();
        assert!(wire["inputs"][0]["statements"][0]["row_shaping"].is_object());
        assert_eq!(wire["protocol_version"], env!("CARGO_PKG_VERSION"));
    }
    assert_eq!(checked, 5);
}

#[test]
fn conditional_frozen_cases_do_not_overclaim_complete_row_membership() {
    let frozen = freeze();
    let mut checked = 0;
    for case in frozen["cases"].as_array().unwrap() {
        if case["classification"] != "conditional" {
            continue;
        }
        checked += 1;
        assert_eq!(case["owner"], "TASK-136");
        assert!(
            case["disposition"].as_str().unwrap().contains("Residual")
                || case["disposition"]
                    .as_str()
                    .unwrap()
                    .contains("parser boundary")
        );
        // All five conditional cases must carry an explicit unproved membership boundary.
        let dialect = if case["id"] == "76-C136-N-sample" {
            "snowflake"
        } else {
            "generic"
        };
        let parser = dialect_from_name(dialect).unwrap();
        let input = SqlInput::inline(case["sql"].as_str().unwrap());
        let configured = [ConfiguredSqlInput::new(
            "TASK-136",
            &input,
            dialect,
            parser.as_ref(),
        )];
        if let Ok(bundle) = analyze_configured_inputs_with_catalog(&configured, &catalog()) {
            let target = bundle.layers().last().unwrap().id();
            let result = physical_finite_page_bag_plan(&bundle, target, &[state(case)]);
            assert!(
                matches!(result, PhysicalFinitePageProof::Residual { .. }),
                "{}: conditional not Residual: {result:?}",
                case["id"]
            );
        }
    }
    assert_eq!(checked, 6);
}
