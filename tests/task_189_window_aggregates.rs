//! TASK-189: independent full physical and DuckDB terminal window aggregate bags.
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
    serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json")).unwrap()
}

fn literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        _ => panic!("unattested physical scalar: {value}"),
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
                        .map(|(name, val)| (name.clone(), literal(val)))
                        .collect(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn catalog(case: &Value) -> RelationCatalog {
    let schema = RelationSchema::new(
        "s",
        [
            ("id", "INTEGER"),
            ("k", "INTEGER"),
            (
                "x",
                case["source_schema"]["x"].as_str().unwrap_or("INTEGER"),
            ),
        ]
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
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            case["id"].as_str().unwrap(),
            &sql,
            dialect,
            parser.as_ref(),
        )],
        &catalog(case),
    )
    .unwrap()
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
                .map(|(name, val)| (name.clone(), literal(val)))
                .collect()
        })
        .collect()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut sorted = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    sorted.sort();
    sorted
}

fn duckdb_oracle(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(id INTEGER, k INTEGER, x INTEGER);")
        .unwrap();
    for physical in case["source_bag"].as_array().unwrap() {
        let scalar = |column: &str| {
            physical[column]
                .as_i64()
                .map_or("NULL".to_string(), |v| v.to_string())
        };
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {}, {});",
            scalar("id"),
            scalar("k"),
            scalar("x")
        ))
        .unwrap();
    }
    let original = db
        .prepare("SELECT id, k, x FROM s ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let original_expected = case["source_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_i64().unwrap(),
                row["k"].as_i64().unwrap(),
                row["x"].as_i64(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        original, original_expected,
        "DuckDB source bag must match frozen physical inputs"
    );
    let mut stmt = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    stmt.query_map([], |row| {
        let mut values = BTreeMap::new();
        for i in 0..row.as_ref().column_count() {
            let name = row.as_ref().column_name(i)?;
            let value = if name == "mean" {
                row.get::<_, Option<f64>>(i)?
                    .map(|n| LiteralValue::Number(n.to_string()))
            } else {
                row.get::<_, Option<i64>>(i)?
                    .map(|n| LiteralValue::Number(n.to_string()))
            }
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
fn complete_portable_partition_running_moving_and_empty_frame_bags_match_duckdb() {
    let matrix = freeze();
    assert_eq!(matrix["version"], 4);
    assert_eq!(matrix["owner"], "TASK-189");
    assert_eq!(
        matrix["fixture_ids"],
        serde_json::json!(["74-C189-P", "74-C189-N"])
    );
    for case in matrix["cases"].as_array().unwrap() {
        if case["proof"] != "Exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case, "duckdb");
        let layer = bundle.layers().last().unwrap().id();
        let proof = physical_finite_window_bag_plan(&bundle, layer, &[source(case)]);
        let PhysicalFiniteWindowProof::Exact(exact) = proof else {
            panic!("{id}: required complete physical proof missing: {proof:?}");
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
            "{id}: duplicate row identities must survive"
        );
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected(case)),
            "{id}: canonical bag"
        );
        if let Some(ids) = case["surviving_source_ids"].as_array() {
            let wanted = ids
                .iter()
                .map(|id| id.as_str().unwrap().to_string())
                .collect::<Vec<_>>();
            let rejected = case["rejected_source_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_string())
                .collect::<Vec<_>>();
            assert_eq!(
                exact.terminal_row_ids, wanted,
                "{id}: QUALIFY TRUE row identities"
            );
            assert_eq!(
                exact.rejected_row_ids, rejected,
                "{id}: QUALIFY NOT TRUE row identities"
            );
            assert_eq!(
                exact.source_row_ids.len(),
                exact.terminal_row_ids.len() + exact.rejected_row_ids.len(),
                "{id}: every physical source row is classified"
            );
        } else {
            assert_eq!(exact.terminal_row_ids, exact.source_row_ids, "{id}");
            assert!(exact.rejected_row_ids.is_empty(), "{id}");
        }
        assert_eq!(
            bag(&duckdb_oracle(case)),
            bag(&expected(case)),
            "{id}: independent DuckDB bag"
        );
        for column in case["typed_terminal_domains"].as_array().unwrap() {
            let name = column["column"].as_str().unwrap();
            let Some(ValueDomain::Set(actual)) = exact.terminal_domains.get(name) else {
                panic!("{id}/{name}: no exact finite domain");
            };
            assert_eq!(actual.mode(), SetMode::Include);
            let observed = actual
                .values()
                .iter()
                .map(|v| format!("{:?}", v.value()))
                .collect::<BTreeSet<_>>();
            let wanted = column["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(literal)
                .map(|v| format!("{v:?}"))
                .collect::<BTreeSet<_>>();
            assert_eq!(observed, wanted, "{id}/{name}: exact value set");
        }
    }
}

#[test]
fn every_portable_exact_case_has_same_semantics_in_all_exposed_parser_dialects() {
    let matrix = freeze();
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for case in matrix["cases"].as_array().unwrap() {
            if case["proof"] != "Exact" {
                continue;
            }
            let id = case["id"].as_str().unwrap();
            Parser::parse_sql(parser.as_ref(), case["sql"].as_str().unwrap())
                .unwrap_or_else(|e| panic!("{dialect}/{id}: required parser syntax: {e}"));
            let bundle = analyze(case, dialect);
            let proof = physical_finite_window_bag_plan(
                &bundle,
                bundle.layers().last().unwrap().id(),
                &[source(case)],
            );
            let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                panic!("{dialect}/{id}: portable physical law not Exact: {proof:?}");
            };
            assert_eq!(bag(&exact.terminal), bag(&expected(case)), "{dialect}/{id}");
        }
    }
}

#[test]
fn complete_invalid_source_is_impossible_and_missing_evidence_is_residual() {
    let matrix = freeze();
    let case = &matrix["cases"][0];
    let bundle = analyze(case, "duckdb");
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
            ("x".to_string(), LiteralValue::Number("1".to_string())),
        ]))
        .unwrap()],
    )
    .unwrap();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[impossible]),
        PhysicalFiniteWindowProof::Impossible { .. }
    ));
}

