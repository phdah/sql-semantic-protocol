//! TASK-121 source/terminal oracle: entire SQL bags, NULLs, duplicates and
//! empty global/grouped distinction, observed through the public protocol seam.

mod common;

use std::collections::BTreeMap;

use common::DIALECTS;
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_aggregate_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteAggregateProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput,
    ValueDomain,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../docs/task-121-acceptance-freeze.json"))
        .expect("task-owned immutable typed source and terminal oracle")
}

fn schema() -> RelationSchema {
    RelationSchema::new(
        "s",
        ["id", "k", "x"]
            .into_iter()
            .map(|name| {
                SchemaColumn::from_sql_type(
                    name,
                    if name == "id" { "VARCHAR" } else { "INTEGER" },
                    "postgresql",
                )
                .expect("valid typed physical column")
            })
            .collect(),
    )
    .expect("valid source schema")
}

fn literal(value: &Value) -> LiteralValue {
    if value.is_null() {
        LiteralValue::Null
    } else if let Some(number) = value.as_i64() {
        LiteralValue::Number(number.to_string())
    } else {
        LiteralValue::Text(value.as_str().expect("source id").to_string())
    }
}

fn source(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        case["source_bag"]
            .as_array()
            .expect("complete source bag")
            .iter()
            .map(|value| {
                PhysicalWriteRow::new(
                    value
                        .as_object()
                        .expect("named source row")
                        .iter()
                        .map(|(key, val)| (key.clone(), literal(val)))
                        .collect(),
                )
                .expect("valid source row")
            })
            .collect(),
    )
    .expect("valid physical relation")
}

fn expected_terminal(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"]
        .as_array()
        .expect("frozen terminal bag")
        .iter()
        .map(|row| {
            row.as_object()
                .expect("named terminal row")
                .iter()
                .map(|(name, value)| (name.to_owned(), literal(value)))
                .collect()
        })
        .collect()
}

