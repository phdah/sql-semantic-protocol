use sql_semantic_protocol::{
    analyze_sql, to_json, AnalysisError, BinaryOperator, ComparisonOperator, DiagnosticArea, Error,
    Expression, JoinKind, LiteralType, LiteralValue, Predicate, Protocol, ProtocolStatement,
    QueryStatement, UnaryOperator,
};
use sqlparser::dialect::{GenericDialect, SnowflakeDialect};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => statement,
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn valid_sql_returns_partial_query_for_caller_selected_dialect() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT 1", "generic", &dialect)
        .expect("valid SQL should cross the public analysis boundary");

    assert_eq!(protocol.protocol_version(), "0.1.0");
    assert_eq!(protocol.source().dialect(), "generic");

    let statement = first_query(&protocol);
    assert!(statement.predicates().where_predicate().is_none());
    assert!(statement
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.area() == DiagnosticArea::Output));
}

#[test]
fn caller_can_supply_a_different_sqlparser_dialect() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql("SELECT 1", "snowflake", &dialect)
        .expect("Snowflake dialect should be supplied by the caller");

    assert_eq!(protocol.source().dialect(), "snowflake");
}

#[test]
fn parse_failures_are_distinct_library_errors() {
    let dialect = GenericDialect {};
    let error = analyze_sql("SELECT (", "generic", &dialect)
        .expect_err("malformed SQL should fail while parsing");

    assert!(matches!(error, Error::Parse(_)));
}

#[test]
fn analysis_failures_are_distinct_library_errors() {
    let dialect = GenericDialect {};
    let error =
        analyze_sql("", "generic", &dialect).expect_err("empty SQL should fail during analysis");

    assert!(matches!(
        error,
        Error::Analysis(AnalysisError::NoStatements)
    ));
}

#[test]
fn serialization_is_separate_from_analysis() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT 1", "generic", &dialect)
        .expect("valid SQL should produce protocol values");
    let json = to_json(&protocol);
    let value: serde_json::Value =
        serde_json::from_str(&json).expect("emitted protocol should be valid JSON");

    assert_eq!(value["protocol_version"], "0.1.0");
    assert_eq!(value["source"]["dialect"], "generic");
    assert_eq!(value["statements"][0]["kind"], "query");
}

#[test]
fn comparison_predicate_is_normalized_and_serialized() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT a FROM t WHERE t.a > 10", "generic", &dialect)
        .expect("comparison should be analyzed");

    let predicate = first_query(&protocol)
        .predicates()
        .where_predicate()
        .expect("WHERE predicate should be present");

    let comparison = match predicate {
        Predicate::Comparison(comparison) => comparison,
        other => panic!("expected comparison predicate, got {other:?}"),
    };

    assert_eq!(comparison.operator(), ComparisonOperator::Gt);

    match comparison.left() {
        Expression::Column(column) => {
            assert_eq!(column.relation(), Some("t"));
            assert_eq!(column.name(), "a");
        }
        other => panic!("expected column expression, got {other:?}"),
    }

    match comparison.right() {
        Expression::Literal(literal) => {
            assert_eq!(literal.literal_type(), LiteralType::Integer);
            assert_eq!(literal.value(), &LiteralValue::Number("10".to_string()));
        }
        other => panic!("expected integer literal, got {other:?}"),
    }

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(
        json["statements"][0]["predicates"]["where"]["operator"],
        "gt"
    );
    assert_eq!(
        json["statements"][0]["predicates"]["where"]["right"]["value"],
        10
    );
}

