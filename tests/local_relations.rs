mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, parse_dbt_catalog,
    parse_dbt_manifest, CaseSourceDomains, ComparisonOperator, ConfiguredSqlInput, Expression,
    LiteralValue, Predicate, ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn,
    SqlInput, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

fn assert_number_range(
    domain: &ValueDomain,
    lower: Option<(&str, bool)>,
    upper: Option<(&str, bool)>,
) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected ranges domain, got {domain:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one range, got {ranges:?}");
    };

    match (range.lower(), lower) {
        (Some(actual), Some((value, inclusive))) => {
            assert_eq!(
                actual.value().value(),
                &LiteralValue::Number(value.to_string())
            );
            assert_eq!(actual.inclusive(), inclusive);
        }
        (None, None) => {}
        other => panic!("unexpected lower bound {other:?}"),
    }
    match (range.upper(), upper) {
        (Some(actual), Some((value, inclusive))) => {
            assert_eq!(
                actual.value().value(),
                &LiteralValue::Number(value.to_string())
            );
            assert_eq!(actual.inclusive(), inclusive);
        }
        (None, None) => {}
        other => panic!("unexpected upper bound {other:?}"),
    }
}

fn first_query(
    protocol: &sql_semantic_protocol::Protocol,
) -> &sql_semantic_protocol::QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

fn assert_join_equality(
    join: &sql_semantic_protocol::Join,
    left_relation: &str,
    left_column: &str,
    right_relation: &str,
    right_column: &str,
) {
    let Some(Predicate::Comparison(comparison)) = join.condition() else {
        panic!(
            "expected equality comparison join condition, got {:?}",
            join.condition()
        );
    };
    assert_eq!(comparison.operator(), ComparisonOperator::Eq);

    let Expression::Column(left) = comparison.left() else {
        panic!(
            "expected physical left join column, got {:?}",
            comparison.left()
        );
    };
    let Expression::Column(right) = comparison.right() else {
        panic!(
            "expected physical right join column, got {:?}",
            comparison.right()
        );
    };

    assert_eq!(left.relation(), Some(left_relation));
    assert_eq!(left.name(), left_column);
    assert_eq!(right.relation(), Some(right_relation));
    assert_eq!(right.name(), right_column);
}

#[test]
fn cte_filter_domains_and_lineage_resolve_to_physical_columns() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH x AS (SELECT a FROM t WHERE a > 1000) SELECT a FROM x",
        "generic",
        &dialect,
    )
    .expect("CTE query should analyze");
    let query = first_query(&protocol);

    let domain = query
        .column_domains()
        .iter()
        .find(|domain| domain.column().relation() == Some("t") && domain.column().name() == "a")
        .expect("CTE filter should constrain t.a");
    assert_number_range(domain.domain(), Some(("1000", false)), None);

    let [source] = query.output().columns()[0].lineage() else {
        panic!("CTE output should have one physical lineage source");
    };
    assert_eq!(source.relation(), "t");
    assert_eq!(source.column(), "a");
    assert_number_range(
        query.output().columns()[0].domain(),
        Some(("1000", false)),
        None,
    );
}

#[test]
fn chained_cte_filters_intersect_on_the_physical_column() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH x AS (SELECT a FROM t WHERE a > 1000), y AS (SELECT a FROM x WHERE a < 2000) SELECT a FROM y",
        "generic",
        &dialect,
    )
    .expect("chained CTE query should analyze");
    let query = first_query(&protocol);

    let domain = query
        .column_domains()
        .iter()
        .find(|domain| domain.column().relation() == Some("t") && domain.column().name() == "a")
        .expect("chained CTE filters should constrain t.a");
    assert_number_range(
        domain.domain(),
        Some(("1000", false)),
        Some(("2000", false)),
    );
    assert_number_range(
        query.output().columns()[0].domain(),
        Some(("1000", false)),
        Some(("2000", false)),
    );
}

