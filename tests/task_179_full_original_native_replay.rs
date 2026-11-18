//! TASK-179 independently pinned original source population and full native
//! terminal/error outcome. The v4 amendment owns every original source and
//! shows negative truth partitions; this is not native multi-vendor evidence.

use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement, ValueDomain};

const FROZEN: &str = include_str!("../docs/task-179-source-and-outcome-evidence-v4.json");
const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const PREDICATES: &str = include_str!("../docs/task-177-predicate-oracles.json");
const ENVIRONMENT: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const WRITES: &str = include_str!("../docs/task-177-write-oracles.json");
const LIFECYCLE: &str = include_str!("../docs/task-177-schema-lifecycle-oracles.json");

fn load(text: &str) -> Value {
    serde_json::from_str(text).expect("versioned source oracle")
}

fn ordered_bag(mut rows: Vec<Value>) -> Vec<Value> {
    rows.sort_by_key(Value::to_string);
    rows
}

fn ints(db: &Connection, sql: &str, width: usize) -> duckdb::Result<Vec<Value>> {
    let mut stmt = db.prepare(sql)?;
    stmt.query_map([], |row| {
        (0..width)
            .map(|column| row.get::<usize, Option<i64>>(column))
            .collect::<duckdb::Result<Vec<_>>>()
    })?
    .map(|row| row.map(|columns| json!(columns)))
    .collect()
}

fn source_value(value: &Value, is_double: bool) -> Option<String> {
    if value.is_null() {
        return None;
    }
    if let Some(raw) = value.as_str() {
        return Some(raw.to_string());
    }
    if is_double {
        return Some(format!(
            "{:.1}",
            value.as_f64().expect("double source scalar")
        ));
    }
    Some(value.to_string())
}

fn normalize_double(text: String) -> String {
    if text.eq_ignore_ascii_case("nan") {
        "NaN".to_string()
    } else if text == "-0.0" {
        "0.0".to_string()
    } else {
        text
    }
}

