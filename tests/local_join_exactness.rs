//! Regression checks for physical join-equality exactness through local relations.
use std::collections::BTreeSet;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, AnalysisBundle, ComposedSemantics,
    ConfiguredSqlInput, ProtocolStatement, RelationCatalog, RelationSchema,
    ResolvedComposedSemantics, SchemaColumn, SqlInput,
};

type PhysicalColumn = (String, String);
type PhysicalEquality = (PhysicalColumn, PhysicalColumn);

fn schema(relation: &str, columns: &[&str]) -> RelationSchema {
    RelationSchema::new(
        relation,
        columns
            .iter()
            .map(|column| {
                SchemaColumn::from_sql_type(*column, "BIGINT", "postgresql")
                    .expect("supported integer datatype")
            })
            .collect(),
    )
    .expect("valid relation schema")
}

fn analyzed(sql: &str) -> AnalysisBundle {
    let catalog = RelationCatalog::from_schemas(&[
        schema("raw.orders", &["order_id", "product_id", "qty"]),
        schema("raw.items", &["order_id", "product_id", "qty"]),
        schema("raw.products", &["product_id", "price"]),
    ])
    .expect("typed source catalog");
    let dialect = dialect_from_name("postgresql").expect("PostgreSQL dialect");
    let input = SqlInput::inline(sql);
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "local-join-exactness",
            &input,
            "postgresql",
            dialect.as_ref(),
        )],
        &catalog,
    )
    .unwrap_or_else(|error| panic!("failed to analyze {sql}: {error}"))
}

fn resolved(bundle: &AnalysisBundle) -> &ResolvedComposedSemantics {
    match bundle.layers().last().expect("query layer").composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics,
        other => panic!("expected composed semantics: {other:?}"),
    }
}

fn equalities(semantics: &ResolvedComposedSemantics) -> BTreeSet<PhysicalEquality> {
    semantics
        .join_equalities()
        .iter()
        .map(|equality| {
            let left = (
                equality.left().relation().to_string(),
                equality.left().column().to_string(),
            );
            let right = (
                equality.right().relation().to_string(),
                equality.right().column().to_string(),
            );
            if left <= right {
                (left, right)
            } else {
                (right, left)
            }
        })
        .collect()
}

fn assert_equivalent_to_inlined(inlined: &str, variants: &[(&str, &str)], count: usize) {
    let baseline = analyzed(inlined);
    let expected = resolved(&baseline);
    assert!(
        expected.condition_exactness().is_exact(),
        "inlined query unexpectedly residual: {inlined}: {:?}",
        expected.condition_exactness().residual_conditions()
    );
    assert_eq!(equalities(expected).len(), count);
    assert!(
        !expected.column_domains().is_empty(),
        "baseline must constrain source columns"
    );

    for (location, sql) in variants {
        let bundle = analyzed(sql);
        let actual = resolved(&bundle);
        assert!(
            actual.condition_exactness().is_exact(),
            "{location} should be exact: {sql}: {:?}",
            actual.condition_exactness().residual_conditions()
        );
        assert_eq!(
            equalities(actual),
            equalities(expected),
            "{location} changed physical join equalities: {sql}"
        );
        assert_eq!(
            actual.join_equalities().len(),
            count,
            "{location} emitted duplicate join equalities: {sql}"
        );
        assert_eq!(
            actual.column_domains(),
            expected.column_domains(),
            "{location} changed physical source domains: {sql}"
        );
    }
}

#[test]
fn typed_cte_nested_cte_and_derived_joins_match_inlined_conditions() {
    let inlined = "SELECT o.order_id FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id WHERE o.order_id > 1 AND i.qty <= 3";
    let variants = [
        (
            "cte_with_inner_join",
            "WITH joined AS (SELECT o.order_id, i.qty FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id WHERE o.order_id > 1 AND i.qty <= 3) SELECT order_id FROM joined",
        ),
        (
            "cte_joining_two_ctes",
            "WITH o AS (SELECT order_id FROM raw.orders WHERE order_id > 1), i AS (SELECT order_id, qty FROM raw.items WHERE qty <= 3) SELECT o.order_id FROM o JOIN i ON o.order_id = i.order_id",
        ),
        (
            "chained_ctes",
            "WITH o AS (SELECT order_id FROM raw.orders WHERE order_id > 1), i AS (SELECT order_id, qty FROM raw.items WHERE qty <= 3), joined AS (SELECT o.order_id FROM o JOIN i ON o.order_id = i.order_id) SELECT order_id FROM joined",
        ),
        (
            "nested_ctes",
            "WITH joined AS (WITH o AS (SELECT order_id FROM raw.orders WHERE order_id > 1), i AS (SELECT order_id, qty FROM raw.items WHERE qty <= 3) SELECT o.order_id FROM o JOIN i ON o.order_id = i.order_id) SELECT order_id FROM joined",
        ),
        (
            "derived_table",
            "SELECT d.order_id FROM (SELECT o.order_id, i.qty FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id WHERE o.order_id > 1 AND i.qty <= 3) d",
        ),
    ];

    assert_equivalent_to_inlined(inlined, &variants, 1);
}

