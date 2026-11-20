//! Independent pre-signoff review of the 138 original SQL cases not covered by
//! the earlier TASK-180 predicate gate. These are source-bounded reviews, not
//! fabricated native-vendor execution or a release authorization.

use std::collections::BTreeSet;

use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-180-acceptance-freeze.json");
const RECONCILIATION: &str = include_str!("../docs/task-178-canonical-reconciliation.json");
const SIGNOFF: &str = include_str!("../docs/task-180-final-signoff-evidence.json");
const REVIEW_192: &str = include_str!("../docs/task-180-pending-review-task-192.json");
const REVIEW_193: &str = include_str!("../docs/task-180-pending-review-task-193.json");
const REVIEW_194: &str = include_str!("../docs/task-180-pending-review-task-194.json");
const REVIEW_195: &str = include_str!("../docs/task-180-pending-review-task-195.json");
const REVIEW_196: &str = include_str!("../docs/task-180-pending-review-task-196.json");
const ORACLE_192: &str = include_str!("../docs/task-192-independent-semantic-review.json");
const ORACLE_193: &str = include_str!("../docs/task-193-independent-semantic-review.json");
const NATIVE_193: &str = include_str!("../docs/task-193-acceptance-freeze.json");
const ORACLE_194: &str = include_str!("../docs/task-194-independent-semantic-review.json");
const NATIVE_194: &str = include_str!("../docs/task-194-independent-bag-review.json");
const ORACLE_195: &str = include_str!("../docs/task-195-independent-semantic-review.json");
const NATIVE_195: &str = include_str!("../docs/task-195-acceptance-freeze.json");
const ORACLE_196: &str = include_str!("../docs/task-196-independent-semantic-review.json");
const NATIVE_196: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned original SQL review evidence")
}

fn original<'a>(source: &'a Value, id: &str) -> &'a Value {
    source["cases"]
        .as_array()
        .expect("independent case inventory")
        .iter()
        .find(|case| case["id"] == id || case["case_id"] == id)
        .unwrap_or_else(|| panic!("{id}: missing independently frozen case"))
}

fn approved_review<'a>(source: &'a Value, id: &str) -> &'a Value {
    source["original_SQL_cases"]
        .as_array()
        .expect("independent original DML/DDL reviews")
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("{id}: missing independently reviewed write"))
}

