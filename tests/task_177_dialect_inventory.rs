//! TASK-177: finite TASK-66 dialect/variant evidence and independent SQL oracles.
//!
//! Inventory tests enforce completeness and fail-closed claims, not semantic
//! certification of every variant. The two portable closed-world oracles are
//! deliberately separate from the exhaustive parser-evidence registry.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, to_json, LiteralValue, ProtocolStatement, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-177-acceptance-freeze.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn parse(contents: &str) -> Value {
    serde_json::from_str(contents).expect("versioned coverage JSON")
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("missing {field}"))
}

fn array<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("missing array {field}"))
}

#[test]
fn every_task_66_variant_has_thirteen_explicit_parser_evidence_cells() {
    let freeze = parse(FREEZE);
    let manifest = parse(COVERAGE);
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-177");
    assert_eq!(freeze["fixture_ids"], serde_json::json!(["88-C177-P", "88-C177-N"]));
    assert_eq!(freeze["variant_count"], 251);
    assert_eq!(freeze["dialect_cell_count"], 3263);
    assert_eq!(freeze["completion"]["child_done"], false);
    assert_eq!(freeze["completion"]["release_approved"], false);

    let expected_dialects: BTreeSet<_> = array(&manifest, "dialects")
        .iter()
        .map(|dialect| text(dialect, "name"))
        .collect();
    let frozen_dialects: BTreeSet<_> = array(&freeze, "dialects")
        .iter()
        .map(|dialect| dialect.as_str().expect("dialect name"))
        .collect();
    assert_eq!(expected_dialects, frozen_dialects);
    assert_eq!(frozen_dialects.len(), 13);

    let fixture_index: BTreeMap<_, _> = array(&manifest, "fixtures")
        .iter()
        .map(|fixture| (text(fixture, "id"), fixture))
        .collect();
    let inventory = array(&freeze, "inventory");
    let mut visited = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut statuses = BTreeMap::<String, usize>::new();
    let mut count = 0;

    for row in inventory {
        let feature_id = text(row, "feature_id");
        let variant = text(row, "variant");
        assert!(visited.insert((feature_id, variant)), "duplicate {feature_id}/{variant}");
        assert!(ids.insert(text(row, "id")), "duplicate fixture ID");
        let feature = array(&manifest, "features")
            .iter()
            .find(|feature| feature["id"] == feature_id)
            .unwrap_or_else(|| panic!("unknown feature {feature_id}"));
        assert!(array(feature, "variants").contains(&Value::from(variant)));
        assert_eq!(row["scope"], feature["scope"]);
        assert_eq!(row["protocol_owners"], feature["protocol_tasks"]);
        assert_eq!(row["consumer_owners"], feature["tdg_tasks"]);

        let cells = row["dialect_cells"]
            .as_object()
            .expect("explicit per-dialect row");
        let names: BTreeSet<_> = cells.keys().map(String::as_str).collect();
        assert_eq!(names, expected_dialects, "{feature_id}/{variant}");

        for dialect in &expected_dialects {
            count += 1;
            let cell = &row["dialect_cells"][dialect];
            assert_eq!(
                text(cell, "id"),
                format!("{}-{dialect}", text(row, "id"))
            );
            let explicit = &feature["variant_overrides"][variant][dialect];
            let expected_status = explicit["parse"].as_str().unwrap_or("unverified");
            assert_eq!(
                text(cell, "parser_evidence"),
                expected_status,
                "{feature_id}/{variant}/{dialect}: evidence must not be inherited from a feature-level sample"
            );
            assert!(matches!(
                expected_status,
                "unverified" | "fixture_tested" | "representative_only"
            ));
            *statuses.entry(expected_status.to_owned()).or_default() += 1;

            let expected_ids = explicit["fixture_ids"].as_array();
            let references = array(cell, "fixture_ids");
            assert_eq!(Some(references), expected_ids.or_else(|| {
                if references.is_empty() {
                    Some(references)
                } else {
                    None
                }
            }));
            assert_eq!(cell["canonical_certified"], false);
            assert_eq!(cell["engine_certified"], false);
            assert_eq!(cell["terminal_oracle_certified"], false);
            assert_eq!(cell["release_gate"], "unverified_or_review_required");

            if expected_status == "unverified" {
                assert!(references.is_empty(), "invented parser evidence");
                assert!(cell["sample_sql"].is_null(), "unverified SQL must not be invented");
            } else {
                assert!(!references.is_empty(), "missing evidence fixture");
                let sql = text(cell, "sample_sql");
                for reference in references {
                    let id = reference.as_str().expect("fixture id");
                    let fixture = fixture_index
                        .get(id)
                        .unwrap_or_else(|| panic!("missing manifest fixture {id}"));
                    assert_eq!(fixture["feature"], feature_id);
                    assert_eq!(fixture["variant_status"], expected_status);
                    assert!(array(fixture, "covered_variants").contains(&Value::from(variant)));
                    assert!(array(fixture, "dialects").contains(&Value::from(*dialect)));
                    assert_eq!(fixture["sql"], sql);
                }
            }
        }
    }

    let expected: BTreeSet<_> = array(&manifest, "features")
        .iter()
        .flat_map(|feature| {
            array(feature, "variants").iter().map(move |variant| {
                (
                    text(feature, "id"),
                    variant.as_str().expect("variant syntax"),
                )
            })
        })
        .collect();
    assert_eq!(visited, expected, "TASK-66 variant inventory drift");
    assert_eq!(count, expected.len() * expected_dialects.len());
    assert_eq!(inventory.len(), 251);
    for (status, count) in statuses {
        assert_eq!(freeze["parser_evidence_counts"][&status], count);
    }
    assert_eq!(
        manifest["task_177_dialect_parser_acceptance"]["freeze"],
        "docs/task-177-acceptance-freeze.json"
    );
    assert_eq!(
        manifest["task_177_dialect_parser_acceptance"]["status"],
        "in_progress_parser_inventory_release_blocked"
    );
}