#[test]
fn freeze_preserves_adapter_dialect_and_conditional_ownership_without_fabricating_signoff() {
    let matrix = freeze();
    assert_eq!(
        matrix["required_adapters"],
        serde_json::json!(["direct_sql", "dbt_compiled", "odcs_schema"])
    );
    assert_eq!(
        matrix["required_dialects"].as_array().unwrap().len(),
        DIALECTS.len()
    );
    assert_eq!(
        matrix["original_parent_ac_map"],
        serde_json::json!([1, 2, 4, 5, 6])
    );
    let mut ids = BTreeSet::new();
    for case in matrix["cases"].as_array().unwrap() {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        assert_eq!(
            case["parser_status_by_dialect"].as_object().unwrap().len(),
            DIALECTS.len()
        );
        assert_eq!(case["expected_adapter_cells"].as_object().unwrap().len(), 3);
        if case["proof"] == "Residual_expected" {
            assert_eq!(case["conditional_owner"], "TASK-189");
            assert!(!case["reason"].as_str().unwrap().is_empty());
            assert_eq!(case["typed_terminal_domains"], serde_json::json!([]));
        }
    }
}

#[test]
fn fractional_integer_avg_is_residual_without_vendor_result_type_law() {
    let case = serde_json::json!({
        "id": "74-C189-AVG-INT-COERCION",
        "sql": "SELECT id, AVG(x) OVER (PARTITION BY k) AS mean FROM s",
        "source_bag": [
            {"id": 1, "k": 0, "x": 1},
            {"id": 2, "k": 0, "x": 2}
        ]
    });
    // DuckDB produces 1.5, but SQL Server AVG(INT) has an integer return
    // type. Parser acceptance and one native oracle cannot establish a
    // portable fractional integer AVG value.
    let engine = duckdb_oracle(&case);
    assert_eq!(
        engine.iter().map(|row| row.get("mean")).collect::<Vec<_>>(),
        vec![
            Some(&LiteralValue::Number("1.5".into())),
            Some(&LiteralValue::Number("1.5".into()))
        ]
    );
    for dialect in DIALECTS {
        let bundle = analyze(&case, dialect);
        let target = bundle.layers().last().unwrap().id();
        assert!(
            matches!(
                physical_finite_window_bag_plan(&bundle, target, &[source(&case)]),
                PhysicalFiniteWindowProof::Residual { .. }
            ),
            "{dialect}: fractional AVG(INT) result type is vendor-dependent"
        );
    }
}

