//! TASK-180: independently replay the remaining original scalar predicates
//! and non-integer native boundaries. Together with the 25 base predicates
//! this accounts for all 43 source-frozen TASK-192 SQL variants.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const MATRIX: &str = include_str!("../docs/task-180-acceptance-freeze.json");
const SCALARS: &str = include_str!("../docs/task-192-native-partitions.json");
const ENV: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const FINAL: &str = include_str!("../docs/task-177-final-native-deterministic.json");
const ORDERING: &str = include_str!("../docs/task-177-native-expansion.json");
const STRINGS: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("frozen original source/oracle evidence")
}

fn sorted_nullable(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut query = db.prepare(sql).expect("native source or terminal SQL");
    let values = query
        .query_map([], |row| row.get::<usize, Option<i64>>(0))
        .expect("native typed integer/NULL bag");
    let mut bag: Vec<_> = values.map(|row| row.expect("native occurrence")).collect();
    bag.sort();
    bag
}

fn sorted_ids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut query = db.prepare(sql).expect("native source ID SQL");
    let values = query
        .query_map([], |row| row.get::<usize, i64>(0))
        .expect("physical source ID bag");
    let mut bag: Vec<_> = values.map(|row| row.expect("native row ID")).collect();
    bag.sort();
    bag
}

fn expected_nullable(value: &Value) -> Vec<Option<i64>> {
    let mut result: Vec<_> = value
        .as_array()
        .expect("complete source or terminal bag")
        .iter()
        .map(|row| row[0].as_i64())
        .collect();
    result.sort();
    result
}

fn expected_source_terminal<'a>(id: &str, env: &'a Value, final_native: &'a Value) -> &'a Value {
    [
        &env["customer_cases"],
        &env["temporal_cases"],
        &env["floating_cases"],
        &final_native["original_integer_selects"],
    ]
    .into_iter()
    .flat_map(|items| items.as_array().expect("frozen source terminal cases"))
    .find(|row| row["id"] == id)
    .map(|row| &row["expected_terminal_bag"])
    .unwrap_or_else(|| panic!("{id}: missing independent full native terminal"))
}

#[test]
fn other_eleven_original_scalar_predicates_have_complete_native_three_valued_partitions() {
    let matrix = parse(MATRIX);
    let native = parse(SCALARS);
    let env = parse(ENV);
    let final_native = parse(FINAL);
    let mut visited = BTreeSet::new();
    for case in native["predicates"].as_array().expect("11 additional native cases") {
        let id = case["id"].as_str().expect("stable original SQL ID");
        assert!(visited.insert(id), "{id}: repeated native fixture");
        let original = matrix["cases"]
            .as_array()
            .expect("163 frozen original cases")
            .iter()
            .find(|row| row["id"] == id)
            .expect("source SQL must belong to full final release matrix");
        assert_eq!(original["owner"], "TASK-192");
        let sql = original["sql"].as_str().expect("unaltered original SQL");
        let group_name = case["group"].as_str().expect("physical source group");
        let group = &native["source_groups"][group_name];
        let fixture = if group_name == "nan" {
            &final_native
        } else {
            &env
        };
        let source_sql = fixture[group["setup"].as_str().expect("input fixture key")]
            .as_str()
            .expect("complete physical setup");
        let relation = group["relation"].as_str().expect("source identity");
        let db = Connection::open_in_memory().expect("isolated native input");
        db.execute_batch(source_sql).expect("full native source materialization");

        let row_count = group["row_count"].as_u64().expect("physical source count") as i64;
        let all_ids = (0..row_count).collect::<Vec<_>>();
        assert_eq!(
            sorted_ids(&db, &format!("SELECT rowid FROM {relation}")),
            all_ids,
            "{id}: full independent physical source identity"
        );
        assert_eq!(
            sorted_nullable(&db, &format!("SELECT id FROM {relation}")),
            expected_nullable(&fixture[group["bag"].as_str().expect("source bag key")]),
            "{id}: source NULL and duplicate occurrence bag"
        );
        let predicate = case["predicate"].as_str().expect("original SQL predicate");
        assert!(
            sql.ends_with(&format!(" WHERE {predicate}")),
            "{id}: no substituted SQL predicate allowed"
        );

        let mut all_truth_cases = Vec::new();
        for (truth, expected) in [
            ("IS TRUE", "expected_true_rowids"),
            ("IS FALSE", "expected_false_rowids"),
            ("IS UNKNOWN", "expected_unknown_rowids"),
        ] {
            let original_ids: Vec<i64> =
                serde_json::from_value(case[expected].clone()).expect("original truth ID bag");
            assert_eq!(
                sorted_ids(
                    &db,
                    &format!("SELECT rowid FROM {relation} WHERE ({predicate}) {truth}")
                ),
                original_ids,
                "{id}: independently proven {truth} native row instances"
            );
            all_truth_cases.extend(original_ids);
        }
        all_truth_cases.sort();
        assert_eq!(all_truth_cases, all_ids, "{id}: every source row must be classified");
        let rejected: Vec<i64> =
            serde_json::from_value(case["expected_not_true_rowids"].clone())
                .expect("independent NOT TRUE oracle");
        assert_eq!(
            sorted_ids(
                &db,
                &format!("SELECT rowid FROM {relation} WHERE ({predicate}) IS NOT TRUE")
            ),
            rejected,
            "{id}: NOT TRUE includes both FALSE and UNKNOWN without row loss"
        );
        assert_eq!(
            sorted_nullable(&db, sql),
            expected_nullable(expected_source_terminal(id, &env, &final_native)),
            "{id}: complete original native terminal bag"
        );
    }
    assert_eq!(visited.len(), 11);
}

