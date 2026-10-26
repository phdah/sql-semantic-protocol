//! TASK-135: source-complete nested CTE pagination at the public physical-proof seam.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_composed_page_membership_plan, physical_finite_composed_page_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalComposedPageProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

const SQL: &str = "WITH page AS (SELECT k FROM s ORDER BY k LIMIT 4) \
SELECT k FROM page ORDER BY k LIMIT 1 OFFSET 2";

fn evaluate(
    sql: &str,
    values: &[Option<i64>],
) -> (sql_semantic_protocol::AnalysisBundle, PhysicalWriteState) {
    let parser = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let inputs = [ConfiguredSqlInput::new(
        "page",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let source = PhysicalWriteState::new(
        "s",
        values
            .iter()
            .map(|value| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    (bundle, source)
}

fn source_and_result_from_duckdb(
    sql: &str,
    values: &[Option<i64>],
) -> (Vec<Option<i64>>, Vec<Option<i64>>) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s (k INTEGER)").unwrap();
    for value in values {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({})",
            value.map_or("NULL".to_string(), |v| v.to_string())
        ))
        .unwrap();
    }
    let query = |sql: &str| {
        let mut statement = db.prepare(sql).unwrap();
        statement
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    (query("SELECT k FROM s ORDER BY k NULLS LAST"), query(sql))
}

#[test]
fn two_pages_compose_without_moving_outer_offset_across_inner_limit() {
    let values = [Some(1), Some(2), Some(3), Some(4), Some(5)];
    let (duck_source, duck_output) = source_and_result_from_duckdb(SQL, &values);
    assert_eq!(duck_source, values);
    assert_eq!(duck_output, vec![Some(3)]);

    let (bundle, source) = evaluate(SQL, &values);
    let layer = bundle.layers().last().unwrap().id();
    let result = physical_finite_composed_page_plan(&bundle, layer, std::slice::from_ref(&source));
    let PhysicalComposedPageProof::Exact(exact) = result else {
        panic!("expected exact nested page: {result:?}");
    };
    assert_eq!(exact.source, "s");
    assert_eq!(
        exact.source_row_ids,
        vec!["s#0", "s#1", "s#2", "s#3", "s#4"]
    );
    assert_eq!(exact.selected_source_ids, vec!["s#2"]);
    assert_eq!(exact.rejected_source_ids, vec!["s#0", "s#1", "s#3", "s#4"]);
    assert_eq!(
        exact.terminal,
        vec![BTreeMap::from([(
            "k".to_string(),
            LiteralValue::Number("3".to_string()),
        )])]
    );
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));
    assert!(matches!(exact.terminal_domains["k"], ValueDomain::Set(_)));

    let (bundle, source) = evaluate(SQL, &values);
    let layer = bundle.layers().last().unwrap().id();
    let wanted = BTreeMap::from([("k".to_string(), LiteralValue::Number("1".to_string()))]);
    assert!(matches!(
        physical_finite_composed_page_membership_plan(&bundle, layer, &[source], &wanted),
        PhysicalComposedPageProof::Impossible { .. }
    ));
}

#[test]
fn nested_page_empty_source_proves_empty_terminal_bag() {
    let (bundle, source) = evaluate(SQL, &[]);
    let layer = bundle.layers().last().unwrap().id();
    let proof = physical_finite_composed_page_plan(&bundle, layer, &[source]);
    let PhysicalComposedPageProof::Exact(exact) = proof else {
        panic!("expected exact empty page: {proof:?}");
    };
    assert!(exact.terminal.is_empty());
    assert!(matches!(exact.terminal_domains["k"], ValueDomain::Empty));
}

#[test]
fn nested_pages_reject_ambiguous_peer_identity_and_implicit_null_order() {
    let (bundle, source) = evaluate(SQL, &[Some(1), Some(2), Some(2), Some(4), Some(5)]);
    let layer = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_composed_page_plan(&bundle, layer, &[source]),
        PhysicalComposedPageProof::Residual { .. }
    ));

    let (bundle, source) = evaluate(SQL, &[Some(1), None, Some(3), Some(4), Some(5)]);
    let layer = bundle.layers().last().unwrap().id();
    assert!(matches!(
        physical_finite_composed_page_plan(&bundle, layer, &[source]),
        PhysicalComposedPageProof::Residual { .. }
    ));
}


#[test]
fn nullable_duplicate_keys_with_unique_tiebreakers_keep_correct_physical_identities() {
    let sql = "WITH page AS (SELECT k, seq FROM s \
ORDER BY k NULLS LAST, seq LIMIT 4) \
SELECT k, seq FROM page ORDER BY k NULLS LAST, seq DESC LIMIT 2 OFFSET 1";
    let values = [(None, 5), (Some(1), 2), (Some(1), 1), (Some(2), 3), (Some(3), 4)];
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER, seq INTEGER)").unwrap();
    for (k, seq) in &values {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {seq})",
            k.map_or("NULL".to_string(), |n| n.to_string())
        )).unwrap();
    }
    let actual = db.prepare(sql).unwrap().query_map([], |row| {
        Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
    }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(actual, vec![(Some(1), 1), (Some(2), 3)]);

    let schema = RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![
            SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("seq", "INTEGER", "postgresql").unwrap(),
        ],
    ).unwrap()]).unwrap();
    let parser = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "TASK-135-null-duplicates", &input, "duckdb", parser.as_ref()
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &schema).unwrap();
    let state = PhysicalWriteState::new(
        "s",
        values.iter().map(|(k, seq)| {
            PhysicalWriteRow::new(BTreeMap::from([
                ("k".to_string(), k.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))),
                ("seq".to_string(), LiteralValue::Number(seq.to_string())),
            ])).unwrap()
        }).collect()
    ).unwrap();
    let target = bundle.layers().last().unwrap().id();
    let proof = physical_finite_composed_page_plan(&bundle, target, &[state]);
    let PhysicalComposedPageProof::Exact(exact) = proof else {
        panic!("NULL and duplicate k with unique secondary ordering must be exact: {proof:?}");
    };
    assert_eq!(exact.source_row_ids, vec!["s#0", "s#1", "s#2", "s#3", "s#4"]);
    assert_eq!(exact.selected_source_ids, vec!["s#2", "s#3"]);
    assert_eq!(exact.rejected_source_ids, vec!["s#0", "s#1", "s#4"]);
    let expected = actual.into_iter().map(|(k, seq)| BTreeMap::from([
        (
            "k".to_string(),
            k.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        ),
        ("seq".to_string(), LiteralValue::Number(seq.to_string()))
    ])).collect::<Vec<_>>();
    assert_eq!(exact.terminal, expected);
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));
    assert_eq!(exact.terminal_domains["seq"].admits_null(), Some(false));
}