#[test]
fn typed_daily_revenue_cte_chain_keeps_both_equalities_and_integer_domains() {
    let inlined = "
        SELECT o.order_id,
               CASE WHEN SUM(i.qty * p.price) > 0 THEN SUM(i.qty * p.price) ELSE 0 END AS revenue
        FROM raw.orders o
        JOIN raw.items i ON o.order_id = i.order_id
        JOIN raw.products p ON i.product_id = p.product_id
        WHERE o.order_id > 1 AND i.qty <= 3 AND p.price > 0
        GROUP BY o.order_id
    ";
    let chained = "
        WITH orders AS (SELECT order_id FROM raw.orders WHERE order_id > 1),
             items AS (SELECT order_id, product_id, qty FROM raw.items WHERE qty <= 3),
             products AS (SELECT product_id, price FROM raw.products WHERE price > 0),
             joined AS (
                SELECT o.order_id, i.qty, p.price FROM orders o
                JOIN items i ON o.order_id = i.order_id
                JOIN products p ON i.product_id = p.product_id
             )
        SELECT order_id,
               CASE WHEN SUM(qty * price) > 0 THEN SUM(qty * price) ELSE 0 END AS revenue
        FROM joined
        GROUP BY order_id
    ";
    assert_equivalent_to_inlined(inlined, &[("daily_revenue", chained)], 2);
}

#[test]
fn computed_aggregate_and_ambiguous_join_columns_remain_residual() {
    for (kind, sql, expected_message) in [
        (
            "computed",
            "WITH x AS (SELECT order_id + 1 AS id FROM raw.orders) SELECT x.id FROM x JOIN raw.items i ON x.id = i.order_id",
            "computed",
        ),
        (
            "aggregate",
            "WITH x AS (SELECT SUM(order_id) AS id FROM raw.orders) SELECT x.id FROM x JOIN raw.items i ON x.id = i.order_id",
            "computed",
        ),
        (
            "ambiguous",
            "WITH x AS (SELECT order_id AS id, product_id AS id FROM raw.orders) SELECT x.id FROM x JOIN raw.items i ON x.id = i.order_id",
            "ambiguous",
        ),
    ] {
        let bundle = analyzed(sql);
        let semantics = resolved(&bundle);
        assert!(
            !semantics.condition_exactness().is_exact(),
            "{kind} local join must not claim exactness: {sql}"
        );
        assert!(
            semantics.join_equalities().is_empty(),
            "{kind} unmappable local join must not invent a physical equality"
        );
        let query = match bundle.inputs()[0].statements().first() {
            Some(ProtocolStatement::Query(query)) => query,
            other => panic!("expected a query, got {other:?}"),
        };
        assert!(
            query.diagnostics().iter().any(|diagnostic| {
                diagnostic.code() == "unresolved_join_column_lineage"
                    && diagnostic.message().contains("id")
                    && diagnostic.message().contains(expected_message)
            }),
            "{kind} must name the unmappable column and cause: {:?}",
            query.diagnostics()
        );
    }
}

#[test]
fn supported_equality_is_not_residual_when_another_on_condition_is_unsupported() {
    let sql = "SELECT o.order_id FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id AND o.qty = i.qty + 1 WHERE o.order_id > 1";
    let bundle = analyzed(sql);
    let semantics = resolved(&bundle);
    assert_eq!(
        equalities(semantics),
        BTreeSet::from([(
            ("raw.items".to_string(), "order_id".to_string()),
            ("raw.orders".to_string(), "order_id".to_string()),
        )])
    );
    assert!(
        !semantics.condition_exactness().is_exact(),
        "computed ON condition must remain residual"
    );
    assert!(
        semantics
            .condition_exactness()
            .residual_conditions()
            .iter()
            .all(|condition| condition.identity() != "join_equality:join:0:0"),
        "the mapped equality cannot also be reported as residual"
    );
}
