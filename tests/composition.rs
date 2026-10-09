mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_inputs, AnalysisBundle, CaseSourceDomains, ComposedSemantics, CompositionFailureReason,
    DatasetRef, Expression, JoinKind, LiteralValue, ResolvedComposedSemantics, SqlInput,
    TransformationLayer, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

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
        .expect("expected named transformation layer")
}

fn resolved(layer: &TransformationLayer) -> &ResolvedComposedSemantics {
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics,
        other => panic!("expected resolved composed semantics, got {other:?}"),
    }
}

#[test]
fn transitive_composition_is_consistent_across_all_exposed_dialects() {
    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let bundle = analyze_inputs(
            &[
                SqlInput::inline(
                    "CREATE TABLE stage_orders AS
                     SELECT id AS order_id, amount
                     FROM raw_orders
                     WHERE amount > 10",
                ),
                SqlInput::inline(
                    "CREATE TABLE core_orders AS
                     SELECT order_id AS final_id, amount
                     FROM stage_orders",
                ),
                SqlInput::inline(
                    "CREATE TABLE mart_orders AS
                     SELECT final_id, amount
                     FROM core_orders
                     WHERE amount < 100",
                ),
            ],
            dialect_name,
            dialect.as_ref(),
        )
        .unwrap_or_else(|error| {
            panic!("dialect {dialect_name} failed transitive composition: {error}")
        });

        let semantics = resolved(layer_for_relation(&bundle, "mart_orders"));
        assert_eq!(
            semantics.dependencies(),
            &["raw_orders".to_string()],
            "dialect {dialect_name} should resolve the same physical dependency"
        );
        assert_eq!(
            semantics.output().columns()[0].lineage()[0].relation(),
            "raw_orders",
            "dialect {dialect_name} should compose field lineage"
        );
        assert_eq!(
            semantics.output().columns()[0].lineage()[0].column(),
            "id",
            "dialect {dialect_name} should preserve rename identity"
        );
        assert_eq!(
            semantics.column_domains().len(),
            1,
            "dialect {dialect_name} should compose the same amount domain"
        );
    }
}

#[test]
fn three_stage_chain_composes_base_lineage_dependencies_and_domains() {
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline(
            "CREATE TABLE stage.orders AS
             SELECT id AS order_id, amount
             FROM raw.orders
             WHERE amount > 10",
        ),
        SqlInput::inline(
            "CREATE TABLE core.orders AS
             SELECT order_id AS final_id, amount
             FROM stage.orders
             WHERE amount < 100",
        ),
        SqlInput::inline(
            "CREATE TABLE mart.orders AS
             SELECT final_id, amount
             FROM core.orders
             WHERE final_id >= 5",
        ),
    ];

    let bundle = analyze_inputs(&inputs, "generic", &dialect).expect("chain should analyze");
    let semantics = resolved(layer_for_relation(&bundle, "mart.orders"));

    assert_eq!(semantics.dependencies(), &["raw.orders".to_string()]);
    assert!(semantics.diagnostics().is_empty());

    let columns = semantics.output().columns();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].name(), "final_id");
    assert_eq!(columns[0].lineage().len(), 1);
    assert_eq!(columns[0].lineage()[0].relation(), "raw.orders");
    assert_eq!(columns[0].lineage()[0].column(), "id");
    assert_eq!(columns[1].name(), "amount");
    assert_eq!(columns[1].lineage().len(), 1);
    assert_eq!(columns[1].lineage()[0].relation(), "raw.orders");
    assert_eq!(columns[1].lineage()[0].column(), "amount");

    let amount = semantics
        .column_domains()
        .iter()
        .find(|domain| {
            domain.column().relation() == Some("raw.orders") && domain.column().name() == "amount"
        })
        .expect("amount domain should propagate to the base column");
    let amount_ranges = match amount.domain() {
        ValueDomain::Ranges(ranges) => ranges.ranges(),
        other => panic!("expected amount range, got {other:?}"),
    };
    assert_eq!(amount_ranges.len(), 1);
    assert_eq!(
        amount_ranges[0]
            .lower()
            .expect("lower amount bound")
            .value()
            .value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(!amount_ranges[0]
        .lower()
        .expect("lower amount bound")
        .inclusive());
    assert_eq!(
        amount_ranges[0]
            .upper()
            .expect("upper amount bound")
            .value()
            .value(),
        &LiteralValue::Number("100".to_string())
    );
    assert!(!amount_ranges[0]
        .upper()
        .expect("upper amount bound")
        .inclusive());

    let id = semantics
        .column_domains()
        .iter()
        .find(|domain| {
            domain.column().relation() == Some("raw.orders") && domain.column().name() == "id"
        })
        .expect("renamed id domain should propagate to the base column");
    let id_ranges = match id.domain() {
        ValueDomain::Ranges(ranges) => ranges.ranges(),
        other => panic!("expected id range, got {other:?}"),
    };
    assert_eq!(
        id_ranges[0]
            .lower()
            .expect("id lower bound")
            .value()
            .value(),
        &LiteralValue::Number("5".to_string())
    );
    assert!(id_ranges[0].lower().expect("id lower bound").inclusive());
}

