mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_sql, to_json, DiagnosticArea, Expression, Protocol, ProtocolStatement, QueryStatement,
    SetOperand, SetOperator, SetQuantifier, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn union_all_merges_positional_lineage_and_keeps_left_output_names() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT id FROM left_table UNION ALL SELECT user_id AS renamed FROM right_table",
        "generic",
        &dialect,
    )
    .expect("UNION ALL should analyze");

    let query = first_query(&protocol);
    let operation = query
        .set_operation()
        .expect("set operation should be present");
    assert_eq!(operation.operator(), SetOperator::Union);
    assert_eq!(operation.quantifier(), SetQuantifier::All);

    let columns = query.output().columns();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name(), "id");
    assert!(matches!(columns[0].expression(), Expression::Unknown(_)));
    assert_eq!(columns[0].lineage().len(), 2);
    assert_eq!(columns[0].lineage()[0].relation(), "left_table");
    assert_eq!(columns[0].lineage()[0].column(), "id");
    assert_eq!(columns[0].lineage()[1].relation(), "right_table");
    assert_eq!(columns[0].lineage()[1].column(), "user_id");

    assert_eq!(
        query
            .dependencies()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["left_table", "right_table"]
    );

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(
        json["inputs"][0]["statements"][0]["set_operation"]["operator"],
        "union"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["set_operation"]["quantifier"],
        "all"
    );
}

#[test]
fn intersect_and_except_are_typed_semantics() {
    let dialect = GenericDialect {};

    for (sql, expected) in [
        (
            "SELECT id FROM left_table INTERSECT SELECT id FROM right_table",
            SetOperator::Intersect,
        ),
        (
            "SELECT id FROM left_table EXCEPT SELECT id FROM right_table",
            SetOperator::Except,
        ),
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).expect("set operation should analyze");
        let operation = first_query(&protocol)
            .set_operation()
            .expect("set operation should be present");

        assert_eq!(operation.operator(), expected);
        assert_eq!(operation.quantifier(), SetQuantifier::Distinct);
        assert_eq!(first_query(&protocol).output().columns().len(), 1);
    }
}

#[test]
fn nested_and_chained_set_operations_preserve_the_operation_tree() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "(SELECT id FROM a UNION ALL SELECT id FROM b) INTERSECT SELECT id FROM c",
        "generic",
        &dialect,
    )
    .expect("nested set operations should analyze");

    let operation = first_query(&protocol)
        .set_operation()
        .expect("outer set operation should be present");
    assert_eq!(operation.operator(), SetOperator::Intersect);
    assert!(matches!(
        operation.left(),
        SetOperand::Operation(inner) if inner.operator() == SetOperator::Union
            && inner.quantifier() == SetQuantifier::All
    ));
    assert!(matches!(operation.right(), SetOperand::Query));
    assert_eq!(first_query(&protocol).output().columns().len(), 1);
}

#[test]
fn arity_mismatch_is_explicit_and_does_not_invent_output() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a, b FROM left_table UNION SELECT a FROM right_table",
        "generic",
        &dialect,
    )
    .expect("parser should preserve an arity mismatch for semantic diagnostics");

    let query = first_query(&protocol);
    assert!(query.output().columns().is_empty());
    assert!(query.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Output
            && diagnostic.code() == "set_operation_arity_mismatch"
    }));
}

#[test]
fn conflicting_branch_domains_degrade_to_unknown() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE a > 10 UNION ALL SELECT a FROM t WHERE a < 0",
        "generic",
        &dialect,
    )
    .expect("branch domains should analyze conservatively");

    let domains = first_query(&protocol).column_domains();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].column().relation(), Some("t"));
    assert_eq!(domains[0].column().name(), "a");
    assert!(matches!(domains[0].domain(), ValueDomain::Unknown(_)));
}

#[test]
fn set_level_order_and_limit_remain_attached_to_the_result() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT id FROM a UNION ALL SELECT id FROM b ORDER BY id LIMIT 5",
        "generic",
        &dialect,
    )
    .expect("set-level clauses should remain attached to the query");

    let query = first_query(&protocol);
    assert_eq!(query.output().columns().len(), 1);
    assert!(query
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_order_by"));
    assert!(query
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_limit"));
}

#[test]
fn standard_union_all_is_analyzed_across_all_exposed_dialects() {
    let sql = "SELECT id FROM left_table UNION ALL SELECT id FROM right_table";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} failed UNION ALL: {error}"));

        let operation = first_query(&protocol)
            .set_operation()
            .expect("UNION ALL should be represented");
        assert_eq!(
            operation.operator(),
            SetOperator::Union,
            "dialect {dialect_name}"
        );
        assert_eq!(
            operation.quantifier(),
            SetQuantifier::All,
            "dialect {dialect_name}"
        );
    }
}

#[test]
fn snowflake_minus_is_normalized_to_except() {
    let dialect = dialect_from_str("snowflake").expect("snowflake dialect should exist");
    let protocol = analyze_sql(
        "SELECT id FROM left_table MINUS SELECT id FROM right_table",
        "snowflake",
        dialect.as_ref(),
    )
    .expect("Snowflake MINUS should parse and analyze");

    let operation = first_query(&protocol)
        .set_operation()
        .expect("MINUS should be represented as a set operation");
    assert_eq!(operation.operator(), SetOperator::Except);
    assert_eq!(operation.quantifier(), SetQuantifier::Distinct);
}

