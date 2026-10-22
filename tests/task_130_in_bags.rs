//! TASK-130: complete source-bag IN/NOT IN three-valued proofs at the public API.
//! DuckDB independently evaluates terminal bags and FALSE versus UNKNOWN cases.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_in_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteInProof, PhysicalInTruth, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-130-acceptance-freeze.json"))
        .expect("versioned TASK-130 acceptance inventory")
}

fn catalog() -> RelationCatalog {
    let source = |name| {
        RelationSchema::new(
            name,
            vec![
                SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").expect("id type"),
                SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").expect("key type"),
                SchemaColumn::from_sql_type("g", "INTEGER", "postgresql").expect("group type"),
                SchemaColumn::from_sql_type("v", "INTEGER", "postgresql")
                    .expect("tuple value type"),
            ],
        )
        .expect("source schema")
    };
    RelationCatalog::from_schemas(&[source("outer_rows"), source("inner_rows")]).expect("catalog")
}

fn source_states(case: &Value) -> Vec<PhysicalWriteState> {
    ["outer_rows", "inner_rows"]
        .into_iter()
        .map(|relation| {
            let rows = case["source_bags"][relation]
                .as_array()
                .expect("source snapshot")
                .iter()
                .map(|row| {
                    let integer = |name| {
                        row[name]
                            .as_i64()
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
                    };
                    PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Text(row["id"].as_str().expect("row id").to_string()),
                        ),
                        ("k".to_string(), integer("k")),
                        ("g".to_string(), integer("g")),
                        ("v".to_string(), integer("v")),
                    ]))
                    .expect("source row")
                })
                .collect();
            PhysicalWriteState::new(relation, rows).expect("physical source")
        })
        .collect()
}

fn evaluate(case: &Value, dialect: &str) -> PhysicalFiniteInProof {
    let sql = SqlInput::inline(case["sql"].as_str().expect("SQL"));
    let parser = dialect_from_name(dialect).expect("project dialect");
    let configured = ConfiguredSqlInput::new("task-130", &sql, dialect, parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog())
        .unwrap_or_else(|e| panic!("{} {dialect}: {e:?}", case["id"]));
    let target = bundle.layers().last().expect("query layer");
    physical_finite_in_bag_plan(&bundle, target.id(), &source_states(case))
}

fn ids(case: &Value, key: &str) -> Vec<String> {
    case[key]
        .as_array()
        .expect("frozen identities")
        .iter()
        .map(|v| v.as_str().expect("id").to_string())
        .collect()
}

fn truth(name: &str) -> PhysicalInTruth {
    match name {
        "true" => PhysicalInTruth::True,
        "false" => PhysicalInTruth::False,
        "unknown" => PhysicalInTruth::Unknown,
        _ => panic!("unrecognized SQL truth"),
    }
}