#[test]
fn composition_is_independent_of_input_order() {
    let dialect = GenericDialect {};
    let statements = [
        "CREATE TABLE stage.orders AS SELECT id AS order_id FROM raw.orders",
        "CREATE TABLE core.orders AS SELECT order_id AS final_id FROM stage.orders",
        "CREATE TABLE mart.orders AS SELECT final_id FROM core.orders WHERE final_id >= 5",
    ];

    let forward = analyze_inputs(
        &statements
            .iter()
            .map(|sql| SqlInput::inline(*sql))
            .collect::<Vec<_>>(),
        "generic",
        &dialect,
    )
    .expect("forward chain should analyze");
    let reverse = analyze_inputs(
        &statements
            .iter()
            .rev()
            .map(|sql| SqlInput::inline(*sql))
            .collect::<Vec<_>>(),
        "generic",
        &dialect,
    )
    .expect("reverse chain should analyze");

    assert_eq!(
        resolved(layer_for_relation(&forward, "mart.orders")),
        resolved(layer_for_relation(&reverse, "mart.orders"))
    );
}

#[test]
fn computed_columns_keep_lineage_but_stop_precise_domain_propagation() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT amount + 1 AS adjusted
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT adjusted
                 FROM stage.orders
                 WHERE adjusted > 10",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("computed chain should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "mart.orders"));
    assert!(semantics.column_domains().is_empty());
    assert!(semantics
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "non_invertible_column_transform"));

    let lineage = semantics.output().columns()[0].lineage();
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage[0].relation(), "raw.orders");
    assert_eq!(lineage[0].column(), "amount");
}

#[test]
fn join_outputs_preserve_multiple_transitive_sources() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT customer_id, amount
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE stage.customers AS
                 SELECT id, score
                 FROM raw.customers",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.summary AS
                 SELECT o.amount + c.score AS combined
                 FROM stage.orders AS o
                 JOIN stage.customers AS c ON o.customer_id = c.id",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("join chain should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "mart.summary"));
    assert_eq!(
        semantics.dependencies(),
        &["raw.customers".to_string(), "raw.orders".to_string()]
    );

    let lineage = semantics.output().columns()[0].lineage();
    assert_eq!(lineage.len(), 2);
    assert_eq!(lineage[0].relation(), "raw.customers");
    assert_eq!(lineage[0].column(), "score");
    assert_eq!(lineage[1].relation(), "raw.orders");
    assert_eq!(lineage[1].column(), "amount");
}

