//! Closed-world table lifecycle operations over an evolving physical catalog.
//!
//! A missing caller population is never proof of table absence. Only a prior
//! executed DROP can certify the IF EXISTS no-op or missing-table contradiction.

use super::*;

pub(super) struct LifecycleOutcome {
    pub(super) after: PhysicalWriteState,
    pub(super) counts: WriteRowCounts,
    pub(super) after_schema: Option<RelationSchema>,
    pub(super) exists_after: bool,
}

pub(super) fn lifecycle_transition(
    action: &WriteLifecycleAction,
    target: &str,
    before: Option<&PhysicalWriteState>,
    schema: Option<&RelationSchema>,
    states: &BTreeMap<String, PhysicalWriteState>,
    created_here: bool,
    proven_absent: bool,
    constrained: bool,
) -> StateResult<LifecycleOutcome> {
    if let WriteLifecycleAction::Drop { if_exists } = action {
        if before.is_none() {
            if !proven_absent {
                return Err(residual_gap(
                    PhysicalWriteStateGap::MissingPopulation,
                    "a missing physical snapshot does not prove DROP target absence",
                ));
            }
            if !*if_exists {
                return Err(impossible("DROP requires a table already proved absent"));
            }
            return Ok(LifecycleOutcome {
                after: PhysicalWriteState {
                    relation: target.to_string(),
                    rows: Vec::new(),
                },
                counts: WriteRowCounts::new(0, 0, 0),
                after_schema: None,
                exists_after: false,
            });
        }
    }
    let prior = before.ok_or_else(|| {
        residual_gap(
            PhysicalWriteStateGap::MissingPopulation,
            "table lifecycle requires an existing complete physical table population",
        )
    })?;
    let prior_schema = schema.ok_or_else(|| {
        residual_gap(
            PhysicalWriteStateGap::MissingSchema,
            "table lifecycle requires a complete typed prior schema",
        )
    })?;
    // A complete in-program CREATE and absence of canonical constraints is
    // evidence that no hidden enforced keys or incoming FKs are bypassed.
    // External metadata alone cannot certify that the constraint inventory
    // is complete, so it must not grant portable DROP/TRUNCATE/RENAME.
    if !created_here || constrained {
        return Err(residual_gap(
            PhysicalWriteStateGap::UnprovedConstraint,
            "lifecycle requires a prior CREATE and a complete constraint-free table declaration",
        ));
    }
    match action {
        WriteLifecycleAction::Truncate => Ok(LifecycleOutcome {
            after: PhysicalWriteState {
                relation: target.to_string(),
                rows: Vec::new(),
            },
            counts: WriteRowCounts::new(
                0,
                0,
                u64::try_from(prior.rows.len())
                    .map_err(|_| residual("TRUNCATE physical row count overflow"))?,
            ),
            after_schema: Some(prior_schema.clone()),
            exists_after: true,
        }),
        WriteLifecycleAction::Drop { .. } => Ok(LifecycleOutcome {
            after: PhysicalWriteState {
                relation: target.to_string(),
                rows: Vec::new(),
            },
            counts: WriteRowCounts::new(
                0,
                0,
                u64::try_from(prior.rows.len())
                    .map_err(|_| residual("DROP physical row count overflow"))?,
            ),
            after_schema: None,
            exists_after: false,
        }),
        WriteLifecycleAction::Rename { to } => {
            if to.trim().is_empty() || to == target {
                return Err(residual("table RENAME requires a distinct named destination"));
            }
            if states.contains_key(to) {
                return Err(impossible("RENAME target relation already exists"));
            }
            let renamed = RelationSchema::new(to.clone(), prior_schema.columns().to_vec())
                .map_err(|_| residual("cannot preserve typed schema on table RENAME"))?;
            Ok(LifecycleOutcome {
                after: PhysicalWriteState {
                    relation: to.clone(),
                    rows: prior.rows.clone(),
                },
                counts: WriteRowCounts::new(0, 0, 0),
                after_schema: Some(renamed),
                exists_after: true,
            })
        }
    }
}