#[test]
fn boolean_predicate_tree_preserves_and_or_not_structure() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE (a = 1 OR b <> 2) AND NOT (c IS NULL)",
        "generic",
        &dialect,
    )
    .expect("boolean predicate should be analyzed");

    let predicate = first_query(&protocol)
        .predicates()
        .where_predicate()
        .expect("WHERE predicate should be present");

    let operands = match predicate {
        Predicate::And(predicate) => predicate.operands(),
        other => panic!("expected AND predicate, got {other:?}"),
    };

    assert!(matches!(operands.first(), Some(Predicate::Or(_))));
    assert!(matches!(operands.get(1), Some(Predicate::Not(_))));

    let not = match operands.get(1) {
        Some(Predicate::Not(predicate)) => predicate,
        other => panic!("expected NOT predicate, got {other:?}"),
    };
    assert!(matches!(not.operand(), Predicate::IsNull(_)));
}

#[test]
fn between_in_and_null_predicates_are_typed() {
    let dialect = GenericDialect {};

    let between = analyze_sql(
        "SELECT a FROM t WHERE a BETWEEN 1 AND 3",
        "generic",
        &dialect,
    )
    .expect("BETWEEN should be analyzed");
    assert!(matches!(
        first_query(&between).predicates().where_predicate(),
        Some(Predicate::Between(_))
    ));

    let in_list = analyze_sql(
        "SELECT a FROM t WHERE a NOT IN (1, 2, 3)",
        "generic",
        &dialect,
    )
    .expect("IN should be analyzed");
    match first_query(&in_list).predicates().where_predicate() {
        Some(Predicate::In(predicate)) => {
            assert!(predicate.negated());
            assert_eq!(predicate.values().len(), 3);
        }
        other => panic!("expected IN predicate, got {other:?}"),
    }

    let is_null = analyze_sql("SELECT a FROM t WHERE a IS NOT NULL", "generic", &dialect)
        .expect("null predicate should be analyzed");
    match first_query(&is_null).predicates().where_predicate() {
        Some(Predicate::IsNull(predicate)) => assert!(predicate.negated()),
        other => panic!("expected IS NULL predicate, got {other:?}"),
    }
}

#[test]
fn reversed_comparison_normalizes_to_equivalent_semantics() {
    let dialect = GenericDialect {};
    let direct = analyze_sql("SELECT a FROM t WHERE a > 10", "generic", &dialect)
        .expect("direct comparison should be analyzed");
    let reversed = analyze_sql("SELECT a FROM t WHERE 10 < a", "generic", &dialect)
        .expect("reversed comparison should be analyzed");

    assert_eq!(
        first_query(&direct).predicates().where_predicate(),
        first_query(&reversed).predicates().where_predicate()
    );
}

#[test]
fn function_unary_and_binary_expressions_are_normalized() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE COALESCE(t.a + 1, -2) >= 10",
        "generic",
        &dialect,
    )
    .expect("supported expression forms should be analyzed");

    let comparison = match first_query(&protocol)
        .predicates()
        .where_predicate()
        .expect("WHERE predicate should be present")
    {
        Predicate::Comparison(comparison) => comparison,
        other => panic!("expected comparison predicate, got {other:?}"),
    };

    let function = match comparison.left() {
        Expression::Function(function) => function,
        other => panic!("expected function expression, got {other:?}"),
    };

    assert_eq!(function.name(), "COALESCE");
    assert!(!function.distinct());
    assert_eq!(function.arguments().len(), 2);

    match function.arguments().first() {
        Some(Expression::Binary(expression)) => {
            assert_eq!(expression.operator(), BinaryOperator::Add);
        }
        other => panic!("expected binary function argument, got {other:?}"),
    }

    match function.arguments().get(1) {
        Some(Expression::Unary(expression)) => {
            assert_eq!(expression.operator(), UnaryOperator::Minus);
        }
        other => panic!("expected unary function argument, got {other:?}"),
    }

    assert!(!first_query(&protocol)
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_function"));
}

