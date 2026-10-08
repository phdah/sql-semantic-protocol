mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_sql, to_json, GroupAggregate, GroupValueTest, GroupWitnessDirection, ProtocolStatement,
    ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

fn analyze(sql: &str) -> sql_semantic_protocol::Protocol {
    analyze_sql(sql, "generic", &GenericDialect {}).expect("valid grouped SQL")
}

fn first_query(
    protocol: &sql_semantic_protocol::Protocol,
) -> &sql_semantic_protocol::QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query: {other:?}"),
    }
}

fn count_sql(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("oracle COUNT")
}

#[test]
fn count_rows_group_witnesses_are_exact_and_impossible_cases_are_empty() {
    let protocol =
        analyze("SELECT category, COUNT(*) AS n FROM sales GROUP BY category HAVING COUNT(*) >= 3");
    let query = first_query(&protocol);
    let witness = query
        .group_witness()
        .expect("HAVING must emit group obligations");
    assert_eq!(witness.boundary(), Some("sales"));
    assert_eq!(witness.aggregate(), Some(GroupAggregate::CountRows));
    assert_eq!(witness.group_keys().len(), 1);
    assert_eq!(witness.group_keys()[0].relation(), Some("sales"));
    assert_eq!(witness.group_keys()[0].name(), "category");
    match witness.qualifying() {
        GroupWitnessDirection::Exact(cases) => {
            assert_eq!(cases.len(), 1);
            assert_eq!(cases[0].min_rows(), 3);
            assert_eq!(cases[0].max_rows(), None);
        }
        other => panic!("unexpected qualifying witness: {other:?}"),
    }
    match witness.rejected() {
        GroupWitnessDirection::Exact(cases) => {
            assert_eq!(cases.len(), 1);
            assert_eq!(cases[0].min_rows(), 1);
            assert_eq!(cases[0].max_rows(), Some(2));
        }
        other => panic!("unexpected rejected witness: {other:?}"),
    }
    let value: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    let statement = &value["inputs"][0]["statements"][0];
    assert_eq!(statement["group_witness"]["qualifying"]["status"], "exact");
    assert_eq!(
        statement["output"]["columns"][1]["domain"]["ranges"][0]["lower"]["value"]["value"],
        3
    );
    assert_eq!(
        statement["output"]["columns"][1]["domain"]["ranges"][0]["lower"]["inclusive"],
        true
    );
    assert_eq!(query.dependencies(), &["sales"]);
    assert!(query.output().columns()[1].lineage().is_empty());

    let impossible =
        analyze("SELECT category, COUNT(*) FROM sales GROUP BY category HAVING COUNT(*) < 1");
    assert!(
        matches!(first_query(&impossible).group_witness().unwrap().qualifying(), GroupWitnessDirection::Exact(cases) if cases.is_empty())
    );
    assert!(matches!(
        first_query(&impossible).output().columns()[1].domain(),
        ValueDomain::Empty
    ));

    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE sales(category VARCHAR, amount BIGINT);
        INSERT INTO sales VALUES ('positive', 1), ('positive', 2), ('positive', 3), ('negative', 4), ('negative', NULL);").unwrap();
    assert_eq!(count_sql(&connection, "SELECT COUNT(*) FROM (SELECT category FROM sales GROUP BY category HAVING COUNT(*) >= 3)"), 1);
    assert_eq!(count_sql(&connection, "SELECT COUNT(*) FROM (SELECT category FROM sales GROUP BY category HAVING COUNT(*) < 1)"), 0);
}

#[test]
fn count_column_distinguishes_rows_and_non_null_contributions() {
    let protocol = analyze(
        "SELECT category, COUNT(amount) FROM sales GROUP BY category HAVING COUNT(amount) = 0",
    );
    let witness = first_query(&protocol).group_witness().unwrap();
    assert_eq!(witness.aggregate(), Some(GroupAggregate::CountValues));
    assert_eq!(witness.argument().unwrap().name(), "amount");
    match witness.qualifying() {
        GroupWitnessDirection::Exact(cases) => {
            assert_eq!(cases[0].min_rows(), 1);
            assert_eq!(cases[0].min_non_null(), 0);
            assert_eq!(cases[0].max_non_null(), Some(0));
        }
        other => panic!("unexpected witness: {other:?}"),
    }
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE sales(category VARCHAR, amount BIGINT);
        INSERT INTO sales VALUES ('nulls', NULL), ('nulls', NULL), ('nonnull', 4);",
        )
        .unwrap();
    assert_eq!(count_sql(&connection, "SELECT COUNT(*) FROM (SELECT category FROM sales GROUP BY category HAVING COUNT(amount) = 0)"), 1);
}

