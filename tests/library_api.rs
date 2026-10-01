use sql_semantic_protocol::{
    analyze_sql, to_json, AnalysisError, BinaryOperator, ComparisonOperator, DiagnosticArea, Error,
    Expression, JoinKind, LiteralExpression, LiteralType, LiteralValue, Predicate, Protocol,
    ProtocolStatement, QueryStatement, SetMode, UnaryOperator, ValueDomain,
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

    assert_eq!(protocol.protocol_version(), "0.2.0");
    assert_eq!(protocol.source().dialect(), "generic");

    let statement = first_query(&protocol);
    assert!(statement.predicates().where_predicate().is_none());
    assert_eq!(statement.output().columns().len(), 1);
    assert_eq!(statement.output().columns()[0].name(), "1");
    assert!(!statement
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "output_analysis_pending"));
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

    assert_eq!(value["protocol_version"], "0.2.0");
    assert_eq!(value["inputs"][0]["dialect"], "generic");
    assert_eq!(value["inputs"][0]["statements"][0]["kind"], "query");
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
        json["inputs"][0]["statements"][0]["predicates"]["where"]["operator"],
        "gt"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["predicates"]["where"]["right"]["value"],
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
fn queryless_create_table_remains_explicitly_unsupported() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("CREATE TABLE t (a INT)", "generic", &dialect)
        .expect("supported parser statement should return protocol output");

    match protocol.statements().first() {
        Some(ProtocolStatement::Unsupported(statement)) => {
            assert_eq!(statement.category(), "create_table");
            assert_eq!(
                statement.diagnostics()[0].code(),
                "unsupported_queryless_create_table"
            );
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
    assert_eq!(
        json["inputs"][0]["statements"][0]["joins"][0]["kind"],
        "left"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["joins"][0]["condition"]["operator"],
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

#[test]
fn multiple_joins_preserve_sql_order_and_alias_identity() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT u.id
         FROM users AS u
         JOIN managers AS m ON u.manager_id = m.id
         JOIN departments AS d ON m.department_id = d.id",
        "generic",
        &dialect,
    )
    .expect("multiple joins should be analyzed");

    let joins = first_query(&protocol).joins();
    assert_eq!(joins.len(), 2);
    assert_eq!(joins[0].left().alias(), Some("u"));
    assert_eq!(joins[0].right().alias(), Some("m"));
    assert_eq!(joins[1].left().alias(), Some("m"));
    assert_eq!(joins[1].right().alias(), Some("d"));
}

#[test]
fn natural_join_reports_unresolved_condition() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT * FROM users AS u NATURAL JOIN managers AS m",
        "generic",
        &dialect,
    )
    .expect("NATURAL JOIN should parse");

    let statement = first_query(&protocol);
    assert_eq!(statement.joins().len(), 1);
    assert!(statement.joins()[0].condition().is_none());
    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Join
            && diagnostic.code() == "unsupported_natural_join_condition"
    }));
}

#[test]
fn ordered_comparison_derives_open_lower_bound_for_non_output_column() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT b FROM t WHERE a > 10", "generic", &dialect)
        .expect("ordered comparison should derive a domain");

    let domains = first_query(&protocol).column_domains();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].column().relation(), Some("t"));
    assert_eq!(domains[0].column().name(), "a");

    let ranges = match domains[0].domain() {
        ValueDomain::Ranges(domain) => domain.ranges(),
        other => panic!("expected range domain, got {other:?}"),
    };
    assert_eq!(ranges.len(), 1);
    let lower = ranges[0].lower().expect("lower bound should exist");
    assert!(!lower.inclusive());
    assert_eq!(lower.value().literal_type(), LiteralType::Integer);
    assert_eq!(
        lower.value().value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(ranges[0].upper().is_none());

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(
        json["inputs"][0]["statements"][0]["column_domains"][0]["domain"]["ranges"][0]["lower"]
            ["value"]["value"],
        10
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["column_domains"][0]["domain"]["ranges"][0]["lower"]
            ["inclusive"],
        false
    );
}

#[test]
fn conjunction_intersects_compatible_range_bounds() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE a >= 10 AND a < 20",
        "generic",
        &dialect,
    )
    .expect("conjunction should derive an intersected range");

    let ranges = match first_query(&protocol).column_domains()[0].domain() {
        ValueDomain::Ranges(domain) => domain.ranges(),
        other => panic!("expected range domain, got {other:?}"),
    };
    assert_eq!(ranges.len(), 1);
    assert!(ranges[0].lower().expect("lower bound").inclusive());
    assert!(!ranges[0].upper().expect("upper bound").inclusive());
}

#[test]
fn contradictory_predicates_produce_empty_domain() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE a > 10 AND a < 5",
        "generic",
        &dialect,
    )
    .expect("contradiction should remain representable");

    assert!(matches!(
        first_query(&protocol).column_domains()[0].domain(),
        ValueDomain::Empty
    ));
}

