mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs, analyze_sql,
    parse_dbt_catalog, parse_dbt_manifest, to_json, BooleanRowConstraint, BooleanTruthCase,
    BooleanWitnessDirection, ComparisonAssumption, ComparisonOperator, ComposedSemantics,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, ConstraintValue, GroupBoundaryKind, ProtocolStatement, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect, PostgreSqlDialect};

fn query(sql: &str) -> sql_semantic_protocol::QueryStatement {
    let protocol = analyze_sql(sql, "generic", &GenericDialect {}).unwrap();
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("expected query statement");
    };
    query.clone()
}

fn typed_bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(sql);
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "typed",
        &input,
        "postgresql",
        &dialect,
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

fn text_bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "VARCHAR(8)", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "VARCHAR(8)", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(sql);
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "typed",
        &input,
        "postgresql",
        &dialect,
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

#[test]
fn binary_attested_like_prefix_preserves_null_and_correlations() {
    let mut bundle = text_bundle("SELECT a FROM t WHERE a LIKE 'ab%' OR b LIKE 'ab%'");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    assert!(matches!(
        semantics.boolean_witnesses()[0].witness().qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    bundle.declare_comparison_assumptions(&[ComparisonAssumption::BinaryCollation]);
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    assert!(matches!(
        semantics.boolean_witnesses()[0].witness().qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    bundle.declare_comparison_assumptions(&[ComparisonAssumption::NoCharPadding]);
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let BooleanRowConstraint::Any(operands) = witness.condition() else {
        panic!("expected coupled OR");
    };
    assert!(operands
        .iter()
        .all(|leaf| matches!(leaf, BooleanRowConstraint::StringPrefix { .. })));
}

#[test]
fn like_prefix_rechecks_enforced_string_constraints() {
    let mut bundle = text_bundle("SELECT a FROM t WHERE a LIKE 'ab%' OR b LIKE 'ab%'");
    bundle.declare_comparison_assumptions(&[
        ComparisonAssumption::BinaryCollation,
        ComparisonAssumption::NoCharPadding,
    ]);
    let constraints = RelationConstraintSet::new(
        "t",
        ["a", "b"]
            .iter()
            .map(|column| {
                RelationConstraint::accepted_values(
                    *column,
                    vec![ConstraintValue::String("zz".to_string())],
                    false,
                    enforced_evidence(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    bundle.enrich_relation_constraints(&[constraints]);
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn unsafe_like_patterns_are_residual_even_with_attestations() {
    for predicate in ["a LIKE 'a_%'", "a ILIKE 'ab%'", "a LIKE 'a%b%'"] {
        let mut bundle = text_bundle(&format!("SELECT a FROM t WHERE {predicate} OR b IS NULL"));
        bundle.declare_comparison_assumptions(&[
            ComparisonAssumption::BinaryCollation,
            ComparisonAssumption::NoCharPadding,
        ]);
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        assert!(
            matches!(
                semantics.boolean_witnesses()[0].witness().qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}"
        );
    }
}

#[test]
fn null_disjunction_has_jointly_evaluated_exact_truth_directions() {
    let query = query("SELECT a FROM t WHERE a IS NULL OR b IS NULL");
    let witness = query.boolean_witness().expect("coupled witness");
    assert_eq!(witness.source_relation(), "t");
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let BooleanRowConstraint::Any(branches) = witness.condition() else {
        panic!("source predicate must preserve OR");
    };
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().all(BooleanRowConstraint::is_exact));
    // Independent scalar domains do not establish an exact OR across two columns.
    assert!(!query.condition_exactness().is_exact());
    // Either disjunct can select a row with any value in the other column.
    // The independent output domain must not be narrowed to NULL.
    assert!(matches!(
        query.output().columns()[0].domain(),
        sql_semantic_protocol::ValueDomain::Unbounded
            | sql_semantic_protocol::ValueDomain::Unknown(_)
    ));
}

#[test]
fn correlated_predicates_preserve_minimal_scalar_domains() {
    let conjunction = typed_bundle("SELECT a FROM t WHERE a > 2 AND a <= 5");
    let ComposedSemantics::Resolved(composed) = conjunction.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let ValueDomain::Ranges(ranges) = composed.output().columns()[0].domain() else {
        panic!("expected narrowed conjunctive output domain");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one scalar range");
    };
    let lower = range.lower().expect("lower bound");
    let upper = range.upper().expect("upper bound");
    assert_eq!(lower.value().value(), &sql_semantic_protocol::LiteralValue::Number("2".into()));
    assert!(!lower.inclusive());
    assert_eq!(upper.value().value(), &sql_semantic_protocol::LiteralValue::Number("5".into()));
    assert!(upper.inclusive());

    let disjunction = typed_bundle("SELECT a FROM t WHERE a > 2 OR b < 0");
    let ComposedSemantics::Resolved(composed) = disjunction.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    assert!(
        matches!(
            composed.output().columns()[0].domain(),
            ValueDomain::Unbounded | ValueDomain::Unknown(_)
        ),
        "cross-column OR cannot tighten the projected a domain independently"
    );
}

#[test]
fn filtered_upstream_domains_survive_without_false_physical_witnesses() {
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage AS SELECT a, b FROM raw_t WHERE a > 2 AND a <= 5",
            ),
            SqlInput::inline(
                "CREATE TABLE mart AS SELECT a FROM stage WHERE a IS NULL OR b IS NULL",
            ),
        ],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let mart = bundle
        .layers()
        .iter()
        .find(|layer| layer.produces().iter().any(|relation| relation.relation_name() == Some("mart")))
        .unwrap();
    let ComposedSemantics::Resolved(composed) = mart.composed_semantics() else {
        panic!("expected composition");
    };
    let ValueDomain::Ranges(ranges) = composed.output().columns()[0].domain() else {
        panic!("composed output must retain the upstream constraint");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one range");
    };
    assert_eq!(
        range.lower().expect("lower bound").value().value(),
        &sql_semantic_protocol::LiteralValue::Number("2".into())
    );
    assert_eq!(
        range.upper().expect("upper bound").value().value(),
        &sql_semantic_protocol::LiteralValue::Number("5".into())
    );
    assert_eq!(
        composed.boolean_witnesses().last().expect("mart witness").boundary_kind(),
        GroupBoundaryKind::Intermediate
    );
}

#[test]
fn untyped_integer_conditions_and_computed_branches_default_to_residual() {
    for sql in [
        "SELECT a FROM t WHERE a > 2 OR b < 0",
        "SELECT a FROM t WHERE a IS NULL OR b IS NULL OR CAST(c AS INT) > 0",
        "SELECT a FROM t WHERE a IS NULL OR b IS NULL OR c LIKE 'x%'",
        "SELECT a FROM t WHERE a IS NULL OR b IS NULL OR abs(c) = 3",
    ] {
        let query = query(sql);
        let witness = query.boolean_witness().expect("correlation evidence");
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{sql}"
        );
        assert!(
            matches!(witness.rejected(), BooleanWitnessDirection::Residual { .. }),
            "{sql}"
        );
    }
}

#[test]
fn typed_integer_disjunction_retains_comparison_operators_without_cross_product() {
    let bundle = typed_bundle("SELECT a FROM t WHERE a > 2 OR b < 0");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let item = &semantics.boolean_witnesses()[0];
    let witness = item.witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let BooleanRowConstraint::Any(operands) = witness.condition() else {
        panic!("expected correlated disjunction");
    };
    assert!(matches!(
        operands[0],
        BooleanRowConstraint::IntegerComparison { literal: 2, .. }
    ));
    assert!(matches!(
        operands[1],
        BooleanRowConstraint::IntegerComparison { literal: 0, .. }
    ));
    assert_ne!(operands[0], operands[1]);
}

#[test]
fn repeated_column_conditions_are_checked_jointly() {
    let bundle = typed_bundle("SELECT a FROM t WHERE (a > 2 AND a < 1) OR b < 0");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));

    let impossible = typed_bundle("SELECT a FROM t WHERE (a > 2 AND a < 1) OR (b > 3 AND b < 2)");
    let ComposedSemantics::Resolved(semantics) = impossible.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    assert!(matches!(
        semantics.boolean_witnesses()[0].witness().qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
}

#[test]
fn conjunctions_are_coupled_even_without_an_or() {
    let impossible = typed_bundle("SELECT a FROM t WHERE a > 2 AND a < 1");
    let ComposedSemantics::Resolved(semantics) = impossible.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(witness.condition(), BooleanRowConstraint::All(_)));
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));

    let feasible = typed_bundle("SELECT a FROM t WHERE a > 2 AND b < 1");
    let ComposedSemantics::Resolved(semantics) = feasible.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

fn enforced_evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "witness-test").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

#[test]
fn enforced_not_null_constraints_reject_impossible_positive_witness() {
    let mut bundle = typed_bundle("SELECT a FROM t WHERE a IS NULL OR b IS NULL");
    let constraints = RelationConstraintSet::new(
        "t",
        vec![
            RelationConstraint::not_null("a", enforced_evidence()).unwrap(),
            RelationConstraint::not_null("b", enforced_evidence()).unwrap(),
        ],
    )
    .unwrap();
    bundle.enrich_relation_constraints(&[constraints]);

    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn enforced_accepted_values_restrict_joint_comparison_feasibility() {
    let mut bundle = typed_bundle("SELECT a FROM t WHERE a > 2 OR b > 2");
    let constraints = RelationConstraintSet::new(
        "t",
        ["a", "b"]
            .iter()
            .map(|name| {
                RelationConstraint::accepted_values(
                    *name,
                    vec![ConstraintValue::Integer(1)],
                    false,
                    enforced_evidence(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    bundle.enrich_relation_constraints(&[constraints]);

    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn identity_arithmetic_is_invertible_but_nonidentity_arithmetic_stays_residual() {
    for sql in [
        "SELECT a FROM t WHERE (a + 0) > 2 OR (0 + b) < 0",
        "SELECT a FROM t WHERE (a - 0) > 2 OR (+b) < 0",
    ] {
        let bundle = typed_bundle(sql);
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        let witness = semantics.boolean_witnesses()[0].witness();
        assert!(matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ));
        assert!(matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ));
    }

    let bundle = typed_bundle("SELECT a FROM t WHERE (a + 1) > 2 OR b < 0");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    assert!(matches!(
        semantics.boolean_witnesses()[0].witness().qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
}

#[test]
fn widening_integer_casts_are_invertible_and_narrowing_casts_are_residual() {
    for sql in [
        "SELECT a FROM t WHERE CAST(a AS BIGINT) > 2 OR CAST(b AS INTEGER) < 0",
        "SELECT a FROM t WHERE 5 < CAST(a AS BIGINT) AND b < 0",
    ] {
        let bundle = typed_bundle(sql);
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        let witness = semantics.boolean_witnesses()[0].witness();
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Exact(_)),
            "{sql}: {:?}",
            witness.qualifying()
        );
        assert!(matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ));
    }

    for sql in [
        "SELECT a FROM t WHERE CAST(a AS SMALLINT) > 2 OR b < 0",
        "SELECT a FROM t WHERE CAST(a AS VARCHAR) > '2' OR b < 0",
        "SELECT a FROM t WHERE TRY_CAST(a AS BIGINT) > 2 OR b < 0",
    ] {
        let bundle = typed_bundle(sql);
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        let witness = semantics.boolean_witnesses()[0].witness();
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{sql}"
        );
    }
}

#[test]
fn casted_integer_offsets_are_inverted_without_overflow() {
    for (predicate, adjusted) in [
        ("CAST(a AS BIGINT) + 1 > 2 OR b < 0", 1),
        ("3 < 1 + CAST(a AS BIGINT) OR b < 0", 2),
        ("CAST(a AS BIGINT) - 2 > 2 OR b < 0", 4),
    ] {
        let bundle = typed_bundle(&format!("SELECT a FROM t WHERE {predicate}"));
        let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        let witness = composed.boolean_witnesses()[0].witness();
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Exact(_)),
            "{predicate}: {:?}",
            witness.qualifying()
        );
        let BooleanRowConstraint::Any(children) = witness.condition() else {
            panic!("expected coupled disjunction");
        };
        assert!(
            matches!(
                &children[0],
                BooleanRowConstraint::IntegerComparison { literal, .. } if *literal == adjusted
            ),
            "{predicate}: {:?}",
            children[0]
        );
    }

    for predicate in [
        "a + 1 > 2 OR b < 0",
        "CAST(a AS INTEGER) + 1 > 2 OR b < 0",
        "CAST(a AS BIGINT) + 9223372036854775807 > 2 OR b < 0",
        "CAST(a AS BIGINT) * 2 > 2 OR b < 0",
    ] {
        let bundle = typed_bundle(&format!("SELECT a FROM t WHERE {predicate}"));
        let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        assert!(
            matches!(
                composed.boolean_witnesses()[0].witness().qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}"
        );
    }
}

#[test]
fn dbt_compiled_sql_and_direct_catalog_sql_emit_the_same_boolean_witness() {
    let mut manifest_value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let sql = "SELECT id, amount FROM warehouse.raw.orders WHERE id > 2 OR amount < 0";
    manifest_value["nodes"]["model.demo.stg_orders"]["compiled_code"] =
        serde_json::Value::String(sql.to_string());
    let manifest = parse_dbt_manifest(&manifest_value.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgres", &dialect).unwrap();
    let stage = dbt
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|item| item.relation_name() == Some("warehouse.analytics.stg_orders"))
        })
        .unwrap();
    let ComposedSemantics::Resolved(dbt_semantics) = stage.composed_semantics() else {
        panic!("dbt composition");
    };
    let dbt_witness = dbt_semantics.boolean_witnesses()[0].witness();

    let source_schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgres").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgres").unwrap(),
        ],
    )
    .unwrap();
    let source_catalog = RelationCatalog::from_schemas(&[source_schema]).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "direct", &input, "postgres", &dialect,
    )];
    let direct = analyze_configured_inputs_with_catalog(&configured, &source_catalog).unwrap();
    let ComposedSemantics::Resolved(direct_semantics) = direct.layers()[0].composed_semantics()
    else {
        panic!("direct composition");
    };
    assert_eq!(
        dbt_witness,
        direct_semantics.boolean_witnesses()[0].witness()
    );
    assert!(matches!(
        dbt_witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
}

#[test]
fn source_witness_is_emitted_locally_and_retains_origin_through_composition() {
    let sql = "CREATE TABLE selected AS SELECT a FROM t WHERE a IS NULL OR b IS NULL";
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(sql),
            SqlInput::inline("CREATE TABLE downstream AS SELECT a FROM selected"),
        ],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let downstream = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|output| output.relation_name() == Some("downstream"))
        })
        .unwrap();
    let ComposedSemantics::Resolved(composed) = downstream.composed_semantics() else {
        panic!("downstream composition should resolve");
    };
    assert_eq!(composed.boolean_witnesses().len(), 1);
    assert_eq!(
        composed.boolean_witnesses()[0].witness().source_relation(),
        "t"
    );
    assert_ne!(
        composed.boolean_witnesses()[0].origin_layer_id(),
        downstream.id()
    );

    let protocol = analyze_sql(sql, "generic", &GenericDialect {}).unwrap();
    let value: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    let witness = &value["inputs"][0]["statements"][0]["boolean_witness"];
    assert_eq!(witness["condition"]["kind"], "any");
    assert_eq!(witness["qualifying"]["truth"], "true");
    assert_eq!(witness["rejected"]["truth"], "not_true");
    assert_eq!(
        witness["condition"]["operands"][0]["column"]["relation"],
        "t"
    );
}

