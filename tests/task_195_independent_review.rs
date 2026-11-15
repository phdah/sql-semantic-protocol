//! TASK-195 independent bounded semantic review. The oracle is computed
//! from SQL algebra on the original physical INSERT population, not from the
//! protocol analyzer or its serialized output. D/R cells remain named
//! unapproved *portability* boundaries, despite each having a reviewed result.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, SetMode, ValueDomain,
};

const REVIEW: &str = include_str!("../docs/task-195-independent-semantic-review.json");
const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn fixture(raw: &str) -> Value {
    serde_json::from_str(raw).expect("versioned independent review and frozen original SQL")
}

fn tuple_sort(rows: &mut [Vec<Value>]) {
    rows.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
}

fn assert_numeric_bag(mut actual: Vec<Vec<Value>>, expected: &Value, label: &str) {
    let mut golden = expected
        .as_array()
        .expect("independent exact literal terminal bag")
        .iter()
        .map(|row| row.as_array().expect("one literal tuple").clone())
        .collect::<Vec<_>>();
    tuple_sort(&mut actual);
    tuple_sort(&mut golden);
    assert_eq!(actual.len(), golden.len(), "{label}: complete row multiplicity");
    for (number, (actual, expected)) in actual.iter().zip(golden.iter()).enumerate() {
        assert_eq!(actual.len(), expected.len(), "{label}: arity at {number}");
        for (column, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            match (actual.as_f64(), expected.as_f64()) {
                (Some(a), Some(b)) => assert!(
                    (a - b).abs() < 1e-10,
                    "{label}: value difference at row {number}, column {column}: {a} vs {b}"
                ),
                (None, None) if actual.is_null() && expected.is_null() => {}
                _ => panic!("{label}: typed NULL/numeric discrepancy {actual} vs {expected}"),
            }
        }
    }
}

fn literal_bag(db: &Connection, sql: &str, arity: usize, floating: bool) -> Vec<Vec<Value>> {
    let mut statement = db.prepare(sql).expect("original DuckDB SQL, not a rewrite");
    statement
        .query_map([], |row| {
            let mut tuple = Vec::with_capacity(arity);
            for index in 0..arity {
                if floating {
                    let value = row.get::<_, Option<f64>>(index)?;
                    tuple.push(value.map_or(Value::Null, |v| serde_json::json!(v)));
                } else {
                    let value = row.get::<_, Option<i64>>(index)?;
                    tuple.push(value.map_or(Value::Null, |v| serde_json::json!(v)));
                }
            }
            Ok(tuple)
        })
        .expect("native terminal")
        .collect::<Result<Vec<_>, _>>()
        .expect("full native output occurrences")
}

fn contains(domain: &ValueDomain, v: Option<f64>) -> bool {
    match domain {
        ValueDomain::Unbounded | ValueDomain::Unknown(_) => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(set) => {
            let exists = set.values().iter().any(|lit| match (v, lit.value()) {
                (None, LiteralValue::Null) => true,
                (Some(value), LiteralValue::Number(raw)) => raw
                    .parse::<f64>()
                    .ok()
                    .is_some_and(|number| (value - number).abs() < 1e-10),
                _ => false,
            });
            match set.mode() {
                SetMode::Include => exists,
                SetMode::Exclude => !exists,
            }
        }
        ValueDomain::Ranges(ranges) => {
            let Some(observed) = v else {
                return false;
            };
            ranges.ranges().iter().any(|range| {
                let lower = range.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(number) = raw.parse::<f64>() else {
                        return false;
                    };
                    observed > number || (observed == number && bound.inclusive())
                });
                let upper = range.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(number) = raw.parse::<f64>() else {
                        return false;
                    };
                    observed < number || (observed == number && bound.inclusive())
                });
                lower && upper
            })
        }
        _ => false,
    }
}

