//! TASK-135 physical proof of set branch pagination before set bag arithmetic.
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_set_tree_membership_plan, physical_finite_set_tree_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const SQL: &str = "(SELECT k FROM s ORDER BY k LIMIT 3) UNION ALL (SELECT k FROM s WHERE k = 5) ORDER BY k LIMIT 3";

fn bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let inputs = [ConfiguredSqlInput::new(
        "TASK-135-branch",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    let schema = RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![
            SchemaColumn::from_sql_type("id", "TEXT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap();
    analyze_configured_inputs_with_catalog(&inputs, &schema).unwrap()
}

fn state(values: &[Option<i64>]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), LiteralValue::Text(format!("row{i}"))),
                    (
                        "k".to_string(),
                        value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn native(sql: &str, values: &[Option<i64>]) -> (Vec<Option<i64>>, Vec<Option<i64>>) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s (id TEXT, k INTEGER)")
        .unwrap();
    for (i, value) in values.iter().enumerate() {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ('row{i}', {})",
            value.map_or("NULL".to_string(), |n| n.to_string())
        ))
        .unwrap();
    }
    let query = |sql: &str| {
        let mut stmt = db.prepare(sql).unwrap();
        stmt.query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    (query("SELECT k FROM s ORDER BY id"), query(sql))
}

#[test]
fn branch_limit_precedes_union_all_and_final_limit() {
    let values = [Some(1), Some(2), Some(3), Some(4), Some(5)];
    let (input, output) = native(SQL, &values);
    assert_eq!(input, values);
    assert_eq!(output, vec![Some(1), Some(2), Some(3)]);
    let analyzed = bundle(SQL);
    let layer = analyzed.layers().last().unwrap().id();
    let proof = physical_finite_set_tree_plan(&analyzed, layer, &[state(&values)]);
    let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
        panic!("branch-local pagination must be exact: {proof:?}");
    };
    let result = exact
        .terminal
        .iter()
        .map(|row| match row.get("k").unwrap() {
            LiteralValue::Number(value) => Some(value.parse::<i64>().unwrap()),
            LiteralValue::Null => None,
            other => panic!("unexpected value {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(result, output);
    assert_eq!(exact.sources, ["s"]);
    assert_eq!(exact.branches.len(), 2);
    assert_eq!(exact.branches[0].selected_row_ids, ["row0", "row1", "row2"]);
    assert_eq!(exact.branches[0].page_rejected_row_ids, ["row3", "row4"]);
    assert_eq!(exact.branches[1].selected_row_ids, ["row4"]);
}

#[test]
fn branch_pagination_zero_terminal_is_exact_not_fabricated() {
    let sql = "(SELECT k FROM s ORDER BY k LIMIT 2) UNION ALL (SELECT k FROM s WHERE k = 5) ORDER BY k LIMIT 0";
    let values = [Some(1), Some(2), Some(3), Some(4), Some(5)];
    assert!(native(sql, &values).1.is_empty());
    let analyzed = bundle(sql);
    let layer = analyzed.layers().last().unwrap().id();
    let proof = physical_finite_set_tree_plan(&analyzed, layer, &[state(&values)]);
    let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
        panic!("set branch with zero terminal must be exact: {proof:?}");
    };
    assert!(exact.terminal.is_empty());
    assert!(exact
        .terminal_domains
        .iter()
        .all(|domain| domain.values.is_empty()));
}

#[test]
fn incomplete_physical_peer_order_or_implicit_null_sort_stays_residual() {
    for values in [
        vec![Some(1), Some(1), Some(3), Some(4), Some(5)],
        vec![Some(1), None, Some(3), Some(4), Some(5)],
    ] {
        let analyzed = bundle(SQL);
        let layer = analyzed.layers().last().unwrap().id();
        assert!(matches!(
            physical_finite_set_tree_plan(&analyzed, layer, &[state(&values)]),
            PhysicalFiniteSetTreeProof::Residual { .. }
        ));
    }
}

#[test]
fn absent_output_of_composed_set_pages_is_impossible_not_residual() {
    let values = [Some(1), Some(2), Some(3), Some(4), Some(5)];
    let analyzed = bundle(SQL);
    let target = analyzed.layers().last().unwrap().id();
    let wanted = BTreeMap::from([("k".to_string(), LiteralValue::Number("4".to_string()))]);
    assert!(matches!(
        physical_finite_set_tree_membership_plan(&analyzed, target, &[state(&values)], &wanted),
        PhysicalFiniteSetTreeProof::Impossible { .. }
    ));
    let selected = BTreeMap::from([("k".to_string(), LiteralValue::Number("2".to_string()))]);
    assert!(matches!(
        physical_finite_set_tree_membership_plan(&analyzed, target, &[state(&values)], &selected),
        PhysicalFiniteSetTreeProof::Exact(_)
    ));
}

#[test]
fn filtered_branch_page_cannot_apply_limit_before_where() {
    // The first branch's WHERE eliminates low keys before its LIMIT.
    // A physical plan that pages the whole source first would be unsound.
    let sql = "(SELECT k FROM s WHERE k > 2 ORDER BY k LIMIT 1) UNION ALL (SELECT k FROM s WHERE k = 1)";
    let values = [Some(1), Some(2), Some(3), Some(4), Some(5)];
    assert_eq!(native(sql, &values).1.len(), 2);
    let analyzed = bundle(sql);
    let layer = analyzed.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_set_tree_plan(&analyzed, layer, &[state(&values)]),
        PhysicalFiniteSetTreeProof::Residual { .. }
    ));
}
