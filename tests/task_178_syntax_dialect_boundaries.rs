//! TASK-178 per-dialect TOP/FETCH/MINUS equivalence boundary oracle.
//! Native output equivalence must never imply vendor parser or transaction laws.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const FIXTURE: &str = include_str!("../docs/task-178-syntax-dialect-boundaries.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("TASK-178 fixed dialect evidence")
}

fn normalized(mut document: Value) -> Value {
    for input in document["inputs"].as_array_mut().expect("inputs") {
        input["dialect"] = Value::Null;
    }
    document
}

fn classification(
    sql: &str,
    reference: &str,
    dialect_name: &str,
    no_ties_equivalence: bool,
) -> char {
    let dialect = dialect_from_name(dialect_name).expect("exposed dialect");
    let left = analyze_sql(sql, dialect_name, dialect.as_ref());
    let right = analyze_sql(reference, dialect_name, dialect.as_ref());
    let (Ok(left), Ok(right)) = (left, right) else {
        return 'R';
    };
    if left
        .statements()
        .iter()
        .chain(right.statements())
        .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_)))
    {
        return 'U';
    }
    if !no_ties_equivalence {
        return 'N';
    }
    let l = normalized(read(&to_json(&left)));
    let r = normalized(read(&to_json(&right)));
    if l == r {
        'C'
    } else {
        'D'
    }
}

#[test]
fn top_fetch_and_minus_have_individually_pinned_thirteen_dialect_parser_evidence() {
    let fixture = read(FIXTURE);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["task_id"], "TASK-178");
    let dialects = fixture["dialects"].as_array().expect("13 project dialects");
    assert_eq!(dialects.len(), 13);
    let mut actual = Vec::new();
    for case in fixture["cases"]
        .as_array()
        .expect("four named syntax variants")
    {
        let id = case["id"].as_str().expect("stable case");
        let sql = case["sql"].as_str().expect("variant SQL");
        let reference = case["reference"].as_str().expect("reference SQL");
        let no_ties_equivalence = id != "88-C178-FETCH-TIES";
        let observed: String = dialects
            .iter()
            .map(|name| {
                classification(
                    sql,
                    reference,
                    name.as_str().expect("dialect"),
                    no_ties_equivalence,
                )
            })
            .collect();
        actual.push(format!("{id}={observed}"));
        assert_eq!(observed.len(), 13);
    }
    let expected: Vec<_> = fixture["cases"]
        .as_array()
        .expect("named syntax variants")
        .iter()
        .map(|case| {
            format!(
                "{}={}",
                case["id"].as_str().expect("case ID"),
                case["expected_mask"]
                    .as_str()
                    .expect("frozen per-dialect evidence")
            )
        })
        .collect();
    assert_eq!(
        actual, expected,
        "all TOP/FETCH/MINUS dialect classifications must be reviewed together"
    );
    assert_eq!(fixture["release_approved"], false);
}

#[test]
fn native_except_and_limit_have_full_null_duplicate_output_and_nonmembership_goldens() {
    let db = duckdb::Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE orders(id INT); CREATE TABLE customers(id INT);
         INSERT INTO orders VALUES (1),(1),(2),(NULL);
         INSERT INTO customers VALUES (1),(1),(3),(NULL);",
    )
    .expect("complete nullable duplicate source bags");
    let rows = |sql: &str| {
        let mut statement = db.prepare(sql).expect("DuckDB native reference");
        let mut bag: Vec<Option<i64>> = statement
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .expect("SQL output")
            .map(|row| row.expect("value"))
            .collect();
        bag.sort();
        bag
    };
    assert_eq!(
        rows("SELECT id FROM orders"),
        vec![None, Some(1), Some(1), Some(2)]
    );
    assert_eq!(
        rows("SELECT id FROM customers"),
        vec![None, Some(1), Some(1), Some(3)]
    );
    assert_eq!(
        rows("SELECT id FROM orders EXCEPT SELECT id FROM customers"),
        vec![Some(2)],
        "EXCEPT must exclude an equal nullable tuple even though WHERE NULL=NULL is UNKNOWN"
    );
    assert_eq!(
        rows("SELECT id FROM orders ORDER BY id NULLS LAST LIMIT 2"),
        vec![Some(1), Some(1)],
        "limit counts duplicate source occurrences"
    );
    assert_eq!(
        rows("SELECT id FROM orders ORDER BY id NULLS LAST LIMIT 2 OFFSET 1"),
        vec![Some(1), Some(2)],
        "pagination preserves terminal bag and exact source occurrence multiplicity"
    );
}


