//! TASK-177 independent physical predicate oracles.
//!
//! A pinned full SQL source bag and manually checked TRUE/FALSE/UNKNOWN
//! partitions are replayed through DuckDB. Public protocol value domains are
//! independently asserted for bounded portable filters. Passing these cases
//! does not certify other SQL shapes or native vendor dialect laws.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, to_json, LiteralValue, ProtocolStatement, SetMode, ValueDomain,
};

const ORACLES: &str = include_str!("../docs/task-177-predicate-oracles.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const SNAPSHOT: &str = include_str!("../docs/task-177-parser-snapshot.json");

fn parse(value: &str) -> Value {
    serde_json::from_str(value).expect("task-owned pinned JSON")
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().expect("required case field")
}

fn rows(value: &Value, field: &str) -> Vec<Vec<Option<i64>>> {
    serde_json::from_value(value[field].clone()).expect("explicit nullable integer row bag")
}

fn actual_rows(db: &Connection, sql: &str) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("DuckDB statement");
    let columns = statement.column_count();
    let rows = statement
        .query_map([], |row| {
            (0..columns)
                .map(|column| row.get::<_, Option<i64>>(column))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native DuckDB query");
    let mut bag: Vec<_> = rows.map(|row| row.expect("physical row")).collect();
    bag.sort();
    bag
}

fn expected_rows(case: &Value, field: &str) -> Vec<Vec<Option<i64>>> {
    let mut expected = rows(case, field);
    expected.sort();
    expected
}

fn expected_ids(case: &Value, field: &str) -> BTreeSet<i64> {
    serde_json::from_value(case[field].clone()).expect("explicit physical source IDs")
}

fn assert_interval(domain: &ValueDomain, case: &Value) {
    let expected = &case["expected_projected_domain"];
    if expected["kind"] == "set_include" {
        let ValueDomain::Set(set) = domain else {
            panic!("{}: equality must preserve a typed include set: {domain:?}", text(case, "id"));
        };
        assert_eq!(set.mode(), SetMode::Include);
        assert_eq!(set.values().len(), 1);
        let value = expected["values"][0].as_i64().expect("equal integer");
        assert_eq!(set.values()[0].value(), &LiteralValue::Number(value.to_string()));
        return;
    }

    let ValueDomain::Ranges(ranges) = domain else {
        panic!("{}: expected constrained interval: {domain:?}", text(case, "id"));
    };
    let [interval] = ranges.ranges() else {
        panic!("{}: expected exactly one source-derived interval", text(case, "id"));
    };
    for (actual, bound) in [
        (interval.lower(), &expected["lower"]),
        (interval.upper(), &expected["upper"]),
    ] {
        if bound.is_null() {
            assert!(actual.is_none(), "{}: unexpected bound", text(case, "id"));
        } else {
            let data = bound.as_array().expect("integer boundary and inclusive flag");
            let actual = actual.expect("typed value bound required");
            assert_eq!(
                actual.value().value(),
                &LiteralValue::Number(data[0].as_i64().expect("boundary integer").to_string())
            );
            assert_eq!(
                actual.inclusive(),
                data[1].as_bool().expect("inclusive/exclusive flag")
            );
        }
    }
}

#[test]
fn pinned_predicate_variants_match_complete_duckdb_bags_and_null_partitions() {
    let fixture = parse(ORACLES);
    let samples = parse(SAMPLES);
    let snapshot = parse(SNAPSHOT);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["owner"], "TASK-177");
    assert_eq!(fixture["case_count"], 25);

    let connection = Connection::open_in_memory().expect("DuckDB");
    connection
        .execute_batch(text(&fixture, "source_setup"))
        .expect("explicit complete physical input bags");
    assert_eq!(
        actual_rows(&connection, "SELECT id, amount FROM orders"),
        expected_rows(&fixture, "orders_source_rows")
    );
    assert_eq!(
        actual_rows(&connection, "SELECT id, amount FROM customers"),
        expected_rows(&fixture, "customers_source_rows")
    );

    let expected_ids: BTreeSet<i64> = rows(&fixture, "orders_source_rows")
        .iter()
        .map(|row| row[0].expect("source row ID"))
        .collect();
    let mut seen = BTreeSet::new();

    for case in fixture["cases"].as_array().expect("oracle cases") {
        let id = text(case, "id");
        assert!(seen.insert(id), "duplicate stable TASK-177 case");
        let sample = samples["cases"]
            .as_array()
            .expect("sample registry")
            .iter()
            .find(|item| item["id"] == id)
            .expect("all oracles must name an actual TASK-66 sample");
        assert_eq!(case["sql"], sample["sql"], "{id}: source SQL drift");
        let sql = text(case, "sql");
        assert_eq!(
            actual_rows(&connection, sql),
            expected_rows(case, "expected_terminal_bag"),
            "{id}: original SQL whole-bag terminal oracle"
        );

        let predicate = text(case, "predicate");
        let relation = text(case, "source_relation");
        for (kind, field) in [
            ("IS TRUE", "expected_selected_rows"),
            ("IS FALSE", "expected_false_rows"),
            ("IS UNKNOWN", "expected_unknown_rows"),
        ] {
            let instrumented =
                format!("SELECT id, amount FROM {relation} WHERE ({predicate}) {kind}");
            assert_eq!(
                actual_rows(&connection, &instrumented),
                expected_rows(case, field),
                "{id}: complete physical {kind} occurrence bag"
            );
        }
        let not_true = format!(
            "SELECT id, amount FROM {relation} WHERE ({predicate}) IS NOT TRUE"
        );
        let mut excluded = expected_rows(case, "expected_false_rows");
        excluded.extend(expected_rows(case, "expected_unknown_rows"));
        excluded.sort();
        assert_eq!(
            actual_rows(&connection, &not_true),
            excluded,
            "{id}: FALSE and UNKNOWN must both be excluded"
        );
        let true_ids = expected_ids(case, "expected_selected_ids");
        let false_ids = expected_ids(case, "expected_false_ids");
        let unknown_ids = expected_ids(case, "expected_unknown_ids");
        assert!(true_ids.is_disjoint(&false_ids), "{id}: TRUE/FALSE overlap");
        assert!(true_ids.is_disjoint(&unknown_ids), "{id}: TRUE/UNKNOWN overlap");
        assert!(false_ids.is_disjoint(&unknown_ids), "{id}: FALSE/UNKNOWN overlap");
        let partition: BTreeSet<_> = true_ids
            .union(&false_ids)
            .copied()
            .chain(unknown_ids)
            .collect();
        assert_eq!(partition, expected_ids, "{id}: physical source partition");

        let dialect = dialect_from_name("duckdb").expect("DuckDB");
        let protocol = analyze_sql(sql, "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}: public analysis error {error}"));
        assert!(
            protocol.statements().iter().all(|statement| matches!(statement, ProtocolStatement::Query(_))),
            "{id}: the public analyzer must not silently drop a filter"
        );
        let emitted: Value =
            serde_json::from_str(&to_json(&protocol)).expect("canonical wire output");
        assert_eq!(emitted["inputs"][0]["statements"][0]["kind"], "query");
        assert!(
            emitted["inputs"][0]["statements"][0]["output"]["columns"][0]["domain"].is_object(),
            "{id}: output domain must be serialized, even when it is Unknown"
        );

        if !case["expected_projected_domain"].is_null() {
            let [ProtocolStatement::Query(query)] = protocol.statements() else {
                panic!("{id}: expected one query statement");
            };
            let [column] = query.output().columns() else {
                panic!("{id}: expected one projected scalar");
            };
            assert_interval(column.domain(), case);
            let expected_kind = match text(&case["expected_projected_domain"], "kind") {
                "set_include" => "set",
                "range" => "ranges",
                other => panic!("{id}: unexpected domain type {other}"),
            };
            assert_eq!(
                emitted["inputs"][0]["statements"][0]["output"]["columns"][0]["domain"]["kind"],
                expected_kind,
                "{id}: typed claim must survive canonical serialization"
            );
            let mut baseline = emitted;
            baseline["inputs"][0]["dialect"] = Value::Null;
            for name in snapshot["dialects"].as_array().expect("13 named dialects") {
                let name = name.as_str().expect("dialect name");
                let dialect = dialect_from_name(name).expect("public dialect");
                let next = analyze_sql(sql, name, dialect.as_ref())
                    .unwrap_or_else(|error| panic!("{id}/{name}: dialect analysis error {error}"));
                let [ProtocolStatement::Query(statement)] = next.statements() else {
                    panic!("{id}/{name}: expected equivalent query");
                };
                assert_interval(statement.output().columns()[0].domain(), case);
                let mut wire: Value =
                    serde_json::from_str(&to_json(&next)).expect("canonical protocol document");
                wire["inputs"][0]["dialect"] = Value::Null;
                assert_eq!(wire, baseline, "{id}/{name}: portable typed semantics drifted");
            }
        }
    }
    assert_eq!(seen.len(), 25);
}