#[test]
fn where_having_and_qualify_keep_clause_context() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE a > 0 GROUP BY a HAVING a < 10 QUALIFY a IS NOT NULL",
        "snowflake",
        &dialect,
    )
    .expect("Snowflake predicate clauses should parse");

    let predicates = first_query(&protocol).predicates();
    assert!(matches!(
        predicates.where_predicate(),
        Some(Predicate::Comparison(_))
    ));
    assert!(matches!(
        predicates.having_predicate(),
        Some(Predicate::Comparison(_))
    ));
    assert!(matches!(
        predicates.qualify_predicate(),
        Some(Predicate::IsNull(_))
    ));
}

#[test]
fn unsupported_predicate_expression_remains_explicit() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE CASE WHEN a > 0 THEN TRUE ELSE FALSE END",
        "generic",
        &dialect,
    )
    .expect("CASE predicate should parse");

    match first_query(&protocol).predicates().where_predicate() {
        Some(Predicate::BooleanExpression(Expression::Unsupported(semantic))) => {
            assert_eq!(semantic.feature(), "expression");
        }
        other => panic!("expected explicit unsupported expression, got {other:?}"),
    }

    assert!(first_query(&protocol)
        .diagnostics()
        .iter()
        .any(|diagnostic| {
            diagnostic.area() == DiagnosticArea::Expression
                && diagnostic.code() == "unsupported_expression"
        }));
}

#[test]
fn unsupported_statement_remains_explicit() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("CREATE TABLE t (a INT)", "generic", &dialect)
        .expect("supported parser statement should return protocol output");

    match protocol.statements().first() {
        Some(ProtocolStatement::Unsupported(statement)) => {
            assert_eq!(statement.category(), "statement");
            assert_eq!(statement.diagnostics()[0].code(), "unsupported_statement");
        }
        other => panic!("expected unsupported statement, got {other:?}"),
    }
}

#[test]
fn unsupported_query_body_is_diagnosed_without_dropping_query() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("VALUES (1)", "generic", &dialect)
        .expect("VALUES query should parse and preserve its query envelope");

    match protocol.statements().first() {
        Some(ProtocolStatement::Query(statement)) => assert!(statement
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "unsupported_query_body")),
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn unsupported_table_factor_is_diagnosed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT * FROM generate_series(1, 10) AS derived",
        "generic",
        &dialect,
    )
    .expect("table-valued function should parse");

    let statement = first_query(&protocol);

    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Source
            && diagnostic.code() == "unsupported_table_factor"
    }));
}

#[test]
fn unsupported_expression_is_diagnosed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE WHEN a > 0 THEN a ELSE 0 END FROM t",
        "generic",
        &dialect,
    )
    .expect("CASE expression should parse");

    let statement = first_query(&protocol);

    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Expression
            && diagnostic.code() == "unsupported_expression"
    }));
}

#[test]
fn unsupported_function_shape_is_diagnosed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT a FROM t WHERE COUNT(*) > 0", "generic", &dialect)
        .expect("COUNT wildcard should parse");

    let comparison = match first_query(&protocol)
        .predicates()
        .where_predicate()
        .expect("WHERE predicate should be present")
    {
        Predicate::Comparison(comparison) => comparison,
        other => panic!("expected comparison predicate, got {other:?}"),
    };

    assert!(matches!(
        comparison.left(),
        Expression::Unsupported(semantic) if semantic.feature() == "function"
    ));
    assert!(first_query(&protocol)
        .diagnostics()
        .iter()
        .any(|diagnostic| {
            diagnostic.area() == DiagnosticArea::Function
                && diagnostic.code() == "unsupported_function"
        }));
}

#[test]
fn relation_sources_preserve_multi_part_names_aliases_and_sorted_dependencies() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.id FROM warehouse.sales.orders AS o, crm.customers AS c",
        "generic",
        &dialect,
    )
    .expect("physical relations should be analyzed");

    let statement = first_query(&protocol);
    assert_eq!(statement.sources().len(), 2);
    assert_eq!(statement.sources()[0].name(), "warehouse.sales.orders");
    assert_eq!(statement.sources()[0].alias(), Some("o"));
    assert_eq!(statement.sources()[1].name(), "crm.customers");
    assert_eq!(statement.sources()[1].alias(), Some("c"));
    assert_eq!(
        statement
            .dependencies()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["crm.customers", "warehouse.sales.orders"]
    );
    assert!(!statement
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "source_analysis_pending"));
}

