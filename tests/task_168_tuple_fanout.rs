//! TASK-168: independently requested typed tuple histograms under LIMIT/QUALIFY.
//!
//! SQL and physical snapshots are fixed input fixtures, not inferred from
//! the implementation's own expected bag.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_single_tuple_histogram_plan, ConfiguredSqlInput, ConstraintValue, LiteralValue,
    PhysicalSingleTupleExact, PhysicalSingleTupleProof, PhysicalTupleBucket,
    PhysicalTupleHistogramGoal, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const LIMIT: &str = "SELECT k, v FROM l ORDER BY v LIMIT 2";
const QUALIFY: &str = "SELECT id, k, v, ROW_NUMBER() OVER (PARTITION BY k ORDER BY v DESC NULLS LAST, id ASC NULLS LAST) AS rn FROM l QUALIFY ROW_NUMBER() OVER (PARTITION BY k ORDER BY v DESC NULLS LAST, id ASC NULLS LAST) = 1";

fn bundle(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let input = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("task-168", &input, dialect, parser.as_ref());
    let schema = RelationSchema::new(
        "l",
        ["id", "k", "v"]
            .iter()
            .map(|column| SchemaColumn::from_sql_type(*column, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn snapshot() -> PhysicalWriteState {
    let data: [(i64, Option<i64>, i64); 3] = [(0, Some(1), 10), (1, Some(1), 20), (2, None, 30)];
    PhysicalWriteState::new(
        "l",
        data.into_iter()
            .map(|(id, k, v)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".into(), LiteralValue::Number(id.to_string())),
                    (
                        "k".into(),
                        k.map_or(LiteralValue::Null, |k| LiteralValue::Number(k.to_string())),
                    ),
                    ("v".into(), LiteralValue::Number(v.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn goal(id: &str, a: (Option<i64>, i64), b: (Option<i64>, i64)) -> PhysicalTupleHistogramGoal {
    let bucket = |(key, value): (Option<i64>, i64)| {
        PhysicalTupleBucket::new(
            vec![
                key.map_or(ConstraintValue::Null, ConstraintValue::Integer),
                ConstraintValue::Integer(value),
            ],
            1,
        )
    };
    PhysicalTupleHistogramGoal::new(id, vec!["k".into(), "v".into()], vec![bucket(a), bucket(b)])
        .unwrap()
}

fn native(sql: &str) -> Vec<(Option<i64>, i64)> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id INTEGER, k INTEGER, v INTEGER);
         INSERT INTO l VALUES (0,1,10),(1,1,20),(2,NULL,30);",
    )
    .unwrap();
    let source_rows: Vec<(i64, Option<i64>, i64)> = db
        .prepare("SELECT id,k,v FROM l ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        source_rows,
        vec![(0, Some(1), 10), (1, Some(1), 20), (2, None, 30)]
    );
    let mut terminal = db
        .prepare(sql)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<(Option<i64>, i64)>, _>>()
        .unwrap();
    terminal.sort();
    terminal
}

#[test]
fn limit_uses_complete_source_and_exact_rejected_rows() {
    let b = bundle(LIMIT, "duckdb");
    let target = b.layers().last().unwrap().id();
    let correct = goal(target, (Some(1), 10), (Some(1), 20));
    let proof = physical_single_tuple_histogram_plan(&b, &[snapshot()], &correct);
    let PhysicalSingleTupleProof::Exact(PhysicalSingleTupleExact::Page(page)) = proof else {
        panic!("expected exact deterministic page with tuple counts, got {proof:?}");
    };
    assert_eq!(page.terminal.len(), 2);
    assert_eq!(page.rows.len(), 3);
    assert_eq!(page.rows.iter().filter(|row| !row.survives).count(), 1);
    assert_eq!(native(LIMIT), vec![(Some(1), 10), (Some(1), 20)]);

    let wrong = goal(target, (Some(1), 10), (Some(1), 30));
    assert!(matches!(
        physical_single_tuple_histogram_plan(&b, &[snapshot()], &wrong),
        PhysicalSingleTupleProof::Impossible { .. }
    ));
}

#[test]
fn qualify_ranking_certifies_selected_and_not_true_physical_rows() {
    let b = bundle(QUALIFY, "duckdb");
    let target = b.layers().last().unwrap().id();
    let correct = goal(target, (Some(1), 20), (None, 30));
    let proof = physical_single_tuple_histogram_plan(&b, &[snapshot()], &correct);
    let PhysicalSingleTupleProof::Exact(PhysicalSingleTupleExact::Window(proof)) = proof else {
        panic!("expected exact QUALIFY correlation and rejected identity: {proof:?}");
    };
    assert_eq!(proof.source_row_ids.len(), 3);
    assert_eq!(proof.terminal_row_ids.len(), 2);
    assert_eq!(proof.rejected_row_ids.len(), 1);
    // DuckDB returns id before k/v for the full query; check native projected
    // bag with an independent SQL projection of the same QUALIFY semantics.
    let sql = "SELECT k, v FROM l QUALIFY ROW_NUMBER() OVER (PARTITION BY k ORDER BY v DESC NULLS LAST, id ASC NULLS LAST) = 1";
    assert_eq!(native(sql), vec![(None, 30), (Some(1), 20)]);

    let wrong = goal(target, (Some(1), 30), (None, 20));
    assert!(matches!(
        physical_single_tuple_histogram_plan(&b, &[snapshot()], &wrong),
        PhysicalSingleTupleProof::Impossible { .. }
    ));
}

#[test]
fn explicit_limit_parser_equivalence_is_not_an_engine_portability_claim() {
    for dialect in DIALECTS {
        let b = bundle(LIMIT, dialect);
        let target = b.layers().last().unwrap().id();
        let exact = goal(target, (Some(1), 10), (Some(1), 20));
        assert!(
            matches!(
                physical_single_tuple_histogram_plan(&b, &[snapshot()], &exact),
                PhysicalSingleTupleProof::Exact(PhysicalSingleTupleExact::Page(_))
            ),
            "{dialect}: supported shared LIMIT must be the same canonical page"
        );
    }
}

#[test]
fn empty_page_has_exact_zero_tuple_histogram_with_attested_columns() {
    let b = bundle("SELECT k, v FROM l ORDER BY v LIMIT 0", "duckdb");
    let target = b.layers().last().unwrap().id();
    let empty =
        PhysicalTupleHistogramGoal::new(target, vec!["k".into(), "v".into()], vec![]).unwrap();
    let proof = physical_single_tuple_histogram_plan(&b, &[snapshot()], &empty);
    let PhysicalSingleTupleProof::Exact(PhysicalSingleTupleExact::Page(page)) = proof else {
        panic!("empty page has a complete source bag and an exact zero-row output: {proof:?}");
    };
    assert!(page.terminal.is_empty());
    assert_eq!(page.rows.len(), 3);
    assert!(page.rows.iter().all(|row| !row.survives));
    assert!(native("SELECT k,v FROM l ORDER BY v LIMIT 0").is_empty());

    let impossible = PhysicalTupleHistogramGoal::new(
        target,
        vec!["k".into(), "v".into()],
        vec![PhysicalTupleBucket::new(
            vec![ConstraintValue::Integer(1), ConstraintValue::Integer(10)],
            1,
        )],
    )
    .unwrap();
    assert!(matches!(
        physical_single_tuple_histogram_plan(&b, &[snapshot()], &impossible),
        PhysicalSingleTupleProof::Impossible { .. }
    ));
}