#[test]
fn equality_and_in_predicates_produce_inclusion_sets() {
    let dialect = GenericDialect {};

    let equality = analyze_sql("SELECT a FROM t WHERE a = 10", "generic", &dialect)
        .expect("equality should derive a finite set");
    let equality_set = match first_query(&equality).column_domains()[0].domain() {
        ValueDomain::Set(domain) => domain,
        other => panic!("expected set domain, got {other:?}"),
    };
    assert_eq!(equality_set.mode(), SetMode::Include);
    assert_eq!(equality_set.values().len(), 1);

    let in_list = analyze_sql(
        "SELECT a FROM t WHERE a IN (3, 1, 2, 2)",
        "generic",
        &dialect,
    )
    .expect("IN should derive a finite set");
    let in_set = match first_query(&in_list).column_domains()[0].domain() {
        ValueDomain::Set(domain) => domain,
        other => panic!("expected set domain, got {other:?}"),
    };
    assert_eq!(in_set.mode(), SetMode::Include);
    assert_eq!(
        in_set
            .values()
            .iter()
            .map(LiteralExpression::value)
            .collect::<Vec<_>>(),
        vec![
            &LiteralValue::Number("1".to_string()),
            &LiteralValue::Number("2".to_string()),
            &LiteralValue::Number("3".to_string()),
        ]
    );
}

#[test]
fn inequality_and_null_predicates_produce_exclusion_sets() {
    let dialect = GenericDialect {};

    let inequality = analyze_sql("SELECT a FROM t WHERE a <> 10", "generic", &dialect)
        .expect("inequality should derive an exclusion set");
    let inequality_set = match first_query(&inequality).column_domains()[0].domain() {
        ValueDomain::Set(domain) => domain,
        other => panic!("expected set domain, got {other:?}"),
    };
    assert_eq!(inequality_set.mode(), SetMode::Exclude);
    assert!(inequality_set
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Number("10".to_string())));
    assert!(inequality_set
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Null));

    let is_not_null = analyze_sql("SELECT a FROM t WHERE a IS NOT NULL", "generic", &dialect)
        .expect("IS NOT NULL should derive an exclusion set");
    let null_set = match first_query(&is_not_null).column_domains()[0].domain() {
        ValueDomain::Set(domain) => domain,
        other => panic!("expected set domain, got {other:?}"),
    };
    assert_eq!(null_set.mode(), SetMode::Exclude);
    assert_eq!(null_set.values().len(), 1);
    assert_eq!(null_set.values()[0].value(), &LiteralValue::Null);
}

#[test]
fn between_and_not_between_preserve_closed_and_disjoint_ranges() {
    let dialect = GenericDialect {};

    let between = analyze_sql(
        "SELECT a FROM t WHERE a BETWEEN 1 AND 3",
        "generic",
        &dialect,
    )
    .expect("BETWEEN should derive a closed range");
    let between_ranges = match first_query(&between).column_domains()[0].domain() {
        ValueDomain::Ranges(domain) => domain.ranges(),
        other => panic!("expected range domain, got {other:?}"),
    };
    assert_eq!(between_ranges.len(), 1);
    assert!(between_ranges[0].lower().expect("lower bound").inclusive());
    assert!(between_ranges[0].upper().expect("upper bound").inclusive());

    let not_between = analyze_sql(
        "SELECT a FROM t WHERE a NOT BETWEEN 1 AND 3",
        "generic",
        &dialect,
    )
    .expect("NOT BETWEEN should derive disjoint ranges");
    let not_between_ranges = match first_query(&not_between).column_domains()[0].domain() {
        ValueDomain::Ranges(domain) => domain.ranges(),
        other => panic!("expected range domain, got {other:?}"),
    };
    assert_eq!(not_between_ranges.len(), 2);
    assert!(not_between_ranges[0].lower().is_none());
    assert!(not_between_ranges[1].upper().is_none());
}

#[test]
fn disjunction_preserves_disjoint_ranges() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT a FROM t WHERE a < 0 OR a > 10", "generic", &dialect)
        .expect("OR should preserve both allowed ranges");

    let ranges = match first_query(&protocol).column_domains()[0].domain() {
        ValueDomain::Ranges(domain) => domain.ranges(),
        other => panic!("expected range domain, got {other:?}"),
    };
    assert_eq!(ranges.len(), 2);
}

#[test]
fn disjunction_across_different_columns_is_unbounded_per_column() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT a, b FROM t WHERE a = 1 OR b = 2",
        "generic",
        &dialect,
    )
    .expect("cross-column OR should remain conservative");

    let domains = first_query(&protocol).column_domains();
    assert_eq!(domains.len(), 2);
    assert!(domains
        .iter()
        .all(|domain| matches!(domain.domain(), ValueDomain::Unbounded)));
}

#[test]
fn column_to_column_comparison_has_explicit_unknown_scalar_domains() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT a FROM t WHERE a > b", "generic", &dialect)
        .expect("column comparison should remain representable");

    let domains = first_query(&protocol).column_domains();
    assert_eq!(domains.len(), 2);
    assert!(domains
        .iter()
        .all(|domain| matches!(domain.domain(), ValueDomain::Unknown(_))));
}

