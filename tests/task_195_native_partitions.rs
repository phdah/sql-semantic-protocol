//! TASK-195 positive/NOT TRUE and stochastic native controls. The exact
//! selected and rejected *value bags* are pinned independently; duplicate
//! physical row IDs with tied sort keys are not made up.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn fixture() -> Value {
    serde_json::from_str(FREEZE).expect("immutable original SQL fixtures")
}

fn tuples(db: &Connection, sql: &str, width: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("native DuckDB SQL");
    let mut result = stmt
        .query_map([], |row| {
            let mut values = Vec::with_capacity(width);
            for index in 0..width {
                values.push(row.get::<_, Option<i64>>(index)?);
            }
            Ok(values)
        })
        .expect("native terminal bag")
        .collect::<Result<Vec<_>, _>>()
        .expect("typed complete output");
    result.sort();
    result
}

fn case<'a>(data: &'a Value, id: &str) -> &'a Value {
    data["cases"]
        .as_array()
        .expect("full original SQL cases")
        .iter()
        .find(|row| row["id"] == id)
        .expect("pinned original SQL case")
}

#[test]
fn original_qualify_positive_and_not_true_bags_partition_complete_candidates() {
    let fixture = fixture();
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(fixture["source_setup"].as_str().expect("fixed source"))
        .expect("source bag with physical NULL and duplicates");

    let complement_sql = [
        (
            "88-C177-S121",
            "SELECT rn FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS rn FROM orders) x WHERE (rn = 1) IS NOT TRUE",
            vec![vec![Some(2)], vec![Some(3)], vec![Some(4)], vec![Some(5)], vec![Some(6)]],
            6,
        ),
        (
            "88-C177-S122",
            "SELECT rn FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS rn FROM orders) x WHERE (rn = 1 AND id > 0) IS NOT TRUE",
            vec![vec![Some(2)], vec![Some(3)], vec![Some(4)], vec![Some(5)], vec![Some(6)]],
            6,
        ),
        (
            "88-C177-S124",
            "SELECT rn FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS rn FROM orders) x WHERE (rn > 1) IS NOT TRUE",
            vec![vec![Some(1)]],
            6,
        ),
    ];
    for (id, rejected_sql, expected_rejected, population) in complement_sql {
        let original = case(&fixture, id);
        let original_sql = original["sql"].as_str().expect("original QUALIFY");
        let selected = tuples(&db, original_sql, 1);
        let expected_selected = original["native_oracle"]["expected_terminal_bag"]
            .as_array()
            .expect("independent original QUALIFY golden")
            .iter()
            .map(|row| vec![row[0].as_i64()])
            .collect::<Vec<_>>();
        assert_eq!(selected, expected_selected, "{id}: selected positive bag");
        let rejected = tuples(&db, rejected_sql, 1);
        assert_eq!(rejected, expected_rejected, "{id}: complete NOT TRUE bag");
        assert_eq!(
            selected.len() + rejected.len(),
            population,
            "{id}: no positive or rejected source occurrence was silently lost"
        );
    }

    let id = "88-C177-S123";
    let original = case(&fixture, id);
    let selected = tuples(&db, original["sql"].as_str().expect("joined QUALIFY"), 2);
    assert_eq!(
        selected,
        vec![vec![Some(1), Some(4)]; 4],
        "{id}: duplicate joined peers form four positive occurrences"
    );
    let rejected = tuples(
        &db,
        "SELECT id, n FROM (SELECT o.id, COUNT(*) OVER (PARTITION BY c.id) AS n FROM orders o JOIN customers c ON o.id = c.id) q WHERE (n > 1) IS NOT TRUE",
        2,
    );
    assert_eq!(
        rejected,
        vec![vec![Some(2), Some(1)]],
        "{id}: exactly one joined candidate fails grouped QUALIFY"
    );
    assert_eq!(
        selected.len() + rejected.len(),
        5,
        "{id}: all 2x2 duplicate matches plus single id=2 match"
    );
}

#[test]
fn original_sampling_queries_never_produce_impossible_source_occurrences() {
    let fixture = fixture();
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(fixture["source_setup"].as_str().expect("source setup"))
        .expect("fixed native source");
    let source = tuples(&db, "SELECT id FROM orders", 1);
    let mut source_multiplicity: BTreeMap<Option<i64>, usize> = BTreeMap::new();
    for row in &source {
        *source_multiplicity.entry(row[0]).or_default() += 1;
    }
    assert_eq!(source.len(), 6);

    let mut executed_cases = 0;
    let mut explicit_parser_boundaries = 0;
    for number in 163..=167 {
        let id = format!("88-C177-S{number:03}");
        let original = case(&fixture, &id);
        let sql = original["sql"].as_str().expect("stochastic original SQL");
        assert_eq!(
            original["native_oracle"]["kind"], "stochastic_conditional",
            "{id}: random sample must not be marked Exact"
        );
        assert!(
            original["native_oracle"]["expected_terminal_bag"].is_null(),
            "{id}: do not record a randomly observed bag as universal output"
        );
        if db.prepare(sql).is_err() {
            // An unsupported original syntax is a parser/engine limitation,
            // never evidence that a different SQL statement executed it.
            explicit_parser_boundaries += 1;
            continue;
        }
        executed_cases += 1;
        for _ in 0..3 {
            let sample = tuples(&db, sql, 1);
            let mut available = source_multiplicity.clone();
            for tuple in sample {
                let value = tuple[0];
                let count = available
                    .get_mut(&value)
                    .expect("sampled physical input value");
                assert!(
                    *count > 0,
                    "{id}: sampled a source occurrence more than once"
                );
                *count -= 1;
            }
        }
    }
    assert_eq!(
        executed_cases + explicit_parser_boundaries,
        5,
        "all five original stochastic SQL shapes have explicit native dispositions"
    );
}
