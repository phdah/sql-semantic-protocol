mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_sql, ConditionClause, ConditionExactnessStatus, Expression, Predicate, Protocol,
    ProtocolStatement, QueryStatement, ResidualConditionReason, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected first query statement, got {other:?}"),
    }
}

fn has_residual(
    query: &QueryStatement,
    reason: ResidualConditionReason,
    clause: ConditionClause,
) -> bool {
    query
        .condition_exactness()
        .residual_conditions()
        .iter()
        .any(|residual| residual.reason() == reason && residual.clause() == clause)
}

fn analyze_generic(sql: &str) -> Protocol {
    analyze_sql(sql, "generic", &GenericDialect {})
        .unwrap_or_else(|error| panic!("generic SQL should analyze: {error}"))
}

#[test]
fn allow_listed_scalar_predicates_are_exact_across_exposed_dialects() {
    let sql = "SELECT a FROM t WHERE a > 1 AND a < 10";

    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name)
            .unwrap_or_else(|| panic!("dialect {dialect_name} should resolve"));
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("{dialect_name} should analyze shared SQL: {error}"));
        let query = first_query(&protocol);

        assert_eq!(
            query.condition_exactness().status(),
            ConditionExactnessStatus::Exact,
            "{dialect_name}"
        );
        assert!(query.condition_exactness().residual_conditions().is_empty());
    }
}

#[test]
fn same_column_disjunction_is_exact_but_cross_column_disjunction_is_residual() {
    let same_column = analyze_generic("SELECT a FROM t WHERE a = 1 OR a = 3");
    let query = first_query(&same_column);
    assert!(query.condition_exactness().is_exact());

    let ValueDomain::Set(domain) = query.column_domains()[0].domain() else {
        panic!("same-column OR should derive a finite set");
    };
    assert_eq!(domain.values().len(), 2);

    let cross_column = analyze_generic("SELECT a, b FROM t WHERE a = 1 OR b = 2");
    let query = first_query(&cross_column);
    assert!(has_residual(
        query,
        ResidualConditionReason::CrossColumnDisjunction,
        ConditionClause::Where,
    ));
    assert!(query
        .condition_exactness()
        .residual_conditions()
        .iter()
        .any(|residual| residual.identity() == "where"));
}

#[test]
fn correlated_or_tree_is_residual_even_when_each_projection_has_a_domain() {
    let protocol =
        analyze_generic("SELECT a, b FROM t WHERE (a = 1 AND b = 2) OR (a = 3 AND b = 4)");
    let query = first_query(&protocol);

    assert!(has_residual(
        query,
        ResidualConditionReason::CrossColumnDisjunction,
        ConditionClause::Where,
    ));
    assert!(query
        .column_domains()
        .iter()
        .any(|domain| domain.column().name() == "a"));
    assert!(query
        .column_domains()
        .iter()
        .any(|domain| domain.column().name() == "b"));
}

#[test]
fn computed_and_pattern_predicates_are_residual() {
    for sql in [
        "SELECT a FROM t WHERE a + 1 > 5",
        "SELECT a FROM t WHERE CAST(a AS INT) > 5",
        "SELECT name FROM t WHERE name LIKE 'x%'",
    ] {
        let protocol = analyze_generic(sql);
        let query = first_query(&protocol);
        assert!(
            has_residual(
                query,
                ResidualConditionReason::ComputedExpression,
                ConditionClause::Where,
            ),
            "{sql}"
        );
    }
}

#[test]
fn pattern_function_and_not_predicates_are_default_denied() {
    let postgres = dialect_from_str("postgres").expect("postgres dialect");
    for sql in [
        "SELECT name FROM t WHERE name LIKE 'x%'",
        "SELECT name FROM t WHERE name ILIKE 'x%'",
        "SELECT name FROM t WHERE name SIMILAR TO 'x%'",
        "SELECT name FROM t WHERE name ~ '^x'",
        "SELECT name FROM t WHERE LENGTH(name) > 2",
    ] {
        let protocol = analyze_sql(sql, "postgres", postgres.as_ref())
            .unwrap_or_else(|error| panic!("postgres should parse {sql}: {error}"));
        assert!(has_residual(
            first_query(&protocol),
            ResidualConditionReason::ComputedExpression,
            ConditionClause::Where,
        ));
    }

    let negated = analyze_generic("SELECT a FROM t WHERE NOT (a + 1 > 5)");
    assert!(has_residual(
        first_query(&negated),
        ResidualConditionReason::LogicalNot,
        ConditionClause::Where,
    ));
}

#[test]
fn non_join_column_comparisons_and_in_subqueries_are_residual() {
    let comparison = analyze_generic("SELECT a, b FROM t WHERE a = b");
    assert!(has_residual(
        first_query(&comparison),
        ResidualConditionReason::ColumnComparison,
        ConditionClause::Where,
    ));

    let in_subquery = analyze_generic("SELECT id FROM t WHERE id IN (SELECT id FROM u)");
    assert!(has_residual(
        first_query(&in_subquery),
        ResidualConditionReason::SubqueryPredicate,
        ConditionClause::Where,
    ));
}

