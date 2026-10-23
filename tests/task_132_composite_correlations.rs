//! TASK-132: public composite correlation proofs against complete independent DuckDB bags.
//! Joined and grouped mandatory cells remain tracked as unverified in the freeze.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_exists_bag_plan,
    physical_finite_joined_exists_bag_plan, PhysicalJoinedExistsProof,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteExistsProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-132-acceptance-freeze.json"))
        .expect("finite task-owned freeze")
}

fn catalog() -> RelationCatalog {
    let schema = |name| {
        RelationSchema::new(
            name,
            ["id", "k", "g"]
                .into_iter()
                .map(|col| {
                    SchemaColumn::from_sql_type(
                        col,
                        if col == "id" { "VARCHAR" } else { "INTEGER" },
                        "postgresql",
                    )
                    .expect("typed column")
                })
                .collect(),
        )
        .expect("source schema")
    };
    RelationCatalog::from_schemas(&[
        schema("outer_rows"),
        schema("inner_rows"),
        schema("dim_rows"),
    ])
    .expect("authoritative catalog")
}

fn states(case: &Value) -> Vec<PhysicalWriteState> {
    case["source_bags"]
        .as_object()
        .expect("frozen physical sources")
        .iter()
        .map(|(relation, rows)| {
            let rows = rows
                .as_array()
                .expect("complete input")
                .iter()
                .map(|row| {
                    let integer = |column| {
                        row[column].as_i64().map_or(LiteralValue::Null, |value| {
                            LiteralValue::Number(value.to_string())
                        })
                    };
                    PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Text(
                                row["id"].as_str().expect("physical id").to_string(),
                            ),
                        ),
                        ("k".to_string(), integer("k")),
                        ("g".to_string(), integer("g")),
                    ]))
                    .expect("physical row")
                })
                .collect();
            PhysicalWriteState::new(relation, rows).expect("physical bag")
        })
        .collect()
}

fn evaluate(case: &Value, dialect: &str) -> PhysicalFiniteExistsProof {
    let sql = SqlInput::inline(case["sql"].as_str().expect("frozen SQL"));
    let parser = dialect_from_name(dialect).expect("published parser dialect");
    let input = ConfiguredSqlInput::new("task-132", &sql, dialect, parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog())
        .expect("canonical SQL analysis");
    let target = bundle.layers().last().expect("target layer");
    physical_finite_exists_bag_plan(&bundle, target.id(), &states(case))
}

fn ids(case: &Value, name: &str) -> Vec<String> {
    case[name]
        .as_array()
        .expect("frozen identities")
        .iter()
        .map(|value| value.as_str().expect("id").to_string())
        .collect()
}

fn expected_terminal(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["expected_terminal_bag"]
        .as_array()
        .expect("full terminal bag")
        .iter()
        .map(|row| {
            let value = row["k"]
                .as_i64()
                .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()));
            BTreeMap::from([("k".to_string(), value)])
        })
        .collect()
}

