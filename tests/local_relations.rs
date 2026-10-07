use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, parse_dbt_catalog,
    parse_dbt_manifest, CaseSourceDomains, ConfiguredSqlInput, Expression, LiteralValue,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
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
fn joins_inside_ctes_are_retained() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH x AS (
            SELECT o.id, c.region
            FROM orders o
            JOIN customers c ON o.customer_id = c.id
         )
         SELECT id, region FROM x",
        "generic",
        &dialect,
    )
    .expect("CTE join should analyze");
    let query = first_query(&protocol);

    assert_eq!(query.joins().len(), 1);
    assert_eq!(query.joins()[0].left().relation(), "orders");
    assert_eq!(query.joins()[0].right().relation(), "customers");
    assert_eq!(
        query.output().columns()[0].lineage()[0].relation(),
        "orders"
    );
    assert_eq!(
        query.output().columns()[1].lineage()[0].relation(),
        "customers"
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
        let Expression::Case(case_expression) = first_query(&protocol).output().columns()[0].expression()
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