fn pairs(db: &Connection, sql: &str) -> Vec<(i64, Option<i64>)> {
    let mut statement = db.prepare(sql).expect("DuckDB statement");
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("DuckDB query");
    rows.map(|row| row.expect("DuckDB row")).collect()
}

fn expected_pairs(case: &Value, key: &str) -> Vec<(i64, Option<i64>)> {
    array(case, key)
        .iter()
        .map(|row| {
            let values = row.as_array().expect("physical row");
            assert_eq!(values.len(), 2);
            (values[0].as_i64().expect("source ID"), values[1].as_i64())
        })
        .collect()
}

#[test]
fn frozen_positive_and_impossible_goldens_match_full_duckdb_bags_and_portable_domains() {
    let freeze = parse(FREEZE);
    let cases = array(&freeze, "golden_cases");
    assert_eq!(cases.len(), 2);

    for case in cases {
        let id = text(case, "id");
        let sql = text(case, "sql");
        let db = Connection::open_in_memory().expect("in-memory DuckDB");
        db.execute_batch(text(case, "setup"))
            .expect("closed physical input fixture");

        let source = pairs(&db, "SELECT id, amount FROM orders ORDER BY id");
        assert_eq!(source, expected_pairs(case, "source_rows"), "{id}: source");
        let predicate = sql
            .split_once(" WHERE ")
            .unwrap_or_else(|| panic!("{id}: expected WHERE"))
            .1;
        let selected = pairs(&db, &format!("{sql} ORDER BY id"));
        let not_true = pairs(
            &db,
            &format!(
                "SELECT id, amount FROM orders WHERE ({predicate}) IS NOT TRUE ORDER BY id"
            ),
        );
        let unknown = pairs(
            &db,
            &format!(
                "SELECT id, amount FROM orders WHERE ({predicate}) IS UNKNOWN ORDER BY id"
            ),
        );
        assert_eq!(selected, expected_pairs(case, "selected_rows"), "{id}: TRUE");
        assert_eq!(not_true, expected_pairs(case, "not_true_rows"), "{id}: NOT TRUE");
        assert_eq!(unknown, expected_pairs(case, "unknown_rows"), "{id}: UNKNOWN");

        let source_ids: BTreeSet<_> = source.iter().map(|row| row.0).collect();
        let true_ids: BTreeSet<_> = selected.iter().map(|row| row.0).collect();
        let rejected_ids: BTreeSet<_> = not_true.iter().map(|row| row.0).collect();
        assert!(true_ids.is_disjoint(&rejected_ids), "{id}: double counted");
        assert_eq!(
            true_ids.union(&rejected_ids).copied().collect::<BTreeSet<_>>(),
            source_ids,
            "{id}: incomplete NOT TRUE partition"
        );
        assert!(unknown.iter().all(|row| rejected_ids.contains(&row.0)));

        // Dialect parsing is a public API seam. This comparison certifies only
        // the two named portable shapes, not vendor runtime or all 3,263 cells.
        let mut baseline: Option<Value> = None;
        for name in array(&freeze, "dialects") {
            let dialect_name = name.as_str().expect("dialect");
            let dialect = dialect_from_name(dialect_name).expect("exposed dialect");
            let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{dialect_name}: {error}"));
            if id == "88-C177-P" {
                let query = match protocol.statements().first() {
                    Some(ProtocolStatement::Query(query)) => query,
                    other => panic!("{id}: expected query, got {other:?}"),
                };
                let ValueDomain::Ranges(ranges) = query.output().columns()[1].domain() else {
                    panic!("{id}: projected amount needs a bounded interval");
                };
                let [range] = ranges.ranges() else {
                    panic!("{id}: expected one interval");
                };
                let lower = range.lower().expect("inclusive 10");
                let upper = range.upper().expect("exclusive 20");
                assert_eq!(lower.value().value(), &LiteralValue::Number("10".into()));
                assert!(lower.inclusive());
                assert_eq!(upper.value().value(), &LiteralValue::Number("20".into()));
                assert!(!upper.inclusive());
            }
            let mut document: Value = serde_json::from_str(&to_json(&protocol))
                .expect("canonical JSON emission");
            assert_eq!(document["inputs"][0]["dialect"], dialect_name);
            document["inputs"][0]["dialect"] = Value::Null;
            if let Some(expected) = &baseline {
                assert_eq!(&document, expected, "{id}/{dialect_name}: canonical drift");
            } else {
                baseline = Some(document);
            }
        }
    }
}
