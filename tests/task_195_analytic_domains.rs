//! Original TASK-195 analytic SQL must emit the strongest portable value
//! domains provable without closed-world row-count assumptions.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn range(sql: &str, dialect_name: &str) -> (Option<(String, bool)>, Option<(String, bool)>) {
    let dialect = dialect_from_name(dialect_name).expect("one of 13 public dialects");
    let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
        .unwrap_or_else(|error| panic!("{dialect_name}: {sql}: {error}"));
    let [ProtocolStatement::Query(query)] = protocol.statements() else {
        panic!("{dialect_name}: original SQL must have a canonical query output");
    };
    let [output] = query.output().columns() else {
        panic!("{dialect_name}: one analytic scalar expected");
    };
    let ValueDomain::Ranges(ranges) = output.domain() else {
        panic!("{dialect_name}: strongest analytic numeric bounds missing: {:?}", output.domain());
    };
    let [interval] = ranges.ranges() else {
        panic!("{dialect_name}: one contiguous numeric interval expected");
    };
    let bound = |bound: &sql_semantic_protocol::Bound| {
        let LiteralValue::Number(value) = bound.value().value() else {
            panic!("{dialect_name}: bound must be numeric");
        };
        (value.clone(), bound.inclusive())
    };
    (interval.lower().map(bound), interval.upper().map(bound))
}

#[test]
fn original_rank_bucket_and_distribution_cases_have_portable_exact_interval_bounds() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("original TASK-195 SQL");
    let specs = [
        ("88-C177-S111", Some(("1", true)), None),
        ("88-C177-S112", Some(("1", true)), None),
        ("88-C177-S113", Some(("1", true)), Some(("3", true))),
        ("88-C177-S114", Some(("0", true)), Some(("1", true))),
        ("88-C177-S115", Some(("0", false)), Some(("1", true))),
        ("88-C177-S118", Some(("0", true)), None),
    ];
    for (id, expected_lower, expected_upper) in specs {
        let original = frozen["cases"]
            .as_array()
            .expect("36 original SQL cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("named original SQL");
        let sql = original["sql"].as_str().expect("immutable sample");
        for dialect in frozen["dialects"].as_array().expect("13 public parsers") {
            let name = dialect.as_str().expect("named parser");
            let (lower, upper) = range(sql, name);
            assert_eq!(
                lower.as_ref().map(|(value, inc)| (value.as_str(), *inc)),
                expected_lower,
                "{id}/{name}: wrong lower bound or inclusivity"
            );
            assert_eq!(
                upper.as_ref().map(|(value, inc)| (value.as_str(), *inc)),
                expected_upper,
                "{id}/{name}: wrong upper bound or inclusivity"
            );
        }
    }
}

#[test]
fn invalid_ntile_buckets_must_not_claim_a_positive_finite_bucket_domain() {
    for sql in [
        "SELECT NTILE(0) OVER (ORDER BY id) FROM orders",
        "SELECT NTILE(-2) OVER (ORDER BY id) FROM orders",
        "SELECT NTILE(amount) OVER (ORDER BY id) FROM orders",
    ] {
        let dialect = dialect_from_name("duckdb").expect("project parser");
        if let Ok(protocol) = analyze_sql(sql, "duckdb", dialect.as_ref()) {
            let [ProtocolStatement::Query(query)] = protocol.statements() else {
                continue;
            };
            let [output] = query.output().columns() else {
                continue;
            };
            assert!(
                matches!(output.domain(), ValueDomain::Unknown(_) | ValueDomain::Unbounded),
                "{sql}: invalid or data-dependent buckets cannot have a finite proved range"
            );
        }
    }
}
