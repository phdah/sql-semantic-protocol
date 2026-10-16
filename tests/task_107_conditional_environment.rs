//! TASK-107 conditional floating environment boundary.
//!
//! Parser-equivalent SQL is not evidence of NaN or signed-zero runtime laws.
//! A typed floating source without vendor attestations must not acquire
//! constructive TRUE/NOT TRUE physical proofs from integer comparison rules.

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, BooleanWitnessDirection,
    ComposedSemantics, ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

#[test]
fn unproven_nan_and_signed_zero_cannot_be_certified_as_exact_integer_equalities() {
    let relation = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "DOUBLE", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[relation]).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    for (id, predicate) in [
        ("E107-V040", "a = CAST('NaN' AS DOUBLE)"),
        ("E107-V041", "a = -0.0"),
    ] {
        let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
        let bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "environment",
                &input,
                "postgresql",
                dialect.as_ref(),
            )],
            &catalog,
        )
        .unwrap_or_else(|error| panic!("{id}: {predicate}: {error}"));
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("{id}: missing typed source outcome");
        };
        assert!(
            !semantics.condition_exactness().is_exact(),
            "{id}: dialect-dependent floating comparison cannot claim Exact"
        );
        for item in semantics.boolean_witnesses() {
            let witness = item.witness();
            assert!(
                matches!(
                    witness.qualifying(),
                    BooleanWitnessDirection::Residual { .. }
                ),
                "{id}: unproven TRUE cannot be constructed: {:?}",
                witness.qualifying()
            );
            assert!(
                matches!(witness.rejected(), BooleanWitnessDirection::Residual { .. }),
                "{id}: unproven NOT TRUE cannot be constructed: {:?}",
                witness.rejected()
            );
        }
    }
}
