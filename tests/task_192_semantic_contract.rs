//! TASK-192 public semantic contract: independently stated expected domain
//! kinds, exactness dispositions and conservative parser boundaries for all
//! 43 original SQL variants × 13 dialects. No vendor runtime is attested.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, to_json, LiteralValue, ProtocolStatement, SetMode, ValueDomain,
};

const CONTRACT: &str = include_str!("../docs/task-192-semantic-cell-ledger.json");
const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const REVIEW: &str = include_str!("../docs/task-192-review-queue.json");
const PRIOR: &str = include_str!("../docs/task-177-predicate-oracles.json");

fn parse(s: &str) -> Value {
    serde_json::from_str(s).expect("versioned independent expected outcome contract")
}

fn attr<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .expect("required semantic contract value")
}

/// Translate only independently frozen signed integer and SQL NULL literals.
/// This deliberately has no fallback that would silently accept a new value type.
fn frozen_literal(value: &LiteralValue) -> Value {
    match value {
        LiteralValue::Number(raw) => json!(raw.parse::<i64>().expect("frozen signed integer")),
        LiteralValue::Null => Value::Null,
        _ => panic!("TASK-192 frozen numeric policy cannot certify other literal types"),
    }
}

/// Compare the actual *values* of every precise projection, not just its
/// domain category. These expectations come from the original native/SQL-law
/// goldens in the acceptance freeze, never from a previously emitted document.
fn assert_independent_projected_domain(domain: &ValueDomain, policy: &Value, identity: &str) {
    match attr(policy, "kind") {
        "precise_constraint" => {
            assert_eq!(policy["nullable"], false, "{identity}: numeric NULL policy");
            let expected = &policy["literal_domain"];
            match attr(expected, "kind") {
                "set_include" | "set_exclude" => {
                    let ValueDomain::Set(set) = domain else {
                        panic!("{identity}: expected independently bounded numeric set, got {domain:?}");
                    };
                    let mode = match set.mode() {
                        SetMode::Include => "set_include",
                        SetMode::Exclude => "set_exclude",
                    };
                    assert_eq!(
                        mode,
                        attr(expected, "kind"),
                        "{identity}: set inclusion/exclusion"
                    );
                    let mut actual = set
                        .values()
                        .iter()
                        .map(|literal| frozen_literal(literal.value()))
                        .collect::<Vec<_>>();
                    let mut frozen = expected["values"]
                        .as_array()
                        .expect("frozen numeric set members")
                        .clone();
                    actual.sort_by_key(Value::to_string);
                    frozen.sort_by_key(Value::to_string);
                    assert_eq!(
                        actual, frozen,
                        "{identity}: exact numeric and NULL set members"
                    );
                }
                "range" | "range_union" => {
                    let ValueDomain::Ranges(ranges) = domain else {
                        panic!("{identity}: expected independently bounded numeric intervals, got {domain:?}");
                    };
                    let actual = ranges
                        .ranges()
                        .iter()
                        .map(|range| {
                            let lower = range.lower().map(|bound| {
                                json!([frozen_literal(bound.value().value()), bound.inclusive()])
                            });
                            let upper = range.upper().map(|bound| {
                                json!([frozen_literal(bound.value().value()), bound.inclusive()])
                            });
                            json!({"lower": lower, "upper": upper})
                        })
                        .collect::<Vec<_>>();
                    let frozen = if expected["kind"] == "range" {
                        vec![json!({"lower": expected["lower"], "upper": expected["upper"]})]
                    } else {
                        expected["ranges"]
                            .as_array()
                            .expect("frozen disjoint numeric intervals")
                            .clone()
                    };
                    assert_eq!(
                        actual, frozen,
                        "{identity}: signed interval bounds, direction and inclusivity"
                    );
                }
                other => panic!("{identity}: unreviewed frozen precise domain shape {other}"),
            }
        }
        "unconstrained_nullable_id" => {
            assert_eq!(policy["null_allowed"], true, "{identity}: SQL NULL policy");
            assert!(
                matches!(domain, ValueDomain::Unbounded),
                "{identity}: unrelated WHERE condition cannot constrain the projected id: {domain:?}"
            );
        }
        "native_evaluation_error" | "unsupported_structural_cast" => {
            assert!(
                matches!(domain, ValueDomain::Unknown(_)),
                "{identity}: no portable successful scalar outcome is certified: {domain:?}"
            );
        }
        other => panic!("{identity}: missing independently frozen projected domain policy {other}"),
    }
}