#[test]
fn derived_table_filters_propagate_to_physical_columns() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT d.a FROM (SELECT a FROM t WHERE a >= 10) d",
        "generic",
        &dialect,
    )
    .expect("derived table query should analyze");
    let query = first_query(&protocol);

    let domain = query
        .column_domains()
        .iter()
        .find(|domain| domain.column().relation() == Some("t") && domain.column().name() == "a")
        .expect("derived-table filter should constrain t.a");
    assert_number_range(domain.domain(), Some(("10", true)), None);
    assert_eq!(query.output().columns()[0].lineage()[0].relation(), "t");
}

#[test]
fn outer_filter_on_plain_derived_column_maps_to_physical_source() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT d.a FROM (SELECT a FROM t) AS d WHERE d.a > 5",
        "generic",
        &dialect,
    )
    .expect("derived-table outer filter should analyze");
    let query = first_query(&protocol);

    let domain = query
        .column_domains()
        .iter()
        .find(|domain| domain.column().relation() == Some("t") && domain.column().name() == "a")
        .expect("outer derived-table filter should constrain t.a");
    assert_number_range(domain.domain(), Some(("5", false)), None);
    assert!(query
        .column_domains()
        .iter()
        .all(|domain| domain.column().relation() != Some("subquery")));
}

#[test]
fn unsupported_cte_predicates_are_reported_explicitly() {
    let dialect = GenericDialect {};
    let cases = [
        (
            "exists",
            "WITH x AS (
                SELECT a FROM t
                WHERE EXISTS (SELECT 1 FROM u WHERE u.id = t.a)
             )
             SELECT a FROM x",
        ),
        (
            "in subquery",
            "WITH x AS (
                SELECT a FROM t
                WHERE a IN (SELECT b FROM u)
             )
             SELECT a FROM x",
        ),
        (
            "logical or",
            "WITH x AS (
                SELECT a FROM t
                WHERE a > 5 OR a < 0
             )
             SELECT a FROM x",
        ),
        (
            "aggregate having",
            "WITH x AS (
                SELECT a FROM t
                GROUP BY a
                HAVING SUM(a) > 5
             )
             SELECT a FROM x",
        ),
        (
            "window qualify",
            "WITH x AS (
                SELECT a, ROW_NUMBER() OVER (ORDER BY a) AS rn
                FROM t
                QUALIFY rn = 1
             )
             SELECT a FROM x",
        ),
    ];

    for (label, sql) in cases {
        let protocol = analyze_sql(sql, "generic", &dialect)
            .unwrap_or_else(|error| panic!("{label} CTE should analyze: {error}"));
        let query = first_query(&protocol);
        assert!(
            query
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "unresolved_local_predicate"),
            "{label} CTE must report the local predicate instead of dropping it"
        );
    }
}

#[test]
fn unsupported_derived_table_predicates_are_reported_explicitly() {
    let dialect = GenericDialect {};
    let cases = [
        (
            "exists",
            "SELECT d.a
             FROM (
                 SELECT a FROM t
                 WHERE EXISTS (SELECT 1 FROM u WHERE u.id = t.a)
             ) AS d",
        ),
        (
            "in subquery",
            "SELECT d.a
             FROM (
                 SELECT a FROM t
                 WHERE a IN (SELECT b FROM u)
             ) AS d",
        ),
        (
            "logical or",
            "SELECT d.a
             FROM (
                 SELECT a FROM t
                 WHERE a > 5 OR a < 0
             ) AS d",
        ),
        (
            "aggregate having",
            "SELECT d.a
             FROM (
                 SELECT a FROM t
                 GROUP BY a
                 HAVING SUM(a) > 5
             ) AS d",
        ),
        (
            "window qualify",
            "SELECT d.a
             FROM (
                 SELECT a, ROW_NUMBER() OVER (ORDER BY a) AS rn
                 FROM t
                 QUALIFY rn = 1
             ) AS d",
        ),
    ];

    for (label, sql) in cases {
        let protocol = analyze_sql(sql, "generic", &dialect)
            .unwrap_or_else(|error| panic!("{label} derived table should analyze: {error}"));
        let query = first_query(&protocol);
        assert!(
            query
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "unresolved_local_predicate"),
            "{label} derived-table predicate must be reported instead of dropped"
        );
    }
}

