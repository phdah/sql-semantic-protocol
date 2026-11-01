//! TASK-158: conditional, complete-bag column schema transitions.
//!
//! This is a row-and-column proof seam, not yet an ordered SQL DDL executor.
//! In particular, callers must attest a complete constraint inventory. An
//! absent inventory, referenced keys or unproved SQL casts fail closed.

use super::*;
use crate::constraints::RelationConstraintSet;
use crate::relation::{RelationSchema, SchemaColumn};

/// One parser-independent change to an existing table's columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalColumnAlterStep {
    /// Append a declared column. None represents SQL NULL, not an inferred DEFAULT.
    Add {
        /// Authoritative target column declaration.
        column: SchemaColumn,
        /// Attested scalar DEFAULT, or SQL NULL if absent.
        default: Option<LiteralValue>,
    },
    /// Drop an existing nonfinal column.
    Drop {
        /// Column to remove.
        column: String,
    },
    /// Rename a column while keeping its type and comparison evidence.
    Rename {
        /// Existing column name.
        old: String,
        /// Replacement column name.
        new: String,
    },
    /// Replace the column type. Only an identical typed declaration is exact
    /// without independently verified SQL cast and rewrite laws.
    SetType {
        /// New typed declaration with the same column name.
        column: SchemaColumn,
    },
}

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
    },
}

fn classify_failure(error: StateFailure) -> PhysicalColumnAlterProof {
    match error {
        StateFailure::Impossible(reason) => PhysicalColumnAlterProof::Impossible { reason },
        StateFailure::Residual(_, reason) => PhysicalColumnAlterProof::Residual { reason },
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
                return classify_failure(error);
            }
        }
    }

    let mut columns = schema.columns().to_vec();
    let mut rows = state.rows().to_vec();
    for step in steps {
        match step {
            PhysicalColumnAlterStep::Add { column, default } => {
                if columns.iter().any(|existing| existing.name() == column.name()) {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("ADD column '{}' already exists", column.name()),
                    };
                }
                let value = default.clone().unwrap_or(LiteralValue::Null);
                if let Err(error) = validate_column_value(&value, column) {
                    return classify_failure(error);
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
                        reason: "dropping the final column has unproved engine DDL laws".to_string(),
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
                let Some(index) = columns.iter().position(|item| item.name() == column.name()) else {
                    return PhysicalColumnAlterProof::Impossible {
                        reason: format!("ALTER TYPE column '{}' does not exist", column.name()),
                    };
                };
                if columns[index] != *column {
                    return PhysicalColumnAlterProof::Residual {
                        reason: "ALTER TYPE requires independently proved cast/rewrite and comparison laws".to_string(),
                    };
                }
            }
        }
    }
    let schema = match RelationSchema::new(schema.relation().to_string(), columns) {
        Ok(schema) => schema,
        Err(error) => {
            return PhysicalColumnAlterProof::Impossible {
                reason: error.to_string(),
            };
        }
    };
    for row in &rows {
        for column in schema.columns() {
            let Some(value) = row.values().get(column.name()) else {
                return PhysicalColumnAlterProof::Impossible {
                    reason: "a physical row omits a declared post-alter column".to_string(),
                };
            };
            if let Err(error) = validate_column_value(value, column) {
                return classify_failure(error);
            }
        }
    }
    PhysicalColumnAlterProof::Exact {
        schema,
        state: PhysicalWriteState {
            relation: state.relation().to_string(),
            rows,
        },
    }
}