fn check_law(domain: &ValueDomain, law: &str, cell: &str) {
    let must_admit = |value| assert!(
        contains(domain, value),
        "{cell}: {law} must admit {value:?}, got {domain:?}"
    );
    let must_reject = |value| assert!(
        !contains(domain, value),
        "{cell}: {law} must reject {value:?}, got {domain:?}"
    );
    match law {
        "grouping_single_bit_0_or_1" => {
            must_admit(Some(0.0));
            must_admit(Some(1.0));
            must_reject(None);
            must_reject(Some(-1.0));
            must_reject(Some(2.0));
        }
        "count_nonnegative" => {
            must_admit(Some(0.0));
            must_admit(Some(1.0));
            must_reject(Some(-1.0));
            must_reject(None);
        }
        "analytic_positive_integer" | "count_current_row_positive" => {
            must_admit(Some(1.0));
            must_admit(Some(4.0));
            must_reject(Some(0.0));
            must_reject(None);
        }
        "ntile_1_through_3" => {
            must_admit(Some(1.0));
            must_admit(Some(3.0));
            must_reject(Some(0.0));
            must_reject(Some(4.0));
            must_reject(None);
        }
        "distribution_0_through_1_inclusive" => {
            must_admit(Some(0.0));
            must_admit(Some(1.0));
            must_reject(Some(-0.1));
            must_reject(Some(1.1));
            must_reject(None);
        }
        "distribution_gt_0_lte_1" => {
            must_admit(Some(0.1));
            must_admit(Some(1.0));
            must_reject(Some(0.0));
            must_reject(Some(1.1));
            must_reject(None);
        }
        "qualify_exact_row_number_1" => {
            must_admit(Some(1.0));
            must_reject(Some(0.0));
            must_reject(Some(2.0));
            must_reject(None);
        }
        "qualify_count_gt_1" | "qualify_row_number_gt_1" => {
            must_admit(Some(2.0));
            must_admit(Some(4.0));
            must_reject(Some(1.0));
            must_reject(None);
        }
        "arithmetic_window_unknown_without_source_bounds" => {
            assert!(
                matches!(domain, ValueDomain::Unknown(_) | ValueDomain::Unbounded),
                "{cell}: an unattested source SUM/AVG cannot have finite numeric limits"
            );
        }
        "nullable_source_id_unbounded" => {
            must_admit(None);
            must_admit(Some(-1_000_000.0));
            must_admit(Some(1_000_000.0));
        }
        other => panic!("{cell}: review references an unimplemented domain law {other}"),
    }
}

