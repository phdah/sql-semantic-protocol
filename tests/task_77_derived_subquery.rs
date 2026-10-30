//! TASK-77 missing AC #4 derived-subquery and AC #1 LATERAL boundaries.
//! Compare an independently executed DuckDB bag with the public canonical proof.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, LiteralValue,
    PhysicalFiniteTableProof, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

const DERIVED: &str = "SELECT * FROM (SELECT * FROM UNNEST([1, NULL, 1]) AS t(v)) AS q";

fn proof(
    sql: &str,
    dialect: &str,
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> Option<PhysicalFiniteTableProof> {
    let parser = dialect_from_name(dialect).expect("exposed parser dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).ok()?;
    let target = bundle.layers().last()?.id();
    Some(physical_finite_table_plan(&bundle, target, wanted))
}

#[test]
fn derived_array_subquery_preserves_complete_native_null_duplicate_and_impossible_bags() {
    let db = Connection::open_in_memory().expect("independent native DuckDB");
    let mut query = db.prepare(DERIVED).expect("native derived subquery SQL");
    let mut native = query
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    native.sort();
    assert_eq!(native, vec![None, Some(1), Some(1)]);

    let Some(PhysicalFiniteTableProof::Exact(exact)) = proof(DERIVED, "duckdb", None) else {
        panic!("bounded source-free derived array subquery must be exact");
    };
    assert_eq!(exact.row_ids, ["star:1", "star:2", "star:3"]);
    assert_eq!(exact.source_row_ids, ["unnest:1", "unnest:2", "unnest:3"]);
    assert_eq!(
        exact.contributors,
        vec![
            vec!["unnest:1".to_string()],
            vec!["unnest:2".to_string()],
            vec!["unnest:3".to_string()]
        ]
    );
    let mut produced = exact
        .terminal
        .iter()
        .map(|row| match &row["v"] {
            LiteralValue::Null => None,
            LiteralValue::Number(n) => Some(n.parse::<i64>().expect("typed integer")),
            other => panic!("unexpected type {other:?}"),
        })
        .collect::<Vec<_>>();
    produced.sort();
    assert_eq!(produced, native);
    let mut original = exact
        .source_rows
        .iter()
        .map(|row| match &row["v"] {
            LiteralValue::Null => None,
            LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
            other => panic!("unexpected source type {other:?}"),
        })
        .collect::<Vec<_>>();
    original.sort();
    assert_eq!(original, native);
    assert_eq!(exact.terminal_domains["v"].admits_null(), Some(true));
    let ValueDomain::Set(domain) = &exact.terminal_domains["v"] else {
        panic!("exact source-free derived subquery must have finite set domain");
    };
    assert_eq!(domain.mode(), SetMode::Include);
    assert_eq!(domain.values().len(), 2, "integer and SQL NULL");
    assert!(domain
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Null));
    assert!(domain
        .values()
        .iter()
        .any(|value| { value.value() == &LiteralValue::Number("1".to_string()) }));

    let missing = BTreeMap::from([("v".into(), LiteralValue::Number("3".into()))]);
    assert!(matches!(
        proof(DERIVED, "duckdb", Some(&missing)),
        Some(PhysicalFiniteTableProof::Impossible { .. })
    ));
}

#[test]
fn derived_finite_subquery_preserves_child_frozen_parser_boundaries() {
    let child: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-137-acceptance-freeze.json"))
            .expect("canonical child array parser matrix");
    let array = child["cases"]
        .as_array()
        .expect("child cases")
        .iter()
        .find(|case| case["id"] == "77-C137-P-unnest")
        .expect("frozen UNNEST obligation");
    let dialects = child["dialects"].as_array().expect("13 dialects");
    assert_eq!(dialects.len(), 13);
    let mut exact = 0;
    let mut boundary = 0;
    for dialect in dialects {
        let name = dialect.as_str().expect("dialect name");
        match (array["parser_cells"][name].as_str().expect("child parser disposition"),
            proof(DERIVED, name, None)) {
            ("exact", Some(PhysicalFiniteTableProof::Exact(result))) => {
                exact += 1;
                assert_eq!(result.row_ids, ["star:1", "star:2", "star:3"]);
                assert_eq!(result.source_row_ids, ["unnest:1", "unnest:2", "unnest:3"]);
                assert_eq!(result.terminal_domains["v"].admits_null(), Some(true));
            }
            ("lexical_boundary", None | Some(PhysicalFiniteTableProof::Residual { .. })) => {
                boundary += 1;
            }
            (expected, actual) => {
                panic!("{name}: derived UNNEST proof does not match child {expected}: {actual:?}")
            }
        }
    }
    assert_eq!(exact, 11, "all eleven portable parser identities");
    assert_eq!(boundary, 2, "SQLite and MSSQL lexical boundaries");
}

#[test]
fn subquery_filter_without_closed_source_proof_is_residual() {
    let sql = "SELECT * FROM (SELECT * FROM UNNEST([1,2]) AS t(v) WHERE v > 1) AS q";
    assert!(!matches!(
        proof(sql, "duckdb", None),
        Some(PhysicalFiniteTableProof::Exact(_))
    ));
}

#[test]
fn correlated_lateral_source_is_never_mistaken_for_independent_literal_fanout() {
    let sql = "SELECT * FROM UNNEST([1,2]) AS t(v) JOIN LATERAL (SELECT t.v AS w) AS u ON TRUE";
    let dialects = [
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
    let mut observed = 0;
    for dialect in dialects {
        if let Some(classification) = proof(sql, dialect, None) {
            observed += 1;
            assert!(
                matches!(classification, PhysicalFiniteTableProof::Residual { .. }),
                "{dialect}: correlated LATERAL cannot acquire source-free Exactness"
            );
        }
    }
    assert!(
        observed > 0,
        "at least one parser must recognize the reviewed LATERAL form"
    );
}
