//! TASK-138 fixed-schema PIVOT/UNPIVOT against independent DuckDB bags.
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, LiteralValue,
    PhysicalFiniteTableProof, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;
fn proof(
    sql: &str,
    dialect: &str,
    want: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalFiniteTableProof {
    let parser = dialect_from_name(dialect).expect("supported dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).expect("parse");
    let layer = bundle.layers().last().expect("one layer");
    physical_finite_table_plan(&bundle, layer.id(), want)
}
fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut values = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    values.sort();
    values
}
fn scalar(value: Option<i64>) -> LiteralValue {
    value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()))
}
fn pivot_oracle(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let conn = Connection::open_in_memory().expect("DuckDB");
    let mut stmt = conn.prepare(sql).expect("reference SQL");
    stmt.query_map([], |row| {
        Ok(BTreeMap::from([
            ("id".to_string(), scalar(row.get::<_, Option<i64>>(0)?)),
            ("one".to_string(), scalar(row.get::<_, Option<i64>>(1)?)),
            ("two".to_string(), scalar(row.get::<_, Option<i64>>(2)?)),
        ]))
    })
    .expect("query")
    .collect::<Result<Vec<_>, _>>()
    .expect("rows")
}
fn unpivot_oracle(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let conn = Connection::open_in_memory().expect("DuckDB");
    let mut stmt = conn.prepare(sql).expect("reference SQL");
    stmt.query_map([], |row| {
        Ok(BTreeMap::from([
            ("id".to_string(), scalar(row.get::<_, Option<i64>>(0)?)),
            ("v".to_string(), scalar(row.get::<_, Option<i64>>(1)?)),
            (
                "n".to_string(),
                LiteralValue::Text(row.get::<_, String>(2)?),
            ),
        ]))
    })
    .expect("query")
    .collect::<Result<Vec<_>, _>>()
    .expect("rows")
}

#[test]
fn complete_fixed_sum_pivot_matches_duckdb_including_duplicates_and_missing_groups() {
    let sql = "SELECT * FROM (VALUES (1, 1, 10), (1, 2, 20), (1, 1, 10), (2, 1, 30)) AS t(id, k, v) PIVOT (SUM(v) FOR k IN (1 AS one, 2 AS two))";
    let PhysicalFiniteTableProof::Exact(result) = proof(sql, "duckdb", None) else {
        panic!("fixed SUM pivot must prove exact finite bags");
    };
    assert_eq!(sorted(&result.terminal), sorted(&pivot_oracle(sql)));
    assert_eq!(
        result.row_ids.len(),
        2,
        "two complete grouped output row identities"
    );
    assert!(result
        .terminal_domains
        .values()
        .all(|domain| !matches!(domain, ValueDomain::Unknown(_))));
    let missing = BTreeMap::from([
        ("id".to_string(), scalar(Some(3))),
        ("one".to_string(), scalar(Some(10))),
        ("two".to_string(), LiteralValue::Null),
    ]);
    assert!(matches!(
        proof(sql, "duckdb", Some(&missing)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn complete_unpivot_exclude_and_include_nulls_match_independent_duckdb_bags() {
    let source = "(VALUES (1, 10, 20), (1, NULL, 30), (2, 10, NULL)) AS t(id, a, b)";
    for (option, expected_size) in [("EXCLUDE NULLS", 4), ("INCLUDE NULLS", 6)] {
        let sql = format!("SELECT * FROM {source} UNPIVOT {option} (v FOR n IN (a, b))");
        let PhysicalFiniteTableProof::Exact(result) = proof(&sql, "duckdb", None) else {
            panic!("{option}: must prove exact complete unpivot bag");
        };
        assert_eq!(result.terminal.len(), expected_size);
        assert_eq!(result.row_ids.len(), expected_size);
        assert_eq!(sorted(&result.terminal), sorted(&unpivot_oracle(&sql)));
        assert!(result
            .terminal_domains
            .values()
            .all(|domain| !matches!(domain, ValueDomain::Unknown(_))));
    }
}

#[test]
fn unproved_source_and_dynamic_pivot_fail_closed() {
    let source = "SELECT * FROM events PIVOT (SUM(v) FOR k IN (1 AS one, 2 AS two))";
    assert!(matches!(
        proof(source, "duckdb", None),
        PhysicalFiniteTableProof::Residual { .. }
    ));
}
