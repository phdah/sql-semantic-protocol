//! Independently specified original-SQL outer source, output alias and
//! normalized predicate-shape expectations. Unlike TASK-192's existing
//! cross-dialect full-document equality test, the expected facts here are not
//! sourced from the analyzer's first generated result.
//! This is necessary but not sufficient for full independent 559-cell signoff.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, Predicate, ProtocolStatement};

const EXPECTED: &str = include_str!("../docs/task-192-original-query-shapes.json");
const ORIGINAL: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn predicate_shape(predicate: Option<&Predicate>) -> &'static str {
    match predicate {
        None => "none",
        Some(Predicate::Comparison(_)) => "comparison",
        Some(Predicate::And(_)) => "and",
        Some(Predicate::Or(_)) => "or",
        Some(Predicate::Not(_)) => "not",
        Some(Predicate::Between(_)) => "between",
        Some(Predicate::In(_)) => "in",
        Some(Predicate::IsNull(_)) => "is_null",
        Some(Predicate::LikePrefix(_)) => "like_prefix",
        Some(Predicate::Exists(_)) => "exists",
        Some(Predicate::InSubquery(_)) => "in_subquery",
        Some(Predicate::QuantifiedSubquery(_)) => "quantified_subquery",
        Some(Predicate::BooleanExpression(_)) => "boolean_expression",
        Some(Predicate::Unknown(_)) => "unknown",
        Some(Predicate::Unsupported(_)) => "unsupported",
    }
}

#[test]
fn all_original_sqls_have_independently_frozen_outer_source_and_predicate_shapes() {
    let fixed: Value = serde_json::from_str(EXPECTED).expect("manual original SQL shapes");
    let original: Value = serde_json::from_str(ORIGINAL).expect("original immutable SQL");
    let expected = fixed["cases"].as_array().expect("43 manual expectations");
    let original_cases = original["cases"].as_array().expect("43 fixed SQL inputs");
    let dialects = original["dialects"].as_array().expect("13 parser names");

    assert_eq!(fixed["schema_version"], 1);
    assert_eq!(fixed["owner"], "TASK-192");
    assert_eq!(fixed["case_count"], 43);
    assert_eq!(fixed["independent_review_signed_off"], false);
    assert_eq!(
        (expected.len(), original_cases.len(), dialects.len()),
        (43, 43, 13)
    );

    let mut seen = BTreeSet::new();
    let mut accepted = 0;
    let mut rejected = 0;
    for independently_expected in expected {
        let id = independently_expected["id"]
            .as_str()
            .expect("fixed original case identity");
        assert!(
            seen.insert(id),
            "{id}: duplicate independently frozen shape"
        );
        let sql = original_cases
            .iter()
            .find(|case| case["id"] == id)
            .expect("no original SQL missing from manual expectations")["sql"]
            .as_str()
            .expect("unaltered original SQL");
        let expected_sources: Vec<String> =
            serde_json::from_value(independently_expected["outer_sources"].clone())
                .expect("manual source list");
        let expected_output = independently_expected["output_column"]
            .as_str()
            .expect("manual output column");
        let expected_where = independently_expected["where_kind"]
            .as_str()
            .expect("manual normalized predicate shape");

        for dialect in dialects {
            let name = dialect.as_str().expect("dialect");
            let parser = dialect_from_name(name).expect("public SQL parser");
            let result = analyze_sql(sql, name, parser.as_ref());
            let identity = format!("{id}/{name}");
            if id == "88-C177-S179" && matches!(name, "bigquery" | "generic") {
                assert!(
                    matches!(result, Err(sql_semantic_protocol::Error::Parse(_))),
                    "{identity}: original parser-rejection boundary"
                );
                rejected += 1;
                continue;
            }
            let parsed = result.unwrap_or_else(|error| panic!("{identity}: {error}"));
            let [ProtocolStatement::Query(query)] = parsed.statements() else {
                panic!("{identity}: exactly one original SELECT query");
            };
            let [output] = query.output().columns() else {
                panic!("{identity}: exactly one output column");
            };
            assert_eq!(
                output.name(),
                expected_output,
                "{identity}: source-independent original SQL projection"
            );
            let actual_sources = query
                .sources()
                .iter()
                .map(|source| source.name().to_string())
                .collect::<Vec<_>>();
            assert_eq!(
                actual_sources, expected_sources,
                "{identity}: only the original outer relation instances"
            );
            assert_eq!(
                predicate_shape(query.predicates().where_predicate()),
                expected_where,
                "{identity}: independently predicted WHERE/3VL operator family"
            );
            accepted += 1;
        }
    }
    assert_eq!(seen.len(), 43);
    assert_eq!((accepted, rejected), (557, 2));
}