#[test]
fn disconnected_components_compose_independently() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.orders"),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
            SqlInput::inline("CREATE TABLE stage.customers AS SELECT id FROM raw.customers"),
            SqlInput::inline("CREATE TABLE mart.customers AS SELECT id FROM stage.customers"),
        ],
        "generic",
        &dialect,
    )
    .expect("disconnected chains should analyze");

    assert_eq!(
        resolved(layer_for_relation(&bundle, "mart.orders")).dependencies(),
        &["raw.orders".to_string()]
    );
    assert_eq!(
        resolved(layer_for_relation(&bundle, "mart.customers")).dependencies(),
        &["raw.customers".to_string()]
    );
}

#[test]
fn ambiguous_producers_leave_composition_explicitly_unresolved() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.one"),
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.two"),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
        ],
        "generic",
        &dialect,
    )
    .expect("ambiguous graph should remain analyzable");

    match layer_for_relation(&bundle, "mart.orders").composed_semantics() {
        ComposedSemantics::Unresolved(semantics) => {
            assert_eq!(
                semantics.reason(),
                CompositionFailureReason::AmbiguousProducer
            );
            assert!(semantics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "ambiguous_relation_producer"));
        }
        other => panic!("expected unresolved composition, got {other:?}"),
    }
}

#[test]
fn bare_query_remains_a_resolved_anonymous_outcome() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline("SELECT id FROM raw.orders")],
        "generic",
        &dialect,
    )
    .expect("bare query should analyze");

    assert!(matches!(
        bundle.layers()[0].produces(),
        [DatasetRef::Anonymous { .. }]
    ));
    assert!(matches!(
        bundle.layers()[0].composed_semantics(),
        ComposedSemantics::Resolved(_)
    ));
}

#[test]
fn case_branch_domains_compose_to_physical_sources_and_survive_copy_layers() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT amount AS value
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT CASE WHEN value > 10 THEN 'high' ELSE 'low' END AS bucket
                 FROM stage.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE final.orders AS
                 SELECT bucket
                 FROM mart.orders",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("CASE composition chain should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "final.orders"));
    let Expression::Case(case_expression) = semantics.output().columns()[0].expression() else {
        panic!("copied composed output should preserve CASE expression");
    };
    let CaseSourceDomains::Reachable { alternatives } =
        case_expression.branches()[0].source_domains()
    else {
        panic!("expected reachable composed CASE branch");
    };
    let [alternative] = alternatives.as_slice() else {
        panic!("expected one CASE branch alternative");
    };
    let [domain] = alternative.column_domains() else {
        panic!("expected one CASE branch source domain");
    };
    assert_eq!(domain.column().relation(), Some("raw.orders"));
    assert_eq!(domain.column().name(), "amount");

    let CaseSourceDomains::Reachable {
        alternatives: else_alternatives,
    } = case_expression.else_source_domains()
    else {
        panic!("expected reachable composed CASE ELSE");
    };
    assert!(else_alternatives
        .iter()
        .flat_map(|alternative| alternative.column_domains())
        .all(|domain| domain.column().relation() == Some("raw.orders")));
}

#[test]
fn computed_composition_hop_makes_case_branch_domains_unknown() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT amount + 1 AS adjusted
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT CASE WHEN adjusted > 10 THEN 'high' ELSE 'low' END AS bucket
                 FROM stage.orders",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("computed CASE composition should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "mart.orders"));
    let Expression::Case(case_expression) = semantics.output().columns()[0].expression() else {
        panic!("expected CASE expression");
    };
    let CaseSourceDomains::Unknown(reason) = case_expression.branches()[0].source_domains() else {
        panic!("computed composition hop should make branch domains unknown");
    };
    assert!(reason.reason().contains("non-identity expression"));
    assert!(matches!(
        case_expression.else_source_domains(),
        CaseSourceDomains::Unknown(_)
    ));
}

