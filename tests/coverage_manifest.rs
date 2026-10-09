//! Executable release-coverage manifest and conservative dialect/engine evidence.
mod common;

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

fn manifest() -> Value {
    serde_json::from_str(include_str!("../docs/coverage-manifest.json"))
        .expect("coverage manifest must be valid JSON")
}

fn required_string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing text field: {key}"))
}

fn required_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("missing array field: {key}"))
}

#[test]
fn manifest_tracks_every_exposed_dialect_and_each_feature_cell() {
    let manifest = manifest();
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["release_target"], "3.0.0");
    assert_eq!(
        manifest["review_status"],
        "maintainer_and_sql_tdg_review_pending"
    );

    let dialects = required_array(&manifest, "dialects");
    let names: BTreeSet<_> = dialects
        .iter()
        .map(|dialect| required_string(dialect, "name"))
        .collect();
    let exposed: BTreeSet<_> = common::DIALECTS
        .iter()
        .copied()
        .filter(|name| *name != "postgres")
        .collect();
    assert_eq!(
        names, exposed,
        "dialect additions require an inventory update"
    );
    assert_eq!(dialects.len(), 13);
    assert!(dialects.iter().any(|dialect| {
        dialect["name"] == "postgresql"
            && required_array(dialect, "aliases").contains(&Value::from("postgres"))
    }));
    for dialect in dialects {
        let name = required_string(dialect, "name");
        assert!(dialect_from_name(name).is_some(), "unknown dialect {name}");
        assert!(!required_string(dialect, "engine_version").is_empty());
    }

    let features = required_array(&manifest, "features");
    assert!(
        features.len() >= 50,
        "coverage inventory unexpectedly shrank"
    );
    let mut feature_ids = BTreeSet::new();
    let mut pending = 0;
    for feature in features {
        let id = required_string(feature, "id");
        assert!(feature_ids.insert(id), "duplicated feature {id}");
        assert!(
            !required_array(feature, "variants").is_empty(),
            "{id} has no variants"
        );
        assert!(
            !required_array(feature, "protocol_tasks").is_empty(),
            "{id} has no protocol task"
        );
        assert!(
            !required_array(feature, "tdg_tasks").is_empty(),
            "{id} has no downstream owner"
        );
        let scope = required_string(feature, "scope");
        assert!(
            matches!(scope, "release_blocking" | "pending_exclusion_approval"),
            "{id} has an unaudited scope: {scope}"
        );
        if scope == "pending_exclusion_approval" {
            pending += 1;
            assert!(
                feature["exclusion"].as_str().is_some(),
                "{id} needs a reason"
            );
        }
        assert_eq!(
            feature["physical_source_positive"], "not_end_to_end_proven",
            "{id}: local witnesses must not count as physical-source proofs"
        );
        assert_eq!(
            feature["physical_source_negative"], "not_end_to_end_proven",
            "{id}: negative witnesses need independent absence proofs"
        );

        let cells = feature["dialects"]
            .as_object()
            .expect("per-dialect evidence");
        assert_eq!(cells.len(), names.len(), "{id} must cover every dialect");
        for name in &names {
            let cell = &feature["dialects"][name];
            assert!(
                matches!(
                    cell["parse"].as_str(),
                    Some("fixture_tested" | "unverified")
                ),
                "{id}/{name}: invalid parser evidence"
            );
            assert!(
                matches!(
                    cell["canonical"].as_str(),
                    Some("fixture_tested" | "unverified")
                ),
                "{id}/{name}: invalid canonical evidence"
            );
            assert_eq!(cell["positive"], "not_end_to_end_proven");
            assert_eq!(cell["negative"], "not_end_to_end_proven");
            assert_eq!(cell["cardinality"], "unverified");
            assert!(
                matches!(
                    cell["oracle"].as_str(),
                    Some("unverified" | "fixture_duckdb_only")
                ),
                "{id}/{name}: invalid execution evidence"
            );
            assert!(
                name == &"duckdb" || cell["oracle"] == "unverified",
                "only DuckDB has a SQL execution oracle"
            );
            let evidence = required_array(cell, "fixture_ids");
            assert_eq!(
                cell["parse"] == "fixture_tested",
                !evidence.is_empty(),
                "{id}/{name}: parser evidence must have a test fixture"
            );
        }
    }
    assert!(
        pending > 0,
        "unsupported scope requires maintainer approval"
    );
    assert!(feature_ids.contains("execution.dbt_fixture"));
    assert!(feature_ids.contains("outcomes.classification"));
}