#[test]
fn integer_sum_requiring_vendor_widening_fails_closed() {
    let case = serde_json::json!({
        "id": "74-C189-INTEGER-OVERFLOW",
        "sql": "SELECT id, SUM(x) OVER (PARTITION BY k) AS total FROM s",
        "source_bag": [
            {"id": 1, "k": 0, "x": 2000000000},
            {"id": 2, "k": 0, "x": 2000000000}
        ]
    });
    let engine = duckdb_oracle(&case);
    assert_eq!(
        engine
            .iter()
            .map(|row| row.get("total"))
            .collect::<Vec<_>>(),
        vec![
            Some(&LiteralValue::Number("4000000000".into())),
            Some(&LiteralValue::Number("4000000000".into()))
        ]
    );
    for dialect in DIALECTS {
        let bundle = analyze(&case, dialect);
        let target = bundle.layers().last().unwrap().id();
        assert!(
            matches!(
                physical_finite_window_bag_plan(&bundle, target, &[source(&case)]),
                PhysicalFiniteWindowProof::Residual { .. }
            ),
            "{dialect}: signed INT accumulator may overflow without promotion"
        );
    }
}

#[test]
fn repeating_average_is_residual_not_rounded_into_false_exactness() {
    let case = serde_json::json!({
        "id": "74-C189-REPEATING",
        "sql": "SELECT id, AVG(x) OVER (PARTITION BY k) AS mean FROM s",
        "source_bag": [
            {"id": 1, "k": 0, "x": 1},
            {"id": 2, "k": 0, "x": 2},
            {"id": 3, "k": 0, "x": 2}
        ]
    });
    let bundle = analyze(&case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[source(&case)]),
        PhysicalFiniteWindowProof::Residual { .. }
    ));
}

#[test]
fn conditional_windows_are_never_mistaken_for_portable_exact_bags() {
    let matrix = freeze();
    let conditional = matrix["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["proof"] == "Residual_expected")
        .collect::<Vec<_>>();
    assert_eq!(conditional.len(), 6);

    // Exercise each of the 78 conditional dialect/parser cells. A parser
    // boundary must not be construed as evidence for native engine behavior.
    let mut exercised = BTreeSet::new();
    for case in conditional {
        let id = case["id"].as_str().unwrap();
        let mut parsed_dialects = 0;
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).unwrap();
            if Parser::parse_sql(parser.as_ref(), case["sql"].as_str().unwrap()).is_err() {
                continue;
            }
            parsed_dialects += 1;
            let bundle = analyze(case, dialect);
            let target = bundle.layers().last().unwrap().id();
            let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
            assert!(
                matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                "{id}/{dialect}: unverified conditional SQL or session law must fail closed, got {proof:?}"
            );
            exercised.insert(format!("{id}/{dialect}"));
        }
        assert!(
            parsed_dialects > 0,
            "{id}: no parser can exercise this conditional fixture"
        );
    }
    assert!(
        exercised.len() >= 6,
        "all six conditional laws must be exercised"
    );
}

#[test]
fn precision_conditional_fixtures_are_distinct_from_integer_exact_cases() {
    let matrix = freeze();
    let cases = matrix["cases"].as_array().unwrap();
    let repeating = cases
        .iter()
        .find(|case| case["id"] == "74-C189-C5")
        .unwrap();
    let decimals = cases
        .iter()
        .find(|case| case["id"] == "74-C189-C6")
        .unwrap();

    // The source's first partition has AVG=(1+2+2)/3, a repeating
    // fraction. v2 accidentally used only integral means.
    let first_partition = repeating["source_bag"].as_array().unwrap();
    assert_eq!(first_partition[0]["x"], 1);
    assert_eq!(first_partition[1]["x"], 2);
    assert_eq!(first_partition[2]["x"], 2);
    assert_eq!(repeating["family"], "repeating_fraction_avg");

    // A DECIMAL precision fixture cannot reuse an INTEGER-typed source.
    assert_eq!(decimals["source_schema"]["x"], "DECIMAL(10,2)");
    assert_eq!(decimals["source_bag"][0]["x"], serde_json::json!(10.2));
    assert_eq!(decimals["source_bag"][2]["x"], serde_json::json!(30.7));
}