#[test]
fn using_join_is_normalized_with_relation_identity() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT x.id FROM warehouse.orders AS x LEFT JOIN crm.customers AS y USING (id)",
        "generic",
        &dialect,
    )
    .expect("USING join should be analyzed");

    let statement = first_query(&protocol);
    let join = statement.joins().first().expect("join should be present");
    assert_eq!(join.kind(), JoinKind::Left);
    assert_eq!(join.left().relation(), "warehouse.orders");
    assert_eq!(join.left().alias(), Some("x"));
    assert_eq!(join.right().relation(), "crm.customers");
    assert_eq!(join.right().alias(), Some("y"));

    let comparison = match join.condition() {
        Some(Predicate::Comparison(comparison)) => comparison,
        other => panic!("expected normalized USING comparison, got {other:?}"),
    };

    match comparison.left() {
        Expression::Column(column) => {
            assert_eq!(column.relation(), Some("x"));
            assert_eq!(column.name(), "id");
        }
        other => panic!("expected left USING column, got {other:?}"),
    }
    match comparison.right() {
        Expression::Column(column) => {
            assert_eq!(column.relation(), Some("y"));
            assert_eq!(column.name(), "id");
        }
        other => panic!("expected right USING column, got {other:?}"),
    }

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(json["statements"][0]["joins"][0]["kind"], "left");
    assert_eq!(
        json["statements"][0]["joins"][0]["condition"]["operator"],
        "eq"
    );
}

#[test]
fn ctes_and_derived_tables_remain_local_while_dependencies_recurse() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH recent AS (
            SELECT id, customer_id FROM raw.orders
        )
        SELECT r.id
        FROM recent AS r
        JOIN (SELECT id FROM crm.customers) AS c
          ON r.customer_id = c.id",
        "generic",
        &dialect,
    )
    .expect("CTE and derived table should be analyzed");

    let statement = first_query(&protocol);
    assert_eq!(statement.sources().len(), 2);
    assert_eq!(statement.sources()[0].name(), "recent");
    assert_eq!(statement.sources()[0].alias(), Some("r"));
    assert_eq!(statement.sources()[1].name(), "subquery");
    assert_eq!(statement.sources()[1].alias(), Some("c"));
    assert_eq!(
        statement
            .dependencies()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["crm.customers", "raw.orders"]
    );
    assert!(!statement.dependencies().iter().any(|name| name == "recent"));
    assert!(!statement
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_cte"));
}

#[test]
fn scalar_subqueries_contribute_physical_dependencies() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT u.id
         FROM app.users AS u
         WHERE EXISTS (
             SELECT 1
             FROM audit.events AS e
             WHERE e.user_id = u.id
         )",
        "generic",
        &dialect,
    )
    .expect("EXISTS subquery should preserve dependency information");

    let statement = first_query(&protocol);
    assert_eq!(
        statement
            .dependencies()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["app.users", "audit.events"]
    );
}

#[test]
fn self_join_sources_remain_distinct_by_alias() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT u1.id
         FROM users AS u1
         JOIN users AS u2 ON u1.manager_id = u2.id",
        "generic",
        &dialect,
    )
    .expect("self join should be analyzed");

    let statement = first_query(&protocol);
    assert_eq!(statement.sources().len(), 2);
    assert_eq!(statement.sources()[0].alias(), Some("u1"));
    assert_eq!(statement.sources()[1].alias(), Some("u2"));
    assert_eq!(
        statement
            .dependencies()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["users"]
    );

    let join = statement
        .joins()
        .first()
        .expect("self join should be present");
    assert_eq!(join.left().alias(), Some("u1"));
    assert_eq!(join.right().alias(), Some("u2"));
}
