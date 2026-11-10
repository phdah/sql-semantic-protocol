//! TASK-192: every successful original integer-valued DuckDB terminal
//! must satisfy all precise public output-domain claims across the 13 parsers.
//! This is a soundness floor, not proof that a domain is maximally restrictive.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, SetMode, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const PRIOR: &str = include_str!("../docs/task-177-predicate-oracles.json");
const ENV: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const FINAL: &str = include_str!("../docs/task-177-final-native-deterministic.json");
const NATIVE: &str = include_str!("../docs/task-192-native-partitions.json");

fn read(s: &str) -> Value {
    serde_json::from_str(s).expect("committed independently frozen JSON")
}

fn native_terminal(
    id: &str,
    prior: &Value,
    freeze: &Value,
    env: &Value,
    final_native: &Value,
    native: &Value,
) -> Vec<Value> {
    let candidates = [
        prior["cases"].as_array().expect("prior truth goldens"),
        env["customer_cases"].as_array().expect("string goldens"),
        env["temporal_cases"].as_array().expect("timestamp goldens"),
        env["floating_cases"]
            .as_array()
            .expect("signed zero goldens"),
        final_native["original_integer_selects"]
            .as_array()
            .expect("native extra goldens"),
    ];
    if let Some(expected) = candidates
        .into_iter()
        .flatten()
        .find(|case| case["id"] == id)
        .map(|case| &case["expected_terminal_bag"])
    {
        return expected
            .as_array()
            .expect("one-column bag")
            .iter()
            .map(|row| row[0].clone())
            .collect();
    }
    if id == "88-C177-S042" {
        return native["other"]
            .as_array()
            .expect("native boundaries")
            .iter()
            .find(|case| case["id"] == id)
            .expect("frozen sort")["expected_ordered_terminal"]
            .as_array()
            .expect("six ordered rows")
            .clone();
    }
    let pattern = &freeze["task_owned_string_pattern_duckdb_oracles"];
    let case = pattern["cases"]
        .as_array()
        .expect("four string controls")
        .iter()
        .find(|c| c["id"] == id)
        .expect("one of 41 original numeric terminals");
    case["selected_ids"]
        .as_array()
        .expect("native selected rows")
        .clone()
}

fn contains(domain: &ValueDomain, value: &Value) -> bool {
    match domain {
        ValueDomain::Unbounded | ValueDomain::Unknown(_) => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(set) => {
            let literal = if value.is_null() {
                LiteralValue::Null
            } else if let Some(integer) = value.as_i64() {
                LiteralValue::Number(integer.to_string())
            } else {
                return false;
            };
            let present = set.values().iter().any(|item| item.value() == &literal);
            match set.mode() {
                SetMode::Include => present,
                SetMode::Exclude => !present,
            }
        }
        ValueDomain::Ranges(ranges) => {
            let Some(value) = value.as_i64().map(i128::from) else {
                return false;
            };
            ranges.ranges().iter().any(|range| {
                let lower = range.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(n) = raw.parse::<i128>() else {
                        return false;
                    };
                    value > n || (value == n && bound.inclusive())
                });
                let upper = range.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(n) = raw.parse::<i128>() else {
                        return false;
                    };
                    value < n || (value == n && bound.inclusive())
                });
                lower && upper
            })
        }
        // An unrecognized future domain cannot silently prove membership.
        _ => false,
    }
}

#[test]
fn all_forty_one_original_native_read_terminals_respect_thirteen_dialect_domains() {
    let freeze = read(FREEZE);
    let prior = read(PRIOR);
    let env = read(ENV);
    let final_native = read(FINAL);
    let native = read(NATIVE);
    let dialects = freeze["dialects"].as_array().expect("13 dialects");
    let mut checked = 0;
    let mut native_rows = 0;
    for case in freeze["cases"].as_array().expect("43 cases") {
        let id = case["id"].as_str().expect("original SQL identity");
        if matches!(id, "88-C177-S033" | "88-C177-S179") {
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        let rows = native_terminal(id, &prior, &freeze, &env, &final_native, &native);
        for dialect in dialects {
            let name = dialect.as_str().expect("public dialect");
            let parser = dialect_from_name(name).expect("registered SQL parser");
            let protocol = analyze_sql(sql, name, parser.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{name}: original SQL: {error}"));
            let [ProtocolStatement::Query(query)] = protocol.statements() else {
                panic!("{id}/{name}: silent unsupported output is not a semantic domain");
            };
            let [column] = query.output().columns() else {
                panic!("{id}/{name}: expected one projected scalar");
            };
            for value in &rows {
                assert!(
                    contains(column.domain(), value),
                    "{id}/{name}: native {value} excluded by emitted output domain {:?}",
                    column.domain()
                );
                native_rows += 1;
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 41 * 13);
    assert!(
        native_rows > 1000,
        "all native terminal occurrences must be visited"
    );
}