#[test]
fn direct_and_grouped_composite_exists_proves_both_directions_across_dialects() {
    let frozen = freeze();
    assert_eq!(frozen["version"], 2);
    assert_eq!(frozen["dialects"].as_array().unwrap().len(), 13);
    let mut obligations = 0;
    for case in frozen["cases"].as_array().expect("cases") {
        if case["variant"] != "direct_composite" && case["variant"] != "grouped_composite" {
            continue;
        }
        for dialect in frozen["dialects"].as_array().expect("dialects") {
            let dialect = dialect.as_str().expect("dialect");
            let PhysicalFiniteExistsProof::Exact(exact) = evaluate(case, dialect) else {
                panic!(
                    "{} {dialect}: complete composite proof must be exact",
                    case["id"]
                );
            };
            assert_eq!(exact.sources, ["outer_rows", "inner_rows"]);
            assert_eq!(exact.selected, ids(case, "selected_outer_ids"));
            assert_eq!(exact.rejected, ids(case, "rejected_not_true_outer_ids"));
            assert_eq!(exact.selected.len() + exact.rejected.len(), 3);
            for (outer_id, inner) in case["complete_matching_inner_ids"].as_object().unwrap() {
                let expected = inner
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                assert_eq!(exact.matching_inner.get(outer_id), Some(&expected));
            }
            assert_eq!(exact.terminal, expected_terminal(case));
            let domain = exact.terminal_domains.get("k").expect("typed terminal");
            let allowed = case["terminal_domain"]["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| i128::from(v.as_i64().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(domain.values(), allowed);
            assert_eq!(
                domain.bounds(),
                allowed.first().copied().zip(allowed.last().copied())
            );
            assert_eq!(
                domain.null_allowed(),
                case["terminal_domain"]["null_allowed"]
            );
            obligations += 1;
        }
    }
    assert_eq!(obligations, 52);
}

#[test]
fn duckdb_checks_full_source_and_terminal_row_identity_bags() {
    let frozen = freeze();
    let mut checked = 0;
    for case in frozen["cases"].as_array().expect("cases") {
        if case["variant"] == "dbt_subquery_orders" {
            continue;
        }
        let db = Connection::open_in_memory().expect("DuckDB");
        for relation in ["outer_rows", "inner_rows", "dim_rows"] {
            if case["source_bags"][relation].is_null() {
                continue;
            }
            db.execute(
                &format!("CREATE TABLE {relation} (id VARCHAR, k INTEGER, g INTEGER)"),
                [],
            )
            .expect("create physical source");
            for row in case["source_bags"][relation].as_array().unwrap() {
                db.execute(
                    &format!("INSERT INTO {relation} VALUES (?, ?, ?)"),
                    params![
                        row["id"].as_str().unwrap(),
                        row["k"].as_i64(),
                        row["g"].as_i64()
                    ],
                )
                .expect("load complete physical source");
            }
            let mut read = db
                .prepare(&format!("SELECT id,k,g FROM {relation} ORDER BY id"))
                .unwrap();
            let got = read
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })
                .unwrap()
                .collect::<duckdb::Result<Vec<_>>>()
                .unwrap();
            let mut expected = case["source_bags"][relation]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    (
                        row["id"].as_str().unwrap().to_string(),
                        row["k"].as_i64(),
                        row["g"].as_i64(),
                    )
                })
                .collect::<Vec<_>>();
            expected.sort();
            assert_eq!(got, expected, "{} complete {relation} source", case["id"]);
        }
        let query =
            case["sql"]
                .as_str()
                .unwrap()
                .replacen("SELECT o.k AS k", "SELECT o.id, o.k AS k", 1);
        let mut stmt = db.prepare(&query).expect("native correlated SQL");
        let mut got = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
            })
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();
        got.sort();
        let mut expected = ids(case, "selected_outer_ids")
            .iter()
            .map(|id| {
                let row = case["source_bags"]["outer_rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["id"] == id.as_str())
                    .unwrap();
                (id.clone(), row["k"].as_i64())
            })
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(
            got, expected,
            "{} complete DuckDB terminal identity bag",
            case["id"]
        );
        checked += 1;
    }
    assert_eq!(checked, 6);
}

#[test]
fn joined_shapes_fail_closed_until_owned_physical_proofs_exist() {
    let frozen = freeze();
    for case in frozen["cases"].as_array().unwrap() {
        if case["variant"] != "joined_composite" {
            continue;
        }
        assert!(
            matches!(
                evaluate(case, "generic"),
                PhysicalFiniteExistsProof::Residual { .. }
            ),
            "{} must not be labeled Exact by a direct-leaf proof",
            case["id"]
        );
    }
}

fn evaluate_joined(case: &Value, dialect: &str) -> PhysicalJoinedExistsProof {
    let sql = SqlInput::inline(case["sql"].as_str().expect("frozen joined SQL"));
    let parser = dialect_from_name(dialect).expect("published dialect");
    let configured = ConfiguredSqlInput::new("task-132", &sql, dialect, parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog())
        .expect("canonical joined SQL analysis");
    let target = bundle.layers().last().expect("joined target");
    physical_finite_joined_exists_bag_plan(&bundle, target.id(), &states(case))
}