fn typed_sources(db: &Connection, case: &Value) -> Vec<Value> {
    let relation = case["source_relation"].as_str().expect("physical relation");
    let columns = case["source_columns"]
        .as_array()
        .expect("source type authority");
    let source_sql = if let Some(sql) = case["source_snapshot_sql"].as_str() {
        sql.to_string()
    } else if columns.len() == 1 {
        format!("SELECT id FROM {relation} ORDER BY rowid")
    } else {
        let second = columns[1].as_str().expect("typed second source column");
        let name = if second.starts_with("name ") {
            "name"
        } else if second.starts_with("created_at ") {
            "created_at"
        } else {
            "amount"
        };
        format!("SELECT id, CAST({name} AS VARCHAR) FROM {relation} ORDER BY rowid")
    };
    let double = columns
        .iter()
        .any(|column| column.as_str().is_some_and(|text| text.contains("DOUBLE")));
    let expected = case["source_bag"].as_array().expect("complete source bags");
    // A single declared source column can still have a two-column snapshot
    // (the identifier-case probes include a text rendering of that column).
    // Check the complete frozen row shape instead of silently discarding it.
    if expected
        .iter()
        .all(|row| row.as_array().is_some_and(|values| values.len() == 1))
    {
        let observed = ints(db, &source_sql, 1).expect("native one-column source");
        assert_eq!(
            ordered_bag(observed.clone()),
            ordered_bag(expected.clone()),
            "{}: complete native one-column source bag",
            case["id"]
        );
        return observed;
    }
    let mut stmt = db.prepare(&source_sql).expect("native full typed input");
    let observed = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .expect("native source rows")
        .map(|row| {
            let (id, scalar) = row.expect("physical input row");
            json!([
                id,
                scalar.map(|value| if double {
                    normalize_double(value)
                } else {
                    value
                })
            ])
        })
        .collect::<Vec<_>>();
    let frozen = expected
        .iter()
        .map(|row| {
            let values = row.as_array().expect("two typed source columns");
            json!([values[0], source_value(&values[1], double)])
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_bag(observed.clone()),
        ordered_bag(frozen),
        "{}: full native source NULL, value and duplicate check",
        case["id"]
    );
    observed
}

#[test]
fn complete_review_matrix_preserves_original_cases_typed_bounds_and_negative_oracles() {
    let frozen = load(FROZEN);
    let matrix = load(MATRIX);
    let original = frozen["cases"]
        .as_array()
        .expect("original physical source cases");
    assert_eq!(frozen["schema_version"], 4);
    assert_eq!(
        frozen["historical_preimplementation_freeze"],
        "docs/task-179-acceptance-freeze.json"
    );
    assert_eq!(frozen["case_count"], 37);
    assert_eq!(frozen["parser_cell_count"], 481);
    assert_eq!(original.len(), 37);
    assert_eq!(frozen["external_native_execution"], false);
    for (case, variant) in original
        .iter()
        .zip(matrix["cases"].as_array().expect("all source SQL cases"))
    {
        let id = case["id"].as_str().expect("law identity");
        assert_eq!(case["id"], variant["id"]);
        assert_eq!(case["source_id"], variant["source_case_id"]);
        assert_eq!(case["sql"], variant["sql"]);
        assert!(case["source_bag"].is_array(), "{id}: physical input absent");
        assert!(
            case["source_columns"].is_array(),
            "{id}: source types absent"
        );
        assert_eq!(case["vendor_runtime_portability"], "unattested");
        assert_eq!(case["native_engine_version"], "1.10506.0");
        let domain = &case["typed_output_claim"];
        assert!(domain["kind"].is_string(), "{id}: no typed outcome law");
        assert!(domain["lower"].is_null() || domain["lower_inclusive"].is_boolean());
        assert!(domain["upper"].is_null() || domain["upper_inclusive"].is_boolean());
        if domain["kind"] == "positive_integer_lower_bound_inclusive" {
            assert_eq!(domain["lower"], 1);
            assert_eq!(domain["lower_inclusive"], true);
            assert_eq!(domain["nullable"], false);
        }
        assert!(
            case["exact_terminal_bag"].is_array()
                || case["exact_terminal_rows_ordered"].is_array()
                || matches!(
                    case["source_id"].as_str(),
                    Some("88-C177-S033" | "88-C177-S162")
                ),
            "{id}: neither complete terminal nor named error/unstable-order residual"
        );
        if variant["family"] == "collation" {
            assert!(domain["name"].is_string());
        }
    }
}

#[test]
fn original_native_sql_replays_physical_bag_and_positive_negative_or_error_terminal() {
    let frozen = load(FROZEN);
    let mut replayed = 0_usize;
    for case in frozen["cases"]
        .as_array()
        .expect("37 original oracle cases")
    {
        let id = case["id"].as_str().expect("source-owned variant");
        let db = Connection::open_in_memory().expect("native DuckDB");
        let setup = case["source_setup"].as_str().expect("fixed source setup");
        if !setup.is_empty() {
            db.execute_batch(setup)
                .unwrap_or_else(|error| panic!("{id}: native setup: {error}"));
        }
        if case["source_relation"].is_null() {
            assert!(case["source_bag"]
                .as_array()
                .expect("empty scalar source")
                .is_empty());
        } else if case["review_owner"] == "TASK-163" {
            let sql = case["source_snapshot_sql"]
                .as_str()
                .expect("original before query");
            let expected = case["source_bag"]
                .as_array()
                .expect("entire initial source bag");
            assert_eq!(
                ordered_bag(ints(&db, sql, 1).expect("transaction source query")),
                ordered_bag(expected.clone()),
                "{id}: original transaction source"
            );
        } else {
            typed_sources(&db, case);
            if let Some(secondary) = case["secondary_source_bag"].as_array() {
                let customer_sql =
                    "SELECT id, CAST(amount AS VARCHAR) FROM customers ORDER BY rowid";
                let mut statement = db.prepare(customer_sql).expect("all source customers");
                let rows = statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, Option<i64>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    })
                    .expect("customers source bag")
                    .map(|row| {
                        let (id, amount) = row.expect("customer row");
                        json!([id, amount])
                    })
                    .collect::<Vec<_>>();
                let expected = secondary
                    .iter()
                    .map(|row| {
                        let pair = row.as_array().expect("customer columns");
                        json!([pair[0], source_value(&pair[1], false)])
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    ordered_bag(rows),
                    ordered_bag(expected),
                    "{id}: all customer physical occurrences"
                );
            }
        }
        let sql = case["sql"].as_str().expect("exact original SQL");
        if case["source_id"] == "88-C177-S033" {
            let evaluation = ints(&db, sql, 1);
            assert!(
                evaluation.is_err(),
                "{id}: native overflow cannot emit a successful scalar"
            );
            assert_eq!(case["typed_output_claim"]["kind"], "unknown");
        } else if case["source_id"] == "88-C177-S162" {
            let rows = ints(&db, sql, 1).expect("DuckDB tied LIMIT");
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0], json!([4]));
            assert!(
                rows[1] == json!([1]) || rows[1] == json!([null]),
                "unproven tie peer must not be fabricated: {rows:?}"
            );
            assert!(case["exact_terminal_bag"].is_null());
        } else if case["review_owner"] == "TASK-196" || case["review_owner"] == "TASK-163" {
            db.execute_batch(sql)
                .unwrap_or_else(|error| panic!("{id}: original native statement: {error}"));
            let query = if case["review_owner"] == "TASK-196" {
                "SELECT id, amount FROM orders"
            } else {
                case["terminal_query"]
                    .as_str()
                    .expect("original transaction terminal SQL")
            };
            let terminal = case["exact_terminal_bag"]
                .as_array()
                .expect("full final native bag");
            let width = if case["review_owner"] == "TASK-196" {
                2
            } else {
                1
            };
            assert_eq!(
                ordered_bag(ints(&db, query, width).expect("entire final native state")),
                ordered_bag(terminal.clone()),
                "{id}: entire native DML/transaction poststate"
            );
        } else {
            let terminal = case["exact_terminal_bag"]
                .as_array()
                .expect("full native SELECT terminal");
            assert_eq!(
                ordered_bag(
                    ints(&db, sql, 1).unwrap_or_else(|error| {
                        panic!("{id}: original native SELECT: {error}")
                    })
                ),
                ordered_bag(terminal.clone()),
                "{id}: native source-complete terminal SQL bag"
            );
        }
        replayed += 1;
    }
    assert_eq!(replayed, 37);
}

