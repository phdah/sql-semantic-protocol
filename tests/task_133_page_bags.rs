//! TASK-133 public API closed-world pagination and independent DuckDB result oracles.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    physical_finite_page_membership_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn catalog() -> RelationCatalog {
    let schema = RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    RelationCatalog::from_schemas(&[schema]).unwrap()
}

fn source(keys: &[Option<i64>]) -> PhysicalWriteState {
    let rows = keys
        .iter()
        .map(|key| {
            let value = key.map_or(LiteralValue::Null, |k| LiteralValue::Number(k.to_string()));
            PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), value)])).unwrap()
        })
        .collect();
    PhysicalWriteState::new("s", rows).unwrap()
}

fn prove(sql: &str, dialect: &str, keys: &[Option<i64>]) -> PhysicalFinitePageProof {
    let parser = dialect_from_name(dialect).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "page",
        &input,
        dialect,
        parser.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap();
    let layer_id = bundle.layers().last().unwrap().id();
    physical_finite_page_bag_plan(&bundle, layer_id, &[source(keys)])
}

fn duckdb_rows(sql: &str, keys: &[Option<i64>]) -> Vec<Option<i64>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER);").unwrap();
    for key in keys {
        let value = key.map_or("NULL".to_string(), |k| k.to_string());
        db.execute_batch(&format!("INSERT INTO s VALUES ({value});"))
            .unwrap();
    }
    // Native source-bag count is checked independently of the interpreter.
    let count: i64 = db
        .query_row("SELECT count(*) FROM s", [], |row| row.get(0))
        .unwrap();
    assert_eq!(usize::try_from(count).unwrap(), keys.len());
    db.prepare(sql)
        .unwrap()
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn selected_values(
    proof: PhysicalFinitePageProof,
    expected: &[i64],
    survivors: &[&str],
    rejected: &[&str],
) {
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("expected an exact closed-world proof, got {proof:?}");
    };
    assert_eq!(exact.source, "s");
    let values = exact
        .terminal
        .iter()
        .map(|row| match row.get("k").unwrap() {
            LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
            other => panic!("unexpected projected scalar: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, expected);
    let selected_ids = exact
        .rows
        .iter()
        .filter(|row| row.survives)
        .map(|row| row.source_row_id.as_str())
        .collect::<Vec<_>>();
    let rejected_ids = exact
        .rows
        .iter()
        .filter(|row| !row.survives)
        .map(|row| row.source_row_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(selected_ids, survivors);
    assert_eq!(rejected_ids, rejected);
    assert_eq!(exact.rows.len(), survivors.len() + rejected.len());
    let Some(domain) = exact.terminal_domains.get("k") else {
        panic!("terminal column domain must be present, even for empty result");
    };
    let displayed = format!("{domain:?}");
    if expected.is_empty() {
        assert!(matches!(domain, ValueDomain::Empty), "{displayed}");
    } else {
        for value in expected {
            assert!(
                displayed.contains(&value.to_string()),
                "{value} absent from terminal domain: {displayed}"
            );
        }
    }
}

#[test]
fn limit_offset_exact_survivor_rejection_and_duckdb_bags() {
    let sql = "SELECT k FROM s ORDER BY k LIMIT 2 OFFSET 1";
    let source = [Some(1), Some(2), Some(3), Some(4)];
    assert_eq!(duckdb_rows(sql, &source), vec![Some(2), Some(3)]);
    selected_values(
        prove(sql, "duckdb", &source),
        &[2, 3],
        &["s#1", "s#2"],
        &["s#0", "s#3"],
    );
}

#[test]
fn fetch_first_proves_exact_source_and_terminal_bag() {
    let sql = "SELECT k FROM s ORDER BY k FETCH FIRST 2 ROWS ONLY";
    let source = [Some(1), Some(2), Some(3), Some(4)];
    assert_eq!(duckdb_rows(sql, &source), vec![Some(1), Some(2)]);
    selected_values(
        prove(sql, "duckdb", &source),
        &[1, 2],
        &["s#0", "s#1"],
        &["s#2", "s#3"],
    );
}

#[test]
fn mssql_top_has_same_canonical_page_bag_as_duckdb_limit() {
    let source = [Some(1), Some(2), Some(3), Some(4)];
    assert_eq!(
        duckdb_rows("SELECT k FROM s ORDER BY k LIMIT 2", &source),
        vec![Some(1), Some(2)]
    );
    selected_values(
        prove("SELECT TOP 2 k FROM s ORDER BY k", "mssql", &source),
        &[1, 2],
        &["s#0", "s#1"],
        &["s#2", "s#3"],
    );
}

#[test]
fn zero_limit_and_empty_source_are_exact_empty_bags() {
    let source = [Some(1), Some(2), Some(3), Some(4)];
    let sql = "SELECT k FROM s ORDER BY k LIMIT 0";
    assert!(duckdb_rows(sql, &source).is_empty());
    selected_values(
        prove(sql, "duckdb", &source),
        &[],
        &[],
        &["s#0", "s#1", "s#2", "s#3"],
    );
    let sql = "SELECT k FROM s ORDER BY k LIMIT 2 OFFSET 1";
    assert!(duckdb_rows(sql, &[]).is_empty());
    selected_values(prove(sql, "duckdb", &[]), &[], &[], &[]);
}

#[test]
fn tied_sort_key_never_claims_exact_row_identity() {
    let proof = prove(
        "SELECT k FROM s ORDER BY k LIMIT 1",
        "duckdb",
        &[Some(2), Some(2), Some(3)],
    );
    assert!(
        matches!(proof, PhysicalFinitePageProof::Residual { .. }),
        "{proof:?}"
    );
}

#[test]
fn pagination_cannot_skip_a_derived_or_filtered_producer_boundary() {
    for sql in [
        "WITH t AS (SELECT k FROM s WHERE k > 2) SELECT k FROM t ORDER BY k LIMIT 2",
        "SELECT k FROM (SELECT k FROM s WHERE k > 2) AS t ORDER BY k LIMIT 2",
        "SELECT k FROM s WHERE k >= 2 ORDER BY k LIMIT 2",
    ] {
        let proof = prove(sql, "duckdb", &[Some(1), Some(2), Some(3), Some(4)]);
        assert!(
            matches!(proof, PhysicalFinitePageProof::Residual { .. }),
            "{sql} must preserve its producer boundary: {proof:?}"
        );
    }
}

#[test]
fn implicit_null_sort_and_missing_order_fail_closed() {
    let nullable = prove(
        "SELECT k FROM s ORDER BY k LIMIT 1",
        "duckdb",
        &[None, Some(2)],
    );
    assert!(
        matches!(nullable, PhysicalFinitePageProof::Residual { .. }),
        "{nullable:?}"
    );
    let underordered = prove("SELECT k FROM s LIMIT 1", "duckdb", &[Some(1), Some(2)]);
    assert!(
        matches!(underordered, PhysicalFinitePageProof::Residual { .. }),
        "{underordered:?}"
    );
}

#[test]
fn requested_terminal_membership_is_exact_or_impossible_from_one_closed_source_bag() {
    let sql = "SELECT k FROM s ORDER BY k LIMIT 2 OFFSET 1";
    let parser = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "page",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap();
    let layer = bundle.layers().last().unwrap().id();
    let state = source(&[Some(1), Some(2), Some(3), Some(4)]);
    for value in [2, 3] {
        let wanted = BTreeMap::from([("k".to_string(), LiteralValue::Number(value.to_string()))]);
        assert!(
            matches!(
                physical_finite_page_membership_plan(
                    &bundle,
                    layer,
                    std::slice::from_ref(&state),
                    &wanted,
                ),
                PhysicalFinitePageProof::Exact(_)
            ),
            "present terminal value {value} must have a source witness"
        );
    }
    for value in [1, 4, 99] {
        let wanted = BTreeMap::from([("k".to_string(), LiteralValue::Number(value.to_string()))]);
        assert!(
            matches!(
                physical_finite_page_membership_plan(
                    &bundle,
                    layer,
                    std::slice::from_ref(&state),
                    &wanted,
                ),
                PhysicalFinitePageProof::Impossible { .. }
            ),
            "rejected or absent terminal value {value} is impossible"
        );
    }
    let impossible_source = source(&[Some(i64::from(i32::MAX) + 1)]);
    assert!(
        matches!(
            physical_finite_page_bag_plan(&bundle, layer, &[impossible_source]),
            PhysicalFinitePageProof::Impossible { .. }
        ),
        "overflowing physical INTEGER must be impossible rather than silently truncated"
    );
}
