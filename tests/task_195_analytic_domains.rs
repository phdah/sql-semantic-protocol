//! Original TASK-195 analytic SQL must emit the strongest portable value
//! domains provable without closed-world row-count assumptions.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

struct AnalyticBound {
    value: String,
    inclusive: bool,
}

struct AnalyticBounds {
    lower: Option<AnalyticBound>,
    upper: Option<AnalyticBound>,
}

fn range(sql: &str, dialect_name: &str) -> AnalyticBounds {
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
        panic!(
            "{dialect_name}: strongest analytic numeric bounds missing: {:?}",
            output.domain()
        );
    };
    let [interval] = ranges.ranges() else {
        panic!("{dialect_name}: one contiguous numeric interval expected");
    };
    let bound = |bound: &sql_semantic_protocol::Bound| {
        let LiteralValue::Number(value) = bound.value().value() else {
            panic!("{dialect_name}: bound must be numeric");
        };
        AnalyticBound {
            value: value.clone(),
            inclusive: bound.inclusive(),
        }
    };
    AnalyticBounds {
        lower: interval.lower().map(bound),
        upper: interval.upper().map(bound),
    }
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
        ("88-C177-S118", Some(("1", true)), None),
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
            let bounds = range(sql, name);
            assert_eq!(
                bounds
                    .lower
                    .as_ref()
                    .map(|bound| (bound.value.as_str(), bound.inclusive)),
                expected_lower,
                "{id}/{name}: wrong lower bound or inclusivity"
            );
            assert_eq!(
                bounds
                    .upper
                    .as_ref()
                    .map(|bound| (bound.value.as_str(), bound.inclusive)),
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
                matches!(
                    output.domain(),
                    ValueDomain::Unknown(_) | ValueDomain::Unbounded
                ),
                "{sql}: invalid or data-dependent buckets cannot have a finite proved range"
            );
        }
    }
}

#[test]
fn implicit_window_frames_count_the_current_row_without_explicit_frame_syntax() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("13 public dialect names");
    let sql_variants = [
        "SELECT COUNT(*) OVER () FROM orders",
        "SELECT COUNT(1) OVER (PARTITION BY amount) FROM orders",
        "SELECT COUNT(*) OVER (ORDER BY id) FROM orders",
        "SELECT COUNT(1) OVER (PARTITION BY amount ORDER BY id) FROM orders",
    ];
    for dialect in frozen["dialects"].as_array().expect("13 dialects") {
        let name = dialect.as_str().expect("public dialect");
        for sql in sql_variants {
            let bounds = range(sql, name);
            assert_eq!(
                bounds
                    .lower
                    .as_ref()
                    .map(|bound| (bound.value.as_str(), bound.inclusive)),
                Some(("1", true)),
                "{name}: {sql} includes the current row even with an implicit frame"
            );
            assert!(
                bounds.upper.is_none(),
                "{name}: {sql} cannot infer a maximum partition cardinality"
            );
        }
    }

    // Explicit frames may omit the current row and can therefore be empty.
    let bounds = range(
        "SELECT COUNT(*) OVER (ORDER BY id ROWS BETWEEN 1 FOLLOWING AND 1 FOLLOWING) FROM orders",
        "duckdb",
    );
    assert_eq!(
        bounds
            .lower
            .as_ref()
            .map(|bound| (bound.value.as_str(), bound.inclusive)),
        Some(("0", true))
    );
}

#[test]
fn zero_offset_window_frame_bounds_are_equivalent_to_current_row() {
    for sql in [
        "SELECT COUNT(*) OVER (ORDER BY id ROWS BETWEEN 0 PRECEDING AND 0 PRECEDING) FROM orders",
        "SELECT COUNT(*) OVER (ORDER BY id ROWS BETWEEN 0 FOLLOWING AND 0 FOLLOWING) FROM orders",
    ] {
        let bounds = range(sql, "duckdb");
        assert_eq!(
            bounds
                .lower
                .as_ref()
                .map(|bound| (bound.value.as_str(), bound.inclusive)),
            Some(("1", true)),
            "{sql}: a zero-offset frame must contain its current row"
        );
        assert!(bounds.upper.is_none(), "{sql}: no finite row count proof");
    }
}

#[test]
fn window_count_of_a_constant_null_is_exactly_zero() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("13 dialects");
    let sql = "SELECT COUNT(NULL) OVER (PARTITION BY id ORDER BY amount) FROM orders";
    for dialect in frozen["dialects"].as_array().expect("public parsers") {
        let name = dialect.as_str().expect("dialect name");
        let parser = dialect_from_name(name).expect("supported parser");
        let protocol = analyze_sql(sql, name, parser.as_ref())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{name}: expected query");
        };
        let [column] = query.output().columns() else {
            panic!("{name}: expected one count scalar");
        };
        let ValueDomain::Set(values) = column.domain() else {
            panic!("{name}: COUNT(NULL) must have exact zero domain");
        };
        assert_eq!(values.mode(), sql_semantic_protocol::SetMode::Include);
        assert_eq!(
            values.values().iter().map(|value| value.value()).collect::<Vec<_>>(),
            vec![&LiteralValue::Number("0".to_string())],
            "{name}: NULL never contributes to COUNT"
        );
    }
}

#[test]
fn grouping_marker_is_a_bit_not_a_nullable_physical_group_key() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("original source inventory");
    let original = frozen["cases"]
        .as_array()
        .expect("36 original SQL cases")
        .iter()
        .find(|case| case["id"] == "88-C177-S104")
        .expect("original GROUPING(id) SQL");
    let sql = original["sql"].as_str().expect("immutable original SQL");
    for dialect in frozen["dialects"].as_array().expect("13 public dialects") {
        let name = dialect.as_str().expect("dialect");
        let parser = dialect_from_name(name).expect("project parser");
        let protocol = analyze_sql(sql, name, parser.as_ref())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{name}: missing original query");
        };
        let output = query.output().columns();
        assert_eq!(output.len(), 2);
        let ValueDomain::Set(markers) = output[0].domain() else {
            panic!("{name}: GROUPING(id) must preserve a proven 0/1 marker");
        };
        assert_eq!(markers.mode(), sql_semantic_protocol::SetMode::Include);
        let actual = markers
            .values()
            .iter()
            .map(|item| item.value().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                LiteralValue::Number("0".to_string()),
                LiteralValue::Number("1".to_string()),
            ],
            "{name}: grouping subtotal marker cannot become NULL"
        );
    }
}