#[test]
fn original_predicate_negative_truth_and_source_bags_match_independent_upstream_authorities() {
    let frozen = load(FROZEN);
    let pred = load(PREDICATES);
    let environment = load(ENVIRONMENT);
    let writes = load(WRITES);
    let lifecycle = load(LIFECYCLE);
    for case in frozen["cases"].as_array().expect("37 cases") {
        let Some(id) = case["source_id"].as_str() else {
            continue;
        };
        let owner = case["review_owner"].as_str().expect("source owner");
        if owner == "TASK-196" {
            let reference = writes["cases"]
                .as_array()
                .expect("16 write cases")
                .iter()
                .find(|row| row["id"] == id)
                .expect("original write case");
            assert_eq!(
                case["exact_terminal_bag"],
                reference["expected_terminal_bag"]
            );
        } else if owner == "TASK-163" {
            let reference = lifecycle["cases"]
                .as_array()
                .expect("original transaction cases")
                .iter()
                .find(|row| row["id"] == id)
                .expect("transaction source");
            assert_eq!(case["source_bag"], reference["expected_source_bag"]);
            assert_eq!(
                case["exact_terminal_bag"],
                reference["expected_terminal_bag"]
            );
        } else if owner == "TASK-192" {
            if let Some(reference) = pred["cases"]
                .as_array()
                .expect("predicate oracles")
                .iter()
                .find(|row| row["id"] == id)
            {
                assert_eq!(case["source_bag"], pred["orders_source_rows"]);
                assert_eq!(
                    case["negative_truth_evidence"]["selected_ids"],
                    reference["expected_selected_ids"]
                );
                assert_eq!(
                    case["negative_truth_evidence"]["rejected_unknown_ids"],
                    reference["expected_unknown_ids"]
                );
                assert_eq!(
                    case["exact_terminal_bag"],
                    reference["expected_terminal_bag"]
                );
            }
            if let Some(reference) = environment["temporal_cases"]
                .as_array()
                .expect("temporal oracles")
                .iter()
                .find(|row| row["id"] == id)
            {
                assert_eq!(case["source_bag"], environment["temporal_source_bag"]);
                assert_eq!(
                    case["exact_terminal_bag"],
                    reference["expected_terminal_bag"]
                );
            }
        }
    }
}

#[test]
fn successful_public_duckdb_analyses_keep_proof_strength_and_no_invented_value_bounds() {
    let frozen = load(FROZEN);
    let parser = dialect_from_name("duckdb").expect("public DuckDB parser");
    let mut checked = 0;
    for case in frozen["cases"].as_array().expect("37 typed SQL cases") {
        let sql = case["sql"].as_str().expect("original SQL");
        let Ok(protocol) = analyze_sql(sql, "duckdb", parser.as_ref()) else {
            // Some original multi-statement transaction controls and vendor
            // spellings are intentionally unsupported by this entry point.
            continue;
        };
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            continue;
        };
        if !sql.trim_start().to_ascii_uppercase().starts_with("SELECT") {
            continue;
        }
        let Some(column) = query.output().columns().first() else {
            panic!("{}: silently lost typed projection", case["id"]);
        };
        match case["typed_output_claim"]["kind"]
            .as_str()
            .expect("typed source law")
        {
            "unbounded" | "nullable_source_id_unbounded" | "integer_unbounded" => {
                assert!(
                    matches!(
                        column.domain(),
                        ValueDomain::Unbounded | ValueDomain::Unknown(_)
                    ),
                    "{}: source-free SELECT invented finite value bounds: {:?}",
                    case["id"],
                    column.domain()
                );
            }
            "unknown" => assert!(
                matches!(column.domain(), ValueDomain::Unknown(_)),
                "{}: an overflow can't certify a numeric result",
                case["id"]
            ),
            "positive_integer_lower_bound_inclusive" => {
                // ROW_NUMBER has a positive numeric law but no fabricated
                // finite upper bound without a complete source cardinality.
                assert!(
                    !matches!(column.domain(), ValueDomain::Empty),
                    "{}: ROW_NUMBER output incorrectly impossible",
                    case["id"]
                );
            }
            other => panic!("{}: unreviewed typed output law {other}", case["id"]),
        };
        checked += 1;
    }
    assert!(
        checked >= 20,
        "must exercise the complete source-free SELECT boundary"
    );
}