#[test]
fn reducible_local_predicates_do_not_emit_unresolved_diagnostics() {
    let dialect = GenericDialect {};
    for sql in [
        "WITH x AS (SELECT a FROM t WHERE a > 5 AND a < 10) SELECT a FROM x",
        "SELECT d.a FROM (SELECT a FROM t WHERE a BETWEEN 5 AND 10) AS d",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect)
            .unwrap_or_else(|error| panic!("reducible local predicate should analyze: {error}"));
        assert!(first_query(&protocol)
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != "unresolved_local_predicate"));
    }
}

#[test]
fn local_predicate_diagnostics_are_shared_across_supported_dialects() {
    let sql =
        "WITH x AS (SELECT a FROM t WHERE a > 5 OR a < 0) SELECT a FROM x";

    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name)
            .unwrap_or_else(|| panic!("dialect {dialect_name} should resolve"));
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} should analyze: {error}"));
        assert!(
            first_query(&protocol)
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "unresolved_local_predicate"),
            "dialect {dialect_name}"
        );
    }
}

#[test]
fn joins_inside_ctes_resolve_equality_columns_to_physical_sources() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH x AS (
            SELECT o.id, o.amount, c.region
            FROM orders o
            JOIN customers c ON o.customer_id = c.id
            WHERE o.amount BETWEEN 10 AND 20
         )
         SELECT id, amount, region FROM x",
        "generic",
        &dialect,
    )
    .expect("CTE join should analyze");
    let query = first_query(&protocol);

    assert_eq!(query.joins().len(), 1);
    assert_eq!(query.joins()[0].left().relation(), "orders");
    assert_eq!(query.joins()[0].right().relation(), "customers");
    assert_join_equality(
        &query.joins()[0],
        "orders",
        "customer_id",
        "customers",
        "id",
    );
    assert_eq!(
        query.output().columns()[0].lineage()[0].relation(),
        "orders"
    );
    assert_eq!(
        query.output().columns()[2].lineage()[0].relation(),
        "customers"
    );
    assert_number_range(
        query.output().columns()[1].domain(),
        Some(("10", true)),
        Some(("20", true)),
    );
}

#[test]
fn catalog_schema_expands_wildcards_through_ctes() {
    let dialect = GenericDialect {};
    let input = SqlInput::inline("WITH base AS (SELECT * FROM raw.orders) SELECT * FROM base");
    let configured = [ConfiguredSqlInput::new(
        "model.demo.orders",
        &input,
        "generic",
        &dialect,
    )];
    let schema = RelationSchema::new(
        "raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "generic").expect("id type"),
            SchemaColumn::from_sql_type("amount", "INTEGER", "generic").expect("amount type"),
        ],
    )
    .expect("schema");
    let catalog = RelationCatalog::from_schemas(&[schema]).expect("catalog");

    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("catalog-backed query should analyze");
    let query = match &bundle.inputs()[0].statements()[0] {
        ProtocolStatement::Query(query) => query,
        other => panic!("expected query statement, got {other:?}"),
    };

    assert_eq!(
        query
            .output()
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["id", "amount"]
    );
    assert!(query
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.code() != "unresolved_wildcard"));
    assert!(query
        .output()
        .columns()
        .iter()
        .all(|column| column.lineage()[0].relation() == "raw.orders"));
}