#[test]
fn complete_scalar_in_and_not_in_bags_prove_null_and_correlated_cases() {
    let f = freeze();
    assert_eq!(f["version"], 2);
    let cases = f["cases"].as_array().expect("finite cases");
    assert_eq!(cases.len(), 15);
    let dialects = f["required_dialects"]
        .as_array()
        .expect("all project dialects");
    assert_eq!(dialects.len(), 13);
    for case in cases {
        for dialect in dialects {
            let dialect = dialect.as_str().expect("dialect");
            let PhysicalFiniteInProof::Exact(exact) = evaluate(case, dialect) else {
                panic!(
                    "{} / {dialect}: mandatory scalar case must be exact",
                    case["id"]
                );
            };
            assert_eq!(exact.sources, ["outer_rows", "inner_rows"]);
            assert_eq!(exact.selected, ids(case, "selected_outer_ids"));
            assert_eq!(exact.rejected, ids(case, "rejected_not_true_outer_ids"));
            for (outer_id, ids) in case["complete_candidate_inner_ids"]
                .as_object()
                .expect("complete candidate identity matrix")
            {
                let expected = ids
                    .as_array()
                    .expect("candidate ids")
                    .iter()
                    .map(|v| v.as_str().expect("candidate id").to_string())
                    .collect::<Vec<_>>();
                assert_eq!(
                    exact.candidate_inner.get(outer_id),
                    Some(&expected),
                    "{} / {dialect}: exact correlated candidates for {outer_id}",
                    case["id"]
                );
            }
            assert_eq!(
                exact.selected.len() + exact.rejected.len(),
                case["source_bags"]["outer_rows"].as_array().unwrap().len()
            );
            for (id, expected) in case["truth"].as_object().expect("truth map") {
                assert_eq!(
                    exact.truth.get(id),
                    Some(&truth(expected.as_str().expect("truth"))),
                    "{} {dialect} row {id}",
                    case["id"]
                );
            }
            let expected_terminal = case["terminal"]
                .as_array()
                .expect("literal output bag")
                .iter()
                .map(|row| {
                    BTreeMap::from([(
                        "k".to_string(),
                        row["k"]
                            .as_i64()
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    )])
                })
                .collect::<Vec<_>>();
            assert_eq!(exact.terminal, expected_terminal);
            let domain = exact.terminal_domains.get("k").expect("typed terminal");
            let mut values = case["terminal"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|row| row["k"].as_i64().map(i128::from))
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            assert_eq!(domain.values(), values.as_slice());
            assert_eq!(
                domain.null_allowed(),
                case["terminal"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["k"].is_null())
            );
            assert_eq!(
                domain.bounds(),
                values.first().copied().zip(values.last().copied())
            );
        }
    }
}

// Independent SQL standard row equality: AND of fieldwise comparisons means
// FALSE dominates UNKNOWN on a definite mismatch. Some DuckDB versions use a
// different native tuple IN NULL rule, so execute the portable truth law as SQL
// without reparsing or invoking the Rust physical evaluator.
fn portable_tuple_truth_sql(case: &Value) -> String {
    let sql = case["sql"].as_str().expect("frozen SQL");
    let negated = sql.contains(" NOT IN ");
    let correlation = if sql.contains("WHERE i.g = o.g") {
        "i.g = o.g AND "
    } else {
        ""
    };
    let comparison = "(o.k = i.k AND o.v = i.v)";
    let any_true =
        format!("EXISTS (SELECT 1 FROM inner_rows i WHERE {correlation}{comparison} IS TRUE)");
    let any_unknown =
        format!("EXISTS (SELECT 1 FROM inner_rows i WHERE {correlation}{comparison} IS NULL)");
    let when_matched = if negated { "false" } else { "true" };
    let when_absent = if negated { "true" } else { "false" };
    format!(
        "CASE WHEN {any_true} THEN '{when_matched}' WHEN {any_unknown} THEN 'unknown' ELSE '{when_absent}' END"
    )
}