#[test]
fn case_branch_domain_composition_is_consistent_across_exposed_dialects() {
    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let bundle = analyze_inputs(
            &[
                SqlInput::inline(
                    "CREATE TABLE stage_orders AS
                     SELECT amount
                     FROM raw_orders",
                ),
                SqlInput::inline(
                    "CREATE TABLE mart_orders AS
                     SELECT CASE WHEN amount > 10 THEN 'high' ELSE 'low' END AS bucket
                     FROM stage_orders",
                ),
            ],
            dialect_name,
            dialect.as_ref(),
        )
        .unwrap_or_else(|error| {
            panic!("dialect {dialect_name} failed CASE domain composition: {error}")
        });

        let semantics = resolved(layer_for_relation(&bundle, "mart_orders"));
        let Expression::Case(case_expression) = semantics.output().columns()[0].expression() else {
            panic!("dialect {dialect_name} should preserve CASE expression");
        };
        let CaseSourceDomains::Reachable { alternatives } =
            case_expression.branches()[0].source_domains()
        else {
            panic!("dialect {dialect_name} should compose reachable CASE domains");
        };
        assert!(alternatives
            .iter()
            .flat_map(|alternative| alternative.column_domains())
            .all(|domain| domain.column().relation() == Some("raw_orders")));
    }
}

#[test]
fn composed_join_equalities_map_multi_column_join_through_producer_layers() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT customer_id, region, amount
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE stage.customers AS
                 SELECT id, region, score
                 FROM raw.customers",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.summary AS
                 SELECT o.amount + c.score AS combined
                 FROM stage.orders AS o
                 JOIN stage.customers AS c
                   ON o.customer_id = c.id
                  AND o.region = c.region",
            ),
            SqlInput::inline(
                "CREATE TABLE final.summary AS
                 SELECT combined
                 FROM mart.summary",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("multi-column join chain should analyze");

    let origin = layer_for_relation(&bundle, "mart.summary").id().to_string();
    let equalities = resolved(layer_for_relation(&bundle, "final.summary")).join_equalities();
    assert_eq!(equalities.len(), 2);

    assert_eq!(equalities[0].left().relation(), "raw.orders");
    assert_eq!(equalities[0].left().column(), "customer_id");
    assert_eq!(equalities[0].left().relation_instance(), "o");
    assert_eq!(equalities[0].right().relation(), "raw.customers");
    assert_eq!(equalities[0].right().column(), "id");
    assert_eq!(equalities[0].right().relation_instance(), "c");
    assert_eq!(equalities[0].join_kind(), JoinKind::Inner);
    assert_eq!(equalities[0].origin_layer_id(), origin);

    assert_eq!(equalities[1].left().relation(), "raw.orders");
    assert_eq!(equalities[1].left().column(), "region");
    assert_eq!(equalities[1].right().relation(), "raw.customers");
    assert_eq!(equalities[1].right().column(), "region");
}

#[test]
fn implicit_where_equi_join_is_a_composed_inner_equality() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE mart.links AS
             SELECT t.id
             FROM raw.t AS t, raw.u AS u
             WHERE t.id = u.id",
        )],
        "generic",
        &dialect,
    )
    .expect("implicit equi-join should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "mart.links"));
    assert!(semantics.condition_exactness().is_exact());
    assert!(semantics.column_domains().is_empty());

    let [equality] = semantics.join_equalities() else {
        panic!("expected one implicit join equality");
    };
    assert_eq!(equality.left().relation(), "raw.t");
    assert_eq!(equality.left().column(), "id");
    assert_eq!(equality.left().relation_instance(), "t");
    assert_eq!(equality.right().relation(), "raw.u");
    assert_eq!(equality.right().column(), "id");
    assert_eq!(equality.right().relation_instance(), "u");
    assert_eq!(equality.join_kind(), JoinKind::Inner);
}

