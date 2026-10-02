mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_sql, AggregateArgument, Expression, GroupBy, GroupingExpression, Predicate, Protocol,
    ProtocolStatement, QueryStatement,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect, PostgreSqlDialect};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn grouped_aggregate_is_supported_across_dialects() {
    let sql = "SELECT category, SUM(amount) AS total FROM sales GROUP BY category HAVING SUM(amount) > 10";
    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name).expect("documented dialect should resolve");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("{dialect_name} should analyze grouped SQL: {error}"));
        let query = first_query(&protocol);
        assert!(matches!(
            query.aggregation().and_then(|aggregation| aggregation.group_by()),
            Some(GroupBy::Expressions(expressions))
                if matches!(
                    expressions.as_slice(),
                    [GroupingExpression::Expression(Expression::Column(column))]
                        if column.name() == "category"
                )
        ));
        assert!(matches!(
            query.output().columns()[1].expression(),
            Expression::AggregateFunction(function)
                if function.name() == "SUM"
                    && matches!(
                        function.arguments(),
                        [AggregateArgument::Expression(Expression::Column(column))]
                            if column.name() == "amount"
                    )
        ));
        assert!(matches!(
            query.predicates().having_predicate(),
            Some(Predicate::Comparison(comparison))
                if matches!(comparison.left(), Expression::AggregateFunction(_))
        ));
    }
}

#[test]
fn aggregate_filter_and_lineage_are_preserved() {
    let dialect = PostgreSqlDialect {};
    let protocol = analyze_sql(
        "SELECT SUM(amount) FILTER (WHERE status = 'paid') AS paid_total FROM sales",
        "postgresql",
        &dialect,
    ).expect("aggregate FILTER should analyze");
    let query = first_query(&protocol);
    let aggregate = match query.output().columns()[0].expression() {
        Expression::AggregateFunction(function) => function,
        other => panic!("expected aggregate function, got {other:?}"),
    };
    assert!(matches!(aggregate.filter(), Some(Predicate::Comparison(_))));
    assert_eq!(
        query.output().columns()[0].lineage().iter().map(|source| source.column()).collect::<Vec<_>>(),
        vec!["amount", "status"]
    );
}

#[test]
fn distinct_and_rollup_are_typed() {
    let dialect = GenericDialect {};
    let distinct = analyze_sql("SELECT DISTINCT category FROM sales", "generic", &dialect)
        .expect("DISTINCT should analyze");
    assert!(first_query(&distinct).aggregation().expect("aggregation").distinct());

    let rollup = analyze_sql(
        "SELECT region, category, SUM(amount) FROM sales GROUP BY ROLLUP(region, category)",
        "generic",
        &dialect,
    ).expect("ROLLUP should analyze");
    assert!(matches!(
        first_query(&rollup).aggregation().and_then(|aggregation| aggregation.group_by()),
        Some(GroupBy::Expressions(expressions))
            if matches!(expressions.as_slice(), [GroupingExpression::Rollup(_)])
    ));
}

#[test]
fn count_wildcard_is_typed_and_having_is_conservative() {
    let dialect = GenericDialect {};
    let count = analyze_sql("SELECT COUNT(*) FROM sales", "generic", &dialect)
        .expect("COUNT wildcard should analyze");
    assert!(matches!(
        first_query(&count).output().columns()[0].expression(),
        Expression::AggregateFunction(function)
            if matches!(function.arguments(), [AggregateArgument::Wildcard])
    ));

    let having = analyze_sql(
        "SELECT category, SUM(amount) FROM sales GROUP BY category HAVING SUM(amount) > 10",
        "generic",
        &dialect,
    ).expect("HAVING should analyze");
    assert!(first_query(&having).column_domains().is_empty());
}
