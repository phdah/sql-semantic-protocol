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

fn classification(sql: &str, reference: &str, dialect_name: &str, no_ties_equivalence: bool) -> char {
    let dialect = dialect_from_name(dialect_name).expect("exposed dialect");
    let left = analyze_sql(sql, dialect_name, dialect.as_ref());
    let right = analyze_sql(reference, dialect_name, dialect.as_ref());
    let (Ok(left), Ok(right)) = (left, right) else {
        return 'R';
    };
    if left.statements().iter().chain(right.statements()).any(|statement| {
        matches!(statement, ProtocolStatement::Unsupported(_))
    }) {
        return 'U';
    }
    if !no_ties_equivalence {
        return 'N';
    }
    let l = normalized(read(&to_json(&left)));
    let r = normalized(read(&to_json(&right)));
    if l == r { 'C' } else { 'D' }
}

#[test]
fn top_fetch_and_minus_have_individually_pinned_thirteen_dialect_parser_evidence() {
    let fixture = read(FIXTURE);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["task_id"], "TASK-178");
    let dialects = fixture["dialects"].as_array().expect("13 project dialects");
    assert_eq!(dialects.len(), 13);
    let mut actual = Vec::new();
    for case in fixture["cases"].as_array().expect("four named syntax variants") {
        let id = case["id"].as_str().expect("stable case");
        let sql = case["sql"].as_str().expect("variant SQL");
        let reference = case["reference"].as_str().expect("reference SQL");
        let no_ties_equivalence = id != "88-C178-FETCH-TIES";
        let observed: String = dialects
            .iter()
            .map(|name| classification(
                sql,
                reference,
                name.as_str().expect("dialect"),
                no_ties_equivalence,
            ))
            .collect();
        actual.push(format!("{id}={observed}"));
        assert_eq!(observed.len(), 13);
        assert_eq!(
            observed,
            case["expected_mask"].as_str().expect("frozen per-dialect evidence"),
            "{id}: observed parser or canonical classification changed; all={actual:?}"
        );
    }
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
    assert_eq!(rows("SELECT id FROM orders"), vec![None,Some(1),Some(1),Some(2)]);
    assert_eq!(rows("SELECT id FROM customers"), vec![None,Some(1),Some(1),Some(3)]);
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
        vec![Some(1),Some(2)],
        "pagination preserves terminal bag and exact source occurrence multiplicity"
    );
}