fn normalized(mut rows: Vec<BTreeMap<String, LiteralValue>>) -> Vec<String> {
    let mut result = rows
        .drain(..)
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn duckdb_terminal(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("independent DuckDB oracle");
    db.execute_batch("CREATE TABLE s(id VARCHAR, k INTEGER, x INTEGER)")
        .expect("schema");
    for row in case["source_bag"].as_array().expect("source bag") {
        let scalar = |column: &str| {
            row[column]
                .as_i64()
                .map_or_else(|| "NULL".to_string(), |n| n.to_string())
        };
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ('{}', {}, {})",
            row["id"].as_str().expect("id"),
            scalar("k"),
            scalar("x")
        ))
        .expect("independent source insertion");
    }
    let mut source_stmt = db
        .prepare("SELECT id, k, x FROM s")
        .expect("physical source query");
    let source_rows = source_stmt
        .query_map([], |row| {
            Ok(BTreeMap::from([
                ("id".to_string(), LiteralValue::Text(row.get::<_, String>(0)?)),
                (
                    "k".to_string(),
                    row.get::<_, Option<i64>>(1)?
                        .map(|n| LiteralValue::Number(n.to_string()))
                        .unwrap_or(LiteralValue::Null),
                ),
                (
                    "x".to_string(),
                    row.get::<_, Option<i64>>(2)?
                        .map(|n| LiteralValue::Number(n.to_string()))
                        .unwrap_or(LiteralValue::Null),
                ),
            ]))
        })
        .expect("read all physical source rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("complete source snapshot");
    let expected_rows = case["source_bag"]
        .as_array()
        .expect("frozen complete physical source")
        .iter()
        .map(|row| {
            row.as_object()
                .expect("source row")
                .iter()
                .map(|(name, value)| (name.clone(), literal(value)))
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        normalized(source_rows),
        normalized(expected_rows),
        "DuckDB source bag and identities must match the frozen physical bag"
    );
    let sql = case["sql"].as_str().expect("frozen SQL");
    let columns = case["terminal_columns"].as_array().expect("projection");
    let mut stmt = db.prepare(sql).expect("valid DuckDB SQL");
    let rows = stmt
        .query_map([], |row| {
            let mut values = BTreeMap::new();
            for (index, column) in columns.iter().enumerate() {
                let name = column.as_str().expect("output name");
                let value = if name == "average" {
                    let number = row.get::<_, Option<f64>>(index)?;
                    number
                        .map(|n| {
                            assert!(n.is_finite() && n.fract() == 0.0, "AVG is not an exact integer");
                            LiteralValue::Number((n as i64).to_string())
                        })
                        .unwrap_or(LiteralValue::Null)
                } else {
                    let number = row.get::<_, Option<i64>>(index)?;
                    number
                        .map(|n| LiteralValue::Number(n.to_string()))
                        .unwrap_or(LiteralValue::Null)
                };
                values.insert(name.to_string(), value);
            }
            Ok(values)
        })
        .expect("execute independent terminal SQL");
    rows.collect::<Result<Vec<_>, _>>()
        .expect("complete DuckDB terminal bag")
}

#[test]
fn complete_aggregate_bags_match_frozen_source_oracles_and_duckdb() {
    let inventory = fixture();
    let source_schema = schema();
    let catalog = RelationCatalog::from_schemas(&[source_schema]).expect("typed catalog");

    for case in inventory["cases"].as_array().expect("finite cases") {
        let id = case["id"].as_str().expect("fixture ID");
        let sql = case["sql"].as_str().expect("SQL");
        let parser = dialect_from_name("generic").expect("generic SQL parser");
        let input = SqlInput::inline(sql);
        let configured = ConfiguredSqlInput::new(id, &input, "generic", parser.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog)
            .unwrap_or_else(|err| panic!("{id}: {err:?}"));
        let target = bundle.layers().last().expect("terminal").id();
        let state = source(case);
        let proof =
            physical_finite_aggregate_bag_plan(&bundle, target, std::slice::from_ref(&state));
        let PhysicalFiniteAggregateProof::Exact(exact) = proof else {
            panic!("{id}: expected exact canonical physical proof, got {proof:?}");
        };
        assert_eq!(exact.source, "s", "{id}");
        assert_eq!(exact.grouped, sql.contains("GROUP BY"), "{id}");
        assert_eq!(exact.groups.len(), exact.terminal.len(), "{id}");
        assert_eq!(
            normalized(exact.terminal.clone()),
            normalized(expected_terminal(case)),
            "{id}: complete finite bag with NULL and duplicate contribution"
        );
        assert_eq!(
            normalized(duckdb_terminal(case)),
            normalized(expected_terminal(case)),
            "{id}: independent DuckDB result"
        );
        let ids = case["source_bag"]
            .as_array()
            .expect("source")
            .iter()
            .enumerate()
            .map(|(index, _)| format!("s#{index}"))
            .collect::<Vec<_>>();
        let mut proven_ids = exact
            .groups
            .iter()
            .flat_map(|g| &g.source_row_ids)
            .cloned()
            .collect::<Vec<_>>();
        proven_ids.sort();
        assert_eq!(
            proven_ids, ids,
            "{id}: all source row identities accounted for"
        );
        for expected in case["typed_terminal_domains"]
            .as_array()
            .expect("typed exact output domains")
        {
            let name = expected["column"].as_str().expect("output");
            let actual = exact.terminal_domains.get(name).expect("canonical domain");
            let expected_values = expected["values"]
                .as_array()
                .expect("exact possible values")
                .iter()
                .map(literal)
                .collect::<Vec<_>>();
            match actual {
                ValueDomain::Empty => assert!(expected_values.is_empty(), "{id}:{name}"),
                ValueDomain::Set(values) => {
                    assert_eq!(values.mode(), SetMode::Include);
                    let mut obtained = values
                        .values()
                        .iter()
                        .map(|v| format!("{:?}", v.value()))
                        .collect::<Vec<_>>();
                    let mut wanted = expected_values
                        .iter()
                        .map(|v| format!("{v:?}"))
                        .collect::<Vec<_>>();
                    obtained.sort();
                    wanted.sort();
                    assert_eq!(obtained, wanted, "{id}:{name}: exact NULL/finite domain");
                }
                other => panic!("{id}:{name} not exact domain: {other:?}"),
            }
            assert_eq!(
                actual.admits_null(),
                Some(expected["contains_null"].as_bool().expect("NULL evidence")),
                "{id}:{name}: SQL NULL domain"
            );
        }
    }
}

#[test]
fn all_thirteen_shared_dialect_paths_analyze_the_same_aggregate_bags() {
    let inventory = fixture();
    let catalog = RelationCatalog::from_schemas(&[schema()]).expect("catalog");
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).expect("exposed dialect");
        for case in inventory["cases"].as_array().expect("mandatory cases") {
            let id = case["id"].as_str().expect("fixture");
            let sql = SqlInput::inline(case["sql"].as_str().expect("SQL"));
            let configured = ConfiguredSqlInput::new(id, &sql, dialect, parser.as_ref());
            let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog)
                .unwrap_or_else(|err| panic!("{dialect}:{id}: {err:?}"));
            let target = bundle.layers().last().expect("target").id();
            let proof = physical_finite_aggregate_bag_plan(&bundle, target, &[source(case)]);
            let PhysicalFiniteAggregateProof::Exact(exact) = proof else {
                panic!("{dialect}:{id}: not exact: {proof:?}");
            };
            assert_eq!(
                normalized(exact.terminal.clone()),
                normalized(expected_terminal(case)),
                "{dialect}:{id}: canonical full terminal bag"
            );
        }
    }
}

