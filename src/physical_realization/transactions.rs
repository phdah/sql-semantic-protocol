//! TASK-162: opt-in, source-complete DuckDB transaction replay.
//!
//! SQL mutations are certified by the canonical typed sequential state graph.
//! Transaction boundaries only snapshot, commit or restore that proven catalog;
//! they never synthesize rows or guess an unsupported engine/session law.

use std::collections::BTreeMap;

use crate::bundle::AnalysisBundle;
use crate::constraints::RelationConstraintSet;
use crate::protocol::WriteKind;
use crate::relation::RelationSchema;

use super::{
    physical_sequential_state_graph_plan, PhysicalScriptCatalog, PhysicalScriptStateNode,
    PhysicalSequentialStateGraphProof, PhysicalWriteState,
};

/// One logical statement of a single-connection DuckDB transaction script.
///
/// A SQL step is one already analyzed canonical SQL statement. Its bundle must
/// contain exactly one layer. Transaction commands are explicit typed controls,
/// not guessed from a parser-dependent unsupported SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalTransactionStep<'a> {
    /// Analyzed DDL, DML or SELECT, executed on the preceding complete catalog.
    Sql(&'a AnalysisBundle),
    /// Canonical SQL plus independently attested constraint enforcement facts.
    ///
    /// A declared PRIMARY KEY without ENFORCED evidence remains uncertain.
    /// Attestation is checked by the canonical constraint validator and
    /// persists with the physical catalog, including across ROLLBACK.
    SqlWithEvidence {
        /// One canonical analyzed SQL statement.
        bundle: &'a AnalysisBundle,
        /// Caller-attested canonical enforced constraints and provenance.
        constraints: &'a [RelationConstraintSet],
    },
    /// Start a new explicit transaction, snapshotting the full typed catalog.
    Begin,
    /// Commit all exact changes since the matching BEGIN.
    Commit,
    /// Restore the entire catalog from the matching BEGIN.
    Rollback,
    /// Engine-dependent named savepoint, not yet certified for native DuckDB.
    Savepoint(&'a str),
    /// Engine-dependent partial rollback, not yet certified for native DuckDB.
    RollbackTo(&'a str),
    /// Engine-dependent savepoint release, not yet certified for native DuckDB.
    Release(&'a str),
}

/// Which exact operation produced a transaction checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalTransactionAction {
    /// Canonical SQL statement.
    Sql,
    /// BEGIN.
    Begin,
    /// COMMIT.
    Commit,
    /// ROLLBACK.
    Rollback,
}

/// Full before/after catalogs and the canonical SQL proof at one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTransactionCheckpoint {
    /// Zero-based position in the caller-supplied script.
    pub step_index: usize,
    /// Certified transaction action.
    pub action: PhysicalTransactionAction,
    /// Complete typed physical state immediately before this action.
    pub before: PhysicalScriptCatalog,
    /// Exact before-state canonical enforced constraints and defaults.
    pub before_constraints: Vec<RelationConstraintSet>,
    /// Complete typed physical state immediately after this action.
    pub after: PhysicalScriptCatalog,
    /// Canonical enforced constraints and defaults after this action.
    pub after_constraints: Vec<RelationConstraintSet>,
    /// Source-complete canonical DDL, DML or SELECT proof for SQL steps.
    pub statement: Option<PhysicalScriptStateNode>,
}

/// Exact single-session transaction execution, including rolled-back states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTransactionScriptExact {
    /// All source-complete checkpoints, in script order.
    pub checkpoints: Vec<PhysicalTransactionCheckpoint>,
    /// Final committed physical catalog, including schemas and duplicate rows.
    pub final_catalog: PhysicalScriptCatalog,
    /// Final authoritative constraint/default evidence of live relations.
    pub final_constraints: Vec<RelationConstraintSet>,
}

/// Classification of the complete transaction script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalTransactionScriptProof {
    /// Every SQL statement and transaction boundary is certified by evidence.
    Exact(Box<PhysicalTransactionScriptExact>),
    /// A proven invalid transaction state or concrete SQL contradiction.
    Impossible {
        /// First contradictory step.
        step_index: usize,
        /// Sound reason the script cannot execute as requested.
        reason: String,
    },
    /// Missing SQL evidence or an unsupported DuckDB/session-specific law.
    Residual {
        /// First unproved step.
        step_index: usize,
        /// Why exactness cannot be certified.
        reason: String,
    },
}

