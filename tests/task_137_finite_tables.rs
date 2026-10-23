//! TASK-137 independent literal-table and array-fanout physical regressions.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, FiniteTableKind, LiteralValue,
    PhysicalFiniteTableProof, SqlInput, ValueDomain,
};

const DIALECTS: [&str; 13] = [
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

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-137-acceptance-freeze.json"))
        .expect("frozen table constructor matrix")
}

fn result(sql: &str, dialect_name: &str) -> PhysicalFiniteTableProof {
    let parser = dialect_from_name(dialect_name).expect("supported project dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, parser.as_ref())
        .unwrap_or_else(|error| panic!("{dialect_name} / {sql}: {error:?}"));
    let layer = bundle.layers().last().expect("one query layer");
    physical_finite_table_plan(&bundle, layer.id(), None)
}

fn db_bag(sql: &str, names: &[&str]) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("DuckDB");
    let mut statement = db.prepare(sql).expect("independent oracle SQL");
    statement
        .query_map([], |row| {
            let mut out = BTreeMap::new();
            for (column, name) in names.iter().enumerate() {
                let value = row.get::<_, Option<i64>>(column)?;
                out.insert(
                    (*name).to_string(),
                    value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                );
            }
            Ok(out)
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rows = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn immutable_matrix_covers_required_variants_without_fabricated_evidence() {
    let matrix = frozen();
    assert_eq!(matrix["version"], 1);
    assert_eq!(matrix["owner"], "TASK-137");
    assert_eq!(
        matrix["fixture_ids"],
        serde_json::json!(["77-C137-P", "77-C137-N"])
    );
    assert_eq!(matrix["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(matrix["cases"].as_array().unwrap().len(), 8);
    assert_eq!(matrix["validation"]["full_ci_signoff"], false);
    for case in matrix["cases"].as_array().unwrap() {
        assert!(case["sql"].as_str().is_some());
        assert!(case["variant"].as_str().is_some());
        assert!(case["domains"].is_object());
        assert!(case["positive"].is_array());
        assert!(case["rejected"].is_array());
        assert_eq!(case["source"], serde_json::json!([]));
    }
}

#[test]
fn values_exact_duplicates_nulls_impossible_absence_and_full_duckdb_bag() {
    for (sql, expected, ids) in [
        (
            "VALUES (1), (2)",
            "SELECT column1 FROM (VALUES (1), (2)) AS t(column1)",
            vec!["values:0", "values:1"],
        ),
        (
            "VALUES (1), (1), (NULL)",
            "SELECT column1 FROM (VALUES (1), (1), (NULL)) AS t(column1)",
            vec!["values:0", "values:1", "values:2"],
        ),
    ] {
        let PhysicalFiniteTableProof::Exact(proof) = result(sql, "generic") else {
            panic!("VALUES must prove a complete bag: {sql}");
        };
        assert_eq!(proof.row_ids, ids, "duplicate sources retain identity");
        assert_eq!(bag(&proof.terminal), bag(&db_bag(expected, &["column1"])));
        assert_eq!(proof.terminal.len(), ids.len());
        assert!(proof.terminal_domains["column1"].admits_null().is_some());
    }
    let parser = dialect_from_name("generic").unwrap();
    let bundle = analyze_inputs(
        &[SqlInput::inline("VALUES (1), (1), (NULL)")],
        "generic",
        parser.as_ref(),
    )
    .unwrap();
    let target = bundle.layers().last().unwrap().id();
    let absent = BTreeMap::from([("column1".to_string(), LiteralValue::Number("9".to_string()))]);
    assert!(matches!(
        physical_finite_table_plan(&bundle, target, Some(&absent)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn unnest_literal_complete_bags_and_ordinality_use_independent_duckdb() {
    let fixtures = [
        (
            "SELECT * FROM UNNEST([1, 2]) AS t(v)",
            "SELECT v FROM UNNEST([1,2]) AS t(v)",
            vec!["v"],
        ),
        (
            "SELECT * FROM UNNEST([1, 2]) WITH ORDINALITY AS t(v, ord)",
            "SELECT v, ord FROM UNNEST([1,2]) WITH ORDINALITY AS t(v,ord)",
            vec!["v", "ord"],
        ),
        (
            "SELECT * FROM UNNEST(CAST([] AS INTEGER[])) AS t(v)",
            "SELECT v FROM UNNEST([]::INTEGER[]) AS t(v)",
            vec!["v"],
        ),
    ];
    for (sql, oracle, names) in fixtures {
        let PhysicalFiniteTableProof::Exact(proof) = result(sql, "duckdb") else {
            panic!("attested literal UNNEST should prove exact output: {sql}");
        };
        assert_eq!(bag(&proof.terminal), bag(&db_bag(oracle, &names)));
        assert!(proof
            .terminal_domains
            .values()
            .all(|domain| { !matches!(domain, ValueDomain::Unknown(_)) }));
        if sql.contains("ORDINALITY") {
            assert_eq!(proof.row_ids, ["unnest:0", "unnest:1"]);
        }
        if sql.contains("CAST([]") {
            assert!(proof.terminal.is_empty());
            assert!(proof.row_ids.is_empty());
        }
    }
}

#[test]
fn no_opaque_element_source_or_unproven_order_is_exact() {
    for sql in [
        "SELECT * FROM UNNEST(opaque_array) AS t(v)",
        "SELECT * FROM UNNEST([1, 2]) AS t(v) LIMIT 1",
        "SELECT * FROM UNNEST([1, 2]) AS t(v) ORDER BY v",
        "SELECT * FROM UNNEST([1, 2]) AS t(v) WHERE v > 1",
        "SELECT * FROM UNNEST([]) AS t(v)",
    ] {
        assert!(
            !matches!(result(sql, "duckdb"), PhysicalFiniteTableProof::Exact(_)),
            "unsupported/opaque table form cannot be exact: {sql}"
        );
    }
}

#[test]
fn canonical_values_are_shared_across_exposed_parser_dialects() {
    for dialect in DIALECTS {
        let PhysicalFiniteTableProof::Exact(proof) = result("VALUES (1), (2)", dialect) else {
            panic!("VALUES is a canonical source-free operation: {dialect}");
        };
        assert_eq!(proof.terminal.len(), 2);
        assert_eq!(proof.row_ids, ["values:0", "values:1"]);
    }
}

#[test]
fn source_free_parser_normalization_retains_operator_identity() {
    let parser = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_inputs(
        &[SqlInput::inline("SELECT * FROM UNNEST([1, 2]) AS t(v)")],
        "duckdb",
        parser.as_ref(),
    )
    .unwrap();
    let query = bundle.inputs()[0]
        .statements()
        .iter()
        .find_map(|statement| match statement {
            sql_semantic_protocol::ProtocolStatement::Query(query) => Some(query),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        query.finite_table().unwrap().kind(),
        FiniteTableKind::Unnest
    );
    assert!(query.dependencies().is_empty());
    assert_eq!(query.finite_table().unwrap().rows().len(), 2);
}