#[test]
fn dbt_artifacts_preserve_cte_domains_and_expand_wildcards() {
    let manifest = parse_dbt_manifest(
        r#"{
          "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "postgres"
          },
          "nodes": {
            "model.demo.orders": {
              "unique_id": "model.demo.orders",
              "resource_type": "model",
              "relation_name": "warehouse.analytics.orders",
              "database": "warehouse",
              "schema": "analytics",
              "original_file_path": "models/orders.sql",
              "language": "sql",
              "raw_code": "select from source",
              "compiled_code": "WITH base AS (SELECT * FROM warehouse.raw.orders WHERE amount > 10) SELECT * FROM base WHERE amount < 20",
              "depends_on": {"nodes": ["source.demo.orders"]}
            }
          },
          "sources": {
            "source.demo.orders": {
              "unique_id": "source.demo.orders",
              "resource_type": "source",
              "relation_name": "warehouse.raw.orders",
              "database": "warehouse",
              "schema": "raw",
              "name": "orders"
            }
          }
        }"#,
    )
    .expect("manifest");
    let catalog = parse_dbt_catalog(
        r#"{
          "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
          },
          "nodes": {
            "model.demo.orders": {
              "unique_id": "model.demo.orders",
              "metadata": {"type": "VIEW", "database": "warehouse", "schema": "analytics", "name": "orders"},
              "columns": {
                "id": {"name": "id", "type": "BIGINT", "index": 1},
                "amount": {"name": "amount", "type": "INTEGER", "index": 2}
              },
              "stats": {}
            }
          },
          "sources": {
            "source.demo.orders": {
              "unique_id": "source.demo.orders",
              "metadata": {"type": "BASE TABLE", "database": "warehouse", "schema": "raw", "name": "orders"},
              "columns": {
                "id": {"name": "id", "type": "BIGINT", "index": 1},
                "amount": {"name": "amount", "type": "INTEGER", "index": 2}
              },
              "stats": {}
            }
          },
          "errors": null
        }"#,
    )
    .expect("catalog");
    let dialect = dialect_from_str("postgres").expect("postgres dialect");
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect("dbt artifacts should analyze");

    let query = match &bundle.inputs()[0].statements()[0] {
        ProtocolStatement::Query(query) => query,
        other => panic!("expected query statement, got {other:?}"),
    };
    assert_eq!(
        query
            .output()
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["id", "amount"]
    );
    let amount = query
        .column_domains()
        .iter()
        .find(|domain| {
            domain.column().relation() == Some("warehouse.raw.orders")
                && domain.column().name() == "amount"
        })
        .expect("dbt CTE should constrain physical amount");
    assert_number_range(amount.domain(), Some(("10", false)), Some(("20", false)));
}

#[test]
fn computed_cte_filters_do_not_map_as_plain_source_constraints() {
    let dialect = GenericDialect {};

    for sql in [
        "WITH x AS (SELECT a - 10 AS b FROM t) SELECT b FROM x WHERE b BETWEEN 0 AND 5",
        "WITH x AS (SELECT SUM(a) AS total FROM t) SELECT total FROM x WHERE total > 100",
        "WITH x AS (SELECT ROW_NUMBER() OVER (ORDER BY a) AS rn FROM t) SELECT rn FROM x WHERE rn = 1",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect)
            .unwrap_or_else(|error| panic!("computed CTE should analyze: {error}"));
        let query = first_query(&protocol);
        let physical = query
            .column_domains()
            .iter()
            .find(|domain| domain.column().relation() == Some("t") && domain.column().name() == "a")
            .expect("computed local predicate should leave an explicit physical unknown");
        assert!(
            matches!(physical.domain(), ValueDomain::Unknown(_)),
            "computed CTE predicate must not be copied onto t.a: {sql}"
        );
    }
}

#[test]
fn computed_derived_table_filter_is_not_mapped_to_the_physical_column() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT d.b FROM (SELECT a - 10 AS b FROM t) AS d WHERE d.b BETWEEN 0 AND 5",
        "generic",
        &dialect,
    )
    .expect("computed derived table should analyze");
    let query = first_query(&protocol);

    assert!(query.column_domains().iter().all(|domain| {
        domain.column().relation() != Some("t")
            || domain.column().name() != "a"
            || matches!(domain.domain(), ValueDomain::Unknown(_))
    }));
}