#[test]
fn identity_lineage_maps_coupled_witnesses_to_physical_source() {
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage AS SELECT a, b FROM raw_t"),
            SqlInput::inline(
                "CREATE TABLE sink AS SELECT a FROM stage WHERE a IS NULL OR b IS NULL",
            ),
        ],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let sink = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|output| output.relation_name() == Some("sink"))
        })
        .unwrap();
    let ComposedSemantics::Resolved(composed) = sink.composed_semantics() else {
        panic!("expected physical composition");
    };
    let witness = &composed.boolean_witnesses()[0];
    assert_eq!(witness.boundary_kind(), GroupBoundaryKind::Physical);
    assert_eq!(witness.witness().source_relation(), "raw_t");
    let BooleanRowConstraint::Any(items) = witness.witness().condition() else {
        panic!("expected source-row disjunction");
    };
    for item in items.iter() {
        let BooleanRowConstraint::NullTest { column, .. } = item else {
            panic!("expected null test");
        };
        assert_eq!(column.relation(), Some("raw_t"));
    }
}

#[test]
fn computed_lineage_does_not_claim_a_physical_boolean_witness() {
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage AS SELECT a + 1 AS a, b FROM raw_t"),
            SqlInput::inline(
                "CREATE TABLE sink AS SELECT a FROM stage WHERE a IS NULL OR b IS NULL",
            ),
        ],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let sink = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|output| output.relation_name() == Some("sink"))
        })
        .unwrap();
    let ComposedSemantics::Resolved(composed) = sink.composed_semantics() else {
        panic!("expected composition");
    };
    let witness = &composed.boolean_witnesses()[0];
    assert_eq!(witness.boundary_kind(), GroupBoundaryKind::Intermediate);
    assert_eq!(witness.witness().source_relation(), "stage");
}

