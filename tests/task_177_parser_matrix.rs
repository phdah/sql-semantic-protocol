//! TASK-177 exhaustive explicit SQL parser-boundary sampling.
//!
//! Each inventoried TASK-66 variant has one committed SQL shape tested with
//! sqlparser for every exposed dialect. A parse result does NOT certify the
//! semantic meaning of the entire family, its adapters or vendor execution.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::dialect_from_name;
use sqlparser::parser::Parser;

const CASES: &str = include_str!("../docs/task-177-parser-samples.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");
const SNAPSHOT: &str = include_str!("../docs/task-177-parser-snapshot.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("valid versioned JSON")
}

fn rows<'a>(source: &'a Value, key: &str) -> &'a [Value] {
    source[key].as_array().expect("versioned array")
}

fn field<'a>(source: &'a Value, key: &str) -> &'a str {
    source[key].as_str().expect("nonempty string")
}

#[test]
fn every_named_variant_has_a_unique_sql_shape_and_owned_parser_classification() {
    let evidence = parse(CASES);
    let manifest = parse(COVERAGE);
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["task_id"], "TASK-177");
    assert_eq!(evidence["case_count"], 251);

    let expected: BTreeSet<_> = rows(&manifest, "features")
        .iter()
        .flat_map(|feature| {
            rows(feature, "variants").iter().map(move |variant| {
                (field(feature, "id"), variant.as_str().expect("variant syntax"))
            })
        })
        .collect();
    let mut ids = BTreeSet::new();
    let mut actual = BTreeSet::new();
    for case in rows(&evidence, "cases") {
        let id = field(case, "id");
        let feature = field(case, "feature_id");
        let variant = field(case, "variant");
        assert!(ids.insert(id), "duplicate parser fixture {id}");
        assert!(actual.insert((feature, variant)), "duplicate variant {feature}/{variant}");
        assert!(!field(case, "sql").is_empty(), "empty SQL for {id}");
        assert_eq!(case["test_scope"], "sqlparser_parser_boundary_only");
        assert_eq!(case["canonical_evidence"], "not_certified");
        assert_eq!(case["adapter_evidence"], "not_certified");
        assert_eq!(case["engine_execution"], "not_certified");
    }
    assert_eq!(actual, expected);
    assert_eq!(ids.len(), 251);
}

#[test]
fn parser_dialect_matrix_matches_fixed_non_tautological_snapshot() {
    let evidence = parse(CASES);
    let manifest = parse(COVERAGE);
    let snapshot = parse(SNAPSHOT);
    let dialects = rows(&evidence, "dialects");
    let expected_dialects: BTreeSet<_> = rows(&manifest, "dialects")
        .iter()
        .map(|d| field(d, "name"))
        .collect();
    let actual_dialects: BTreeSet<_> = dialects
        .iter()
        .map(|d| d.as_str().expect("dialect name"))
        .collect();
    assert_eq!(actual_dialects, expected_dialects);
    assert_eq!(dialects.len(), 13);
    assert_eq!(snapshot["schema_version"], 1);
    assert_eq!(snapshot["task_id"], "TASK-177");
    assert_eq!(snapshot["dialects"], evidence["dialects"]);

    let mut observed = BTreeMap::new();
    for case in rows(&evidence, "cases") {
        let id = field(case, "id");
        let sql = field(case, "sql");
        let mut mask = String::new();
        for dialect_name in dialects {
            let name = dialect_name.as_str().expect("dialect name");
            let dialect = dialect_from_name(name).expect("public dialect");
            let outcome = Parser::parse_sql(dialect.as_ref(), sql);
            mask.push(match outcome {
                Ok(statements) if !statements.is_empty() => 'P',
                _ => 'R',
            });
        }
        observed.insert(id.to_owned(), mask);
    }
    assert_eq!(observed.len(), 251);

    let expected = snapshot["matrix"]
        .as_object()
        .expect("complete independently pinned matrix");
    if expected.is_empty() {
        panic!(
            "TASK177_OBSERVED_MATRIX={}",
            serde_json::to_string(&observed).expect("serializable observed matrix")
        );
    }

    let expected_ids: BTreeSet<_> = expected.keys().map(String::as_str).collect();
    let observed_ids: BTreeSet<_> = observed.keys().map(String::as_str).collect();
    assert_eq!(expected_ids, observed_ids, "frozen parser case scope changed");
    for (id, result) in observed {
        let frozen = expected
            .get(&id)
            .and_then(Value::as_str)
            .expect("frozen per-dialect parser mask");
        assert_eq!(frozen.len(), 13, "{id}: incomplete parser snapshot");
        assert!(frozen.chars().all(|status| matches!(status, 'P' | 'R')));
        assert_eq!(result, frozen, "{id}: parser behavior changed; review SQL dialect boundary before editing snapshot");
    }

    assert_eq!(snapshot["vendor_execution_certified"], false);
    assert_eq!(snapshot["canonical_semantics_certified"], false);
}
