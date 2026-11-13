//! TASK-193 independent source-instance and SQL three-valued-logic matrix.
//! Literal expected row-identity bags are derived from frozen physical source
//! populations, never from protocol-emitted documents or SQL rendering.
//! The historical 24-case preimplementation v1 freeze stays immutable.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement};

const MATRIX: &str = include_str!("../docs/task-193-source-instance-matrix-v2.json");
const ORIGINAL: &str = include_str!("../docs/task-193-acceptance-freeze.json");

fn read(s: &str) -> Value {
    serde_json::from_str(s).expect("frozen task-owned evidence")
}

fn bag(db: &Connection, sql: &str, width: usize) -> Vec<Vec<Option<i64>>> {
    let mut query = db.prepare(sql).expect("original or occurrence-level DuckDB SQL");
    let rows = query
        .query_map([], |row| {
            (0..width)
                .map(|column| row.get::<_, Option<i64>>(column))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native complete bag");
    let mut actual: Vec<_> = rows.map(|row| row.expect("typed DuckDB row")).collect();
    actual.sort();
    actual
}

fn expected(json: &Value) -> Vec<Vec<Option<i64>>> {
    let mut rows: Vec<Vec<Option<i64>>> =
        serde_json::from_value(json.clone()).expect("literal complete source-instance bag");
    rows.sort();
    rows
}

fn source() -> Connection {
    let matrix = read(ORIGINAL);
    let db = Connection::open_in_memory().expect("DuckDB in-memory");
    db.execute_batch(matrix["source_setup"].as_str().expect("pinned source DDL"))
        .expect("complete original physical source");
    db
}

#[test]
fn all_original_variants_have_independently_frozen_full_instance_bags() {
    let matrix = read(MATRIX);
    let original = read(ORIGINAL);
    assert_eq!(matrix["schema_version"], 1);
    assert_eq!(matrix["case_count"], 24);
    assert_eq!(matrix["dialect_cell_count"], 312);
    let cases = matrix["rows"].as_array().expect("24 original cases");
    let db = source();
    let mut visited = BTreeSet::new();
    let mut counted = 0;
    for case in cases {
        let id = case["case_id"].as_str().expect("original ID");
        assert!(visited.insert(id), "{id}: case repeated");
        let prior = original["cases"]
            .as_array()
            .expect("original freeze")
            .iter()
            .find(|original| original["id"] == id)
            .expect("must refer to original SQL");
        assert_eq!(case["original_sql"], prior["sql"], "{id}: original syntax drift");
        assert_eq!(
            case["expected_original_terminal_bag"], prior["expected_terminal_bag"],
            "{id}: independent row-identity oracle must project exactly the fixed terminal"
        );
        let sql = case["source_instance_sql"].as_str().expect("native rowid SQL");
        let want = expected(&case["expected_source_instance_bag"]);
        let width = want
            .first()
            .map_or_else(
                || {
                    if id == "88-C177-S060" {
                        2
                    } else {
                        panic!("{id}: empty identity bag without a pinned arity")
                    }
                },
                Vec::len,
            );
        assert_eq!(
            bag(&db, sql, width),
            want,
            "{id}: native physical alias identities, NULL extension and duplicates"
        );
        assert_eq!(
            want.len(),
            case["expected_source_instance_count"]
                .as_u64()
                .expect("independent expected output count") as usize,
            "{id}: complete multiset count"
        );
        counted += want.len();
        let dialect = dialect_from_name("duckdb").expect("public parser");
        let protocol = analyze_sql(
            case["original_sql"].as_str().expect("original SQL"),
            "duckdb",
            dialect.as_ref(),
        )
        .expect("original portable SQL");
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{id}: must retain canonical query statement");
        };
        assert_eq!(
            query.output().columns().len(),
            case["expected_original_columns"].as_u64().expect("original arity") as usize,
            "{id}: source-independent output schema"
        );
    }
    assert_eq!(visited.len(), 24);
    assert!(counted > 100, "do not drop joined multiplicity to unique values");
}

#[test]
fn every_join_predicate_has_a_complete_true_false_unknown_partner_partition() {
    let matrix = read(MATRIX);
    assert_eq!(matrix["partner_truth_probe_count"], 21);
    let db = source();
    let mut checked = 0;
    for case in matrix["rows"].as_array().expect("original variants") {
        let probe = &case["truth_probe"];
        if !probe.is_object() {
            continue;
        }
        let id = case["case_id"].as_str().expect("case ID");
        let sources = probe["cartesian_source_sql"]
            .as_str()
            .expect("closed Cartesian source instances");
        let predicate = probe["predicate"].as_str().expect("original ON predicate law");
        let mut complete = BTreeSet::new();
        let mut total = 0;
        for (suffix, key) in [
            ("IS TRUE", "expected_true_pairs"),
            ("IS FALSE", "expected_false_pairs"),
            ("IS UNKNOWN", "expected_unknown_pairs"),
        ] {
            let sql = format!(
                "SELECT {} FROM {sources} WHERE ({predicate}) {suffix}",
                if sources.contains("orders a") {
                    "a.rowid, b.rowid"
                } else {
                    "l.rowid, r.rowid"
                }
            );
            let want = expected(&probe[key]);
            assert_eq!(bag(&db, &sql, 2), want, "{id}: {suffix} full partner rows");
            for pair in want {
                assert!(complete.insert(pair), "{id}: pair occurs in two truth partitions");
                total += 1;
            }
        }
        assert_eq!(
            total,
            probe["expected_pair_count"].as_u64().expect("Cartesian cardinality") as usize,
            "{id}: FALSE/UNKNOWN complement must cover every input pair"
        );
        assert_eq!(complete.len(), total, "{id}: duplicate source IDs are not collapsed");
        checked += 1;
    }
    assert_eq!(checked, 21);
}

#[test]
fn shared_source_joins_use_distinct_alias_instances_but_one_physical_bag() {
    let matrix = read(MATRIX);
    let db = source();
    for (id, arity, expected_count) in [
        ("88-C177-S062", 2, 7),
        ("88-C177-S063", 3, 11),
        ("88-C177-S064", 3, 5),
        ("88-C177-S065", 2, 7),
        ("88-C177-S152", 2, 6),
    ] {
        let case = matrix["rows"]
            .as_array()
            .expect("original source instances")
            .iter()
            .find(|case| case["case_id"] == id)
            .expect("named physical alias variant");
        let aliases = case["source_instances"].as_array().expect("identity names");
        assert_eq!(
            aliases.len(), arity,
            "{id}: each alias refers to a named source/producer"
        );
        let sql = case["source_instance_sql"].as_str().expect("physical SQL");
        let actual = bag(&db, sql, arity);
        assert_eq!(actual, expected(&case["expected_source_instance_bag"]));
        assert_eq!(actual.len(), expected_count);
    }
}