fn impossible(step_index: usize, reason: impl Into<String>) -> PhysicalTransactionScriptProof {
    PhysicalTransactionScriptProof::Impossible {
        step_index,
        reason: reason.into(),
    }
}

fn residual(step_index: usize, reason: impl Into<String>) -> PhysicalTransactionScriptProof {
    PhysicalTransactionScriptProof::Residual {
        step_index,
        reason: reason.into(),
    }
}

fn complete_catalog(
    catalog: &PhysicalScriptCatalog,
) -> Result<PhysicalScriptCatalog, &'static str> {
    if catalog.states.len() != catalog.schemas.len() {
        return Err("physical state and schema catalog lengths disagree");
    }
    let mut sorted = BTreeMap::<String, (PhysicalWriteState, RelationSchema)>::new();
    for (state, schema) in catalog.states.iter().zip(&catalog.schemas) {
        if state.relation() != schema.relation()
            || state.exact_domains(schema).is_none()
            || !super::write_states::certified_source_row_types(state, schema)
        {
            return Err("physical state and authoritative typed schema disagree");
        }
        if sorted
            .insert(
                state.relation().to_string(),
                (state.clone(), schema.clone()),
            )
            .is_some()
        {
            return Err("duplicate physical relation identity in one catalog");
        }
    }
    let (states, schemas) = sorted.into_values().unzip();
    Ok(PhysicalScriptCatalog { states, schemas })
}