#[test]
fn case_branch_domains_stop_at_computed_local_columns() {
    let dialect = GenericDialect {};

    for sql in [
        "WITH x AS (SELECT a - 10 AS b FROM t)
         SELECT CASE WHEN b > 0 THEN 'positive' ELSE 'other' END AS bucket FROM x",
        "SELECT CASE WHEN d.b > 0 THEN 'positive' ELSE 'other' END AS bucket
         FROM (SELECT a - 10 AS b FROM t) AS d",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect)
            .unwrap_or_else(|error| panic!("computed CASE source should analyze: {error}"));
        let Expression::Case(case_expression) =
            first_query(&protocol).output().columns()[0].expression()
        else {
            panic!("expected CASE output");
        };

        assert!(
            matches!(
                case_expression.branches()[0].source_domains(),
                CaseSourceDomains::Unknown(_)
            ),
            "CASE source domains must not cross a computed local column: {sql}"
        );
        assert!(matches!(
            case_expression.else_source_domains(),
            CaseSourceDomains::Unknown(_)
        ));
    }
}

#[test]
fn chained_cte_join_columns_resolve_through_plain_copy_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH
            orders_cte AS (
                SELECT order_id, customer_id FROM raw.orders
            ),
            customers_cte AS (
                SELECT id AS customer_id FROM raw.customers
            ),
            joined AS (
                SELECT o.order_id, c.customer_id
                FROM orders_cte o
                JOIN customers_cte c ON o.customer_id = c.customer_id
            )
         SELECT order_id, customer_id FROM joined",
        "generic",
        &dialect,
    )
    .expect("chained CTE join should analyze");
    let query = first_query(&protocol);

    assert_eq!(query.joins().len(), 1);
    assert_join_equality(
        &query.joins()[0],
        "raw.orders",
        "customer_id",
        "raw.customers",
        "id",
    );
    assert_eq!(query.dependencies(), ["raw.customers", "raw.orders"]);
}

#[test]
fn derived_table_join_columns_resolve_to_physical_sources() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT d.order_id, c.id
         FROM (
             SELECT id AS order_id, customer_id
             FROM raw.orders
         ) d
         JOIN raw.customers c ON d.customer_id = c.id",
        "generic",
        &dialect,
    )
    .expect("derived-table join should analyze");
    let query = first_query(&protocol);

    assert_eq!(query.joins().len(), 1);
    assert_join_equality(
        &query.joins()[0],
        "raw.orders",
        "customer_id",
        "raw.customers",
        "id",
    );
}

#[test]
fn computed_local_join_columns_are_explicitly_unresolved() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH x AS (
            SELECT id + 1 AS join_id
            FROM raw.orders
         )
         SELECT x.join_id, c.id
         FROM x
         JOIN raw.customers c ON x.join_id = c.id",
        "generic",
        &dialect,
    )
    .expect("computed local join should analyze");
    let query = first_query(&protocol);

    let Some(Predicate::Comparison(comparison)) = query.joins()[0].condition() else {
        panic!("expected join comparison");
    };
    assert!(matches!(comparison.left(), Expression::Unknown(_)));
    let Expression::Column(right) = comparison.right() else {
        panic!("expected physical right join column");
    };
    assert_eq!(right.relation(), Some("raw.customers"));
    assert_eq!(right.name(), "id");
    assert!(query
        .diagnostics()
        .iter()
        .any(|diagnostic| { diagnostic.code() == "unresolved_join_column_lineage" }));
}

#[test]
fn unused_cte_relation_semantics_do_not_leak() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH
            unused AS (
                SELECT a.id
                FROM ghost.a a
                JOIN ghost.b b ON a.id = b.id
                WHERE a.id > 10
            ),
            used AS (
                SELECT id FROM live.orders
            )
         SELECT id FROM used",
        "generic",
        &dialect,
    )
    .expect("unused CTE query should analyze");
    let query = first_query(&protocol);

    assert!(query.joins().is_empty());
    assert_eq!(query.dependencies(), ["live.orders"]);
    assert!(query
        .column_domains()
        .iter()
        .all(|domain| { !matches!(domain.column().relation(), Some("ghost.a" | "ghost.b")) }));
}