#[test]
fn output_columns_preserve_order_aliases_expressions_and_direct_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.id AS order_id, o.total + 1 AS adjusted_total
         FROM sales.orders AS o
         WHERE o.status = 'open'",
        "generic",
        &dialect,
    )
    .expect("output columns should be analyzed");

    let statement = first_query(&protocol);
    let columns = statement.output().columns();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].name(), "order_id");
    assert_eq!(columns[1].name(), "adjusted_total");
    assert!(matches!(columns[0].expression(), Expression::Column(_)));
    assert!(matches!(columns[1].expression(), Expression::Binary(_)));

    assert_eq!(columns[0].lineage().len(), 1);
    assert_eq!(columns[0].lineage()[0].relation(), "sales.orders");
    assert_eq!(columns[0].lineage()[0].column(), "id");
    assert_eq!(columns[1].lineage().len(), 1);
    assert_eq!(columns[1].lineage()[0].relation(), "sales.orders");
    assert_eq!(columns[1].lineage()[0].column(), "total");

    assert!(columns
        .iter()
        .flat_map(|column| column.lineage())
        .all(|source| source.column() != "status"));
}

#[test]
fn expression_lineage_contains_every_contributing_source_column() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.subtotal + o.tax AS gross FROM sales.orders AS o",
        "generic",
        &dialect,
    )
    .expect("expression lineage should be analyzed");

    let lineage = first_query(&protocol).output().columns()[0].lineage();
    assert_eq!(lineage.len(), 2);
    assert_eq!(lineage[0].relation(), "sales.orders");
    assert_eq!(lineage[0].column(), "subtotal");
    assert_eq!(lineage[1].relation(), "sales.orders");
    assert_eq!(lineage[1].column(), "tax");
}

#[test]
fn cte_and_derived_table_lineage_resolve_to_physical_columns() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH recent AS (
            SELECT o.id AS order_id, o.customer_id
            FROM raw.orders AS o
        )
        SELECT r.order_id, c.name
        FROM recent AS r
        JOIN (
            SELECT customer_id, name
            FROM raw.customers
        ) AS c ON r.customer_id = c.customer_id",
        "generic",
        &dialect,
    )
    .expect("local relation lineage should resolve recursively");

    let columns = first_query(&protocol).output().columns();
    assert_eq!(columns.len(), 2);

    assert_eq!(columns[0].lineage().len(), 1);
    assert_eq!(columns[0].lineage()[0].relation(), "raw.orders");
    assert_eq!(columns[0].lineage()[0].column(), "id");

    assert_eq!(columns[1].lineage().len(), 1);
    assert_eq!(columns[1].lineage()[0].relation(), "raw.customers");
    assert_eq!(columns[1].lineage()[0].column(), "name");
}

#[test]
fn aggregate_and_window_outputs_retain_argument_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT SUM(o.amount) AS total_amount,
                SUM(o.amount) OVER () AS window_total
         FROM sales.orders AS o",
        "generic",
        &dialect,
    )
    .expect("aggregate and window expressions should preserve source lineage");

    let columns = first_query(&protocol).output().columns();
    assert_eq!(columns.len(), 2);

    for column in columns {
        assert_eq!(column.lineage().len(), 1);
        assert_eq!(column.lineage()[0].relation(), "sales.orders");
        assert_eq!(column.lineage()[0].column(), "amount");
    }
}

#[test]
fn wildcard_output_remains_explicitly_unresolved() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT * FROM sales.orders", "generic", &dialect)
        .expect("wildcard query should remain representable");

    let statement = first_query(&protocol);
    let column = &statement.output().columns()[0];
    assert_eq!(column.name(), "*");
    assert!(matches!(column.expression(), Expression::Unknown(_)));
    assert!(column.lineage().is_empty());
    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Output && diagnostic.code() == "unresolved_wildcard"
    }));
}

#[test]
fn ambiguous_unqualified_output_lineage_is_not_guessed() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT id FROM users AS u JOIN orders AS o ON u.id = o.user_id",
        "generic",
        &dialect,
    )
    .expect("ambiguous output should remain representable");

    let statement = first_query(&protocol);
    assert!(statement.output().columns()[0].lineage().is_empty());
    assert!(statement.diagnostics().iter().any(|diagnostic| {
        diagnostic.area() == DiagnosticArea::Output
            && diagnostic.code() == "ambiguous_output_lineage"
    }));
}

#[test]
fn output_json_contains_projection_expression_and_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.id AS order_id FROM sales.orders AS o",
        "generic",
        &dialect,
    )
    .expect("output should serialize");

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(
        json["inputs"][0]["statements"][0]["output"]["columns"][0]["name"],
        "order_id"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["output"]["columns"][0]["expression"]["name"],
        "id"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["output"]["columns"][0]["lineage"][0]["relation"],
        "sales.orders"
    );
    assert_eq!(
        json["inputs"][0]["statements"][0]["output"]["columns"][0]["lineage"][0]["column"],
        "id"
    );
}