#[test]
fn filtered_or_limited_identity_projections_keep_intermediate_witnesses() {
    for producer in [
        "CREATE TABLE stage AS SELECT a, b FROM raw_t WHERE a > 0",
        "CREATE TABLE stage AS SELECT a, b FROM raw_t LIMIT 1",
        "CREATE TABLE stage AS SELECT DISTINCT a, b FROM raw_t",
    ] {
        let bundle = analyze_inputs(
            &[
                SqlInput::inline(producer),
                SqlInput::inline(
                    "CREATE TABLE sink AS SELECT a FROM stage WHERE a IS NULL OR b IS NULL",
                ),
            ],
            "generic",
            &GenericDialect {},
        )
        .unwrap();
        let sink = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .produces()
                    .iter()
                    .any(|output| output.relation_name() == Some("sink"))
            })
            .unwrap();
        let ComposedSemantics::Resolved(composed) = sink.composed_semantics() else {
            panic!("expected composition");
        };
        let witness = &composed.boolean_witnesses()[0];
        assert_eq!(
            witness.boundary_kind(),
            GroupBoundaryKind::Intermediate,
            "{producer}"
        );
        assert_eq!(witness.witness().source_relation(), "stage");
    }
}

#[test]
fn dialects_preserve_the_same_null_sensitive_source_tree() {
    let sql = "SELECT a FROM t WHERE a IS NULL OR b IS NOT NULL";
    for name in DIALECTS {
        let dialect = dialect_from_str(name).expect("dialect");
        let protocol = analyze_sql(sql, name, dialect.as_ref()).expect("shared syntax");
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("expected query in {name}");
        };
        let witness = query.boolean_witness().expect("witness for every dialect");
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Exact(_)),
            "{name}"
        );
        assert!(
            matches!(witness.rejected(), BooleanWitnessDirection::Exact(_)),
            "{name}"
        );
    }
}