#[test]
fn local_join_resolution_is_shared_across_supported_dialects() {
    let sql = "WITH
        x AS (SELECT id, customer_id FROM orders),
        y AS (SELECT id FROM customers),
        joined AS (
            SELECT x.id
            FROM x
            JOIN y ON x.customer_id = y.id
        )
        SELECT id FROM joined";

    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name)
            .unwrap_or_else(|| panic!("dialect {dialect_name} should resolve"));
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} should analyze: {error}"));
        let query = first_query(&protocol);

        assert_eq!(query.joins().len(), 1, "dialect {dialect_name}");
        assert_join_equality(
            &query.joins()[0],
            "orders",
            "customer_id",
            "customers",
            "id",
        );
        assert!(
            query
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code() != "unresolved_join_column_lineage"),
            "dialect {dialect_name}"
        );
    }
}

#[test]
fn dbt_cte_chain_join_columns_resolve_to_physical_sources() {
    let manifest = parse_dbt_manifest(
        r#"{
          "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "postgres"
          },
          "nodes": {
            "model.demo.joined": {
              "unique_id": "model.demo.joined",
              "resource_type": "model",
              "relation_name": "warehouse.analytics.joined",
              "database": "warehouse",
              "schema": "analytics",
              "original_file_path": "models/joined.sql",
              "language": "sql",
              "raw_code": "compiled fixture",
              "compiled_code": "WITH orders AS (SELECT order_id FROM warehouse.raw.orders), order_items AS (SELECT order_id FROM warehouse.raw.order_items), joined AS (SELECT o.order_id FROM orders o JOIN order_items oi ON o.order_id = oi.order_id) SELECT order_id FROM joined",
              "depends_on": {
                "nodes": ["source.demo.orders", "source.demo.order_items"]
              }
            }
          },
          "sources": {
            "source.demo.orders": {
              "unique_id": "source.demo.orders",
              "resource_type": "source",
              "relation_name": "warehouse.raw.orders",
              "database": "warehouse",
              "schema": "raw",
              "name": "orders"
            },
            "source.demo.order_items": {
              "unique_id": "source.demo.order_items",
              "resource_type": "source",
              "relation_name": "warehouse.raw.order_items",
              "database": "warehouse",
              "schema": "raw",
              "name": "order_items"
            }
          }
        }"#,
    )
    .expect("manifest");
    let catalog = parse_dbt_catalog(
        r#"{
          "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
          },
          "nodes": {
            "model.demo.joined": {
              "unique_id": "model.demo.joined",
              "metadata": {
                "type": "VIEW",
                "database": "warehouse",
                "schema": "analytics",
                "name": "joined"
              },
              "columns": {
                "order_id": {"name": "order_id", "type": "BIGINT", "index": 1}
              },
              "stats": {}
            }
          },
          "sources": {
            "source.demo.orders": {
              "unique_id": "source.demo.orders",
              "metadata": {
                "type": "BASE TABLE",
                "database": "warehouse",
                "schema": "raw",
                "name": "orders"
              },
              "columns": {
                "order_id": {"name": "order_id", "type": "BIGINT", "index": 1}
              },
              "stats": {}
            },
            "source.demo.order_items": {
              "unique_id": "source.demo.order_items",
              "metadata": {
                "type": "BASE TABLE",
                "database": "warehouse",
                "schema": "raw",
                "name": "order_items"
              },
              "columns": {
                "order_id": {"name": "order_id", "type": "BIGINT", "index": 1}
              },
              "stats": {}
            }
          },
          "errors": null
        }"#,
    )
    .expect("catalog");
    let dialect = dialect_from_str("postgres").expect("postgres dialect");
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect("dbt artifacts should analyze");
    let query = match &bundle.inputs()[0].statements()[0] {
        ProtocolStatement::Query(query) => query,
        other => panic!("expected query statement, got {other:?}"),
    };

    assert_eq!(query.joins().len(), 1);
    assert_join_equality(
        &query.joins()[0],
        "warehouse.raw.orders",
        "order_id",
        "warehouse.raw.order_items",
        "order_id",
    );
}