#[test]
fn all_559_original_public_cells_have_explicit_expected_semantic_dispositions() {
    let contract = parse(CONTRACT);
    let original = parse(FREEZE);
    let prior = parse(PRIOR);
    let review = parse(REVIEW);
    assert_eq!(contract["schema_version"], 1);
    assert_eq!(contract["owner"], "TASK-192");
    assert_eq!(contract["case_count"], 43);
    assert_eq!(contract["cell_count"], 559);
    assert_eq!(contract["release_approved"], false);
    assert_eq!(
        contract["independent_review_signed_off"], review["independent_signoff"],
        "semantic contract must agree with the independently recorded review"
    );

    let cases: BTreeMap<&str, &Value> = original["cases"]
        .as_array()
        .expect("original 43 cases")
        .iter()
        .map(|case| (attr(case, "id"), case))
        .collect();
    let policies: BTreeMap<&str, &Value> = contract["cases"]
        .as_array()
        .expect("independent per-case expected outcomes")
        .iter()
        .map(|case| (attr(case, "id"), case))
        .collect();
    assert_eq!(cases.len(), 43);
    assert_eq!(policies.len(), 43);

    for (id, expected) in &policies {
        let original_case = cases.get(id).expect("policy must name original SQL");
        assert_eq!(
            expected["independent_projected_domain_policy"],
            original_case["typed_output_expectation"],
            "{id}: independent typed SQL outcome policy changed"
        );
        assert_eq!(
            expected["native_duckdb_oracle"],
            original_case["source_and_terminal_duckdb_oracle"]["source"],
            "{id}: link original native source and terminal bag evidence"
        );
        if expected["expected_output_domain_kind"] == "set"
            || expected["expected_output_domain_kind"] == "ranges"
        {
            let pinned = prior["cases"]
                .as_array()
                .expect("native numeric domain oracles")
                .iter()
                .find(|row| row["id"] == *id)
                .expect("precise domain must have an independent typed numeric oracle");
            assert_eq!(
                expected["independent_projected_domain_policy"]["literal_domain"],
                pinned["expected_projected_domain"],
                "{id}: number, NULL exclusion or interval inclusivity differs"
            );
        }
    }

    let mut seen = BTreeSet::new();
    let mut normalized_documents = BTreeMap::<&str, Value>::new();
    let mut accepted = 0;
    let mut rejected = 0;
    let mut repaired = 0;
    for cell in contract["cells"]
        .as_array()
        .expect("559 independent dialect cells")
    {
        let id = attr(cell, "case_id");
        let dialect_name = attr(cell, "dialect");
        let identity = attr(cell, "id");
        assert_eq!(identity, format!("{id}/{dialect_name}"));
        assert!(
            seen.insert(identity),
            "{identity}: duplicate dialect proof cell"
        );
        assert_eq!(cell["expected_typed_domain_case_ref"], id);
        assert_eq!(cell["native_source_terminal_case_ref"], id);
        assert!(
            cell["independently_reviewed"].is_boolean(),
            "{identity}: every semantic cell needs an explicit independent-review state"
        );
        assert_eq!(cell["external_vendor_runtime"], "unattested");

        let case = cases.get(id).expect("original variant SQL");
        let index = original["dialects"]
            .as_array()
            .expect("13 public dialects")
            .iter()
            .position(|dialect| dialect == dialect_name)
            .expect("public parser identity");
        assert_eq!(
            cell["historical_status"],
            case["dialect_cells"][index]["observed_parser_canonical_status"],
            "{identity}: original frozen parser observation was overwritten"
        );
        let policy = policies.get(id).expect("expected case semantics");
        let name = dialect_from_name(dialect_name).expect("exposed SQL parser");
        let result = analyze_sql(attr(case, "sql"), dialect_name, name.as_ref());

        if cell["expected_public_result"] == "parse_rejected" {
            assert!(
                matches!(result, Err(sql_semantic_protocol::Error::Parse(_))),
                "{identity}: parser boundary requires an actual parse rejection"
            );
            assert_eq!(cell["historical_status"], "R");
            rejected += 1;
            continue;
        }

        let protocol = result.unwrap_or_else(|error| {
            panic!("{identity}: expected public SQL semantics, got {error}")
        });
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{identity}: expected one complete public query statement");
        };
        let [projected] = query.output().columns() else {
            panic!("{identity}: expected exactly one public projected column");
        };
        assert_independent_projected_domain(
            projected.domain(),
            &policy["independent_projected_domain_policy"],
            identity,
        );

        let mut wire = parse(&to_json(&protocol));
        {
            let statement = &wire["inputs"][0]["statements"][0];
            assert_eq!(
                statement["kind"], "query",
                "{identity}: no silent statement drop"
            );
            assert_eq!(
                statement["output"]["columns"][0]["domain"]["kind"],
                cell["expected_projected_domain_kind"],
                "{identity}: independent strongest sound output-domain category"
            );
            let actual_exactness = &statement["condition_exactness"];
            assert_eq!(
                actual_exactness["status"], cell["expected_condition_exactness"],
                "{identity}: expected exact/conditional/residual classification"
            );
            if cell["expected_residual_reason"].is_string() {
                let residuals = actual_exactness["residual_conditions"]
                    .as_array()
                    .expect("explicit residual causes");
                assert_eq!(residuals.len(), 1, "{identity}: no hidden residual clauses");
                assert_eq!(
                    residuals[0]["reason"], cell["expected_residual_reason"],
                    "{identity}: preserve the named unapproved semantic gap"
                );
                assert_eq!(
                    residuals[0]["clause"], "where",
                    "{identity}: original predicate residual must identify WHERE"
                );
                assert_eq!(
                    residuals[0]["identity"],
                    if id == "88-C177-S040" {
                        "where:nan_implicit_cast"
                    } else {
                        "where"
                    },
                    "{identity}: stable, independently specified residual location"
                );
            } else {
                assert!(
                    actual_exactness["residual_conditions"]
                        .as_array()
                        .is_none_or(|items| items.is_empty()),
                    "{identity}: an exact/conditional predicate cannot hide a residual"
                );
            }
            // Check the actual assumption *names* and source identities for
            // the two remaining conditional original SQLs. A status of
            // "conditional" alone does not identify what must be attested.
            if matches!(id, "88-C177-S037" | "88-C177-S039") {
                let expected = if id == "88-C177-S037" {
                    vec!["session_time_zone"]
                } else {
                    vec!["binary_collation", "no_char_padding"]
                };
                let requirements = actual_exactness["comparison_assumptions"]
                    .as_array()
                    .expect("explicit conditional source laws");
                let mut actual = requirements
                    .iter()
                    .map(|requirement| {
                        assert_eq!(
                            requirement["clause"], "where",
                            "{identity}: source law clause"
                        );
                        assert_eq!(
                            requirement["declared"], false,
                            "{identity}: parser cannot attest session settings"
                        );
                        assert!(
                            requirement["identity"]
                                .as_str()
                                .is_some_and(|name| name.starts_with("column_domain:")),
                            "{identity}: assumption needs an exact source-column identity"
                        );
                        requirement["name"].as_str().expect("typed assumption name")
                    })
                    .collect::<Vec<_>>();
                actual.sort_unstable();
                assert_eq!(
                    actual, expected,
                    "{identity}: independently required source laws"
                );
            }
            assert_eq!(
                policy["expected_output_domain_kind"], cell["expected_projected_domain_kind"],
                "{identity}: per-case output policy"
            );
        }

        for input in wire["inputs"]
            .as_array_mut()
            .expect("public protocol input list")
        {
            input["dialect"] = Value::Null;
        }
        if let Some(expected) = normalized_documents.get(id) {
            assert_eq!(
                &wire, expected,
                "{identity}: entire public protocol document changed across portable parsers"
            );
        } else {
            normalized_documents.insert(id, wire);
        }
        if cell["repaired_historical_drift"] == true {
            assert_eq!(identity, "88-C177-S179/duckdb");
            assert_eq!(cell["historical_status"], "D");
            repaired += 1;
        }
        accepted += 1;
    }
    assert_eq!(seen.len(), 559);
    assert_eq!(normalized_documents.len(), 43);
    assert_eq!((accepted, rejected, repaired), (557, 2, 1));
}
