//! TASK-122 frozen source/terminal SQL bag and NOT TRUE HAVING oracle.
mod common;

use std::collections::BTreeMap;
use common::DIALECTS;
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_aggregate_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalAggregateTruth, PhysicalFiniteAggregateProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn,
    SetMode, SqlInput, ValueDomain,
};

fn inventory() -> Value {
    serde_json::from_str(include_str!("../docs/task-122-acceptance-freeze.json"))
        .expect("frozen task-122 cases")
}

fn schema() -> RelationSchema {
    RelationSchema::new(
        "s",
        ["id", "k", "x"].into_iter().map(|name| {
            SchemaColumn::from_sql_type(
                name, if name == "id" { "VARCHAR" } else { "INTEGER" }, "postgresql"
            ).expect("typed source")
        }).collect(),
    ).expect("valid schema")
}

fn literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        Value::String(s) => LiteralValue::Text(s.to_string()),
        _ => panic!("invalid frozen scalar: {value}"),
    }
}

fn state(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new("s",
        case["source_bag"].as_array().expect("physical bag").iter().map(|row| {
            PhysicalWriteRow::new(
                row.as_object().expect("row").iter()
                    .map(|(name, v)| (name.clone(), literal(v))).collect()
            ).expect("physical row")
        }).collect()
    ).expect("complete physical bag")
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rendered = rows.iter().map(|row| format!("{row:?}")).collect::<Vec<_>>();
    rendered.sort();
    rendered
}

fn terminal(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"].as_array().expect("output bag").iter().map(|row| {
        row.as_object().expect("terminal row").iter()
            .map(|(k, v)| (k.clone(), literal(v))).collect()
    }).collect()
}

fn duckdb(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE s(id VARCHAR, k INTEGER, x INTEGER)")
        .expect("source schema");
    for row in case["source_bag"].as_array().expect("source") {
        let k = row["k"].as_i64().map_or("NULL".into(), |v| v.to_string());
        let x = row["x"].as_i64().map_or("NULL".into(), |v| v.to_string());
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ('{}', {k}, {x})",
            row["id"].as_str().expect("source identity")
        )).expect("insert complete source row");
    }
    let mut stmt = db.prepare(case["sql"].as_str().expect("SQL")).expect("DuckDB query");
    let columns = case["terminal_columns"].as_array().expect("output names");
    stmt.query_map([], |row| {
        let mut result = BTreeMap::new();
        for (index, name) in columns.iter().enumerate() {
            let value = row.get::<_, Option<i64>>(index)?
                .map(|v| LiteralValue::Number(v.to_string()))
                .unwrap_or(LiteralValue::Null);
            result.insert(name.as_str().expect("column").to_string(), value);
        }
        Ok(result)
    }).expect("execute DuckDB query")
      .collect::<Result<Vec<_>,_>>().expect("DuckDB terminal bag")
}

fn proof(case: &Value, dialect: &str) -> PhysicalFiniteAggregateProof {
    let parser = dialect_from_name(dialect).expect("public dialect");
    let sql = SqlInput::inline(case["sql"].as_str().expect("frozen SQL"));
    let input = ConfiguredSqlInput::new(
        case["id"].as_str().expect("fixture"), &sql, dialect, parser.as_ref()
    );
    let catalog = RelationCatalog::from_schemas(&[schema()]).expect("typed catalog");
    let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).expect("SQL parses");
    let target = bundle.layers().last().expect("terminal").id();
    physical_finite_aggregate_bag_plan(&bundle, target, &[state(case)])
}