#[test]
fn remaining_138_original_case_oracles_and_1794_dialect_dispositions_are_reconciled() {
    let freeze = read(FREEZE);
    let signoff = read(SIGNOFF);
    let reconciliation = read(RECONCILIATION);
    let normalized: BTreeSet<_> = reconciliation["resolved_grouping_cells"]
        .as_array()
        .expect("independent canonical AST drift reconciliations")
        .iter()
        .map(|cell| {
            format!(
                "{}/{}",
                cell["case_id"].as_str().expect("original case"),
                cell["dialect"].as_str().expect("dialect")
            )
        })
        .collect();
    assert_eq!(normalized.len(), 22);
    let owners = [
        ("TASK-192", REVIEW_192, 18_usize, 234_usize),
        ("TASK-193", REVIEW_193, 24, 312),
        ("TASK-194", REVIEW_194, 22, 286),
        ("TASK-195", REVIEW_195, 36, 468),
        ("TASK-196", REVIEW_196, 38, 494),
    ];
    let references = [
        read(ORACLE_192),
        read(ORACLE_193),
        read(ORACLE_194),
        read(ORACLE_195),
        read(ORACLE_196),
    ];
    let native = [
        Value::Null,
        read(NATIVE_193),
        read(NATIVE_194),
        read(NATIVE_195),
        read(NATIVE_196),
    ];
    let mut case_ids = BTreeSet::new();
    let mut cell_ids = BTreeSet::new();
    for (i, (owner, review_source, case_count, cell_count)) in owners.iter().enumerate() {
        let review = read(review_source);
        assert_eq!(review["task_id"], "TASK-180");
        assert_eq!(review["semantic_owner"], *owner);
        assert_eq!(review["case_count"], *case_count);
        assert_eq!(review["cell_count"], *cell_count);
        assert_eq!(review["release_ready"], false);
        assert_eq!(review["external_vendor_native_execution_attested"], false);
        let cases = review["cases"].as_array().expect("original cases");
        assert_eq!(cases.len(), *case_count);
        for candidate in cases {
            let id = candidate["id"]
                .as_str()
                .expect("frozen original case identity");
            assert!(
                case_ids.insert(id.to_owned()),
                "{id}: case reviewed more than once"
            );
            let frozen = original(&freeze, id);
            assert_eq!(frozen["owner"], *owner);
            for field in [
                ("original_sql", "sql"),
                ("feature_id", "feature_id"),
                ("semantic_owner", "owner"),
                ("owner_review_ref", "owner_review_ref"),
                ("adapter_review_ref", "adapter_review_ref"),
            ] {
                assert_eq!(
                    candidate[field.0], frozen[field.1],
                    "{id}: changed original source or owner"
                );
            }
            assert_eq!(candidate["native_vendor_execution_attested"], false);
            let independent = if *owner == "TASK-196" {
                approved_review(&references[i], id)
            } else {
                original(&references[i], id)
            };
            let source_oracle = if *owner == "TASK-192" {
                Value::Null
            } else {
                original(&native[i], id).clone()
            };
            match *owner {
                "TASK-192" => {
                    assert_eq!(
                        candidate["typed_portable_law"],
                        independent["independent_expectation"]
                    );
                    assert_eq!(candidate["original_sql"], independent["original_sql"]);
                    assert_eq!(
                        candidate["original_source_terminal_oracle"],
                        independent["native_oracle"]
                    );
                    assert_eq!(candidate["original_oracle_status"], independent["decision"]);
                }
                "TASK-193" => {
                    assert_eq!(
                        candidate["independent_sql_law"]["join_kinds"],
                        independent["independent_expected_join_kinds"]
                    );
                    assert_eq!(
                        candidate["independent_sql_law"]["dependencies"],
                        independent["independent_expected_physical_dependencies"]
                    );
                    assert_eq!(
                        candidate["independent_sql_law"]["projection_lineage"],
                        independent["independent_expected_projection_lineage"]
                    );
                    assert_eq!(
                        candidate["complete_native_terminal_bag"],
                        source_oracle["expected_terminal_bag"]
                    );
                    assert_eq!(
                        candidate["source_bounded_typed_column_domains"],
                        source_oracle["fixture_observed_column_domains"]
                    );
                    assert_eq!(
                        candidate["full_source_and_terminal_occurrences"]["expected_terminal"],
                        independent["independent_expected_terminal_occurrences"]
                    );
                    assert_eq!(
                        candidate["physical_original_syntax_boundary"],
                        source_oracle["original_duckdb_syntax_boundary"]
                    );
                    assert_eq!(
                        candidate["fixture_domains_are_not_portable_sql_bounds"],
                        true
                    );
                }
                "TASK-194" => {
                    assert_eq!(candidate["sql_multiset_law"], independent["semantic_law"]);
                    assert_eq!(
                        candidate["independent_terminal_bag"],
                        source_oracle["independent_terminal_bag"]
                    );
                    assert_eq!(
                        candidate["independent_non_emitted_candidate_bag"],
                        source_oracle["independent_non_emitted_candidate_bag"]
                    );
                    assert_eq!(
                        candidate["source_arm_row_occurrences"],
                        source_oracle["source_arms"]
                    );
                    assert_eq!(
                        candidate["expected_output_names"],
                        independent["expected_output_column_names"]
                    );
                    assert_eq!(
                        candidate["expected_output_type"],
                        independent["expected_output_type"]
                    );
                    assert_eq!(
                        candidate["independent_semantic_review_status"],
                        independent["review_disposition"]
                    );
                }
                "TASK-195" => {
                    assert_eq!(
                        candidate["independent_sql_algebra_law"],
                        independent["independent_sql_algebra_law"]
                    );
                    assert_eq!(
                        candidate["portable_typed_output_laws"],
                        independent["public_output_laws"]
                    );
                    assert_eq!(
                        candidate["positive_original_terminal_bag"],
                        independent["expected_native_terminal_bag"]
                    );
                    assert_eq!(
                        candidate["rejected_candidate_bag"],
                        independent["expected_native_rejected_candidate_bag"]
                    );
                    assert_eq!(
                        candidate["original_native_execution_disposition"],
                        independent["original_native_execution_disposition"]
                    );
                    assert_eq!(
                        candidate["original_native_fixture"],
                        source_oracle["native_oracle"]
                    );
                    assert_eq!(
                        candidate["source_instance_ids"],
                        independent["physical_source_instance_ids"]
                    );
                }
                "TASK-196" => {
                    assert_eq!(candidate["original_sql"], independent["original_sql"]);
                    assert_eq!(
                        candidate["independent_sql_law"],
                        independent["independent_sql_law"]
                    );
                    assert_eq!(candidate["review_source"], independent["evidence_owner"]);
                    assert_eq!(
                        candidate["physical_native_class"],
                        independent["physical_native_class"]
                    );
                    assert_eq!(candidate["native_oracle"], source_oracle["native_oracle"]);
                }
                _ => unreachable!("five frozen owner families"),
            }
            let candidate_cells = candidate["cells"]
                .as_array()
                .expect("13 review dispositions");
            let frozen_cells = frozen["cells"].as_array().expect("13 dialect originals");
            assert_eq!(candidate_cells.len(), 13);
            for (candidate_cell, frozen_cell) in candidate_cells.iter().zip(frozen_cells) {
                let key = candidate_cell["id"]
                    .as_str()
                    .expect("source/dialect review identity");
                assert!(cell_ids.insert(key.to_owned()), "{key}: duplicate review");
                for field in [
                    "id",
                    "dialect",
                    "original_canonical_class",
                    "independent_bounded_review",
                ] {
                    assert_eq!(
                        candidate_cell[field], frozen_cell[field],
                        "{key}: source status was altered"
                    );
                }
                assert_eq!(candidate_cell["native_vendor_execution_attested"], false);
                let verdict = candidate_cell["review_status"]
                    .as_str()
                    .expect("bounded verdict");
                assert!(
                    matches!(
                        verdict,
                        "Exact" | "Impossible" | "reviewed_conditional_residual"
                    ),
                    "{key}: unsupported review verdict"
                );
                let historical_drift_normalized = normalized.contains(key);
                if historical_drift_normalized {
                    assert_eq!(frozen_cell["original_canonical_class"], "D");
                    if candidate_cell["review_status"] == "Exact" {
                        assert_eq!(
                            candidate_cell["exception_resolution_ref"],
                            "docs/task-178-canonical-reconciliation.json",
                            "{key}: Exact normalized drift requires the exact reviewed source"
                        );
                    }
                }
                let normal = frozen_cell["original_canonical_class"] == "C";
                let eligible = (normal || historical_drift_normalized)
                    && match *owner {
                        "TASK-192" => false,
                        "TASK-193" => {
                            independent["native_original_syntax_boundary"].is_null()
                                && !id.ends_with("S152")
                        }
                        "TASK-194" => {
                            independent["review_disposition"]
                                == "reviewed_exact_original_SQL_typed_portable_semantics"
                                && !source_oracle["independent_terminal_bag"].is_null()
                        }
                        "TASK-195" => {
                            independent["original_native_execution_disposition"]
                                == "complete_original_DuckDB_bag_verified"
                                && !independent["expected_native_terminal_bag"].is_null()
                        }
                        "TASK-196" => {
                            let source_cell = independent["cells"]
                                .as_array()
                                .expect("independent DML/DDL parser reviews")
                                .iter()
                                .find(|cell| cell["id"] == key)
                                .expect("original DML/DDL cell review");
                            assert_eq!(
                                candidate_cell["owner_cell_decision"],
                                source_cell["reviewer_classification"],
                                "{key}: cannot borrow another semantic reviewer decision"
                            );
                            independent["physical_native_class"]
                                != "metadata_adapter_only_requires_authoritative_fixture"
                                && source_cell["final_acceptance"]
                                    == "bounded_portable_under_attested_source_bag_and_oracle"
                        }
                        _ => unreachable!("five independently reviewed semantic owners"),
                    };
                let impossible = *owner == "TASK-194"
                    && independent["semantic_law"] == "semantic_impossible_incompatible_arity"
                    && independent["review_disposition"]
                        == "approved_fail_closed_arity_without_fake_terminal"
                    && candidate["expected_output_names"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
                    && source_oracle["independent_terminal_bag"].is_null()
                    && source_oracle["source_arms"].as_array().is_some_and(|arms| {
                        arms.len() == 2
                            && arms[0]["column_order"]
                                .as_array()
                                .zip(arms[1]["column_order"].as_array())
                                .is_some_and(|(left, right)| left.len() != right.len())
                    });
                assert_eq!(
                    verdict,
                    if eligible {
                        "Exact"
                    } else if impossible {
                        "Impossible"
                    } else {
                        "reviewed_conditional_residual"
                    },
                    "{key}: an owner-unapproved semantic or unexecuted native proof cannot be promoted"
                );
                assert!(candidate_cell["review_reason"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()));
            }
        }
    }
    assert_eq!(case_ids.len(), 138);
    assert_eq!(cell_ids.len(), 1794);
    let signed = signoff["approved_cells"]
        .as_array()
        .expect("separately approved predicate cells");
    assert_eq!(signed.len(), 325);
    let mut covered = cell_ids.clone();
    for cell in signed {
        assert!(covered.insert(
            cell["id"]
                .as_str()
                .expect("approved predicate cell")
                .to_owned()
        ));
    }
    let required: BTreeSet<_> = freeze["cases"]
        .as_array()
        .expect("163 original cases")
        .iter()
        .flat_map(|case| case["cells"].as_array().expect("13 parser dialects"))
        .map(|cell| cell["id"].as_str().expect("frozen ID").to_owned())
        .collect();
    assert_eq!(
        covered, required,
        "every 2,119-cell source review must be accounted for"
    );
    assert_eq!(
        signoff["task180_signed_off"], false,
        "the next agent owns final signoff"
    );
}

fn serialized_bag(source: &Value) -> Option<Vec<String>> {
    let rows = source.as_array()?;
    let mut values = rows
        .iter()
        .map(|row| serde_json::to_string(row).expect("original row tuple"))
        .collect::<Vec<_>>();
    values.sort();
    Some(values)
}

#[test]
fn independently_reviewed_set_cases_conserve_complete_candidate_tuple_multisets() {
    let original_bags = read(NATIVE_194);
    let reviewers = read(REVIEW_194);
    let mut checked = 0_usize;
    for case in reviewers["cases"].as_array().expect("22 original set laws") {
        let id = case["id"].as_str().expect("original SQL");
        let bag = original(&original_bags, id);
        let Some(mut emitted) = serialized_bag(&case["independent_terminal_bag"]) else {
            assert!(bag["independent_terminal_bag"].is_null());
            continue;
        };
        let rejected = serialized_bag(&case["independent_non_emitted_candidate_bag"])
            .expect("original non-emitted tuple occurrences");
        emitted.extend(rejected);
        emitted.sort();
        assert_eq!(
            emitted,
            serialized_bag(&bag["independent_source_candidate_bag"])
                .expect("complete independently counted candidate source bag"),
            "{id}: tuple candidates cannot be independently invented or discarded"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 20,
        "only the two explicit non-executable set originals are excluded"
    );
}

#[test]
fn original_mismatched_set_arity_is_impossible_for_all_dialects() {
    use sql_semantic_protocol::{
        analyze_sql, dialect_from_name, DiagnosticArea, ProtocolStatement,
    };
    let review = read(REVIEW_194);
    let case = original(&review, "88-C177-S086");
    let sql = case["original_sql"]
        .as_str()
        .expect("frozen incompatible set SQL");
    let cells = case["cells"].as_array().expect("13 original dialects");
    assert_eq!(cells.len(), 13);
    for cell in cells {
        let dialect = cell["dialect"].as_str().expect("supported dialect");
        let parser = dialect_from_name(dialect).expect("public parser");
        let protocol = analyze_sql(sql, dialect, parser.as_ref())
            .unwrap_or_else(|error| panic!("{dialect}: original SQL: {error}"));
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{dialect}: invalid set cannot be represented by another statement");
        };
        assert!(
            query.output().columns().is_empty(),
            "{dialect}: mismatched 1-vs-2 SELECT arms must have no output"
        );
        assert!(
            query.diagnostics().iter().any(|diagnostic| {
                diagnostic.area() == DiagnosticArea::Output
                    && diagnostic.code() == "set_operation_arity_mismatch"
            }),
            "{dialect}: no fabricated row or typed output from incompatible set branches"
        );
        assert_eq!(cell["review_status"], "Impossible");
        assert_eq!(cell["native_vendor_execution_attested"], false);
    }
}


#[test]
fn original_unattested_write_actions_cannot_disappear_from_an_ordered_program() {
    use sql_semantic_protocol::{
        analyze_inputs, dialect_from_name, physical_ordered_write_state_plan, PhysicalWriteStatePlan,
        ProtocolStatement, SqlInput,
    };

    let freeze = read(FREEZE);
    let originals = ["S199", "S200", "S203", "S206", "S220"];
    let mut exercised = 0;
    for suffix in originals {
        let id = format!("88-C177-{suffix}");
        let source = original(&freeze, &id);
        let sql = source["sql"].as_str().expect("unchanged mandatory write");
        for cell in source["cells"].as_array().expect("all 13 parser dialects") {
            if cell["original_canonical_class"] != "U" {
                assert_eq!(
                    cell["original_canonical_class"], "R",
                    "{id}: only parser rejection can precede unsupported analysis"
                );
                continue;
            }
            let dialect = cell["dialect"].as_str().expect("supported parser");
            let parser = dialect_from_name(dialect).expect("public parser");
            // A preceding table creation and a following read must not make
            // an unknown intermediate mutation silently harmless.
            let program = format!(
                "CREATE TABLE orders (id INT, amount INT); {sql}; SELECT id FROM orders"
            );
            let analyzed = analyze_inputs(&[SqlInput::inline(&program)], dialect, parser.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{dialect}: program parse: {error}"));
            assert!(
                analyzed.inputs()[0].statements().iter().any(|statement| {
                    matches!(statement, ProtocolStatement::Unsupported(_))
                }),
                "{id}/{dialect}: the unproved write must remain explicit"
            );
            assert!(
                matches!(
                    physical_ordered_write_state_plan(&analyzed, &[]),
                    PhysicalWriteStatePlan::Residual { .. }
                ),
                "{id}/{dialect}: subsequent reads cannot erase unknown mutation effects"
            );
            exercised += 1;
        }
    }
    assert_eq!(exercised, 55, "all historical untyped write U cells");
}
