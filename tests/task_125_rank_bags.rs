//! TASK-125: frozen portable rank/peer oracles through the public physical API.
//! Independent DuckDB execution checks the full physical bag and terminal bag.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_rank_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteRankProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput,
    ValueDomain,
};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

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
    serde_json::from_str(include_str!("../docs/task-125-acceptance-freeze.json")).unwrap()
}

fn value(v: &Value) -> LiteralValue {
    match v {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        Value::String(s) => LiteralValue::Text(s.clone()),
        _ => panic!("unexpected scalar in frozen matrix: {v}"),
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
                        .map(|(name, cell)| (name.clone(), value(cell)))
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
        [("id", "VARCHAR"), ("k", "INTEGER"), ("x", "INTEGER")]
            .into_iter()
            .map(|(name, kind)| SchemaColumn::from_sql_type(name, kind, "postgresql").unwrap())
            .collect(),
    )
    .unwrap();
    RelationCatalog::from_schemas(&[schema]).unwrap()
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
                .map(|(name, cell)| (name.clone(), value(cell)))
                .collect()
        })
        .collect()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn execute_duckdb(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(id VARCHAR, k INTEGER, x INTEGER);")
        .unwrap();
    for item in case["source_bag"].as_array().unwrap() {
        let id = item["id"].as_str().unwrap();
        let k = item["k"]
            .as_i64()
            .map_or("NULL".to_string(), |n| n.to_string());
        let x = item["x"]
            .as_i64()
            .map_or("NULL".to_string(), |n| n.to_string());
        db.execute_batch(&format!("INSERT INTO s VALUES ('{id}', {k}, {x});"))
            .unwrap();
    }
    // The expected source snapshot is independent of the protocol interpreter.
    let source_rows = db
        .prepare("SELECT id,k,x FROM s ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
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
        .map(|row| {
            (
                row["id"].as_str().unwrap().to_string(),
                row["k"].as_i64(),
                row["x"].as_i64(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(source_rows, expected_source, "physical bag and NULLs");

    let mut statement = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    statement
        .query_map([], |row| {
            let mut projected = BTreeMap::new();
            for index in 0..row.as_ref().column_count() {
                let name = row.as_ref().column_name(index)?;
                let output = if name == "id" {
                    row.get::<_, Option<String>>(index)?
                        .map(LiteralValue::Text)
                        .unwrap_or(LiteralValue::Null)
                } else {
                    row.get::<_, Option<i64>>(index)?
                        .map(|n| LiteralValue::Number(n.to_string()))
                        .unwrap_or(LiteralValue::Null)
                };
                projected.insert(name.to_string(), output);
            }
            Ok(projected)
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn analyze(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let inputs = [ConfiguredSqlInput::new(
        case["id"].as_str().unwrap(),
        &sql,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&inputs, &catalog()).unwrap()
}

#[test]
fn source_identity_peers_nulls_and_complete_terminal_domains_match_duckdb() {
    let inventory = freeze();
    for case in inventory["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if case["exact"] != true {
            continue;
        }
        let b = analyze(case, "duckdb");
        let target = b.layers().last().unwrap().id();
        let proof = physical_finite_rank_bag_plan(&b, target, &[source(case)]);
        let PhysicalFiniteRankProof::Exact(exact) = proof else {
            panic!("{id}: incomplete portable ranking proof: {proof:?}");
        };
        assert_eq!(exact.source, "s", "{id}");
        assert_eq!(
            exact.ranked_rows.len(),
            case["source_bag"].as_array().unwrap().len(),
            "{id}"
        );
        let row_ids = exact
            .ranked_rows
            .iter()
            .map(|r| r.source_row_id.clone())
            .collect::<Vec<_>>();
        let expected_ids = (0..row_ids.len())
            .map(|n| format!("s#{n}"))
            .collect::<Vec<_>>();
        assert_eq!(
            row_ids, expected_ids,
            "{id}: one identity per physical bag entry"
        );
        if let Some(rejected) = case["rejected_source_ids"].as_array() {
            let actual = exact
                .ranked_rows
                .iter()
                .filter(|r| !r.survives)
                .map(|r| r.source_row_id.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                rejected
                    .iter()
                    .map(|id| id.as_str().unwrap().to_string())
                    .collect::<Vec<_>>(),
                "{id}: NOT TRUE identities"
            );
        }
        if let Some(ranks) = case["expected_rank"].as_array() {
            assert_eq!(
                exact.ranked_rows.iter().map(|r| r.rank).collect::<Vec<_>>(),
                ranks
                    .iter()
                    .map(|r| r.as_u64().unwrap())
                    .collect::<Vec<_>>(),
                "{id}: peers have SQL RANK gaps"
            );
        }
        if let Some(ranks) = case["expected_dense_rank"].as_array() {
            assert_eq!(
                exact
                    .ranked_rows
                    .iter()
                    .map(|r| r.dense_rank)
                    .collect::<Vec<_>>(),
                ranks
                    .iter()
                    .map(|r| r.as_u64().unwrap())
                    .collect::<Vec<_>>(),
                "{id}: dense peers"
            );
        }
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected(case)),
            "{id}: full canonical terminal"
        );
        assert_eq!(
            bag(&execute_duckdb(case)),
            bag(&expected(case)),
            "{id}: DuckDB independent full terminal"
        );
        for name in case["full_terminal_bag"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row.as_object().unwrap().keys())
        {
            let domain = exact.terminal_domains.get(name).unwrap();
            let ValueDomain::Set(set) = domain else {
                panic!("{id}/{name}: domain not finite: {domain:?}");
            };
            assert_eq!(set.mode(), SetMode::Include);
            let observed = set
                .values()
                .iter()
                .map(|item| format!("{:?}", item.value()))
                .collect::<std::collections::BTreeSet<_>>();
            let expected = exact
                .terminal
                .iter()
                .filter_map(|row| row.get(name))
                .map(|value| format!("{value:?}"))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                observed, expected,
                "{id}/{name}: finite exact domain including NULL"
            );
        }
    }
}

#[test]
fn all_exposed_dialects_either_prove_portable_law_or_report_qualify_parser_boundary() {
    let inventory = freeze();
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for case in inventory["cases"].as_array().unwrap() {
            let sql = case["sql"].as_str().unwrap();
            if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                assert!(
                    case["parser_condition"].is_string(),
                    "{dialect}: mandatory portable SQL must parse: {}",
                    case["id"]
                );
                continue;
            }
            let bundle = analyze(case, dialect);
            let target = bundle.layers().last().unwrap().id();
            let proof = physical_finite_rank_bag_plan(&bundle, target, &[source(case)]);
            if case["proof"] == "Residual" {
                assert!(
                    matches!(proof, PhysicalFiniteRankProof::Residual { .. }),
                    "{dialect}/{}: conditional ranking law was overclaimed: {proof:?}",
                    case["id"]
                );
            } else {
                let PhysicalFiniteRankProof::Exact(exact) = proof else {
                    panic!(
                        "{dialect}/{}: mandatory full-bag ranking is not Exact: {proof:?}",
                        case["id"]
                    );
                };
                assert_eq!(
                    bag(&exact.terminal),
                    bag(&expected(case)),
                    "{dialect}/{}",
                    case["id"]
                );
            }
        }
    }
}

#[test]
fn nondeterministic_row_number_implicit_null_order_and_missing_evidence_fail_closed() {
    let inventory = freeze();
    for case in inventory["cases"].as_array().unwrap() {
        if case["proof"] != "Residual" {
            continue;
        }
        let bundle = analyze(case, "duckdb");
        let target = bundle.layers().last().unwrap().id();
        assert!(
            matches!(
                physical_finite_rank_bag_plan(&bundle, target, &[source(case)]),
                PhysicalFiniteRankProof::Residual { .. }
            ),
            "{}",
            case["id"]
        );
    }
    let case = &inventory["cases"][0];
    let bundle = analyze(case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_rank_bag_plan(&bundle, target, &[]),
        PhysicalFiniteRankProof::Residual { .. }
    ));
    let state = source(case);
    assert!(matches!(
        physical_finite_rank_bag_plan(&bundle, target, &[state.clone(), state]),
        PhysicalFiniteRankProof::Impossible { .. }
    ));
}