#[test]
fn impossible_positive_direction_is_residual_even_with_known_integer_types() {
    let bundle = typed_bundle("SELECT a FROM t WHERE a > 2147483647 OR b > 2147483647");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn signed_integer_literals_are_proven_without_string_based_reparsing() {
    let bundle = typed_bundle("SELECT a FROM t WHERE a > -2 OR b < +3");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    let BooleanRowConstraint::Any(operands) = witness.condition() else {
        panic!("expected disjunction");
    };
    assert!(matches!(
        operands[0],
        BooleanRowConstraint::IntegerComparison { literal: -2, .. }
    ));
    assert!(matches!(
        operands[1],
        BooleanRowConstraint::IntegerComparison { literal: 3, .. }
    ));
}

fn sql_and(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

fn sql_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    }
}

fn witness_truth(
    constraint: &BooleanRowConstraint,
    a: Option<i32>,
    b: Option<i32>,
) -> Option<bool> {
    match constraint {
        BooleanRowConstraint::All(operands) => {
            let mut result = Some(true);
            for operand in operands.iter() {
                result = sql_and(result, witness_truth(operand, a, b));
            }
            result
        }
        BooleanRowConstraint::Any(operands) => {
            let mut result = Some(false);
            for operand in operands.iter() {
                result = sql_or(result, witness_truth(operand, a, b));
            }
            result
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected source column {other}"),
            };
            Some(value.is_none() != *negated)
        }
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected source column {other}"),
            };
            value.map(|v| {
                let v = i64::from(v);
                match operator {
                    ComparisonOperator::Eq => v == *literal,
                    ComparisonOperator::Neq => v != *literal,
                    ComparisonOperator::Lt => v < *literal,
                    ComparisonOperator::Lte => v <= *literal,
                    ComparisonOperator::Gt => v > *literal,
                    ComparisonOperator::Gte => v >= *literal,
                    ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                        panic!("null-safe comparisons must remain residual")
                    }
                }
            })
        }
        BooleanRowConstraint::StringPrefix { .. } => {
            panic!("integer differential fixture cannot evaluate string prefix")
        }
        BooleanRowConstraint::Residual { reason } => {
            panic!("differential fixture cannot evaluate residual: {reason}")
        }
    }
}

