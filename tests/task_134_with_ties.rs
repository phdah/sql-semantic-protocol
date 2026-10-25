//! TASK-134: complete physical WITH TIES bags and independent DuckDB rank oracles.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    physical_finite_page_membership_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn prove(
    sql: &str,
    values: &[Option<i64>],
) -> (
    PhysicalFinitePageProof,
    sql_semantic_protocol::AnalysisBundle,
) {
    let parser = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "ties",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let schema = RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let rows = values
        .iter()
        .map(|v| {
            let literal = v.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()));
            PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), literal)])).unwrap()
        })
        .collect();
    let source = PhysicalWriteState::new("s", rows).unwrap();
    let layer_id = bundle.layers().last().unwrap().id();
    let proof = physical_finite_page_bag_plan(&bundle, layer_id, &[source]);
    (proof, bundle)
}

fn terminal_values(proof: &PhysicalFinitePageProof) -> Vec<Option<i64>> {
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("expected complete exact page, got {proof:?}");
    };
    exact
        .terminal
        .iter()
        .map(|row| match row.get("k").unwrap() {
            LiteralValue::Null => None,
            LiteralValue::Number(n) => Some(n.parse().unwrap()),
            other => panic!("unexpected value: {other:?}"),
        })
        .collect()
}

#[test]
fn fetch_with_ties_keeps_entire_cutoff_group_and_rejects_rest() {
    let sql = "SELECT k FROM s ORDER BY k DESC FETCH FIRST 2 ROWS WITH TIES";
    let values = [Some(3), Some(2), Some(2), Some(1)];
    // DuckDB's RANK oracle is independent of the protocol's page executor.
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER); INSERT INTO s VALUES (3),(2),(2),(1);")
        .unwrap();
    let native = db.prepare(
        "SELECT k FROM (SELECT k, RANK() OVER (ORDER BY k DESC) AS r FROM s) WHERE r <= 2 ORDER BY k DESC"
    ).unwrap().query_map([], |row| row.get::<_, i64>(0)).unwrap()
        .collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(native, vec![3, 2, 2]);
    let (proof, bundle) = prove(sql, &values);
    assert_eq!(
        terminal_values(&proof),
        native.into_iter().map(Some).collect::<Vec<_>>()
    );
    let PhysicalFinitePageProof::Exact(exact) = &proof else {
        unreachable!()
    };
    assert_eq!(
        exact
            .rows
            .iter()
            .filter(|row| row.survives)
            .map(|row| row.source_row_id.as_str())
            .collect::<Vec<_>>(),
        vec!["s#0", "s#1", "s#2"]
    );
    assert_eq!(
        exact
            .rows
            .iter()
            .filter(|row| !row.survives)
            .map(|row| row.source_row_id.as_str())
            .collect::<Vec<_>>(),
        vec!["s#3"]
    );
    assert_eq!(exact.rows[1].order_position, exact.rows[2].order_position);
    assert!(matches!(
        exact.terminal_domains.get("k"),
        Some(ValueDomain::Set(_))
    ));
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));

    let state = PhysicalWriteState::new(
        "s",
        values
            .iter()
            .map(|v| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    v.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let layer = bundle.layers().last().unwrap().id();
    let rejected = BTreeMap::from([("k".to_string(), LiteralValue::Number("1".to_string()))]);
    assert!(matches!(
        physical_finite_page_membership_plan(&bundle, layer, &[state], &rejected),
        PhysicalFinitePageProof::Impossible { .. }
    ));
}

#[test]
fn with_ties_on_nullable_key_needs_explicit_null_ordering() {
    let values = [None, Some(2), Some(1)];
    let (exact, _) = prove(
        "SELECT k FROM s ORDER BY k NULLS FIRST FETCH FIRST 1 ROW WITH TIES",
        &values,
    );
    assert_eq!(terminal_values(&exact), vec![None]);
    let PhysicalFinitePageProof::Exact(whole) = exact else {
        unreachable!()
    };
    assert_eq!(whole.terminal_domains["k"].admits_null(), Some(true));
    assert!(matches!(
        prove(
            "SELECT k FROM s ORDER BY k FETCH FIRST 1 ROW WITH TIES",
            &values
        )
        .0,
        PhysicalFinitePageProof::Residual { .. }
    ));
}

#[test]
fn ordinary_fetch_must_not_pick_an_arbitrary_tied_identity() {
    let (proof, _) = prove(
        "SELECT k FROM s ORDER BY k DESC FETCH FIRST 2 ROWS ONLY",
        &[Some(3), Some(2), Some(2), Some(1)],
    );
    assert!(matches!(proof, PhysicalFinitePageProof::Residual { .. }));
}


fn observed_bag(proof: &PhysicalFinitePageProof) -> Vec<String> {
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("expected exact distinct bag, got {proof:?}");
    };
    let mut bag = exact.terminal.iter().map(|row| format!("{row:?}")).collect::<Vec<_>>();
    bag.sort();
    bag
}

fn duckdb_distinct(sql: &str, values: &[Option<i64>]) -> Vec<String> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER)").unwrap();
    for value in values {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({})",
            value.map_or("NULL".to_string(), |v| v.to_string())
        )).unwrap();
    }
    let native = db.prepare(sql).unwrap().query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    let mut bag = native.into_iter().map(|value| {
        format!("{:?}", BTreeMap::from([(
            "k".to_string(),
            value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        )]))
    }).collect::<Vec<_>>();
    bag.sort();
    bag
}

#[test]
fn distinct_eliminates_duplicate_values_but_retains_both_source_contributors() {
    let sql = "SELECT DISTINCT k FROM s ORDER BY k LIMIT 3";
    let values = [Some(1), Some(1), Some(2), Some(3)];
    let (proof, _) = prove(sql, &values);
    assert_eq!(observed_bag(&proof), duckdb_distinct(sql, &values));
    let PhysicalFinitePageProof::Exact(exact) = proof else { unreachable!() };
    assert_eq!(exact.terminal.len(), 3);
    assert_eq!(exact.rows.iter().filter(|row| row.survives).count(), 4);
    assert_eq!(exact.terminal_contributors[0], vec!["s#0", "s#1"]);
    assert_eq!(exact.terminal_contributors[1], vec!["s#2"]);
    assert_eq!(exact.terminal_contributors[2], vec!["s#3"]);
    assert!(matches!(exact.terminal_domains["k"], ValueDomain::Set(_)));
}

#[test]
fn distinct_on_identical_projection_proves_complete_bag_without_chosen_identity() {
    let sql = "SELECT DISTINCT ON (k) k FROM s ORDER BY k";
    let values = [Some(1), Some(1), Some(2)];
    let (proof, _) = prove(sql, &values);
    assert_eq!(observed_bag(&proof), duckdb_distinct(sql, &values));
    let PhysicalFinitePageProof::Exact(exact) = proof else { unreachable!() };
    assert_eq!(exact.terminal.len(), 2);
    assert_eq!(exact.terminal_contributors[0], vec!["s#0", "s#1"]);
    assert_eq!(exact.terminal_contributors[1], vec!["s#2"]);
}

#[test]
fn qualify_and_unattested_distinct_on_producer_never_invent_page_membership() {
    let values = [Some(1), Some(2), Some(3)];
    let (proof, _) = prove(
        "SELECT k FROM s QUALIFY ROW_NUMBER() OVER (ORDER BY k) <= 2 ORDER BY k LIMIT 1",
        &values,
    );
    assert!(matches!(proof, PhysicalFinitePageProof::Residual { .. }));
}