/// Certify a finite DuckDB single-session transaction script.
///
/// Every SQL step delegates to the canonical TASK-161 sequential state graph,
/// seeded with the preceding complete typed catalog. BEGIN snapshots *all* live
/// physical states and schemas. ROLLBACK restores that exact population, not a
/// reconstructed or independently generated approximation. COMMIT retains it.
///
/// Savepoints are deliberately Residual: only native DuckDB support plus an
/// independent executable oracle can justify an exact rollback-to/release law.
/// Concurrent sessions, external side effects and implicit vendor transaction
/// rules are outside this single-session proof.
pub fn physical_duckdb_transaction_plan(
    steps: &[PhysicalTransactionStep<'_>],
    initial: &PhysicalScriptCatalog,
) -> PhysicalTransactionScriptProof {
    let mut current = match complete_catalog(initial) {
        Ok(catalog) => catalog,
        Err(reason) => return residual(0, reason),
    };
    let mut active_transaction: Option<(
        PhysicalScriptCatalog,
        BTreeMap<String, RelationConstraintSet>,
    )> = None;
    let mut live_constraints = BTreeMap::<String, RelationConstraintSet>::new();
    let mut checkpoints = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        let before = current.clone();
        let before_constraints = live_constraints.values().cloned().collect();
        let (action, statement) = match step {
            PhysicalTransactionStep::Sql(bundle)
            | PhysicalTransactionStep::SqlWithEvidence { bundle, .. } => {
                // A layer count alone is insufficient: unsupported transaction
                // SQL can live alongside one valid query in the same bundle.
                // Never drop that statement or prove a non-DuckDB session law.
                let [input] = bundle.inputs() else {
                    return residual(index, "transaction SQL step requires exactly one input");
                };
                if bundle.layers().len() != 1
                    || input.statements().len() != 1
                    || input.dialect() != "duckdb"
                {
                    return residual(
                        index,
                        "transaction SQL step requires one DuckDB canonical statement",
                    );
                }
                // Previously certified DDL constraints are carried to each
                // subsequent statement. A fresh SQL bundle alone does not
                // attest that an earlier PRIMARY KEY or CHECK disappeared.
                let mut execution = (**bundle).clone();
                for schema in &current.schemas {
                    if !execution.attest_physical_source_schema(schema.clone()) {
                        return residual(
                            index,
                            "SQL metadata contradicts the preceding live typed schema",
                        );
                    }
                }
                execution.enrich_relation_constraints(
                    &live_constraints.values().cloned().collect::<Vec<_>>(),
                );
                if let PhysicalTransactionStep::SqlWithEvidence { constraints, .. } = step {
                    execution.enrich_relation_constraints(constraints);
                }
                let graph = match physical_sequential_state_graph_plan(&execution, &current.states)
                {
                    PhysicalSequentialStateGraphProof::Exact(graph) => graph,
                    PhysicalSequentialStateGraphProof::Impossible { reason } => {
                        return impossible(index, reason);
                    }
                    PhysicalSequentialStateGraphProof::Residual { reason } => {
                        return residual(index, reason);
                    }
                };
                let mut nodes = graph.nodes.into_iter();
                let Some(node) = nodes.next() else {
                    return residual(index, "canonical SQL graph has no statement proof");
                };
                if nodes.next().is_some()
                    || node.before != current
                    || node.after.states != graph.final_states
                    || complete_catalog(&node.after).is_err()
                {
                    return residual(
                        index,
                        "SQL step does not reconcile with the complete transaction catalog",
                    );
                }
                if let Some(write) = &node.write {
                    let target = write.before().relation();
                    if matches!(write.kind(), WriteKind::SchemaAlter) {
                        // ALTER can create/drop/rename default-producing columns.
                        // Its canonical poststate is proven in TASK-158, but
                        // carrying default/constraint metadata across *separate*
                        // transaction SQL units belongs to TASK-164. Never
                        // certify a later omitted-column INSERT from stale data.
                        return residual(
                            index,
                            "TASK-164: ALTER default and constraint evidence is not versioned across transaction SQL units",
                        );
                    }
                    if !write.after_exists() {
                        live_constraints.remove(target);
                    } else if write.after().relation() != target {
                        if let Some(mut set) = live_constraints.remove(target) {
                            set.rename_physical_relation(write.after().relation());
                            live_constraints.insert(write.after().relation().to_string(), set);
                        }
                    }
                }
                for set in execution.relation_constraints() {
                    if !node
                        .after
                        .states
                        .iter()
                        .any(|s| s.relation() == set.relation())
                    {
                        continue;
                    }
                    match live_constraints.get(set.relation()) {
                        None => { live_constraints.insert(set.relation().to_string(), set.clone()); }
                        Some(existing) if existing == set => {}
                        Some(_) => return residual(
                            index,
                            "conflicting canonical constraint declarations across transaction steps",
                        ),
                    }
                }
                current = node.after.clone();
                (PhysicalTransactionAction::Sql, Some(node))
            }
            PhysicalTransactionStep::Begin => {
                if active_transaction.is_some() {
                    return impossible(index, "DuckDB does not support nested BEGIN transactions");
                }
                active_transaction = Some((current.clone(), live_constraints.clone()));
                (PhysicalTransactionAction::Begin, None)
            }
            PhysicalTransactionStep::Commit => {
                if active_transaction.take().is_none() {
                    return impossible(index, "COMMIT has no active DuckDB transaction");
                }
                (PhysicalTransactionAction::Commit, None)
            }
            PhysicalTransactionStep::Rollback => {
                let Some((original, original_constraints)) = active_transaction.take() else {
                    return impossible(index, "ROLLBACK has no active DuckDB transaction");
                };
                current = original;
                live_constraints = original_constraints;
                (PhysicalTransactionAction::Rollback, None)
            }
            PhysicalTransactionStep::Savepoint(name)
            | PhysicalTransactionStep::RollbackTo(name)
            | PhysicalTransactionStep::Release(name) => {
                if name.trim().is_empty() {
                    return impossible(index, "transaction savepoint identity is empty");
                }
                if active_transaction.is_none() {
                    return impossible(index, "savepoint operation requires an active transaction");
                }
                return residual(
                    index,
                    "TASK-162: native DuckDB SAVEPOINT/ROLLBACK TO/RELEASE execution law is unverified",
                );
            }
        };
        checkpoints.push(PhysicalTransactionCheckpoint {
            step_index: index,
            action,
            before,
            before_constraints,
            after: current.clone(),
            after_constraints: live_constraints.values().cloned().collect(),
            statement,
        });
    }
    if active_transaction.is_some() {
        return residual(
            steps.len(),
            "uncommitted explicit transaction at end of script",
        );
    }
    PhysicalTransactionScriptProof::Exact(Box::new(PhysicalTransactionScriptExact {
        checkpoints,
        final_catalog: current,
        final_constraints: live_constraints.into_values().collect(),
    }))
}