fn string_witness_truth(
    condition: &BooleanRowConstraint,
    a: Option<&str>,
    b: Option<&str>,
) -> Option<bool> {
    match condition {
        BooleanRowConstraint::All(operands) => {
            let mut result = Some(true);
            for operand in operands.iter() {
                result = sql_and(result, string_witness_truth(operand, a, b));
            }
            result
        }
        BooleanRowConstraint::Any(operands) => {
            let mut result = Some(false);
            for operand in operands.iter() {
                result = sql_or(result, string_witness_truth(operand, a, b));
            }
            result
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected source column {other}"),
            };
            Some(value.is_none() != *negated)
        }
        BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated,
        } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected source column {other}"),
            };
            value.map(|text| text.starts_with(prefix) != *negated)
        }
        other => panic!("unsupported string differential node: {other:?}"),
    }
}

#[test]
fn duckdb_like_prefix_differential_covers_null_negation_and_nested_coupling() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a VARCHAR, b VARCHAR);
         INSERT INTO t VALUES (NULL,'ab'), ('a','z'), ('ab','ab'),
         ('zz',NULL), ('xy','xy'), ('AB','ab'), ('',''),
         ('abc','def'), ('aba','zz');",
    )
    .unwrap();
    for predicate in [
        "a LIKE 'ab%' OR b LIKE 'ab%'",
        "a LIKE 'ab%' AND b NOT LIKE 'ab%'",
        "a LIKE 'ab%' OR a LIKE 'a%'",
        "a LIKE 'a%' AND a NOT LIKE 'ab%'",
        "a NOT LIKE 'ab%' OR b IS NULL",
    ] {
        let mut bundle = text_bundle(&format!("SELECT a FROM t WHERE {predicate}"));
        bundle.declare_comparison_assumptions(&[
            ComparisonAssumption::BinaryCollation,
            ComparisonAssumption::NoCharPadding,
        ]);
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected composition");
        };
        let witness = semantics.boolean_witnesses()[0].witness();
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            ),
            "{predicate}"
        );
        let mut statement = db
            .prepare(&format!("SELECT a, b, ({predicate}) FROM t"))
            .unwrap();
        let actual = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<bool>>(2)?,
                ))
            })
            .unwrap();
        for row in actual {
            let (a, b, sql_result) = row.unwrap();
            let computed = string_witness_truth(witness.condition(), a.as_deref(), b.as_deref());
            assert_eq!(
                computed, sql_result,
                "LIKE witness differs for {predicate} at ({a:?}, {b:?})"
            );
        }
    }
}