#[test]
fn ranking_and_navigation_compose_with_exact_aggregate_rows_and_domains() {
    // TASK-189 AC #5: preserve the complete source and DuckDB terminal bags.
    let case = serde_json::json!({
        "id": "74-C189-AC5-MIXED",
        "sql": "SELECT id, k, x, ROW_NUMBER() OVER (PARTITION BY k ORDER BY id NULLS LAST) AS rn, RANK() OVER (PARTITION BY k ORDER BY id NULLS LAST) AS rk, DENSE_RANK() OVER (PARTITION BY k ORDER BY id NULLS LAST) AS dr, LAG(x) OVER (PARTITION BY k ORDER BY id NULLS LAST) AS previous_x, COUNT(*) OVER (PARTITION BY k ORDER BY id NULLS LAST ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS n, SUM(x) OVER (PARTITION BY k ORDER BY id NULLS LAST ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS total FROM s",
        "source_bag": [
            {"id": 1, "k": 0, "x": 10},
            {"id": 2, "k": 0, "x": null},
            {"id": 3, "k": 0, "x": 30},
            {"id": 4, "k": 1, "x": 5}
        ],
        "full_terminal_bag": [
            {"id": 1, "k": 0, "x": 10, "rn": 1, "rk": 1, "dr": 1, "previous_x": null, "n": 1, "total": 10},
            {"id": 2, "k": 0, "x": null, "rn": 2, "rk": 2, "dr": 2, "previous_x": 10, "n": 2, "total": 10},
            {"id": 3, "k": 0, "x": 30, "rn": 3, "rk": 3, "dr": 3, "previous_x": null, "n": 3, "total": 40},
            {"id": 4, "k": 1, "x": 5, "rn": 1, "rk": 1, "dr": 1, "previous_x": null, "n": 1, "total": 5}
        ]
    });
    let bundle = analyze(&case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    let PhysicalFiniteWindowProof::Exact(exact) =
        physical_finite_window_bag_plan(&bundle, target, &[source(&case)])
    else {
        panic!("unique ORDER BY must prove mixed ranking/navigation/aggregate windows");
    };
    assert_eq!(exact.source_row_ids, ["s#0", "s#1", "s#2", "s#3"]);
    assert_eq!(exact.terminal_row_ids, exact.source_row_ids);
    assert!(exact.rejected_row_ids.is_empty());
    assert_eq!(bag(&exact.terminal), bag(&expected(&case)));
    assert_eq!(bag(&exact.terminal), bag(&duckdb_oracle(&case)));
    for column in ["rn", "rk", "dr", "n", "total", "previous_x"] {
        let Some(ValueDomain::Set(domain)) = exact.terminal_domains.get(column) else {
            panic!("missing exact physical domain for {column}");
        };
        let actual = domain
            .values()
            .iter()
            .map(|item| format!("{:?}", item.value()))
            .collect::<BTreeSet<_>>();
        let expected_values = exact
            .terminal
            .iter()
            .map(|row| format!("{:?}", row.get(column).unwrap()))
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected_values, "{column} exact finite domain");
    }
}

#[test]
fn mixed_ranking_and_rows_aggregates_reject_ambiguous_peers() {
    let case = serde_json::json!({
        "id": "74-C189-AC5-PEERS",
        "sql": "SELECT id, ROW_NUMBER() OVER (PARTITION BY k ORDER BY id NULLS LAST) AS rn, SUM(x) OVER (PARTITION BY k ORDER BY id NULLS LAST ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS total FROM s",
        "source_bag": [
            {"id": 1, "k": 0, "x": 10},
            {"id": 1, "k": 0, "x": 30}
        ]
    });
    let bundle = analyze(&case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[source(&case)]),
        PhysicalFiniteWindowProof::Residual { .. }
    ));
}