#[test]
fn joined_composite_exists_carries_full_source_triplets_and_closed_world_absence() {
    let frozen = freeze();
    let mut checked = 0;
    for case in frozen["cases"].as_array().expect("cases") {
        if case["variant"] != "joined_composite" {
            continue;
        }
        for dialect in frozen["dialects"].as_array().expect("dialects") {
            let dialect = dialect.as_str().expect("dialect");
            let PhysicalJoinedExistsProof::Exact(exact) = evaluate_joined(case, dialect) else {
                panic!("{} {dialect}: nested JOIN must certify complete source bags", case["id"]);
            };
            assert_eq!(exact.sources, ["outer_rows", "inner_rows", "dim_rows"]);
            assert_eq!(exact.selected, ids(case, "selected_outer_ids"));
            assert_eq!(exact.rejected, ids(case, "rejected_not_true_outer_ids"));
            assert_eq!(exact.terminal, expected_terminal(case));
            assert_eq!(exact.selected.len() + exact.rejected.len(), 3);
            for (outer_id, partners) in case["complete_matching_inner_ids"].as_object().unwrap() {
                let expected = partners.as_array().unwrap().iter().map(|value| {
                    let named = value.as_str().unwrap();
                    let (inner, joined) = named.split_once('@').expect("frozen pair");
                    [inner.to_string(), joined.to_string()]
                }).collect::<Vec<_>>();
                assert_eq!(exact.matching_pairs.get(outer_id), Some(&expected));
            }
            let domain = exact.terminal_domains.get("k").expect("typed terminal");
            let allowed = case["terminal_domain"]["values"]
                .as_array().unwrap().iter()
                .map(|value| i128::from(value.as_i64().unwrap())).collect::<Vec<_>>();
            assert_eq!(domain.values(), allowed);
            assert_eq!(domain.bounds(), allowed.first().copied().zip(allowed.last().copied()));
            assert_eq!(domain.null_allowed(), case["terminal_domain"]["null_allowed"]);
            checked += 1;
        }
    }
    assert_eq!(checked, 26);
}

#[test]
fn joined_correlation_rejects_unproven_join_forms_and_duplicate_physical_ids() {
    let frozen = freeze();
    let case = frozen["cases"].as_array().unwrap().iter()
        .find(|case| case["variant"] == "joined_composite" && case["fixture_id"] == "75-C132-P")
        .expect("positive nested join fixture");
    let unsupported = [
        case["sql"].as_str().unwrap().replace("i.k = d.k", "i.k > d.k"),
        case["sql"].as_str().unwrap().replace("JOIN dim_rows d", "LEFT JOIN dim_rows d"),
        case["sql"].as_str().unwrap().replace("i.k = o.k AND i.g = o.g", "i.k = o.k OR i.g = o.g"),
    ];
    for sql in unsupported {
        let mut variant = case.clone();
        variant["sql"] = Value::String(sql);
        assert!(matches!(
            evaluate_joined(&variant, "generic"),
            PhysicalJoinedExistsProof::Residual { .. }
        ), "uncertified semantics must fail closed");
    }
    let mut invalid = states(case);
    let inner = invalid.iter().position(|state| state.relation() == "inner_rows").unwrap();
    let mut rows = invalid[inner].rows().to_vec();
    rows.push(rows[0].clone());
    invalid[inner] = PhysicalWriteState::new("inner_rows", rows).expect("contradictory bag");
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let parser = dialect_from_name("generic").unwrap();
    let configured = ConfiguredSqlInput::new("task-132", &sql, "generic", parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog()).unwrap();
    let target = bundle.layers().last().unwrap();
    assert!(matches!(
        physical_finite_joined_exists_bag_plan(&bundle, target.id(), &invalid),
        PhysicalJoinedExistsProof::Impossible { .. }
    ));
}