#[test]
fn three_non_predicate_originals_keep_native_error_ordering_and_struct_boundaries() {
    let matrix = parse(MATRIX);
    let native = parse(SCALARS);
    let ordering = parse(ORDERING);
    let original = |id: &str| -> &str {
        matrix["cases"]
            .as_array()
            .expect("source-frozen 163 cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original SQL variant")["sql"]
            .as_str()
            .expect("SQL")
    };
    let boundary = native["other"].as_array().expect("three original exception cases");
    assert_eq!(boundary.len(), 3);

    let db = Connection::open_in_memory().expect("native fixed-width overflow");
    let sql = original("88-C177-S033");
    assert_eq!(
        boundary.iter().find(|c| c["id"] == "88-C177-S033").expect("overflow")["expected"],
        "evaluation_error"
    );
    assert!(
        db.prepare(sql)
            .and_then(|mut stmt| stmt.query([]).and_then(|mut rows| rows.next().map(|_| ())))
            .is_err(),
        "signed overflow must not invent a successful native value"
    );

    let db = Connection::open_in_memory().expect("NULL and duplicate ordering");
    db.execute_batch(ordering["ordering_source_setup"].as_str().expect("source SQL"))
        .expect("complete order source bag");
    let order = original("88-C177-S042");
    let mut statement = db.prepare(order).expect("ordered output");
    let observed: Vec<Option<i64>> = statement
        .query_map([], |row| row.get(0))
        .expect("ordered nullable integer IDs")
        .map(|row| row.expect("native order row"))
        .collect();
    let frozen_order = boundary
        .iter()
        .find(|case| case["id"] == "88-C177-S042")
        .expect("original order law");
    let expected: Vec<Option<i64>> = frozen_order["expected_ordered_terminal"]
        .as_array()
        .expect("literal ordered bag")
        .iter()
        .map(Value::as_i64)
        .collect();
    assert_eq!(observed, expected, "ordered output is not just an unordered bag");

    let original_struct = original("88-C177-S179");
    let db = Connection::open_in_memory().expect("native STRUCT result");
    let sql = format!("SELECT v.a, v.b FROM ({original_struct}) derived");
    let mut query = db.prepare(&sql).expect("struct-field projection");
    let mut rows = query.query([]).expect("one original scalar");
    let row = rows.next().expect("native row").expect("one struct scalar");
    let fields = &boundary
        .iter()
        .find(|case| case["id"] == "88-C177-S179")
        .expect("source-frozen STRUCT")["expected_fields"];
    assert_eq!(row.get::<_, i64>(0).expect("field a"), fields["a"].as_i64().expect("int"));
    assert_eq!(row.get::<_, String>(1).expect("field b"), fields["b"].as_str().expect("text"));
    assert!(rows.next().expect("native source exhausted").is_none());
}

