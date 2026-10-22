//! TASK-129: frozen full-bag proofs through the public SQL and physical API.
//! Native DuckDB results are collected independently of the implementation.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_exists_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteExistsProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-129-acceptance-freeze.json"))
        .expect("versioned TASK-129 acceptance freeze")
}

fn catalog() -> RelationCatalog {
    let source = |name| {
        RelationSchema::new(
            name,
            vec![
                SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql")
                    .expect("typed row identity"),
                SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                    .expect("nullable signed key"),
            ],
        )
        .expect("physical source schema")
    };
    RelationCatalog::from_schemas(&[source("outer_rows"), source("inner_rows")]).expect("catalog")
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    ["outer_rows", "inner_rows"]
        .into_iter()
        .map(|source| {
            let rows = case["source_bags"][source]
                .as_array()
                .expect("physical rows")
                .iter()
                .map(|row| {
                    let key = row["k"]
                        .as_i64()
                        .map_or(LiteralValue::Null, |k| LiteralValue::Number(k.to_string()));
                    PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Text(
                                row["id"].as_str().expect("stable row id").to_string(),
                            ),
                        ),
                        ("k".to_string(), key),
                    ]))
                    .expect("full physical row")
                })
                .collect();
            PhysicalWriteState::new(source, rows).expect("closed physical source")
        })
        .collect()
}

fn expected_ids(case: &Value, field: &str) -> Vec<String> {
    case[field]
        .as_array()
        .expect("frozen row identities")
        .iter()
        .map(|v| v.as_str().expect("source identity").to_string())
        .collect()
}

