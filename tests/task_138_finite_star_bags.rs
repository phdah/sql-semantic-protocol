//! TASK-138 independent source-free star-transform physical bag oracles.
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, LiteralValue,
    PhysicalFiniteTableProof, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn result(
    sql: &str,
    dialect: &str,
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalFiniteTableProof {
    let parser = dialect_from_name(dialect).expect("supported dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).expect("parse");
    let layer = bundle.layers().last().expect("one query layer");
    physical_finite_table_plan(&bundle, layer.id(), wanted)
}

fn number(n: i64) -> LiteralValue {
    LiteralValue::Number(n.to_string())
}

fn complete_bag(sql: &str, columns: &[&str]) -> Vec<BTreeMap<String, LiteralValue>> {
    let connection = Connection::open_in_memory().expect("DuckDB");
    let mut prepared = connection.prepare(sql).expect("reference SQL");
    prepared
        .query_map([], |row| {
            let mut values = BTreeMap::new();
            for (index, name) in columns.iter().enumerate() {
                let value = row.get::<_, Option<i64>>(index)?;
                values.insert(
                    (*name).to_string(),
                    value.map_or(LiteralValue::Null, number),
                );
            }
            Ok(values)
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("reference rows")
}

fn sorted_bag(bag: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rows = bag.iter().map(|row| format!("{row:?}")).collect::<Vec<_>>();
    rows.sort();
    rows
}

const INPUT: &str = "(VALUES (1, 10, 20), (1, NULL, 30), (2, 10, NULL)) AS t(id, a, b)";

#[test]
fn exact_finite_star_projections_match_independent_duckdb_complete_bags() {
    let cases = [
        (
            "bigquery",
            "SELECT * EXCEPT (b)",
            "SELECT id, a",
            vec!["id", "a"],
        ),
        (
            "duckdb",
            "SELECT * EXCLUDE b",
            "SELECT id, a",
            vec!["id", "a"],
        ),
        (
            "snowflake",
            "SELECT * RENAME b AS c",
            "SELECT id, a, b AS c",
            vec!["id", "a", "c"],
        ),
        (
            "bigquery",
            "SELECT * REPLACE (9 AS b)",
            "SELECT id, a, 9 AS b",
            vec!["id", "a", "b"],
        ),
    ];
    for (dialect, projection, oracle, names) in cases {
        let sql = format!("{projection} FROM {INPUT}");
        let reference = format!("{oracle} FROM {INPUT}");
        let PhysicalFiniteTableProof::Exact(exact) = result(&sql, dialect, None) else {
            panic!("{dialect}: expected exact finite physical star proof for {sql}");
        };
        assert_eq!(
            exact.row_ids.len(),
            3,
            "preserve all three distinct producer rows"
        );
        assert_eq!(
            sorted_bag(&exact.terminal),
            sorted_bag(&complete_bag(&reference, &names))
        );
        for name in names {
            assert!(exact.terminal_domains.contains_key(name));
            assert!(!matches!(
                exact.terminal_domains[name],
                ValueDomain::Unknown(_)
            ));
        }
    }
}

#[test]
fn closed_world_negative_star_membership_is_impossible_not_residual() {
    let sql = format!("SELECT * EXCLUDE b FROM {INPUT}");
    let missing = BTreeMap::from([("id".to_string(), number(99)), ("a".to_string(), number(1))]);
    assert!(matches!(
        result(&sql, "duckdb", Some(&missing)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn unsupported_star_source_must_not_be_promoted_to_exact() {
    assert!(matches!(
        result("SELECT * EXCLUDE b FROM physical_table", "duckdb", None),
        PhysicalFiniteTableProof::Residual { .. }
    ));
}
