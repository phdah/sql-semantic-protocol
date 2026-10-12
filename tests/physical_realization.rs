//! TASK-68 physical leaf topology and the conservative single-row proof boundary.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_row_count_plan,
    physical_joint_source_plan, physical_rejected_row_count_plan, physical_row_count_plan,
    physical_source_plan, physical_unconditional_delete_plan, AnalysisBundle, BooleanRowConstraint,
    BooleanTruthCase, ConfiguredSqlInput, ConstraintValue, JoinPopulationPattern, OutcomeGoal,
    OutcomeGoalStatus, OutcomeWitness, OutputDistribution, OutputValueCount, PhysicalPlanRef,
    PhysicalProofGap, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessFormula, WitnessObligation,
};

fn bundle(queries: &[&str], dialect: &str) -> AnalysisBundle {
    let schemas = ["t", "l", "r", "stage", "mart"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["a", "b", "k"]
                    .into_iter()
                    .map(|name| {
                        SchemaColumn::from_sql_type(name, "INTEGER", "postgresql")
                            .expect("integer column")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    bundle_with_schemas(queries, dialect, &schemas)
}

fn bundle_with_schemas(
    queries: &[&str],
    dialect: &str,
    schemas: &[RelationSchema],
) -> AnalysisBundle {
    let dialect_impl = dialect_from_name(dialect).expect("recognized dialect");
    let catalog = RelationCatalog::from_schemas(schemas).expect("catalog");
    let sources = queries
        .iter()
        .map(|q| SqlInput::inline(*q))
        .collect::<Vec<_>>();
    let ids = (0..queries.len())
        .map(|i| format!("q-{i}"))
        .collect::<Vec<_>>();
    let inputs = sources
        .iter()
        .zip(&ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, dialect, dialect_impl.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&inputs, &catalog).expect("analysis")
}

#[path = "physical_realization/filters.rs"]
mod filters;
#[path = "physical_realization/graph.rs"]
mod graph;
#[path = "physical_realization/group_rank_set.rs"]
mod group_rank_set;
#[path = "physical_realization/joins.rs"]
mod joins;
#[path = "physical_realization/joint_filters.rs"]
mod joint_filters;
#[path = "physical_realization/joint_graph.rs"]
mod joint_graph;
#[path = "physical_realization/joint_operators.rs"]
mod joint_operators;
#[path = "physical_realization/lineage.rs"]
mod lineage;
#[path = "physical_realization/mixed.rs"]
mod mixed;
#[path = "physical_realization/nested_sets.rs"]
mod nested_sets;
#[path = "physical_realization/producers.rs"]
mod producers;