#[test]
fn duckdb_independently_proves_complete_terminal_bags_and_three_valued_truth() {
    for case in freeze()["cases"].as_array().expect("cases") {
        let db = Connection::open_in_memory().expect("DuckDB");
        for relation in ["outer_rows", "inner_rows"] {
            db.execute(
                &format!("CREATE TABLE {relation} (id VARCHAR, k INTEGER, g INTEGER, v INTEGER)"),
                [],
            )
            .expect("create native source");
            for row in case["source_bags"][relation].as_array().expect("rows") {
                db.execute(
                    &format!("INSERT INTO {relation} VALUES (?, ?, ?, ?)"),
                    params![
                        row["id"].as_str().expect("id"),
                        row["k"].as_i64(),
                        row["g"].as_i64(),
                        row["v"].as_i64(),
                    ],
                )
                .expect("insert native source row");
            }
        }
        let sql = case["sql"].as_str().expect("SQL");
        let tuple = sql.contains("(o.k, o.v)");
        let projected = if tuple {
            format!(
                "SELECT o.id, o.k AS k FROM outer_rows o WHERE ({}) = 'true'",
                portable_tuple_truth_sql(case)
            )
        } else {
            sql.replacen("SELECT o.k AS k", "SELECT o.id, o.k AS k", 1)
        };
        let mut stmt = db.prepare(&projected).expect("native SQL");
        let mut actual = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
            })
            .expect("native terminal")
            .collect::<duckdb::Result<Vec<_>>>()
            .expect("terminal rows");
        actual.sort();
        let mut expected = ids(case, "selected_outer_ids")
            .iter()
            .map(|id| {
                let k = case["source_bags"]["outer_rows"]
                    .as_array()
                    .expect("outer bag")
                    .iter()
                    .find(|r| r["id"] == id.as_str())
                    .expect("selected source")["k"]
                    .as_i64();
                (id.clone(), k)
            })
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(actual, expected, "{} native terminal bag", case["id"]);

        // Native CASE distinguishes SQL FALSE from UNKNOWN, unlike a WHERE
        // complement which would incorrectly classify NULL comparisons.
        let predicate = sql.split_once(" WHERE ").expect("WHERE clause").1;
        let truth_expression = if tuple {
            portable_tuple_truth_sql(case)
        } else {
            format!(
                "CASE WHEN {predicate} THEN 'true' WHEN NOT ({predicate}) THEN 'false' ELSE 'unknown' END"
            )
        };
        let truth_sql = format!("SELECT o.id, {truth_expression} FROM outer_rows o");
        let mut stmt = db.prepare(&truth_sql).expect("native truth expression");
        let actual_truth = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("native truth rows")
            .collect::<duckdb::Result<Vec<_>>>()
            .expect("native truth")
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let expected_truth = case["truth"]
            .as_object()
            .expect("truth")
            .iter()
            .map(|(id, v)| (id.clone(), v.as_str().unwrap().to_string()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            actual_truth, expected_truth,
            "{} native three-valued truth",
            case["id"]
        );
    }
}

#[test]
fn unsupported_nested_predicate_must_fail_closed() {
    let mut case = freeze()["cases"][4].clone();
    case["sql"] = Value::String(
        "SELECT o.k AS k FROM outer_rows o WHERE o.k IN (SELECT i.k FROM inner_rows i WHERE i.g > o.g)"
            .to_string(),
    );
    assert!(matches!(
        evaluate(&case, "generic"),
        PhysicalFiniteInProof::Residual { .. }
    ));
}

#[test]
fn duplicate_physical_row_ids_cannot_certify_a_closed_world_bag() {
    let f = freeze();
    let case = &f["cases"][0];
    let mut states = source_states(case);
    let mut rows = states[0].rows().to_vec();
    rows.push(rows[0].clone());
    states[0] = PhysicalWriteState::new("outer_rows", rows).expect("source snapshot");
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let parser = dialect_from_name("generic").expect("dialect");
    let input = ConfiguredSqlInput::new("task-130", &sql, "generic", parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog()).expect("bundle");
    let target = bundle.layers().last().expect("target");
    assert!(matches!(
        physical_finite_in_bag_plan(&bundle, target.id(), &states),
        PhysicalFiniteInProof::Impossible { .. }
    ));
}

#[test]
fn compound_key_membership_is_independently_certified() {
    let frozen = freeze();
    let cases = frozen["cases"].as_array().expect("finite cases");
    let tuples = cases
        .iter()
        .filter(|case| case["id"].as_str().unwrap().contains("tuple"))
        .collect::<Vec<_>>();
    assert_eq!(tuples.len(), 9);
    for case in tuples {
        let PhysicalFiniteInProof::Exact(exact) = evaluate(case, "duckdb") else {
            panic!("{} must prove tuple membership exactly", case["id"]);
        };
        assert_eq!(exact.selected, ids(case, "selected_outer_ids"));
        assert_eq!(exact.rejected, ids(case, "rejected_not_true_outer_ids"));
    }
}
