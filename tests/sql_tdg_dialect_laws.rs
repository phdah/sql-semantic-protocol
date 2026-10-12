//! Cross-dialect canonical equivalence for SQL shapes pinned by TASK-99.
//! A dialect that cannot parse a fixture must not inherit semantic certification.

mod common;

use common::DIALECTS;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, Error};

#[test]
fn shared_pinned_operator_shapes_have_identical_canonical_outcomes() {
    let cases = [
        (
            "range",
            include_str!("fixtures/sql_tdg/raw_sql/range_sampling.sql"),
        ),
        (
            "insert",
            include_str!("fixtures/sql_tdg/raw_sql/insert_workflow.sql"),
        ),
        (
            "join",
            "SELECT orders.customer_id, orders.amount, orders.amount_bucket
             FROM stage_orders AS orders
             JOIN stage_customers AS customers
               ON orders.customer_id = customers.customer_id",
        ),
        (
            "group",
            "SELECT enriched.amount_bucket, COUNT(*) AS order_count,
                    MAX(enriched.amount) AS max_amount
             FROM core_enriched AS enriched
             WHERE enriched.amount = 100
             GROUP BY enriched.amount_bucket
             HAVING COUNT(*) >= 1 ORDER BY enriched.amount_bucket",
        ),
        (
            "set",
            "SELECT marker FROM set_a UNION ALL
             SELECT marker FROM set_b ORDER BY marker LIMIT 2",
        ),
        (
            "exists",
            "SELECT 1 AS marker FROM raw_exists_orders AS orders
             WHERE orders.amount = 100
               AND EXISTS (SELECT 1 FROM raw_exists_customers AS customers
                           WHERE customers.customer_id = orders.customer_id)",
        ),
        (
            "rank",
            "SELECT ROW_NUMBER() OVER (PARTITION BY category ORDER BY score) AS rn
             FROM raw_window WHERE category = 'keep' AND score = 10",
        ),
    ];
    for (label, sql) in cases {
        let mut baseline: Option<Value> = None;
        let mut parsed_dialects = 0;
        let mut rejected_dialects = 0;
        for &dialect in DIALECTS {
            let parser = dialect_from_name(dialect).expect("known exposed dialect");
            match analyze_sql(sql, dialect, parser.as_ref()) {
                Ok(protocol) => {
                    parsed_dialects += 1;
                    let mut canonical: Value =
                        serde_json::from_str(&to_json(&protocol)).expect("canonical JSON");
                    // Dialect is the parser's provenance and not relational meaning.
                    canonical["inputs"][0]["dialect"] = Value::Null;
                    if let Some(expected) = &baseline {
                        assert_eq!(
                            &canonical, expected,
                            "{label}/{dialect}: equivalent SQL has different canonical meaning"
                        );
                    } else {
                        baseline = Some(canonical);
                    }
                }
                Err(Error::Parse(_)) => rejected_dialects += 1,
                Err(Error::Analysis(error)) => {
                    panic!("{label}/{dialect}: parsed shared SQL lost canonical analysis: {error}")
                }
            }
        }
        assert!(
            parsed_dialects >= 3,
            "{label}: no independently compared dialects"
        );
        assert_eq!(
            parsed_dialects + rejected_dialects,
            DIALECTS.len(),
            "{label}: parser boundary was not audited"
        );
    }
}
