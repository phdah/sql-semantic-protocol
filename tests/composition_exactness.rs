use sql_semantic_protocol::{
    analyze_inputs, analyze_sql, AnalysisBundle, ComposedSemantics, ProtocolStatement,
    ResidualConditionReason, ResolvedComposedSemantics, SqlInput, TransformationLayer, ValueDomain,
};
use sqlparser::dialect::GenericDialect;

fn first_query(
    protocol: &sql_semantic_protocol::Protocol,
) -> &sql_semantic_protocol::QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

fn resolved(layer: &TransformationLayer) -> &ResolvedComposedSemantics {
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics,
        other => panic!("expected resolved composed semantics, got {other:?}"),
    }
}

fn layer_for_relation<'a>(bundle: &'a AnalysisBundle, relation: &str) -> &'a TransformationLayer {
    bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|dataset| dataset.relation_name() == Some(relation))
        })
        .expect("expected relation-producing layer")
}

#[test]
fn unknown_domains_survive_local_relation_intersections() {
    let dialect = GenericDialect {};

    for sql in [
        "WITH x AS (SELECT a - 10 AS b FROM t WHERE a > 3) SELECT b FROM x WHERE b BETWEEN 0 AND 5",
        "WITH x AS (SELECT SUM(a) AS b FROM t WHERE a > 3) SELECT b FROM x WHERE b BETWEEN 0 AND 5",
        "WITH x AS (SELECT ROW_NUMBER() OVER (ORDER BY a) AS b FROM t WHERE a > 3) SELECT b FROM x WHERE b BETWEEN 1 AND 5",
        "WITH x AS (SELECT CASE WHEN a > 10 THEN 1 ELSE 2 END AS b FROM t WHERE a > 3) SELECT b FROM x WHERE b = 1",
        "WITH x AS (SELECT a - 10 AS b FROM t WHERE a > 3), y AS (SELECT b FROM x WHERE b BETWEEN 0 AND 5) SELECT b FROM y",
        "SELECT b FROM (SELECT a - 10 AS b FROM t WHERE a > 3) d WHERE b BETWEEN 0 AND 5",
    ] {
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], "generic", &dialect)
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        let semantics = resolved(bundle.layers().first().expect("query layer"));

        assert!(
            semantics
                .column_domains()
                .iter()
                .any(|domain| matches!(domain.domain(), ValueDomain::Unknown(_))),
            "computed local predicate must leave an unknown source domain: {sql}"
        );
        assert!(
            !semantics.condition_exactness().is_exact(),
            "computed local predicate must be residual: {sql}"
        );
        assert!(
            matches!(semantics.output().columns()[0].domain(), ValueDomain::Unknown(_)),
            "computed local output domain must remain conservative: {sql}"
        );
    }
}

#[test]
fn unused_ctes_do_not_contribute_semantics_or_diagnostics() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "WITH unused AS (SELECT * FROM ghost) SELECT a FROM t",
        )],
        "generic",
        &dialect,
    )
    .expect("query with unused CTE should analyze");
    let layer = bundle.layers().first().expect("query layer");
    let query = layer.query().expect("query layer should expose its query");
    let semantics = resolved(layer);

    assert_eq!(query.dependencies(), &["t".to_string()]);
    assert!(query.joins().is_empty());
    assert!(semantics.condition_exactness().is_exact());
    assert!(query
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.code() != "unresolved_wildcard"));
    assert!(semantics.diagnostics().is_empty());
}

#[test]
fn safe_local_relations_match_their_inlined_form() {
    let dialect = GenericDialect {};
    let local = analyze_sql(
        "WITH x AS (SELECT a FROM t WHERE a > 3) SELECT a FROM x WHERE a < 5",
        "generic",
        &dialect,
    )
    .expect("local query should analyze");
    let inlined = analyze_sql("SELECT a FROM t WHERE a > 3 AND a < 5", "generic", &dialect)
        .expect("inlined query should analyze");

    let local = first_query(&local);
    let inlined = first_query(&inlined);
    assert_eq!(local.column_domains(), inlined.column_domains());
    assert_eq!(
        local.condition_exactness().status(),
        inlined.condition_exactness().status()
    );
}

#[test]
fn composed_exactness_includes_upstream_layer_origins() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE VIEW stage AS SELECT a FROM t WHERE a = 1 OR b = 2"),
            SqlInput::inline("SELECT a FROM stage"),
        ],
        "generic",
        &dialect,
    )
    .expect("pipeline should analyze");

    let stage = layer_for_relation(&bundle, "stage");
    let final_layer = bundle.layers().last().expect("final layer");
    let semantics = resolved(final_layer);
    let residual = semantics
        .condition_exactness()
        .residual_conditions()
        .iter()
        .find(|residual| residual.reason() == ResidualConditionReason::CrossColumnDisjunction)
        .expect("upstream residual should be composed");

    assert_eq!(residual.origin_layer_id(), Some(stage.id()));
    assert_eq!(residual.origin_scope(), Some("query"));
}

#[test]
fn composed_exactness_identifies_local_cte_scope() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "WITH x AS (SELECT a FROM t WHERE a = 1 OR b = 2) SELECT a FROM x",
        )],
        "generic",
        &dialect,
    )
    .expect("CTE query should analyze");

    let layer = bundle.layers().first().expect("query layer");
    let semantics = resolved(layer);
    let residual = semantics
        .condition_exactness()
        .residual_conditions()
        .iter()
        .find(|residual| {
            residual.reason() == ResidualConditionReason::CrossColumnDisjunction
                && residual.origin_scope() == Some("cte:x")
        })
        .expect("CTE residual should keep its local scope");

    assert_eq!(residual.origin_layer_id(), Some(layer.id()));
}
