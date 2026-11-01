//! TASK-158: conditional, complete-bag column schema transitions.
//!
//! This is a row-and-column proof seam, not yet an ordered SQL DDL executor.
//! In particular, callers must attest a complete constraint inventory. An
//! absent inventory, referenced keys or unproved SQL casts fail closed.

use super::*;
use crate::constraints::RelationConstraintSet;
use crate::relation::RelationSchema;

pub use crate::protocol::ColumnAlterStep as PhysicalColumnAlterStep;

/// A complete physical column transformation, or an explicit proof boundary.
///
/// Exact proves only the stated row/schema projection under the caller's
/// complete no-constraints attestation, not vendor DDL runtime behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalColumnAlterProof {
    /// Every original row occurrence is preserved with an exact new schema.
    Exact {
        /// Complete post-alter schema.
        schema: RelationSchema,
        /// Complete post-alter bag, with duplicate occurrences preserved.
        state: PhysicalWriteState,
    },
    /// The declared operation contradicts an attested schema or value.
    Impossible {
        /// Proven contradiction.
        reason: String,
    },
    /// SQL cast, constraints, or required execution evidence is unavailable.
    Residual {
        /// Missing proof.
        reason: String,
        /// Attested schema before the unproved operation.
        before_schema: RelationSchema,
        /// Intended schema when its typed shape is independently known.
        attempted_schema: Option<RelationSchema>,
    },
}

fn classify_failure(error: StateFailure, schema: &RelationSchema) -> PhysicalColumnAlterProof {
    match error {
        StateFailure::Impossible(reason) => PhysicalColumnAlterProof::Impossible { reason },
        StateFailure::Residual(_, reason) => PhysicalColumnAlterProof::Residual {
            reason,
            before_schema: schema.clone(),
            attempted_schema: None,
        },
    }
}

/// Apply a finite column transformation to a complete caller-attested bag.
///
/// `constraint_inventory` is `Some(&[])` only when the caller independently
/// attests that no relevant physical constraints exist. `None` means the
/// inventory is unknown. Any nonempty inventory remains residual until
/// preservation of keys, checks, defaults and references can be proved.
/// This does not parse SQL or certify transactional/vendor ALTER semantics.
pub fn physical_alter_column_snapshot(
    schema: &RelationSchema,
    state: &PhysicalWriteState,
    steps: &[PhysicalColumnAlterStep],
    constraint_inventory: Option<&[RelationConstraintSet]>,
) -> PhysicalColumnAlterProof {
    if schema.relation() != state.relation() {
        return PhysicalColumnAlterProof::Impossible {
            reason: "physical relation and attested schema identities differ".to_string(),
        };
    }
    if constraint_inventory.is_none() || constraint_inventory.is_some_and(|sets| !sets.is_empty()) {
        return PhysicalColumnAlterProof::Residual {
            reason: "complete constraint-free ALTER schema attestation is missing".to_string(),
            before_schema: schema.clone(),
            attempted_schema: None,
        };
    }
    if state.exact_domains(schema).is_none() {
        return PhysicalColumnAlterProof::Impossible {
            reason: "physical row columns contradict the complete pre-alter schema".to_string(),
        };
    }
    for row in state.rows() {
        for column in schema.columns() {
            let Some(value) = row.values().get(column.name()) else {
                return PhysicalColumnAlterProof::Impossible {
                    reason: "a physical row omits a declared pre-alter column".to_string(),
                };
            };
            if let Err(error) = validate_column_value(value, column) {
                return classify_failure(error, schema);
            }
        }
    }

    let mut columns = schema.columns().to_vec();
    let mut rows = state.rows().to_vec();
    for step in steps {
        match step {
            PhysicalColumnAlterStep::Add { column, default } => {
                if columns
                    .iter()
                    .any(|existing| existing.name() == column.name())
                {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("ADD column '{}' already exists", column.name()),
                    };
                }
                let value = default.clone().unwrap_or(LiteralValue::Null);
                if let Err(error) = validate_column_value(&value, column) {
                    return classify_failure(error, schema);
                }
                for row in &mut rows {
                    row.values.insert(column.name().to_string(), value.clone());
                }
                columns.push(column.clone());
            }
            PhysicalColumnAlterStep::Drop { column } => {
                let Some(index) = columns.iter().position(|item| item.name() == column) else {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("DROP column '{column}' does not exist"),
                    };
                };
                if columns.len() == 1 {
                    return PhysicalColumnAlterProof::Residual {
                        reason: "dropping the final column has unproved engine DDL laws"
                            .to_string(),
                        before_schema: schema.clone(),
                        attempted_schema: None,
                    };
                }
                columns.remove(index);
                for row in &mut rows {
                    row.values.remove(column);
                }
            }
            PhysicalColumnAlterStep::Rename { old, new } => {
                let Some(index) = columns.iter().position(|item| item.name() == old) else {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("RENAME column '{old}' does not exist"),
                    };
                };
                if old != new && columns.iter().any(|item| item.name() == new) {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("RENAME destination '{new}' already exists"),
                    };
                }
                let renamed = match columns[index].clone().renamed(new.clone()) {
                    Ok(column) => column,
                    Err(error) => {
                        return PhysicalColumnAlterProof::Impossible {
                            reason: error.to_string(),
                        };
                    }
                };
                columns[index] = renamed;
                if old != new {
                    for row in &mut rows {
                        if let Some(value) = row.values.remove(old) {
                            row.values.insert(new.clone(), value);
                        }
                    }
                }
            }
            PhysicalColumnAlterStep::SetType { column } => {
                let Some(index) = columns.iter().position(|item| item.name() == column.name())
                else {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("ALTER TYPE column '{}' does not exist", column.name()),
                    };
                };
                if columns[index] != *column {
                    return PhysicalColumnAlterProof::Residual {
                        reason: "ALTER TYPE requires independently proved cast/rewrite and comparison laws".to_string(),
                        before_schema: schema.clone(),
                        attempted_schema: RelationSchema::new(
                            schema.relation().to_string(),
                            columns.iter().enumerate().map(|(i, old)| {
                                if i == index { column.clone() } else { old.clone() }
                            }).collect(),
                        ).ok().map(|next| match schema.source_kind() {
                            Some(kind) => next.with_source_kind(kind),
                            None => next,
                        }),
                    };
                }
            }
        }
    }
    let mut resulting_schema = match RelationSchema::new(schema.relation().to_string(), columns) {
        Ok(schema) => schema,
        Err(error) => {
            return PhysicalColumnAlterProof::Impossible {
                reason: error.to_string(),
            };
        }
    };
    if let Some(source_kind) = schema.source_kind() {
        resulting_schema = resulting_schema.with_source_kind(source_kind);
    }
    for row in &rows {
        for column in resulting_schema.columns() {
            let Some(value) = row.values().get(column.name()) else {
                return PhysicalColumnAlterProof::Impossible {
                    reason: "a physical row omits a declared post-alter column".to_string(),
                };
            };
            if let Err(error) = validate_column_value(value, column) {
                return classify_failure(error, schema);
            }
        }
    }
    PhysicalColumnAlterProof::Exact {
        schema: resulting_schema,
        state: PhysicalWriteState {
            relation: state.relation().to_string(),
            rows,
        },
    }
}
