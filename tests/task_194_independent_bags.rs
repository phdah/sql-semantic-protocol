//! TASK-194 independent two-column and original 22-case closed-world proof.
//! Each original leaf occurrence has a stable source-row identity; expected
//! discarded candidates are literal tuple bags, not column-wise projections.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-194-independent-bag-review.json");
const FREEZE: &str = include_str!("../docs/task-194-acceptance-freeze.json");

type Tuple = Vec<Option<i64>>;
type Bag = Vec<Tuple>;

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("versioned reviewed closed-world fixture")
}

fn bag(value: &Value) -> Bag {
    let mut rows = value
        .as_array()
        .expect("independently pinned positional tuple rows")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("positional tuple")
                .iter()
                .map(|item| {
                    if item.is_null() {
                        None
                    } else {
                        Some(item.as_i64().expect("signed integer SQL scalar"))
                    }
                })
                .collect::<Tuple>()
        })
        .collect::<Bag>();
    rows.sort();
    rows
}

fn native_rows(db: &Connection, sql: &str, arity: usize) -> Bag {
    let mut statement = db.prepare(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    let mut rows = statement
        .query_map([], |row| {
            (0..arity)
                .map(|i| row.get::<_, Option<i64>>(i))
                .collect::<Result<Tuple, _>>()
        })
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
        .collect::<Result<Bag, _>>()
        .expect("native complete SQL multiset");
    rows.sort();
    rows
}

fn subtracted_candidates(candidate: Bag, output: &Bag) -> Bag {
    let mut counts = BTreeMap::new();
    for row in candidate {
        *counts.entry(row).or_insert(0_u64) += 1;
    }
    for row in output {
        let remaining = counts.get_mut(row).expect("output subset of source candidates");
        assert!(*remaining > 0, "candidate multiplicity cannot be negative");
        *remaining -= 1;
    }
    counts
        .into_iter()
        .flat_map(|(row, count)| (0..count).map(move |_| row.clone()))
        .collect()
}

fn inserted_scalar(value: &Value) -> String {
    value.as_i64().map_or_else(|| "NULL".to_string(), |n| n.to_string())
}

#[test]
fn original_22_cases_prove_each_physical_occurrence_and_rejected_candidate_bag() {
    let reviewed = read(REVIEW);
    let frozen = read(FREEZE);
    assert_eq!(reviewed["count"], 22);
    assert_eq!(reviewed["parser_cells"], 286);
    assert_eq!(reviewed["task88_approved"], false);
    let db = Connection::open_in_memory().expect("independent DuckDB");
    db.execute_batch(frozen["source_setup"].as_str().expect("complete source setup"))
        .expect("all physical tables seeded");
    let mut reviewed_ids = BTreeSet::new();
    let mut reviewed_native = 0;
    let mut rejected_occurrences = 0;
    for case in reviewed["cases"].as_array().expect("every fixed original case") {
        let id = case["case_id"].as_str().expect("original identity");
        assert!(reviewed_ids.insert(id), "{id}: duplicate");
        let original = frozen["cases"]
            .as_array()
            .expect("fixed originals")
            .iter()
            .find(|prior| prior["id"] == id)
            .expect("only original bounded SQL cases");
        assert_eq!(case["sql"], original["sql"], "{id}: independent SQL provenance");
        assert_eq!(case["expected_column_names"], original["expected_output_column_names"]);
        let mut actual_candidates = Vec::new();
        for arm in case["source_arms"].as_array().expect("separate source arms") {
            let relation = arm["relation"].as_str().expect("real relation");
            let projected = arm["column_order"]
                .as_array()
                .expect("columns in original branch order")
                .iter()
                .map(|value| value.as_str().expect("named column"))
                .collect::<Vec<_>>();
            let actual = native_rows(
                &db,
                &format!("SELECT {} FROM {relation}", projected.join(",")),
                projected.len(),
            );
            assert_eq!(
                actual,
                bag(&arm["projected_bag"]),
                "{id}: every physical source occurrence must be included"
            );
            let ids = arm["source_row_ids"].as_array().expect("source occurrence IDs");
            let indexed = native_rows(
                &db,
                &format!("SELECT rowid,{} FROM {relation}", projected.join(",")),
                projected.len() + 1,
            );
            assert_eq!(ids.len(), indexed.len(), "{id}: no dropped physical rowid");
            for (i, entry) in indexed.iter().enumerate() {
                assert_eq!(entry[0], Some(i as i64), "{id}: true physical rowid");
                assert_eq!(
                    ids[i], format!("{relation}#{i}"),
                    "{id}: stable source row identity, including repeated source branches"
                );
            }
            actual_candidates.extend(actual);
        }
        actual_candidates.sort();
        match case["reviewed_status"].as_str().expect("review disposition") {
            "explicit_noncertified_boundary" => {
                assert!(case["independent_terminal_bag"].is_null());
                assert!(case["independent_non_emitted_candidate_bag"].is_null());
                assert!(case["candidate_sql"].is_null());
                assert!(
                    id == "88-C177-S084" || id == "88-C177-S086",
                    "{id}: unknown boundary cannot be silently approved"
                );
            }
            "independent_typed_set_law_and_native_oracle" => {
                let sql = case["sql"].as_str().expect("original SQL");
                let arity = original["expected_output_columns"].as_u64().expect("native width")
                    as usize;
                let candidates = native_rows(
                    &db,
                    case["candidate_sql"].as_str().expect("all original source candidates"),
                    arity,
                );
                assert_eq!(
                    actual_candidates, candidates,
                    "{id}: the independently selected arms must match native bag UNION ALL"
                );
                assert_eq!(
                    candidates,
                    bag(&case["independent_source_candidate_bag"]),
                    "{id}: original SQL candidate bag is literal"
                );
                let actual = native_rows(&db, sql, arity);
                let expected = bag(&case["independent_terminal_bag"]);
                assert_eq!(actual, expected, "{id}: independent terminal multiset");
                assert_eq!(
                    actual,
                    bag(&original["expected_terminal_bag"]),
                    "{id}: original native evidence reconciles with independently derived tuple law"
                );
                let rejected = subtracted_candidates(candidates, &actual);
                assert_eq!(
                    rejected,
                    bag(&case["independent_non_emitted_candidate_bag"]),
                    "{id}: exact discarded tuple occurrences, not per-column rejection"
                );
                assert_eq!(
                    case["expected_rejected_candidate_occurrences"],
                    rejected.len() as u64,
                    "{id}: reviewed negative count"
                );
                let impossible = &case["impossible_count_goal"];
                let forbidden_tuple = bag(&serde_json::json!([impossible["tuple"]]));
                let present = actual.iter().filter(|row| *row == &forbidden_tuple[0]).count();
                assert_eq!(
                    present + 1,
                    impossible["count"].as_u64().expect("impossible multiplicity") as usize,
                    "{id}: request for one more occurrence must be Impossible"
                );
                rejected_occurrences += rejected.len();
                reviewed_native += 1;
            }
            other => panic!("{id}: unknown review disposition {other}"),
        }
    }
    assert_eq!(reviewed_ids.len(), 22);
    assert_eq!(reviewed_native, 20);
    assert_eq!(rejected_occurrences, 106);
}

#[test]
fn nine_adversarial_two_column_cases_distinguish_tuple_multiplicity_and_null() {
    let fixture = read(REVIEW);
    let adversarial = &fixture["adversarial"];
    assert_eq!(adversarial["case_count"], 9);
    let db = Connection::open_in_memory().expect("fresh native instance");
    db.execute_batch("CREATE TABLE l(a INTEGER,b INTEGER); CREATE TABLE r(a INTEGER,b INTEGER);")
        .expect("typed two-column physical sources");
    for (source_name, table) in [("l", "left"), ("r", "right")] {
        for row in adversarial["source_bags"][table]
            .as_array()
            .expect("complete physical bag")
        {
            let r = row.as_array().expect("two columns");
            db.execute_batch(&format!(
                "INSERT INTO {source_name} VALUES ({},{});",
                inserted_scalar(&r[0]),
                inserted_scalar(&r[1])
            ))
            .expect("one stable physical occurrence");
        }
        assert_eq!(
            native_rows(&db, &format!("SELECT a,b FROM {source_name}"), 2),
            bag(&adversarial["source_bags"][table]),
            "{source_name}: duplicates and NULL remain intact"
        );
    }
    let mut names = BTreeSet::new();
    for case in adversarial["cases"].as_array().expect("all nine adversarial laws") {
        let id = case["id"].as_str().expect("adversarial identity");
        assert!(names.insert(id), "{id}: duplicate");
        let original = native_rows(&db, case["sql"].as_str().expect("SQL"), 2);
        let candidates = native_rows(&db, case["candidate_sql"].as_str().expect("source bags"), 2);
        assert_eq!(original, bag(&case["expected_terminal_bag"]), "{id}: output bag");
        assert_eq!(
            candidates,
            bag(&case["expected_candidate_bag"]),
            "{id}: independent pre-set source bag"
        );
        let rejected = subtracted_candidates(candidates, &original);
        assert_eq!(
            rejected,
            bag(&case["expected_non_emitted_candidate_bag"]),
            "{id}: tuple-level negative bag"
        );
        assert_eq!(original.len() as u64, case["expected_terminal_count"], "{id}");
        assert_eq!(rejected.len() as u64, case["expected_rejected_count"], "{id}");
        assert!(
            !original.contains(&vec![Some(1), Some(2)]),
            "{id}: cannot synthesize an unattested tuple from separate column values"
        );
    }
    assert_eq!(names.len(), 9);
    // INTERSECT ALL uses min(3,1) for (NULL,1) and min(2,2) for (1,NULL).
    let inter = adversarial["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "intersect_all")
        .unwrap();
    assert_eq!(
        bag(&inter["expected_terminal_bag"]),
        vec![vec![None, Some(1)], vec![Some(1), None], vec![Some(1), None]]
    );
    // EXCEPT ALL requires saturated subtraction, retaining two (NULL,1) rows.
    let except = adversarial["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "except_all")
        .unwrap();
    assert_eq!(
        bag(&except["expected_terminal_bag"]),
        vec![
            vec![None, Some(1)],
            vec![None, Some(1)],
            vec![Some(2), Some(2)],
            vec![Some(2), Some(2)],
            vec![Some(3), Some(3)]
        ]
    );
}