#[test]
fn nonintegral_avg_and_unproven_queries_never_masquerade_as_exact() {
    let catalog = RelationCatalog::from_schemas(&[schema()]).expect("catalog");
    let parser = dialect_from_name("generic").expect("dialect");
    let state = PhysicalWriteState::new(
        "s",
        [1, 2]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), LiteralValue::Text(format!("{index}"))),
                    ("k".to_string(), LiteralValue::Number("1".into())),
                    ("x".to_string(), LiteralValue::Number(value.to_string())),
                ]))
                .expect("row")
            })
            .collect(),
    )
    .expect("source");

    for sql in [
        "SELECT AVG(x) AS average FROM s",
        "SELECT SUM(x) AS total FROM s WHERE x > 1",
        "SELECT COUNT(DISTINCT x) AS count_x FROM s",
    ] {
        let input = SqlInput::inline(sql);
        let configured = ConfiguredSqlInput::new("negative", &input, "generic", parser.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).expect("SQL");
        let target = bundle.layers().last().expect("terminal").id();
        assert!(
            matches!(
                physical_finite_aggregate_bag_plan(&bundle, target, std::slice::from_ref(&state)),
                PhysicalFiniteAggregateProof::Residual { .. }
            ),
            "{sql}: unproved operator must fail closed"
        );
    }
}

#[test]
fn duplicate_physical_source_identity_is_impossible_but_missing_evidence_is_residual() {
    let inventory = fixture();
    let case = &inventory["cases"][0];
    let catalog = RelationCatalog::from_schemas(&[schema()]).expect("catalog");
    let parser = dialect_from_name("generic").expect("dialect");
    let sql = SqlInput::inline(case["sql"].as_str().expect("SQL"));
    let configured = ConfiguredSqlInput::new("negative", &sql, "generic", parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).expect("SQL");
    let target = bundle.layers().last().expect("terminal").id();
    let state = source(case);
    assert!(matches!(
        physical_finite_aggregate_bag_plan(&bundle, target, &[state.clone(), state]),
        PhysicalFiniteAggregateProof::Impossible { .. }
    ));
    assert!(matches!(
        physical_finite_aggregate_bag_plan(&bundle, target, &[]),
        PhysicalFiniteAggregateProof::Residual { .. }
    ));
}