#[test]
fn top_fetch_and_limit_preserve_the_same_typed_count_and_sort_shape_without_ties() {
    let fixture = read(FIXTURE);
    let cases = fixture["cases"].as_array().expect("four SQL spellings");
    for id in ["88-C178-TOP", "88-C178-FETCH", "88-C178-FETCH-TIES"] {
        let case = cases
            .iter()
            .find(|case| case["id"] == id)
            .expect("named row selection variant");
        let is_ties = id == "88-C178-FETCH-TIES";
        for name in fixture["dialects"].as_array().expect("13 dialects") {
            let name = name.as_str().expect("dialect");
            let dialect = dialect_from_name(name).expect("public parser");
            let analyze = |sql: &str| {
                let bundle = analyze_sql(sql, name, dialect.as_ref())
                    .expect("syntax recognized by all 13 parser dialects");
                read(&to_json(&bundle))
            };
            let left = analyze(case["sql"].as_str().expect("source SQL"));
            let right = analyze(case["reference"].as_str().expect("reference SQL"));
            let output_left = &left["inputs"][0]["statements"][0]["output"];
            let output_right = &right["inputs"][0]["statements"][0]["output"];
            assert_eq!(output_left, output_right, "{id}/{name}: typed output drift");
            let shape_left = &left["inputs"][0]["statements"][0]["row_shaping"];
            let shape_right = &right["inputs"][0]["statements"][0]["row_shaping"];
            assert!(shape_left.is_object(), "{id}/{name}: TOP or FETCH must be a typed row-selection plan");
            assert!(shape_right.is_object(), "{id}/{name}: LIMIT reference plan missing");
            if is_ties {
                assert_eq!(shape_left["with_ties"], true, "{id}/{name}: ties cannot be lost");
                assert_eq!(shape_right["with_ties"], false);
                assert_ne!(shape_left, shape_right, "{id}/{name}: tied selection is not LIMIT");
            } else {
                assert_eq!(shape_left, shape_right, "{id}/{name}: TOP/FETCH with no ties must preserve the exact canonical row-selection contract");
            }
        }
    }
}

#[test]
fn with_ties_is_not_limit_when_a_boundary_has_three_equal_peers() {
    let db = duckdb::Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE orders(id INT); INSERT INTO orders VALUES (1),(1),(1),(2);")
        .expect("three identical physical key occurrences");
    let values = |sql: &str| {
        let mut stmt = db.prepare(sql).expect("native reference syntax");
        let mut out: Vec<i64> = stmt
            .query_map([], |row| row.get(0))
            .expect("reference rows")
            .map(|item| item.expect("integer key"))
            .collect();
        out.sort();
        out
    };
    assert_eq!(
        values("SELECT id FROM orders ORDER BY id LIMIT 2"),
        vec![1,1],
        "LIMIT 2 selects exactly two physical row occurrences"
    );
    assert_eq!(
        values("SELECT id FROM orders QUALIFY RANK() OVER (ORDER BY id) <= 2"),
        vec![1,1,1],
        "equivalent native rank control retains every tied boundary peer"
    );
    assert!(
        db.prepare("SELECT id FROM orders ORDER BY id FETCH FIRST 2 ROWS WITH TIES").is_err(),
        "DuckDB rejecting the original syntax does not license claim of its native execution"
    );
}