#[test]
fn variant_evidence_uses_fail_closed_defaults_without_inheriting_feature_claims() {
    let manifest = manifest();
    let default = &manifest["variant_evidence_defaults"];
    assert_eq!(default["parse"], "unverified");
    assert_eq!(default["physical_positive"], "not_end_to_end_proven");
    assert_eq!(default["physical_negative"], "not_end_to_end_proven");
    assert_eq!(default["cardinality"], "unverified");
    assert_eq!(default["engine_oracle"], "unverified");

    let names: BTreeSet<_> = required_array(&manifest, "dialects")
        .iter()
        .map(|dialect| required_string(dialect, "name"))
        .collect();
    let fixtures = required_array(&manifest, "fixtures");
    let mut expanded_cells = 0;

    for feature in required_array(&manifest, "features") {
        let variants = required_array(feature, "variants");
        let overrides = feature["variant_overrides"]
            .as_object()
            .expect("explicit sparse per-variant overrides");
        for variant in overrides.keys() {
            assert!(
                variants.contains(&Value::from(variant.as_str())),
                "override must belong to an inventoried variant: {variant}"
            );
        }
        for variant in variants {
            let syntax = variant.as_str().expect("variant syntax");
            for name in &names {
                expanded_cells += 1;
                let explicit = &feature["variant_overrides"][syntax][name];
                let parse = explicit["parse"]
                    .as_str()
                    .unwrap_or(default["parse"].as_str().expect("default parser status"));
                assert!(
                    matches!(
                        parse,
                        "unverified" | "fixture_tested" | "representative_only"
                    ),
                    "unexpected per-variant evidence for {syntax}/{name}"
                );
                if parse == "fixture_tested" {
                    assert!(
                        fixtures.iter().any(|fixture| {
                            fixture["feature"] == feature["id"]
                                && required_array(fixture, "dialects").contains(&Value::from(*name))
                                && required_string(fixture, "sql").contains(syntax)
                        }),
                        "{syntax}/{name}: variant claims need an actual SQL fixture"
                    );
                }
                if !explicit.is_null() {
                    assert!(explicit["parse"].as_str().is_some());
                }
            }
        }
    }
    assert!(expanded_cells >= 3_000, "variant coverage silently shrank");
}

#[test]
fn parser_and_analysis_claims_are_exercised_by_manifest_fixtures() {
    let manifest = manifest();
    let fixtures = required_array(&manifest, "fixtures");
    let features = required_array(&manifest, "features");
    let mut checked = BTreeSet::new();

    for fixture in fixtures {
        let fixture_id = required_string(fixture, "id");
        let feature_id = required_string(fixture, "feature");
        let feature = features
            .iter()
            .find(|candidate| candidate["id"] == feature_id)
            .unwrap_or_else(|| panic!("unknown fixture feature {feature_id}"));
        let sql = required_string(fixture, "sql");

        for dialect in required_array(fixture, "dialects") {
            let name = dialect.as_str().expect("dialect name");
            assert!(checked.insert((fixture_id, name)), "duplicate fixture");
            let cell = &feature["dialects"][name];
            assert_eq!(cell["parse"], "fixture_tested", "{fixture_id}/{name}");
            assert!(required_array(cell, "fixture_ids").contains(&Value::from(fixture_id)));
            let dialect = dialect_from_name(name).expect("fixture dialect must exist");
            let analyzed = analyze_sql(sql, name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{fixture_id}/{name} must analyze: {error}"));
            if fixture["expect"] == "diagnostic" {
                let expected = required_string(fixture, "diagnostic");
                assert!(
                    to_json(&analyzed).contains(expected),
                    "{fixture_id}/{name}: must expose residual {expected}"
                );
            } else {
                assert_eq!(fixture["expect"], "analyzed", "{fixture_id}/{name}");
            }
        }
    }
}

#[test]
fn duckdb_oracles_cover_feasible_impossible_null_and_duplicate_cases() {
    let manifest = manifest();
    let cases = required_array(&manifest, "duckdb_oracle_cases");
    let mut kinds = BTreeSet::new();

    for case in cases {
        let id = required_string(case, "id");
        kinds.insert(required_string(case, "kind"));
        let connection = Connection::open_in_memory().expect("DuckDB oracle");
        connection
            .execute_batch(required_string(case, "setup"))
            .unwrap_or_else(|error| panic!("{id}: setup failed: {error}"));
        let query = format!(
            "SELECT COUNT(*) FROM ({}) AS coverage_oracle",
            required_string(case, "sql")
        );
        let count: i64 = connection
            .query_row(&query, [], |row| row.get(0))
            .unwrap_or_else(|error| panic!("{id}: query failed: {error}"));
        assert_eq!(
            count,
            case["expected_rows"].as_i64().expect("expected row count"),
            "{id}: observed SQL result changed"
        );
    }
    assert_eq!(
        kinds,
        BTreeSet::from(["duplicate", "feasible", "impossible", "null"])
    );
}
