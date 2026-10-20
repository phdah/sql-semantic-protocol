//! TASK-189 independent DECIMAL source and terminal bag oracles.
//! No f64 is used to compare numeric values: DuckDB DECIMAL results are
//! serialized as base-10 strings and normalized only for equivalent trailing
//! zeroes. Canonical protocol proofs are evaluated independently of DuckDB.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_window_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
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

fn cases() -> Value {
    serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json"))
        .expect("versioned TASK-189 acceptance matrix")
}

fn literal(value: &Value) -> LiteralValue {
    if value.is_null() {
        LiteralValue::Null
    } else {
        LiteralValue::Number(value.to_string())
    }
}

fn schema(case: &Value) -> RelationCatalog {
    let cols = ["id", "k", "x"]
        .into_iter()
        .map(|name| {
            SchemaColumn::from_sql_type(
                name,
                case["source_schema"][name].as_str().unwrap(),
                "postgresql",
            )
            .unwrap()
        })
        .collect();
    RelationCatalog::from_schemas(&[RelationSchema::new("s", cols).unwrap()]).unwrap()
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
                        .map(|(key, value)| (key.clone(), literal(value)))
                        .collect(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn analyze(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            case["id"].as_str().unwrap(),
            &sql,
            dialect,
            parser.as_ref(),
        )],
        &schema(case),
    )
    .unwrap()
}

fn canonical(text: &str) -> String {
    if text == "NULL" {
        return text.into();
    }
    match text.split_once('.') {
        Some((integer, fractional)) => {
            let fraction = fractional.trim_end_matches('0');
            if fraction.is_empty() {
                integer.to_string()
            } else {
                format!("{integer}.{fraction}")
            }
        }
        None => text.to_string(),
    }
}

fn normalized(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut items = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|(key, value)| {
                    let number = match value {
                        LiteralValue::Null => "NULL".to_string(),
                        LiteralValue::Number(text) => canonical(text),
                        _ => panic!("physical decimal oracle has non-numeric value"),
                    };
                    format!("{key}={number}")
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect::<Vec<_>>();
    items.sort();
    items
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
                .map(|(key, value)| (key.clone(), literal(value)))
                .collect()
        })
        .collect()
}