#[test]
fn duckdb_differential_matches_generated_witness_for_every_source_row() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL,1), (1,NULL), (NULL,NULL), (1,1);
         INSERT INTO t VALUES (3,NULL), (NULL,-1), (2147483647,-2147483648);",
    )
    .unwrap();
    for predicate in [
        "a IS NULL OR b IS NULL",
        "a > 2 OR b < 0",
        "a > -2 OR b <= 1",
        "(a > 2 AND a < 1) OR b < 0",
        "(a + 0) > 2 OR (0 + b) < 0",
        "CAST(a AS BIGINT) + 1 > 2 OR b < 0",
        "CAST(a AS BIGINT) - 2 > 2 OR b < 0",
        "3 < 1 + CAST(a AS BIGINT) OR b < 0",
        "a > 2 AND b < 0",
        "CAST(a AS BIGINT) > 2 OR b < 0",
        "CAST(a AS INTEGER) > 2 AND b < 0",
        "a IS NULL AND b IS NOT NULL",
        "(a > 2 OR b < 0) AND a < 5",
    ] {
        let bundle = typed_bundle(&format!("SELECT a FROM t WHERE {predicate}"));
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected resolved composed semantics");
        };
        let witness = semantics.boolean_witnesses()[0].witness();
        assert!(matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(_)
        ));

        let mut statement = db
            .prepare(&format!("SELECT a, b, ({predicate}) FROM t"))
            .unwrap();
        let actual = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<i32>>(0)?,
                    row.get::<_, Option<i32>>(1)?,
                    row.get::<_, Option<bool>>(2)?,
                ))
            })
            .unwrap();
        for row in actual {
            let (a, b, sql_result) = row.unwrap();
            let computed = witness_truth(witness.condition(), a, b);
            assert_eq!(
                computed, sql_result,
                "witness differs from DuckDB for {predicate} at ({a:?}, {b:?})"
            );
            assert_eq!(computed == Some(true), sql_result == Some(true));
            assert_eq!(computed != Some(true), sql_result != Some(true));
        }
    }
}