#[test]
fn fractional_fifth_is_residual_when_float_engine_cannot_represent_value_exactly() {
    let case = serde_json::json!({
        "id": "74-C189-FLOAT-ROUNDING",
        "sql": "SELECT id, AVG(x) OVER (PARTITION BY k) AS mean FROM s",
        "source_bag": [
            {"id": 1, "k": 0, "x": 0},
            {"id": 2, "k": 0, "x": 1},
            {"id": 3, "k": 0, "x": 0},
            {"id": 4, "k": 0, "x": 0},
            {"id": 5, "k": 0, "x": 0}
        ]
    });
    let bundle = analyze(&case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_window_bag_plan(&bundle, target, &[source(&case)]),
        PhysicalFiniteWindowProof::Residual { .. }
    ));
}

#[test]
fn nullable_sum_qualify_distinguishes_true_false_and_unknown() {
    let case = serde_json::json!({
        "id": "74-C189-QUALIFY-SUM",
        "sql": "SELECT id, k, x, SUM(x) OVER (PARTITION BY k) AS total FROM s QUALIFY SUM(x) OVER (PARTITION BY k) >= 10",
        "source_bag": [
            {"id": 1, "k": 0, "x": 10},
            {"id": 2, "k": 0, "x": null},
            {"id": 3, "k": 0, "x": 30},
            {"id": 4, "k": 1, "x": null},
            {"id": 5, "k": 2, "x": 5}
        ],
        "full_terminal_bag": [
            {"id": 1, "k": 0, "x": 10, "total": 40},
            {"id": 2, "k": 0, "x": null, "total": 40},
            {"id": 3, "k": 0, "x": 30, "total": 40}
        ]
    });
    let bundle = analyze(&case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    let PhysicalFiniteWindowProof::Exact(exact) =
        physical_finite_window_bag_plan(&bundle, target, &[source(&case)])
    else {
        panic!("all-NULL and below-threshold partitions must remain NOT TRUE");
    };
    assert_eq!(exact.terminal_row_ids, ["s#0", "s#1", "s#2"]);
    assert_eq!(exact.rejected_row_ids, ["s#3", "s#4"]);
    assert_eq!(exact.source_row_ids.len(), 5);
    assert_eq!(bag(&exact.terminal), bag(&expected(&case)));
    assert_eq!(bag(&exact.terminal), bag(&duckdb_oracle(&case)));
}

#[test]
fn rank_and_running_count_compose_through_qualify_without_expanding_rows() {
    let case = serde_json::json!({
        "id": "74-C189-QUALIFY-MIXED",
        "sql": "SELECT id, k, x, ROW_NUMBER() OVER (PARTITION BY k ORDER BY id NULLS LAST) AS rn, COUNT(*) OVER (PARTITION BY k ORDER BY id NULLS LAST ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS n FROM s QUALIFY COUNT(*) OVER (PARTITION BY k ORDER BY id NULLS LAST ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) >= 2",
        "source_bag": [
            {"id": 1, "k": 0, "x": 10},
            {"id": 2, "k": 0, "x": null},
            {"id": 3, "k": 0, "x": 30},
            {"id": 4, "k": 1, "x": 5},
            {"id": 5, "k": 1, "x": 5}
        ],
        "full_terminal_bag": [
            {"id": 2, "k": 0, "x": null, "rn": 2, "n": 2},
            {"id": 3, "k": 0, "x": 30, "rn": 3, "n": 3},
            {"id": 5, "k": 1, "x": 5, "rn": 2, "n": 2}
        ]
    });
    let bundle = analyze(&case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    let PhysicalFiniteWindowProof::Exact(exact) =
        physical_finite_window_bag_plan(&bundle, target, &[source(&case)])
    else {
        panic!("mixed ranking and bounded COUNT QUALIFY should be jointly exact");
    };
    assert_eq!(exact.terminal_row_ids, ["s#1", "s#2", "s#4"]);
    assert_eq!(exact.rejected_row_ids, ["s#0", "s#3"]);
    assert_eq!(bag(&exact.terminal), bag(&expected(&case)));
    assert_eq!(bag(&exact.terminal), bag(&duckdb_oracle(&case)));
}
