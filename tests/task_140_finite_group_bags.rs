//! TASK-140 public complete-bag proof for finite fanout followed by grouping.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, FiniteColumnType, LiteralValue,
    PhysicalFiniteTableProof, SqlInput,
};

fn analyzed(sql: &str, dialect_name: &str) -> (sql_semantic_protocol::AnalysisBundle, String) {
    let dialect = dialect_from_name(dialect_name).expect("exposed parser dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref())
        .expect("valid query");
    let target = bundle
        .layers()
        .last()
        .expect("one terminal layer")
        .id()
        .to_string();
    (bundle, target)
}

fn source_free(sql: &str) -> PhysicalFiniteTableProof {
    let (bundle, target) = analyzed(sql, "duckdb");
    physical_finite_table_plan(&bundle, &target, None)
}

fn duckdb_bag(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("independent DuckDB");
    let mut stmt = db.prepare(sql).expect("valid native oracle SQL");
    stmt.query_map([], |row| {
        let v = row.get::<_, Option<i64>>(0)?;
        let n = row.get::<_, i64>(1)?;
        Ok(BTreeMap::from([
            (
                "v".to_string(),
                v.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
            ),
            ("n".to_string(), LiteralValue::Number(n.to_string())),
        ]))
    })
    .expect("native rows")
    .collect::<Result<Vec<_>, _>>()
    .expect("complete native bag")
}

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut keys = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

#[test]
fn frozen_inventory_names_both_directions_and_join_and_adapter_obligations() {
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-140-acceptance-freeze.json"))
            .expect("versioned task-owned acceptance inventory");
    assert_eq!(matrix["owner"], "TASK-140");
    assert_eq!(
        matrix["fixture_ids"],
        serde_json::json!(["77-C140-P", "77-C140-N"])
    );
    assert_eq!(matrix["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(matrix["adapter_sources"].as_array().unwrap().len(), 3);
    assert!(matrix["variants"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["family"] == "unnest_join_group"));
    assert_eq!(matrix["verification"]["task_done"], false);
}

#[test]
fn finite_unnest_duplicates_group_into_exact_counts_with_complete_source_identities() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) GROUP BY v";
    let PhysicalFiniteTableProof::Exact(proof) = source_free(sql) else {
        panic!("closed literal UNNEST followed by GROUP BY should be exact");
    };
    assert_eq!(proof.source_row_ids, ["unnest:1", "unnest:2", "unnest:3"]);
    assert_eq!(
        proof.contributors,
        [
            ["unnest:1".to_string(), "unnest:2".to_string()].to_vec(),
            ["unnest:3".to_string()].to_vec()
        ]
    );
    assert_eq!(proof.terminal.len(), 2);
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
    assert_eq!(proof.terminal_column_types["v"], FiniteColumnType::Integer);
    assert_eq!(proof.terminal_column_types["n"], FiniteColumnType::Integer);
    assert!(proof
        .terminal_domains
        .values()
        .all(|value| !matches!(value, sql_semantic_protocol::ValueDomain::Unknown(_))));
}

#[test]
fn absent_group_is_impossible_from_complete_finite_source() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) GROUP BY v";
    let (bundle, target) = analyzed(sql, "duckdb");
    let wanted = BTreeMap::from([
        ("v".to_string(), LiteralValue::Number("3".to_string())),
        ("n".to_string(), LiteralValue::Number("1".to_string())),
    ]);
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, Some(&wanted)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn finite_values_group_preserves_null_as_one_group_and_duplicate_producers() {
    let sql = "SELECT v, COUNT(*) AS n FROM (VALUES (1),(1),(NULL)) AS t(v) GROUP BY v";
    let PhysicalFiniteTableProof::Exact(proof) = source_free(sql) else {
        panic!("complete VALUES bag with NULL grouping must be exact");
    };
    assert_eq!(proof.source_row_ids, ["values:0", "values:1", "values:2"]);
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
    assert!(proof
        .terminal
        .iter()
        .any(|row| row["v"] == LiteralValue::Null
            && row["n"] == LiteralValue::Number("1".to_string())));
    assert_eq!(proof.contributors.iter().map(Vec::len).sum::<usize>(), 3);
}

#[test]
fn opaque_fanout_cannot_claim_exact_closed_world_membership() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST(opaque_array) AS t(v) GROUP BY v";
    let (bundle, target) = analyzed(sql, "duckdb");
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, None),
        PhysicalFiniteTableProof::Residual { .. }
    ));
}