#[test]
fn subquery_predicates_are_residual_and_nested_scope_has_its_own_contract() {
    let protocol =
        analyze_generic("SELECT id FROM t WHERE EXISTS (SELECT 1 FROM u WHERE u.id = t.id)");
    let query = first_query(&protocol);

    assert!(has_residual(
        query,
        ResidualConditionReason::SubqueryPredicate,
        ConditionClause::Where,
    ));

    let Some(Predicate::Exists(exists)) = query.predicates().where_predicate() else {
        panic!("expected EXISTS predicate");
    };
    assert_eq!(
        exists.subquery().condition_exactness().status(),
        ConditionExactnessStatus::Residual
    );
    assert!(exists
        .subquery()
        .condition_exactness()
        .residual_conditions()
        .iter()
        .any(|residual| residual.reason() == ResidualConditionReason::CorrelatedSubquery));
}

#[test]
fn scalar_subquery_scope_emits_domains_and_exactness() {
    let protocol = analyze_generic("SELECT (SELECT x FROM u WHERE x > 5) AS x FROM t");
    let query = first_query(&protocol);
    let Expression::ScalarSubquery(expression) = query.output().columns()[0].expression() else {
        panic!("expected scalar subquery output");
    };

    assert!(expression.subquery().condition_exactness().is_exact());
    assert_eq!(expression.subquery().column_domains().len(), 1);
    assert!(expression.subquery().joins().is_empty());
}

#[test]
fn having_and_qualify_are_residual_row_conditions() {
    let having = analyze_generic(
        "SELECT category, COUNT(*) AS n FROM t GROUP BY category HAVING COUNT(*) > 2",
    );
    assert!(has_residual(
        first_query(&having),
        ResidualConditionReason::Having,
        ConditionClause::Having,
    ));

    let snowflake = dialect_from_str("snowflake").expect("snowflake dialect");
    let qualify = analyze_sql(
        "SELECT a, ROW_NUMBER() OVER (ORDER BY a) AS rn FROM t QUALIFY rn = 1",
        "snowflake",
        snowflake.as_ref(),
    )
    .expect("QUALIFY should parse in snowflake");
    assert!(has_residual(
        first_query(&qualify),
        ResidualConditionReason::Qualify,
        ConditionClause::Qualify,
    ));
}

#[test]
fn group_by_without_having_is_row_set_shaping_not_a_residual_condition() {
    let protocol = analyze_generic("SELECT category, COUNT(*) FROM t GROUP BY category");
    assert!(first_query(&protocol).condition_exactness().is_exact());
}

#[test]
fn row_set_operators_are_residual_but_order_by_and_plain_distinct_are_not() {
    let ordered = analyze_generic("SELECT DISTINCT a FROM t ORDER BY a");
    assert!(first_query(&ordered).condition_exactness().is_exact());

    let limited = analyze_generic("SELECT a FROM t LIMIT 10 OFFSET 2");
    let query = first_query(&limited);
    assert!(has_residual(
        query,
        ResidualConditionReason::Limit,
        ConditionClause::RowSetOperator,
    ));
    assert!(has_residual(
        query,
        ResidualConditionReason::Offset,
        ConditionClause::RowSetOperator,
    ));

    let fetched = analyze_generic("SELECT a FROM t FETCH FIRST 1 ROW ONLY");
    assert!(has_residual(
        first_query(&fetched),
        ResidualConditionReason::Fetch,
        ConditionClause::RowSetOperator,
    ));

    let postgres = dialect_from_str("postgres").expect("postgres dialect");
    let distinct_on = analyze_sql(
        "SELECT DISTINCT ON (a) a, b FROM t ORDER BY a, b",
        "postgres",
        postgres.as_ref(),
    )
    .expect("DISTINCT ON should parse in postgres");
    assert!(has_residual(
        first_query(&distinct_on),
        ResidualConditionReason::DistinctOn,
        ConditionClause::RowSetOperator,
    ));
}

#[test]
fn table_sample_is_residual() {
    let protocol = analyze_generic("SELECT a FROM t TABLESAMPLE SYSTEM (10)");
    assert!(has_residual(
        first_query(&protocol),
        ResidualConditionReason::TableSample,
        ConditionClause::RowSetOperator,
    ));
}

#[test]
fn set_operations_are_residual() {
    for sql in [
        "SELECT a FROM t WHERE a = 1 UNION ALL SELECT a FROM u WHERE a = 2",
        "SELECT a FROM t INTERSECT SELECT a FROM u",
        "SELECT a FROM t EXCEPT SELECT a FROM u",
    ] {
        let protocol = analyze_generic(sql);
        assert!(has_residual(
            first_query(&protocol),
            ResidualConditionReason::SetOperation,
            ConditionClause::SetOperation,
        ));
    }
}