#[test]
fn four_original_like_and_ilike_cases_keep_true_false_unknown_and_duplicate_values() {
    let matrix = parse(MATRIX);
    let reviewed = parse(STRINGS);
    let literals = &reviewed["task_owned_string_pattern_duckdb_oracles"];
    let db = Connection::open_in_memory().expect("native isolated text source");
    db.execute_batch(literals["source_setup"].as_str().expect("7 source rows"))
        .expect("complete original text inputs");
    assert_eq!(
        sorted_nullable(&db, "SELECT id FROM customers"),
        vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6), Some(7)]
    );

    let mut visited = BTreeSet::new();
    let mut dialect_cases = 0;
    for case in literals["cases"].as_array().expect("4 original pattern oracles") {
        let id = case["id"].as_str().expect("original SQL ID");
        assert!(visited.insert(id), "{id}: duplicate predicate review");
        let original = matrix["cases"]
            .as_array()
            .expect("all 163 original SQL variants")
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("original predicate case");
        assert_eq!(original["owner"], "TASK-192");
        let sql = original["sql"].as_str().expect("unmodified source SQL");
        let predicate = case["predicate"].as_str().expect("literal original predicate");
        assert_eq!(sql, format!("SELECT id FROM customers WHERE {predicate}"));

        let get_ids = |field: &str| -> Vec<i64> {
            serde_json::from_value(case[field].clone()).expect("frozen source occurrence IDs")
        };
        let true_ids = get_ids("selected_ids");
        let false_ids = get_ids("false_ids");
        let unknown_ids = get_ids("unknown_ids");
        let all = {
            let mut ids = true_ids.clone();
            ids.extend(false_ids.iter());
            ids.extend(unknown_ids.iter());
            ids.sort();
            ids
        };
        assert_eq!(all, (1..=7).collect::<Vec<_>>(), "{id}: full nullable text source");
        for (truth, expected) in [
            ("IS TRUE", true_ids.clone()),
            ("IS FALSE", false_ids.clone()),
            ("IS UNKNOWN", unknown_ids.clone()),
        ] {
            assert_eq!(
                sorted_ids(
                    &db,
                    &format!("SELECT id FROM customers WHERE ({predicate}) {truth}")
                ),
                expected,
                "{id}: original source true/false/unknown text bag"
            );
        }
        let mut rejected = false_ids;
        rejected.extend(unknown_ids);
        rejected.sort();
        assert_eq!(
            sorted_ids(
                &db,
                &format!("SELECT id FROM customers WHERE ({predicate}) IS NOT TRUE")
            ),
            rejected,
            "{id}: NOT TRUE cannot be reduced to FALSE alone"
        );
        assert_eq!(
            sorted_ids(&db, sql),
            true_ids,
            "{id}: full original terminal physical ID bag"
        );

        for cell in original["cells"].as_array().expect("13 dialects") {
            let name = cell["dialect"].as_str().expect("parser");
            let parser = sql_semantic_protocol::dialect_from_name(name)
                .expect("exposed parser dialect");
            match sql_semantic_protocol::analyze_sql(sql, name, parser.as_ref()) {
                Ok(protocol) => {
                    let [sql_semantic_protocol::ProtocolStatement::Query(query)] =
                        protocol.statements()
                    else {
                        panic!("{id}/{name}: original source SQL silently dropped");
                    };
                    assert_eq!(query.output().columns().len(), 1);
                }
                Err(sql_semantic_protocol::Error::Parse(_))
                    if cell["original_canonical_class"] == "R" => {}
                Err(error) => panic!("{id}/{name}: unreviewed parser boundary: {error}"),
            }
            dialect_cases += 1;
        }
    }
    assert_eq!(visited.len(), 4);
    assert_eq!(dialect_cases, 52);
}
