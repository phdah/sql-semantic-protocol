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

fn native_source_bag(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("native DuckDB");
    let mut stmt = db.prepare(sql).expect("literal physical source SQL");
    stmt.query_map([], |r| {
        let v = r.get::<_, Option<i64>>(0)?;
        Ok(BTreeMap::from([(
            "v".to_string(),
            v.map_or(LiteralValue::Null, |value| LiteralValue::Number(value.to_string())),
        )]))
    })
    .expect("native complete physical rows")
    .collect::<Result<Vec<_>, _>>()
    .expect("all source rows")
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

#[test]
fn joined_finite_fanout_proves_complete_bags_pairs_and_typed_terminal_domains() {
    let sql = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v GROUP BY t.v";
    let p = source_free(sql);
    let PhysicalFiniteTableProof::Exact(proof) = p else {
        panic!("two-source closed INNER equality bag must be exact: {p:?}");
    };
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
    assert_eq!(proof.source_row_ids.len(), 5, "five matched JOIN producers");
    assert_eq!(proof.contributors.iter().map(Vec::len).sum::<usize>(), 5);
    assert_eq!(
        proof.physical_input_ids["left"],
        ["left/unnest:1", "left/unnest:2", "left/unnest:3"]
    );
    assert_eq!(
        proof.physical_input_ids["right"],
        ["right/values:0", "right/values:1", "right/values:2"]
    );
    assert_eq!(proof.physical_inputs["left"].len(), 3);
    assert_eq!(proof.physical_inputs["right"].len(), 3);
    assert_eq!(
        sorted(&proof.physical_inputs["left"]),
        sorted(&native_source_bag("SELECT v FROM UNNEST([1,1,2]) AS t(v)"))
    );
    assert_eq!(
        sorted(&proof.physical_inputs["right"]),
        sorted(&native_source_bag("SELECT v FROM (VALUES (1),(1),(2)) AS t(v)"))
    );
    assert_eq!(
        proof.joined_selected,
        [
            ["left/unnest:1".to_string(), "right/values:0".to_string()],
            ["left/unnest:1".to_string(), "right/values:1".to_string()],
            ["left/unnest:2".to_string(), "right/values:0".to_string()],
            ["left/unnest:2".to_string(), "right/values:1".to_string()],
            ["left/unnest:3".to_string(), "right/values:2".to_string()]
        ],
        "every physical matched pair has deterministic provenance"
    );
    assert_eq!(proof.joined_selected.len(), 5);
    assert_eq!(proof.joined_not_true.len(), 4);
    assert_eq!(proof.terminal_column_types["v"], FiniteColumnType::Integer);
    assert_eq!(proof.terminal_column_types["n"], FiniteColumnType::Integer);
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
fn finite_join_null_keys_are_not_true_and_reversed_equality_is_equivalent() {
    let first = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2,NULL]) AS t(v) JOIN (VALUES(1),(1),(2),(NULL)) AS u(v) ON t.v = u.v GROUP BY t.v";
    let reverse = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2,NULL]) AS t(v) JOIN (VALUES(1),(1),(2),(NULL)) AS u(v) ON u.v = t.v GROUP BY t.v";
    for sql in [first, reverse] {
        let p = source_free(sql);
        let PhysicalFiniteTableProof::Exact(proof) = p else {
            panic!("nullable equality must use SQL TRUE/NOT TRUE: {p:?}");
        };
        assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
        assert_eq!(proof.joined_selected.len(), 5);
        assert_eq!(proof.joined_not_true.len(), 11);
        assert!(proof
            .joined_not_true
            .iter()
            .any(|ids| ids[0] == "left/unnest:4"));
        assert!(proof
            .joined_not_true
            .iter()
            .any(|ids| ids[1] == "right/values:3"));
        assert!(proof
            .terminal
            .iter()
            .all(|row| row["v"] != LiteralValue::Null));
    }
}

#[test]
fn non_equality_join_and_unproven_multiple_sources_fail_closed() {
    for sql in [
        "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v > u.v GROUP BY t.v",
        "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v AND t.v > 0 GROUP BY t.v",
        "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v GROUP BY u.v",
    ] {
        let (bundle, target) = analyzed(sql, "duckdb");
        assert!(matches!(
            physical_finite_table_plan(&bundle, &target, None),
            PhysicalFiniteTableProof::Residual { .. }
        ), "{sql}: only attested exact join can be exact");
    }
}


#[test]
fn diagnostic_join_ast_for_task140() {
    use sqlparser::ast::{SetExpr, Statement};
    use sqlparser::dialect::DuckDbDialect;
    use sqlparser::parser::Parser;
    let sql = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v GROUP BY t.v";
    let ast = Parser::parse_sql(&DuckDbDialect {}, sql).expect("AST");
    let Statement::Query(query) = &ast[0] else { panic!("not SELECT"); };
    let SetExpr::Select(select) = query.body.as_ref() else { panic!("not select"); };
    let from = &select.from[0];
    panic!("TASK140_AST left={:?}, join={:?}, projection={:?}, group={:?}",
        from.relation, from.joins, select.projection, select.group_by);
}