#[test]
fn branch_specific_predicate_domains_are_not_collapsed_into_one_witness() {
    let dialect = GenericDialect {};
    let sql = "SELECT a FROM t WHERE a > 10 UNION ALL SELECT a FROM t WHERE a < 0";
    let protocol = analyze_sql(sql, "generic", &dialect).expect("set operation");
    let query = first_query(&protocol);
    let operation = query.set_operation().expect("set operation");
    let branches = operation.branches();

    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].identity(), "body:left");
    assert_eq!(branches[1].identity(), "body:right");
    assert!(branches[0].predicates().where_predicate().is_some());
    assert!(branches[1].predicates().where_predicate().is_some());
    assert_eq!(branches[0].sources()[0].name(), "t");
    assert_eq!(branches[1].sources()[0].name(), "t");
    assert_eq!(branches[0].output().columns()[0].name(), "a");

    let emitted: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("valid json");
    let membership = &emitted["inputs"][0]["statements"][0]["set_operation"]["membership"];
    assert_eq!(membership["tuple_equality"], "not_distinct");
    assert_eq!(membership["multiplicity_rule"], "sum");
    assert_eq!(membership["branches"].as_array().unwrap().len(), 2);
    assert_ne!(
        membership["branches"][0]["column_domains"],
        membership["branches"][1]["column_domains"],
        "opposing branch source domains must remain independent"
    );
    assert_eq!(membership["qualifying_witness"]["status"], "residual");
    assert_eq!(membership["non_qualifying_witness"]["status"], "residual");
    assert!(!query.condition_exactness().is_exact());
}

#[test]
fn nested_set_operations_keep_stable_leaf_branch_identities() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "(SELECT a FROM x UNION ALL SELECT a FROM y) EXCEPT SELECT a FROM z",
        "generic",
        &dialect,
    )
    .expect("nested set operation");
    let branches = first_query(&protocol)
        .set_operation()
        .expect("set operation")
        .branches();
    assert_eq!(
        branches.iter().map(|b| b.identity()).collect::<Vec<_>>(),
        vec!["body:left:query:left", "body:left:query:right", "body:right"]
    );
}

#[test]
fn set_tuple_multiplicity_rules_match_duckdb_with_duplicates_and_null() {
    use duckdb::Connection;
    use sql_semantic_protocol::SetMultiplicityRule;

    let connection = Connection::open_in_memory().expect("DuckDB");
    connection.execute_batch(
        "CREATE TABLE l (v INTEGER); CREATE TABLE r (v INTEGER);
         INSERT INTO l VALUES (1), (1), (NULL), (NULL), (2);
         INSERT INTO r VALUES (1), (NULL), (NULL), (3);",
    ).expect("seed fixture");

    let cases = [
        ("UNION ALL", SetMultiplicityRule::Sum),
        ("UNION", SetMultiplicityRule::UnionDistinct),
        ("INTERSECT ALL", SetMultiplicityRule::Minimum),
        ("INTERSECT", SetMultiplicityRule::IntersectDistinct),
        ("EXCEPT ALL", SetMultiplicityRule::SaturatingDifference),
        ("EXCEPT", SetMultiplicityRule::ExceptDistinct),
    ];
    let dialect = GenericDialect {};
    for (operator, rule) in cases {
        let sql = format!("SELECT v FROM l {operator} SELECT v FROM r");
        let protocol = analyze_sql(&sql, "generic", &dialect).expect("analyze SQL");
        let actual_rule = first_query(&protocol)
            .set_operation()
            .expect("operation")
            .multiplicity_rule()
            .expect("positional set rule");
        assert_eq!(actual_rule, rule, "{operator}");

        let mut statement = connection
            .prepare(&format!(
                "SELECT v, COUNT(*) FROM ({sql}) x GROUP BY v ORDER BY v NULLS FIRST"
            ))
            .expect("prepare oracle query");
        let results = statement
            .query_map([], |row| {
                Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, u64>(1)?))
            })
            .expect("query oracle")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect rows");

        for (value, observed_count) in results {
            let left_count = match value {
                None => 2,
                Some(1) => 2,
                Some(2) => 1,
                _ => 0,
            };
            let right_count = match value {
                None => 2,
                Some(1) => 1,
                Some(3) => 1,
                _ => 0,
            };
            assert_eq!(
                rule.evaluate(left_count, right_count),
                observed_count,
                "{operator} value {value:?}"
            );
        }
        for (left, right) in [(0, 0), (1, 0), (0, 1), (3, 2), (0, 4)] {
            assert!(rule.evaluate(left, right) <= left.saturating_add(right));
        }
    }
}

#[test]
fn set_operators_share_null_safe_positional_multiplicity_contract() {
    use sql_semantic_protocol::SetMultiplicityRule;
    assert_eq!(SetMultiplicityRule::Sum.evaluate(2, 3), 5);
    assert_eq!(SetMultiplicityRule::UnionDistinct.evaluate(2, 3), 1);
    assert_eq!(SetMultiplicityRule::Minimum.evaluate(2, 3), 2);
    assert_eq!(SetMultiplicityRule::IntersectDistinct.evaluate(2, 0), 0);
    assert_eq!(SetMultiplicityRule::SaturatingDifference.evaluate(2, 3), 0);
    assert_eq!(SetMultiplicityRule::ExceptDistinct.evaluate(2, 0), 1);
}