fn evaluate(
    case: &Value,
    dialect: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteExistsProof {
    let sql = SqlInput::inline(case["sql"].as_str().expect("frozen SQL"));
    let parser = dialect_from_name(dialect).expect("project dialect");
    let configured = ConfiguredSqlInput::new("task-129", &sql, dialect, parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog())
        .unwrap_or_else(|error| panic!("{} / {dialect}: {error:?}", case["id"]));
    let target = bundle.layers().last().expect("canonical query target");
    physical_finite_exists_bag_plan(&bundle, target.id(), snapshots)
}

#[test]
fn finite_complete_bags_prove_every_frozen_case_across_thirteen_dialects() {
    let frozen = freeze();
    assert_eq!(frozen["version"], 2);
    let cases = frozen["cases"].as_array().expect("case list");
    assert_eq!(cases.len(), 6);
    let dialects = frozen["dialect_identities"].as_array().expect("dialects");
    assert_eq!(dialects.len(), 13);

    for case in cases {
        for adapter in ["direct_sql", "dbt_compiled", "odcs"] {
            assert_eq!(
                case["adapter_status"][adapter], "verified",
                "{} / {adapter}: no unchecked mandatory adapter cells",
                case["id"]
            );
        }
        assert_eq!(case["duckdb_source_and_terminal_oracle"], "verified");
        for column in ["outer_identity", "outer_key", "inner_identity", "inner_key"] {
            assert!(
                case["source_column_instances"][column].as_str().is_some(),
                "{}: missing stable physical column instance {column}",
                case["id"]
            );
        }
        for dialect in dialects {
            let dialect = dialect.as_str().expect("dialect name");
            assert_eq!(
                case["parser_status"][dialect], "parser_equivalent",
                "{} / {dialect}: no unchecked mandatory parser cells",
                case["id"]
            );
            let proof = evaluate(case, dialect, &snapshots(case));
            let PhysicalFiniteExistsProof::Exact(exact) = proof else {
                panic!("{} / {dialect}: {proof:?}", case["id"]);
            };
            assert_eq!(
                exact.sources,
                ["outer_rows".to_string(), "inner_rows".to_string()]
            );
            assert!(exact.nodes.len() >= 3, "canonical source and target graph");
            assert_eq!(
                exact.selected,
                expected_ids(case, "selected_outer_ids"),
                "{} / {dialect}: TRUE source identities",
                case["id"]
            );
            assert_eq!(
                exact.rejected,
                expected_ids(case, "rejected_not_true_outer_ids"),
                "{} / {dialect}: FALSE/UNKNOWN source identities",
                case["id"]
            );
            let expected_partners: BTreeMap<String, Vec<String>> =
                serde_json::from_value(case["complete_inner_partners"].clone())
                    .expect("frozen whole-inner matching populations");
            assert_eq!(
                exact.matching_inner, expected_partners,
                "{} / {dialect}: full inner match/absence evidence",
                case["id"]
            );
            let actual_bag: Vec<Option<i128>> = exact
                .terminal
                .iter()
                .map(|row| match row.get("k").expect("copied projected key") {
                    LiteralValue::Number(value) => {
                        Some(value.parse::<i128>().expect("signed output"))
                    }
                    LiteralValue::Null => None,
                    value => panic!("unexpected output {value:?}"),
                })
                .collect();
            let expected_bag: Vec<Option<i128>> = case["expected_terminal_bag"]
                .as_array()
                .expect("complete terminal bag")
                .iter()
                .map(|row| row["k"].as_i64().map(i128::from))
                .collect();
            assert_eq!(actual_bag, expected_bag, "{} / {dialect}", case["id"]);
            let domain = exact
                .terminal_domains
                .get("k")
                .expect("typed output domain");
            let expected = &case["terminal_value_domain"];
            let expected_values = expected["allowed"]
                .as_array()
                .expect("finite values")
                .iter()
                .map(|v| i128::from(v.as_i64().expect("integer")))
                .collect::<Vec<_>>();
            assert_eq!(domain.values(), expected_values);
            assert_eq!(
                domain.null_allowed(),
                expected["contains_null"]
                    .as_bool()
                    .expect("NULL disposition")
            );
            assert_eq!(
                domain.bounds(),
                expected_values
                    .first()
                    .zip(expected_values.last())
                    .map(|(a, b)| (*a, *b))
            );
        }
    }
}

#[test]
fn independent_duckdb_oracles_assert_entire_source_and_terminal_bags() {
    let frozen = freeze();
    for case in frozen["cases"].as_array().expect("cases") {
        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(
            "CREATE TABLE outer_rows(id VARCHAR NOT NULL, k INTEGER);
             CREATE TABLE inner_rows(id VARCHAR NOT NULL, k INTEGER);",
        )
        .expect("attested source types");
        for relation in ["outer_rows", "inner_rows"] {
            let sql = format!("INSERT INTO {relation}(id,k) VALUES (?, ?)");
            for row in case["source_bags"][relation]
                .as_array()
                .expect("source bag")
            {
                let id = row["id"].as_str().expect("source id");
                let key = row["k"].as_i64();
                db.execute(&sql, params![id, key])
                    .expect("insert source row");
            }
        }
        // Round-trip the complete source bags through the independent engine:
        // frozen inputs are evidence, not assumptions about successful INSERTs.
        for relation in ["outer_rows", "inner_rows"] {
            let sql = format!("SELECT id, k FROM {relation} ORDER BY id");
            let mut rows = db.prepare(&sql).expect("native complete source read");
            let actual = rows
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let key: Option<i64> = row.get(1)?;
                    Ok((id, key))
                })
                .expect("native complete source bag")
                .collect::<duckdb::Result<Vec<_>>>()
                .expect("all physical source identities");
            let mut expected = case["source_bags"][relation]
                .as_array()
                .expect("frozen physical source")
                .iter()
                .map(|row| {
                    (
                        row["id"].as_str().expect("stable ID").to_string(),
                        row["k"].as_i64(),
                    )
                })
                .collect::<Vec<_>>();
            expected.sort();
            assert_eq!(actual, expected, "{}: {relation} full source", case["id"]);
        }
        // Native query independently exposes IDs as well as projected values.
        // Expected rows are fixed by the acceptance freeze, not by the Rust evaluator.
        let sql = case["sql"].as_str().expect("frozen SQL").replacen(
            "SELECT o.k AS k",
            "SELECT o.id, o.k AS k",
            1,
        );
        let mut statement = db.prepare(&sql).expect("DuckDB parser and planner");
        let actual = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let key: Option<i64> = row.get(1)?;
                Ok((id, key))
            })
            .expect("DuckDB evaluated rows")
            .collect::<duckdb::Result<Vec<_>>>()
            .expect("DuckDB complete output bag");
        let mut actual = actual;
        actual.sort();
        let mut expected = case["selected_outer_ids"]
            .as_array()
            .expect("selected IDs")
            .iter()
            .map(|id| {
                let id = id.as_str().expect("stable id");
                let key = case["source_bags"]["outer_rows"]
                    .as_array()
                    .expect("outer rows")
                    .iter()
                    .find(|row| row["id"] == id)
                    .expect("selected physical row")["k"]
                    .as_i64();
                (id.to_string(), key)
            })
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(actual, expected, "{}: native complete output", case["id"]);
        // EXISTS and NOT EXISTS are logical complements for the same complete
        // physical bags. Independently execute the inverse and assert every
        // SQL NOT TRUE row, including outer NULL and unmatched non-NULL keys.
        let inverse_sql = if sql.contains(" WHERE NOT EXISTS ") {
            sql.replacen(" WHERE NOT EXISTS ", " WHERE EXISTS ", 1)
        } else {
            sql.replacen(" WHERE EXISTS ", " WHERE NOT EXISTS ", 1)
        };
        let mut inverse = db.prepare(&inverse_sql).expect("native inverse predicate");
        let mut actual_rejected = inverse
            .query_map([], |row| row.get::<_, String>(0))
            .expect("native inverse output")
            .collect::<duckdb::Result<Vec<_>>>()
            .expect("complete NOT TRUE identities");
        actual_rejected.sort();
        let mut expected_rejected = expected_ids(case, "rejected_not_true_outer_ids");
        expected_rejected.sort();
        assert_eq!(
            actual_rejected, expected_rejected,
            "{}: independent NOT TRUE source identities",
            case["id"]
        );
    }
}