#[test]
fn finite_having_bags_and_complete_false_unknown_groups_match_duckdb() {
    for case in inventory()["cases"].as_array().expect("finite matrix") {
        let id = case["id"].as_str().expect("id");
        let result = proof(case, "generic");
        let PhysicalFiniteAggregateProof::Exact(exact) = result else {
            panic!("{id}: expected exact complete physical proof, got {result:?}");
        };
        assert_eq!(bag(&exact.terminal), bag(&terminal(case)), "{id}: exact full terminal bag");
        assert_eq!(bag(&duckdb(case)), bag(&terminal(case)), "{id}: independently executed SQL");
        assert_eq!(exact.groups.len(), case["groups"].as_array().expect("groups").len());
        for (group, expected) in exact.groups.iter().zip(case["groups"].as_array().expect("groups")) {
            let truth = match expected["having"].as_str().expect("SQL truth") {
                "TRUE" => PhysicalAggregateTruth::True,
                "FALSE" => PhysicalAggregateTruth::False,
                "UNKNOWN" => PhysicalAggregateTruth::Unknown,
                other => panic!("unsupported fixture truth: {other}"),
            };
            assert_eq!(group.having_truth, Some(truth), "{id}: group-wide HAVING truth");
            let ids = expected["source_row_ids"].as_array().expect("group identities")
                .iter().map(|v| v.as_str().expect("source row").to_string()).collect::<Vec<_>>();
            assert_eq!(group.source_row_ids, ids, "{id}: complete source identity");
        }
        let mut covered = exact.groups.iter()
            .flat_map(|g| g.source_row_ids.iter().cloned()).collect::<Vec<_>>();
        covered.sort();
        let expected = (0..case["source_bag"].as_array().expect("source").len())
            .map(|i| format!("s#{i}")).collect::<Vec<_>>();
        assert_eq!(covered, expected, "{id}: no source rows erased by rejected groups");
        for column in &case["terminal_columns"].as_array().expect("output columns") {
            let name = column.as_str().expect("name");
            let actual = exact.terminal_domains.get(name).expect("exact domain");
            let expected = &case["typed_terminal_domains"].as_array().expect("domains")
                .iter().find(|v| v["column"] == name).expect("named domain");
            let mut wanted = expected["values"].as_array().expect("finite set")
                .iter().map(|v| format!("{:?}",literal(v))).collect::<Vec<_>>();
            wanted.sort();
            if wanted.is_empty() {
                assert!(matches!(actual, ValueDomain::Empty), "{id}:{name}: empty domain");
            } else {
                let ValueDomain::Set(values) = actual else {panic!("{id}:{name}: {actual:?}")};
                assert_eq!(values.mode(), SetMode::Include);
                let mut got = values.values().iter()
                    .map(|v| format!("{:?}",v.value())).collect::<Vec<_>>();
                got.sort();
                assert_eq!(got, wanted, "{id}:{name}: exact finite output domain");
            }
        }
    }
}

#[test]
fn all_thirteen_parser_dialects_prove_shared_distinct_having_semantics() {
    let fixture = inventory();
    for dialect in DIALECTS {
        for case in fixture["cases"].as_array().expect("cases") {
            if case["id"] == "73-C122-P-filter" {
                // FILTER grammar requires dialect-specific parser classification,
                // not a claim of native warehouse FILTER execution.
                continue;
            }
            let result = proof(case, dialect);
            assert!(matches!(result, PhysicalFiniteAggregateProof::Exact(_)),
                "{dialect}:{}: {result:?}",case["id"]);
        }
    }
}

#[test]
fn unsupported_filter_type_and_distinct_sum_fail_closed() {
    let parser = dialect_from_name("generic").expect("parser");
    let catalog = RelationCatalog::from_schemas(&[schema()]).expect("catalog");
    let cases = inventory();
    let state = state(&cases["cases"][0]);
    for sql in [
        "SELECT k, SUM(DISTINCT x) AS total FROM s GROUP BY k",
        "SELECT k, COUNT(x) FILTER (WHERE id > 'a') AS n FROM s GROUP BY k",
        "SELECT k, COUNT(DISTINCT x + 1) AS n FROM s GROUP BY k",
        "SELECT k, COUNT(x) AS n FROM s GROUP BY k HAVING COUNT(x) > 1 AND SUM(id) > 0",
    ] {
        let input = SqlInput::inline(sql);
        let config = ConfiguredSqlInput::new("fail-closed", &input, "generic", parser.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[config], &catalog).expect("parse");
        let target = bundle.layers().last().expect("target").id();
        let result = physical_finite_aggregate_bag_plan(&bundle, target, &[state.clone()]);
        assert!(matches!(result, PhysicalFiniteAggregateProof::Residual { .. }),
            "{sql}: unproved execution law claimed exact: {result:?}");
    }
}