#[test]
fn chained_cte_join_equalities_compose_to_physical_sources() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT customer_id, region_id
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE stage.customers AS
                 SELECT id
                 FROM raw.customers",
            ),
            SqlInput::inline(
                "CREATE TABLE stage.regions AS
                 SELECT id
                 FROM raw.regions",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.final AS
                 WITH eligible AS (
                     SELECT o.customer_id, o.region_id
                     FROM stage.orders AS o
                     JOIN stage.customers AS c ON o.customer_id = c.id
                 ),
                 enriched AS (
                     SELECT e.customer_id
                     FROM eligible AS e
                     JOIN stage.regions AS r ON e.region_id = r.id
                 )
                 SELECT customer_id
                 FROM enriched",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("dbt-style CTE join chain should analyze");

    let equalities = resolved(layer_for_relation(&bundle, "mart.final")).join_equalities();
    assert_eq!(equalities.len(), 2);
    assert!(equalities.iter().any(|equality| {
        equality.left().relation() == "raw.orders"
            && equality.left().column() == "customer_id"
            && equality.right().relation() == "raw.customers"
            && equality.right().column() == "id"
    }));
    assert!(equalities.iter().any(|equality| {
        equality.left().relation() == "raw.orders"
            && equality.left().column() == "region_id"
            && equality.right().relation() == "raw.regions"
            && equality.right().column() == "id"
    }));
}

#[test]
fn outer_and_self_join_equalities_remain_conservative() {
    let dialect = GenericDialect {};
    let outer = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE mart.outer_join AS
             SELECT t.id
             FROM raw.t AS t
             LEFT JOIN raw.u AS u ON t.id = u.id",
        )],
        "generic",
        &dialect,
    )
    .expect("outer join should analyze");
    let outer = resolved(layer_for_relation(&outer, "mart.outer_join"));
    let [equality] = outer.join_equalities() else {
        panic!("outer join should retain its equality");
    };
    assert_eq!(equality.join_kind(), JoinKind::Left);
    assert!(!outer.condition_exactness().is_exact());

    let self_join = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE mart.self_join AS
             SELECT a.id
             FROM raw.t AS a
             JOIN raw.t AS b ON a.id = b.id",
        )],
        "generic",
        &dialect,
    )
    .expect("self join should analyze");
    let self_join = resolved(layer_for_relation(&self_join, "mart.self_join"));
    let [equality] = self_join.join_equalities() else {
        panic!("self-join equality must preserve independent input instances");
    };
    assert_eq!(equality.left().relation(), "raw.t");
    assert_eq!(equality.right().relation(), "raw.t");
    assert_eq!(equality.left().relation_instance(), "a");
    assert_eq!(equality.right().relation_instance(), "b");
    assert!(!self_join.condition_exactness().is_exact());
}

#[test]
fn set_branch_evidence_survives_a_downstream_producer_layer() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage_union AS
                 SELECT id FROM raw_a WHERE id > 10
                 UNION ALL
                 SELECT id FROM raw_b WHERE id < 0",
            ),
            SqlInput::inline("CREATE TABLE mart_union AS SELECT id FROM stage_union"),
        ],
        "generic",
        &dialect,
    )
    .expect("analyze and compose set producer");

    let stage = layer_for_relation(&bundle, "stage_union");
    let mart = resolved(layer_for_relation(&bundle, "mart_union"));
    assert_eq!(mart.set_operations().len(), 1);
    let inherited = &mart.set_operations()[0];
    assert_eq!(inherited.origin_layer_id(), stage.id());
    assert_eq!(inherited.operation().branches().len(), 2);
    assert_ne!(
        inherited.operation().branches()[0].column_domains(),
        inherited.operation().branches()[1].column_domains()
    );
    assert!(!mart.condition_exactness().is_exact());

    let json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&bundle))
            .expect("valid protocol JSON");
    let layers = json["layers"].as_array().expect("layers");
    let mart_layer = layers
        .iter()
        .find(|layer| layer["id"] == layer_for_relation(&bundle, "mart_union").id())
        .expect("mart layer");
    let inherited_json = &mart_layer["composed_semantics"]["set_operations"][0];
    assert_eq!(inherited_json["origin_layer_id"], stage.id());
    assert_eq!(
        inherited_json["operation"]["membership"]["branches"][0]["identity"],
        "body:left"
    );
}