#[test]
fn independent_reconstruction_verifies_every_original_native_terminal_bag() {
    let review = fixture(REVIEW);
    let original = fixture(FREEZE);
    assert_eq!(review["schema_version"], 1);
    assert_eq!(review["task_id"], "TASK-195");
    assert_eq!(review["reviewed_original_cases"], 36);
    assert_eq!(review["independently_computed_original_DuckDB_complete_bags"], 28);
    assert_eq!(review["conditionally_unprovable_original_native_bags"], 8);
    assert_eq!(review["source_setup"], original["source_setup"]);

    let db = Connection::open_in_memory().expect("DuckDB native original oracle");
    db.execute_batch(review["source_setup"].as_str().expect("reviewed original source setup"))
        .expect("all physical NULL and duplicate source tuples");
    let ordered = db
        .prepare("SELECT id, amount FROM orders ORDER BY rowid")
        .expect("physical source row identity")
        .query_map([], |row| Ok((
            row.get::<_, Option<i64>>(0)?,
            row.get::<_, Option<i64>>(1)?,
        )))
        .expect("complete physical rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("all six physical rows");
    let reviewed = review["source_rows"]
        .as_array()
        .expect("independently enumerated physical rows")
        .iter()
        .map(|row| (row["id"].as_i64(), row["amount"].as_i64()))
        .collect::<Vec<_>>();
    assert_eq!(ordered, reviewed, "original physical source identities are not deduplicated");

    let mut cases = BTreeSet::new();
    let mut complete = 0;
    let mut native_rows = 0;
    for case in review["cases"].as_array().expect("36 reviewed cases") {
        let id = case["case_id"].as_str().expect("source identity");
        assert!(cases.insert(id), "{id}: duplicate independent review");
        let baseline = original["cases"]
            .as_array()
            .expect("original freeze")
            .iter()
            .find(|item| item["id"] == id)
            .expect("no original SQL dropped");
        assert_eq!(case["original_sql"], baseline["sql"], "{id}: immutable SQL");
        assert_eq!(case["feature_id"], baseline["feature_id"], "{id}: ownership");
        let expected = &case["expected_native_terminal_bag"];
        if expected.is_null() {
            assert_eq!(case["expected_native_terminal_count"], Value::Null);
            assert!(baseline["native_oracle"]["expected_terminal_bag"].is_null());
            assert!(case["independent_sql_algebra_law"].is_string());
            continue;
        }
        complete += 1;
        let width = case["expected_output_columns"].as_u64().expect("typed original width") as usize;
        let floating = matches!(id, "88-C177-S114" | "88-C177-S115" | "88-C177-S119");
        let native = literal_bag(
            &db,
            case["original_sql"].as_str().expect("original SQL"),
            width,
            floating,
        );
        native_rows += native.len();
        assert_numeric_bag(native.clone(), expected, id);
        assert_numeric_bag(native, &baseline["native_oracle"]["expected_terminal_bag"], id);
        assert_eq!(case["expected_native_terminal_count"], serde_json::json!(
            expected.as_array().expect("full reviewed bag").len()
        ));
        if let Some(rejected) = case["expected_native_rejected_candidate_count"].as_u64() {
            let population = case["physical_source_candidates"].as_u64().expect("source bag") as usize;
            assert_eq!(
                rejected as usize + expected.as_array().expect("selected rows").len(),
                population,
                "{id}: selected plus NOT TRUE/out-of-page candidate counts"
            );
            assert_eq!(
                rejected as usize,
                case["expected_native_rejected_candidate_bag"]
                    .as_array().expect("independently reconstructed rejection bag").len(),
                "{id}: rejected candidates must be explicitly enumerated"
            );
        }
    }
    assert_eq!(cases.len(), 36);
    assert_eq!(complete, 28);
    assert!(native_rows >= 150, "all complete native multiplicities checked");
}

#[test]
fn all_468_reviewed_parser_cells_preserve_exact_bounded_typed_laws() {
    let review = fixture(REVIEW);
    let freeze = fixture(FREEZE);
    assert_eq!(review["reviewed_parser_cells"], 468);
    assert_eq!(review["reviewed_portable_equal_cells"], 446);
    assert_eq!(review["reviewed_canonical_drift_boundaries_unapproved"], 21);
    assert_eq!(review["reviewed_parser_rejection_unapproved"], 1);
    assert_eq!(review["external_vendor_runtime_attested"], false);
    assert_eq!(review["parent_TASK_88_approved"], false);
    assert_eq!(review["release_TASK_91_approved"], false);

    let mut seen = BTreeSet::new();
    let mut histogram = [0; 3];
    let mut typed_claims = 0;
    for cell in review["cells"].as_array().expect("468 individually reviewed dialect cells") {
        let id = cell["cell_id"].as_str().expect("stable original/dialect");
        assert!(seen.insert(id), "{id}: duplicated reviewer result");
        assert_eq!(cell["reviewed"], true, "{id}: unreviewed");
        assert_eq!(cell["vendor_runtime_approved"], false, "{id}: vendor approval invented");

        let case = review["cases"]
            .as_array()
            .expect("36 independent SQL laws")
            .iter()
            .find(|case| case["case_id"] == cell["case_id"])
            .expect("reviewed original SQL reference");
        let prior = freeze["cases"]
            .as_array()
            .expect("original cases")
            .iter()
            .find(|entry| entry["id"] == case["case_id"])
            .expect("original SQL exists");
        let dialect_name = cell["dialect"].as_str().expect("public dialect");
        let historical = prior["cells"]
            .as_array()
            .expect("complete original dialects")
            .iter()
            .find(|entry| entry["dialect"] == dialect_name)
            .expect("historical parser cell");
        let status = historical["original_observed_status"].as_str().expect("historical status");
        assert_eq!(cell["original_parser_canonical_status"], status, "{id}");
        let dialect = dialect_from_name(dialect_name).expect("registered parser");
        let protocol = analyze_sql(
            case["original_sql"].as_str().expect("immutable SQL"),
            dialect_name,
            dialect.as_ref(),
        );
        match status {
            "R" => {
                assert!(protocol.is_err(), "{id}: parser rejection was silently accepted");
                assert_eq!(cell["independent_review_disposition"],
                    "reviewed_parser_rejection_handoff_TASK_191");
                histogram[2] += 1;
            }
            "C" | "D" => {
                if status == "C" {
                    histogram[0] += 1;
                    let disposition = cell["independent_review_disposition"]
                        .as_str().expect("bounded certification");
                    assert!(
                        disposition == "reviewed_portable_sql_outcome_and_DuckDB_native_bag"
                            || disposition == "reviewed_conditional_or_engine_rejected_not_exact",
                        "{id}: approved parser equivalence cannot invent native vendor execution"
                    );
                } else {
                    histogram[1] += 1;
                    assert_eq!(cell["independent_review_disposition"],
                        "reviewed_canonical_drift_not_claimed_equivalent_handoff_TASK_191");
                }
                let protocol = protocol.unwrap_or_else(|error| panic!("{id}: {error}"));
                let [ProtocolStatement::Query(query)] = protocol.statements() else {
                    panic!("{id}: parser-accepted variant omitted its output");
                };
                let actual = query.output().columns();
                assert_eq!(
                    actual.len(),
                    case["expected_output_columns"].as_u64().expect("source arity") as usize,
                    "{id}: all native output columns must exist in canonical semantics"
                );
                let laws = cell["public_output_laws"].as_array().expect("independently reviewed laws");
                assert_eq!(laws, &case["public_output_laws"], "{id}: original SQL law drift");
                assert_eq!(laws.len(), actual.len(), "{id}: law arity");
                for (column, law) in actual.iter().zip(laws) {
                    check_law(column.domain(), law.as_str().expect("independent typed law"), id);
                    typed_claims += 1;
                }
            }
            other => panic!("{id}: unmanaged original status {other}"),
        }
    }
    assert_eq!(seen.len(), 468);
    assert_eq!(histogram, [446, 21, 1]);
    assert!(typed_claims >= 490, "two-column grouping and joined QUALIFY tested");
}