fn duckdb_oracle(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(&format!(
        "CREATE TABLE s (id INTEGER, k INTEGER, x {});",
        case["source_schema"]["x"].as_str().unwrap()
    ))
    .unwrap();
    for row in case["source_bag"].as_array().unwrap() {
        let val = |name: &str| {
            if row[name].is_null() {
                "NULL".to_string()
            } else {
                row[name].to_string()
            }
        };
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {}, {});",
            val("id"),
            val("k"),
            val("x")
        ))
        .unwrap();
    }
    let mut original = db
        .prepare("SELECT CAST(x AS VARCHAR) FROM s ORDER BY id")
        .unwrap();
    let input = original
        .query_map([], |row| row.get::<_, Option<String>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let wanted = case["source_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            if row["x"].is_null() {
                None
            } else {
                Some(row["x"].to_string())
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        input
            .iter()
            .map(|v| v.as_deref().map(canonical))
            .collect::<Vec<_>>(),
        wanted
            .iter()
            .map(|v| v.as_deref().map(canonical))
            .collect::<Vec<_>>(),
        "independent DuckDB complete source bag"
    );

    let columns = case["full_terminal_bag"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let sql = format!(
        "SELECT {} FROM ({}) AS evaluated ORDER BY id",
        columns
            .iter()
            .map(|key| format!("CAST({key} AS VARCHAR) AS {key}"))
            .collect::<Vec<_>>()
            .join(", "),
        case["sql"].as_str().unwrap()
    );
    let mut stmt = db.prepare(&sql).unwrap();
    stmt.query_map([], |row| {
        columns
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let value: Option<String> = row.get(i)?;
                Ok((
                    (*key).to_string(),
                    value
                        .map(LiteralValue::Number)
                        .unwrap_or(LiteralValue::Null),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, duckdb::Error>>()
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

#[test]
fn fixed_point_exact_cases_match_all_dialects_and_independent_duckdb_bags() {
    let freeze = cases();
    assert_eq!(freeze["version"], 4);
    for case in freeze["numeric_oracle_cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["proof"] == "Exact")
    {
        let id = case["id"].as_str().unwrap();
        let reference = normalized(&expected(case));
        assert_eq!(
            normalized(&duckdb_oracle(case)),
            reference,
            "{id}: DuckDB oracle"
        );
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).unwrap();
            Parser::parse_sql(parser.as_ref(), case["sql"].as_str().unwrap())
                .unwrap_or_else(|err| panic!("{id}/{dialect}: parser boundary: {err}"));
            let bundle = analyze(case, dialect);
            let layer = bundle.layers().last().unwrap().id();
            let proof = physical_finite_window_bag_plan(&bundle, layer, &[source(case)]);
            let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                panic!("{id}/{dialect}: missing numeric proof: {proof:?}");
            };
            assert_eq!(normalized(&exact.terminal), reference, "{id}/{dialect}");
            assert_eq!(
                exact.source_row_ids.len(),
                case["source_bag"].as_array().unwrap().len()
            );
            assert_eq!(exact.terminal_row_ids, exact.source_row_ids);
            assert!(exact.rejected_row_ids.is_empty());
            for specified in case["typed_terminal_domains"].as_array().unwrap() {
                let name = specified["column"].as_str().unwrap();
                let Some(ValueDomain::Set(domain)) = exact.terminal_domains.get(name) else {
                    panic!("{id}/{dialect}/{name}: required typed output domain missing");
                };
                let expected = specified["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| {
                        if value.is_null() {
                            "NULL".to_string()
                        } else {
                            canonical(&value.to_string())
                        }
                    })
                    .collect::<BTreeSet<_>>();
                let actual = domain
                    .values()
                    .iter()
                    .map(|value| match value.value() {
                        LiteralValue::Null => "NULL".to_string(),
                        LiteralValue::Number(number) => canonical(number),
                        _ => panic!("unexpected nonnumeric typed domain"),
                    })
                    .collect::<BTreeSet<_>>();
                assert_eq!(actual, expected, "{id}/{dialect}/{name} frozen domain");
                assert_eq!(
                    actual.contains("NULL"),
                    specified["contains_null"].as_bool().unwrap()
                );
                if specified["sql_type"]
                    .as_str()
                    .unwrap()
                    .starts_with("DECIMAL")
                {
                    for value in domain.values() {
                        if value.value() != &LiteralValue::Null {
                            assert_eq!(
                                value.literal_type(),
                                sql_semantic_protocol::LiteralType::Decimal,
                                "{id}/{dialect}/{name}: must retain decimal domain type"
                            );
                        }
                    }
                }
            }
            for (name, domain) in &exact.terminal_domains {
                let ValueDomain::Set(values) = domain else {
                    panic!("{id}/{name}: expected exact finite value set");
                };
                let actual = values
                    .values()
                    .iter()
                    .map(|literal| match literal.value() {
                        LiteralValue::Null => "NULL".to_string(),
                        LiteralValue::Number(value) => canonical(value),
                        _ => panic!("unproved scalar output"),
                    })
                    .collect::<BTreeSet<_>>();
                let expected_values = exact
                    .terminal
                    .iter()
                    .map(|row| match row.get(name).unwrap() {
                        LiteralValue::Null => "NULL".to_string(),
                        LiteralValue::Number(value) => canonical(value),
                        _ => panic!("unproved scalar output"),
                    })
                    .collect::<BTreeSet<_>>();
                assert_eq!(actual, expected_values, "{id}/{dialect}/{name}");
            }
        }
    }
}

#[test]
fn decimal_overflow_and_rounding_uncertainty_stay_residual() {
    let matrix = cases();
    for case in matrix["numeric_oracle_cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["proof"] == "Residual_expected")
    {
        let bundle = analyze(case, "duckdb");
        let target = bundle.layers().last().unwrap().id();
        assert!(
            matches!(
                physical_finite_window_bag_plan(&bundle, target, &[source(case)]),
                PhysicalFiniteWindowProof::Residual { .. }
            ),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn conditional_decimal_native_duckdb_oracles_do_not_overclaim_portable_exactness() {
    let matrix = cases();
    let mut executed = 0;
    for case in matrix["numeric_oracle_cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["proof"] == "Residual_expected")
    {
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            case["native_execution_disposition_by_dialect"]["duckdb"],
            "duckdb_native_oracle_executed_in_ci"
        );
        assert_eq!(
            normalized(&duckdb_oracle(case)),
            normalized(&expected(case)),
            "{id}: independently executed native decimal terminal bag"
        );
        for dialect in DIALECTS {
            let status = case["native_execution_disposition_by_dialect"][dialect]
                .as_str()
                .unwrap();
            if *dialect != "duckdb" {
                assert_eq!(status, "residual_native_vendor_or_session_unattested");
            }
            let bundle = analyze(case, dialect);
            let target = bundle.layers().last().unwrap().id();
            assert!(
                matches!(
                    physical_finite_window_bag_plan(&bundle, target, &[source(case)]),
                    PhysicalFiniteWindowProof::Residual { .. }
                ),
                "{id}/{dialect}: native DuckDB result cannot prove vendor portability"
            );
        }
        executed += 1;
    }
    assert_eq!(executed, 2);
}