#[test]
fn inner_join_scalar_filters_are_domains_and_equality_joins_remain_exact() {
    let protocol = analyze_generic("SELECT t.x FROM t JOIN u ON t.x = u.y AND t.a > 5");
    let query = first_query(&protocol);

    assert!(query.condition_exactness().is_exact());
    let domain = query
        .column_domains()
        .iter()
        .find(|domain| domain.column().relation() == Some("t") && domain.column().name() == "a")
        .expect("inner join scalar filter should become a source domain");
    let ValueDomain::Ranges(ranges) = domain.domain() else {
        panic!("join scalar filter should derive a range");
    };
    assert_eq!(ranges.ranges().len(), 1);
}

#[test]
fn outer_joins_and_self_joins_are_default_denied() {
    let outer = analyze_generic("SELECT t.id FROM t LEFT JOIN u ON t.id = u.id");
    assert!(has_residual(
        first_query(&outer),
        ResidualConditionReason::OuterJoin,
        ConditionClause::JoinOn,
    ));

    let self_join = analyze_generic("SELECT a.id FROM t a JOIN t b ON a.id = b.id");
    assert!(has_residual(
        first_query(&self_join),
        ResidualConditionReason::RepeatedSourceInstance,
        ConditionClause::RowSetOperator,
    ));
}

#[test]
fn non_equality_column_join_is_residual() {
    let protocol = analyze_generic("SELECT t.id FROM t JOIN u ON t.score > u.score");
    assert!(has_residual(
        first_query(&protocol),
        ResidualConditionReason::ColumnComparison,
        ConditionClause::JoinOn,
    ));
}

#[test]
fn condition_affecting_diagnostics_default_to_residual() {
    let bigquery = dialect_from_str("bigquery").expect("bigquery dialect");
    let protocol = analyze_sql(
        "SELECT * FROM UNNEST([1, 2, 3]) AS value",
        "bigquery",
        bigquery.as_ref(),
    )
    .expect("UNNEST should remain analyzable");
    let query = first_query(&protocol);

    assert!(query
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_table_factor"));
    assert!(has_residual(
        query,
        ResidualConditionReason::AnalysisDiagnostic,
        ConditionClause::RowSetOperator,
    ));
}

#[test]
fn null_membership_is_explicit_for_every_domain_kind() {
    assert_eq!(ValueDomain::Unbounded.admits_null(), Some(true));

    let is_null = analyze_generic("SELECT a FROM t WHERE a IS NULL");
    assert_eq!(
        first_query(&is_null).column_domains()[0]
            .domain()
            .admits_null(),
        Some(true)
    );

    let is_not_null = analyze_generic("SELECT a FROM t WHERE a IS NOT NULL");
    assert_eq!(
        first_query(&is_not_null).column_domains()[0]
            .domain()
            .admits_null(),
        Some(false)
    );

    let not_equal = analyze_generic("SELECT a FROM t WHERE a <> 1");
    assert_eq!(
        first_query(&not_equal).column_domains()[0]
            .domain()
            .admits_null(),
        Some(false)
    );

    let not_in = analyze_generic("SELECT a FROM t WHERE a NOT IN (1, 2)");
    assert_eq!(
        first_query(&not_in).column_domains()[0]
            .domain()
            .admits_null(),
        Some(false)
    );

    let distinct = analyze_generic("SELECT a FROM t WHERE a IS DISTINCT FROM 1");
    assert_eq!(
        first_query(&distinct).column_domains()[0]
            .domain()
            .admits_null(),
        Some(true)
    );

    let range = analyze_generic("SELECT a FROM t WHERE a > 1");
    assert_eq!(
        first_query(&range).column_domains()[0]
            .domain()
            .admits_null(),
        Some(false)
    );

    let empty = analyze_generic("SELECT a FROM t WHERE a = NULL");
    assert_eq!(
        first_query(&empty).column_domains()[0]
            .domain()
            .admits_null(),
        Some(false)
    );

    let unknown = analyze_generic("SELECT a FROM t WHERE a + 1 > 5");
    let domain = unknown
        .statements()
        .first()
        .and_then(|statement| match statement {
            ProtocolStatement::Query(query) => query.column_domains().first(),
            _ => None,
        })
        .expect("computed predicate should retain an unknown domain");
    assert_eq!(domain.domain().admits_null(), None);
}

#[test]
fn emitted_exactness_is_deterministic_and_present_on_composed_semantics() {
    let first = analyze_generic("SELECT a FROM t WHERE a > 1");
    let second = analyze_generic("SELECT a FROM t WHERE a > 1");

    let first_json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_json(&first)).expect("valid JSON");
    let second_json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_json(&second)).expect("valid JSON");

    assert_eq!(first_json, second_json);
    assert_eq!(
        first_json["inputs"][0]["statements"][0]["condition_exactness"]["status"],
        "exact"
    );
    assert_eq!(
        first_json["layers"][0]["composed_semantics"]["condition_exactness"]["status"],
        "exact"
    );
}
