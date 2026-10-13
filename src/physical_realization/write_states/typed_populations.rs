//! Schema-backed complete physical populations with Arrow-like nested values.
//!
//! Reuse the canonical scalar constraint verifier for independently checked
//! scalar columns. Nested values cannot be silently cast to SQL literals, and
//! constraints requiring nested comparison remain residual.

use super::typed_values::{physical_typed_value_plan, PhysicalTypeProof, PhysicalTypedValue};
use super::*;

/// A complete physical row retaining binary, array, map and struct values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTypedWriteRow {
    values: BTreeMap<String, PhysicalTypedValue>,
}

impl PhysicalTypedWriteRow {
    /// Construct a physical row with distinct, nonempty column identities.
    pub fn new(values: BTreeMap<String, PhysicalTypedValue>) -> Option<Self> {
        if values.keys().any(|name| name.trim().is_empty()) {
            None
        } else {
            Some(Self { values })
        }
    }

    /// Exact physical typed values indexed by canonical column.
    pub fn values(&self) -> &BTreeMap<String, PhysicalTypedValue> {
        &self.values
    }
}

/// A complete physical bag for a single schema-backed relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTypedWriteState {
    relation: String,
    rows: Vec<PhysicalTypedWriteRow>,
}

impl PhysicalTypedWriteState {
    /// Construct a population retaining row order and duplicate multiplicity.
    pub fn new(relation: impl Into<String>, rows: Vec<PhysicalTypedWriteRow>) -> Option<Self> {
        let relation = relation.into();
        if relation.trim().is_empty() {
            None
        } else {
            Some(Self { relation, rows })
        }
    }

    /// Canonical relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Complete typed physical bag, including duplicates.
    pub fn rows(&self) -> &[PhysicalTypedWriteRow] {
        &self.rows
    }
}

/// Proof classification for a complete multi-relation typed population.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalTypedPopulationPlan {
    /// All supplied physical populations satisfy their provable schema laws.
    Feasible {
        final_states: Vec<PhysicalTypedWriteState>,
    },
    /// A supplied physical row contradicts an attested necessary law.
    Impossible {
        relation: Option<String>,
        reason: String,
    },
    /// An unproved cast, comparison or schema obligation prevents certification.
    Residual {
        relation: Option<String>,
        reason: String,
    },
}

fn classify(
    relation: Option<String>,
    proof: PhysicalTypeProof,
) -> Option<PhysicalTypedPopulationPlan> {
    match proof {
        PhysicalTypeProof::Feasible => None,
        PhysicalTypeProof::Impossible { reason } => {
            Some(PhysicalTypedPopulationPlan::Impossible { relation, reason })
        }
        PhysicalTypeProof::Residual { reason } => {
            Some(PhysicalTypedPopulationPlan::Residual { relation, reason })
        }
    }
}

/// Verify complete, named Arrow-like populations against canonical schemas and
/// cross-relation constraints without converting nested values to SQL literals.
///
/// Scalar columns are validated with the same laws as the ordered DML engine;
/// nested values are validated structurally, and only *unconstrained* nested
/// columns are replaced by private NULL sentinels for the scalar constraint
/// verifier. Those sentinels are never exposed as constructed output. Key, FK,
/// CHECK or accepted-values rules on nested columns always remain residual
/// rather than inheriting fabricated equality or ordering semantics.
pub fn physical_typed_closed_world_schema_plan(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalTypedWriteState],
) -> PhysicalTypedPopulationPlan {
    let mut typed = BTreeMap::<String, PhysicalTypedWriteState>::new();
    let mut scalar_snapshots = Vec::new();
    for state in snapshots {
        let relation = Some(state.relation.clone());
        if typed
            .insert(state.relation.clone(), state.clone())
            .is_some()
        {
            return PhysicalTypedPopulationPlan::Impossible {
                relation,
                reason: "duplicate complete typed physical source identity".to_string(),
            };
        }
        let Some(schema) = bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation() == state.relation)
        else {
            return PhysicalTypedPopulationPlan::Residual {
                relation,
                reason: "missing canonical typed source schema".to_string(),
            };
        };
        let mut projected = Vec::new();
        for row in &state.rows {
            if row.values.len() != schema.columns().len() {
                return PhysicalTypedPopulationPlan::Impossible {
                    relation,
                    reason: "typed physical row arity contradicts declared schema".to_string(),
                };
            }
            let mut scalar_values = BTreeMap::new();
            for column in schema.columns() {
                let Some(value) = row.values.get(column.name()) else {
                    return PhysicalTypedPopulationPlan::Impossible {
                        relation,
                        reason: format!("typed physical row omits column {}", column.name()),
                    };
                };
                let proof = match value {
                    PhysicalTypedValue::Scalar(literal) => {
                        match super::type_validation::validate_column_value(literal, column) {
                            Ok(()) => PhysicalTypeProof::Feasible,
                            Err(StateFailure::Impossible(reason)) => {
                                PhysicalTypeProof::Impossible { reason }
                            }
                            Err(StateFailure::Residual(_, reason)) => {
                                PhysicalTypeProof::Residual { reason }
                            }
                        }
                    }
                    _ => physical_typed_value_plan(value, column.data_type()),
                };
                if let Some(outcome) = classify(Some(state.relation.clone()), proof) {
                    return outcome;
                }
                let scalar = match value {
                    PhysicalTypedValue::Scalar(literal) => literal.clone(),
                    _ => {
                        // A nested constrained key cannot masquerade as NULL in the
                        // existing scalar equality/ordering and FK verifier.
                        let constrained = bundle.relation_constraints().iter().any(|set| {
                            (set.relation() == state.relation && set.computed_keys().iter().any(|key| {
                                key.expressions().iter().any(|expr| expr.referenced_columns().contains(&column.name()))
                            })) || set.constraints().iter().any(|constraint| {
                                (set.relation() == state.relation
                                    && constraint.columns().iter().any(|name| name == column.name()))
                                    || matches!(constraint, RelationConstraint::ForeignKey(fk)
                                        if fk.referenced_relation() == state.relation
                                        && fk.referenced_columns().iter().any(|name| name == column.name()))
                            })
                        });
                        if constrained {
                            return PhysicalTypedPopulationPlan::Residual {
                                relation,
                                reason: format!(
                                    "nested column {} has no canonical key or CHECK comparison law",
                                    column.name()
                                ),
                            };
                        }
                        LiteralValue::Null
                    }
                };
                scalar_values.insert(column.name().to_string(), scalar);
            }
            projected.push(PhysicalWriteRow {
                values: scalar_values,
            });
        }
        scalar_snapshots.push(PhysicalWriteState {
            relation: state.relation.clone(),
            rows: projected,
        });
    }

    match physical_closed_world_schema_plan(bundle, &scalar_snapshots) {
        PhysicalWriteStatePlan::Feasible { .. } => PhysicalTypedPopulationPlan::Feasible {
            final_states: typed.into_values().collect(),
        },
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            PhysicalTypedPopulationPlan::Impossible {
                relation: None,
                reason,
            }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => PhysicalTypedPopulationPlan::Residual {
            relation: None,
            reason,
        },
    }
}