#[test]
fn sum_min_max_emit_rejected_null_and_value_proofs_with_oracle_checks() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE sales(category VARCHAR, amount BIGINT);
        INSERT INTO sales VALUES ('positive', 8), ('positive', 6), ('negative', 2), ('negative', 3), ('nulls', NULL);").unwrap();

    for (function, op, threshold, expected, kind) in [
        ("SUM", ">", 10, 1, GroupAggregate::Sum),
        ("MIN", ">=", 6, 1, GroupAggregate::Min),
        ("MAX", "=", 8, 1, GroupAggregate::Max),
    ] {
        let sql = format!("SELECT category, {function}(amount) AS v FROM sales GROUP BY category HAVING {function}(amount) {op} {threshold}");
        let protocol = analyze(&sql);
        let query = first_query(&protocol);
        let witness = query.group_witness().unwrap();
        assert_eq!(witness.aggregate(), Some(kind));
        match witness.qualifying() {
            GroupWitnessDirection::Exact(cases) => {
                assert!(!cases.is_empty());
                assert!(cases
                    .iter()
                    .all(|case| case.min_non_null() == 1 && !case.tests().is_empty()));
                assert!(cases
                    .iter()
                    .flat_map(|case| case.tests())
                    .all(|test| matches!(
                        test,
                        GroupValueTest::Every { .. }
                            | GroupValueTest::Some { .. }
                            | GroupValueTest::Sum { .. }
                    )));
            }
            other => panic!("unexpected proof: {other:?}"),
        }
        match witness.rejected() {
            GroupWitnessDirection::Exact(cases) => {
                assert!(cases.iter().any(|case| case.max_non_null() == Some(0)));
            }
            other => panic!("unexpected negative proof: {other:?}"),
        }
        let oracle = format!("SELECT COUNT(*) FROM ({sql})");
        assert_eq!(count_sql(&connection, &oracle), expected, "{function}");
        let output = &serde_json::from_str::<serde_json::Value>(&to_json(&protocol)).unwrap()
            ["inputs"][0]["statements"][0]["output"]["columns"][1]["domain"];
        assert_eq!(output["kind"], if op == "=" { "set" } else { "ranges" });
    }
}

#[test]
fn complex_predicates_and_uncontrolled_source_boundaries_remain_residual() {
    for sql in [
        "SELECT category, SUM(amount) FROM sales GROUP BY category HAVING SUM(amount) > 10 OR SUM(amount) < 0",
        "SELECT category, SUM(amount) FROM sales WHERE amount > 0 GROUP BY category HAVING SUM(amount) > 10",
        "SELECT s.category, SUM(s.amount) FROM sales s JOIN products p ON s.category = p.category GROUP BY s.category HAVING SUM(s.amount) > 10",
        "WITH staged AS (SELECT category, amount FROM sales) SELECT category, SUM(amount) FROM staged GROUP BY category HAVING SUM(amount) > 10",
        "SELECT category, COUNT(DISTINCT amount) FROM sales GROUP BY category HAVING COUNT(DISTINCT amount) > 2",
    ] {
        let protocol = analyze(sql);
        let witness = first_query(&protocol).group_witness().expect("HAVING witness must remain explicit");
        assert!(matches!(witness.qualifying(), GroupWitnessDirection::Residual { .. }), "{sql}");
        assert!(matches!(witness.rejected(), GroupWitnessDirection::Residual { .. }), "{sql}");
    }
}

#[test]
fn simple_count_contract_is_dialect_independent() {
    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name).expect("known dialect");
        let protocol = analyze_sql(
            "SELECT category, COUNT(*) FROM sales GROUP BY category HAVING COUNT(*) > 2",
            dialect_name,
            dialect.as_ref(),
        )
        .unwrap_or_else(|error| panic!("{dialect_name}: {error}"));
        assert!(
            matches!(
                first_query(&protocol).group_witness().unwrap().qualifying(),
                GroupWitnessDirection::Exact(_)
            ),
            "{dialect_name}"
        );
    }
}

#[test]
fn grouped_witness_provenance_is_preserved_across_producer_layers() {
    use sql_semantic_protocol::{analyze_inputs, to_bundle_json, SqlInput};
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline("CREATE TABLE mart.groups AS SELECT category, COUNT(*) AS total FROM raw.sales GROUP BY category HAVING COUNT(*) > 2"),
        SqlInput::inline("SELECT category, total FROM mart.groups WHERE total > 4"),
    ];
    let bundle = analyze_inputs(&inputs, "generic", &dialect).expect("composed bundle");
    let value: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let layers = value["layers"].as_array().expect("transformation layers");
    let producer = layers
        .iter()
        .find(|layer| layer["consumes"][0] == "raw.sales")
        .expect("producer");
    let consumer = layers
        .iter()
        .find(|layer| layer["consumes"][0] == "mart.groups")
        .expect("consumer");
    let proof = &producer["composed_semantics"]["group_witnesses"][0];
    assert_eq!(proof["witness"]["qualifying"]["status"], "exact");
    assert_eq!(proof["witness"]["boundary"], "raw.sales");
    assert_eq!(proof["boundary_kind"], "physical");
    let inherited = &consumer["composed_semantics"]["group_witnesses"][0];
    assert_eq!(inherited["origin_layer_id"], proof["origin_layer_id"]);
    assert_eq!(inherited["witness"], proof["witness"]);
    assert_eq!(inherited["boundary_kind"], "physical");
}