#[test]
fn unsupported_nested_filter_and_unshaped_queries_fail_closed() {
    let frozen = freeze();
    let case = &frozen["cases"][0];
    for sql in [
        "SELECT o.k AS k FROM outer_rows o WHERE EXISTS (SELECT 1 FROM inner_rows i WHERE i.k > o.k)",
        "SELECT DISTINCT o.k AS k FROM outer_rows o WHERE EXISTS (SELECT 1 FROM inner_rows i WHERE i.k = o.k)",
        "SELECT o.k AS k FROM outer_rows o WHERE EXISTS (SELECT 1 FROM inner_rows i WHERE i.k = o.k) LIMIT 1",
    ] {
        let mut variant = case.clone();
        variant["sql"] = Value::String(sql.to_string());
        assert!(
            matches!(
                evaluate(&variant, "generic", &snapshots(case)),
                PhysicalFiniteExistsProof::Residual { .. }
            ),
            "{sql} must not silently become an exact proof"
        );
    }
}

#[test]
fn repeated_physical_row_ids_cannot_certify_closed_world_absence() {
    let frozen = freeze();
    let case = &frozen["cases"][1];
    let states = snapshots(case);
    let duplicate = states[0].rows()[0].clone();
    let mut outer = states[0].rows().to_vec();
    outer.push(duplicate);
    let invalid = vec![
        PhysicalWriteState::new("outer_rows", outer).expect("source"),
        states[1].clone(),
    ];
    assert!(matches!(
        evaluate(case, "generic", &invalid),
        PhysicalFiniteExistsProof::Impossible { .. }
    ));
}
