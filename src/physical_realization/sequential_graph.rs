//! TASK-161: source-complete, typed statement-state dependency graph.
//!
//! This is a view of the already certified ordered lifecycle interpreter.
//! It never executes SQL twice, fabricates intermediate states, or treats a
//! SELECT as a new physical producer.

use std::collections::BTreeMap;

use crate::bundle::AnalysisBundle;
use crate::relation::RelationSchema;

use super::{
    physical_ordered_lifecycle_plan, PhysicalLifecycleRead, PhysicalOrderedLifecycleExact,
    PhysicalOrderedLifecycleProof, PhysicalWriteColumnDomain, PhysicalWriteState,
    PhysicalWriteTransition,
};

/// One closed-world live catalog before or after a SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalScriptCatalog {
    /// All live complete relation populations, sorted by relation identity.
    pub states: Vec<PhysicalWriteState>,
    /// Authoritative typed schema for each live population, in matching order.
    pub schemas: Vec<RelationSchema>,
}

impl PhysicalScriptCatalog {
    /// Compute exact schema-backed value frequencies, preserving SQL NULL.
    pub fn exact_domains(&self) -> Option<Vec<(String, Vec<PhysicalWriteColumnDomain>)>> {
        if self.states.len() != self.schemas.len() {
            return None;
        }
        self.states
            .iter()
            .zip(&self.schemas)
            .map(|(state, schema)| {
                Some((state.relation().to_string(), state.exact_domains(schema)?))
            })
            .collect()
    }
}

/// The origin of a physical relation version consumed by a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalScriptProducer {
    /// The caller-attested complete initial population.
    Initial,
    /// A specific prior SQL statement, not an inferred intermediate producer.
    Statement(String),
}

/// How a statement consumes a physical relation version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalScriptSourceRole {
    /// A write consumes the existing target relation state.
    TargetPrestate,
    /// A SELECT or write source reads an existing relation state.
    ReadSource,
}

/// A versioned physical-state dependency; edges never point forward in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalScriptStateEdge {
    /// Physical relation whose complete bag is consumed.
    pub relation: String,
    /// Unique attested version producing that bag.
    pub producer: PhysicalScriptProducer,
    /// Consuming statement layer in canonical SQL order.
    pub consumer_layer_id: String,
    /// Target-prestate or independent read-source dependency.
    pub role: PhysicalScriptSourceRole,
}

/// One SQL statement and both of its complete typed catalog snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalScriptStateNode {
    /// Stable canonical layer identity.
    pub layer_id: String,
    /// Complete live catalog before this statement.
    pub before: PhysicalScriptCatalog,
    /// Complete live catalog after this statement.
    pub after: PhysicalScriptCatalog,
    /// Certified mutation, affected occurrence ordinals and row counts, if a write.
    pub write: Option<PhysicalWriteTransition>,
    /// Certified read bag and TRUE/NOT TRUE source occurrences, if a SELECT.
    pub read: Option<PhysicalLifecycleRead>,
}

/// An exact ordered graph with explicit source versions and typed snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSequentialStateGraphExact {
    /// Statements in canonical SQL order.
    pub nodes: Vec<PhysicalScriptStateNode>,
    /// Read and mutation dependencies in deterministic statement/source order.
    pub edges: Vec<PhysicalScriptStateEdge>,
    /// Complete final live physical populations.
    pub final_states: Vec<PhysicalWriteState>,
}

/// Soundness classification of the entire ordered graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalSequentialStateGraphProof {
    /// Every statement and physical population was certified.
    Exact(Box<PhysicalSequentialStateGraphExact>),
    /// A concrete statement, schema or state contradiction.
    Impossible { reason: String },
    /// Missing data or an unproved operation, dependency or engine law.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalSequentialStateGraphProof {
    PhysicalSequentialStateGraphProof::Residual {
        reason: reason.into(),
    }
}

fn catalog(
    states: Vec<PhysicalWriteState>,
    schemas: Vec<RelationSchema>,
) -> Option<PhysicalScriptCatalog> {
    if states.len() != schemas.len()
        || states.iter().zip(&schemas).any(|(state, schema)| {
            state.relation() != schema.relation() || state.exact_domains(schema).is_none()
        })
    {
        return None;
    }
    Some(PhysicalScriptCatalog { states, schemas })
}

fn connect(
    edges: &mut Vec<PhysicalScriptStateEdge>,
    producers: &BTreeMap<String, PhysicalScriptProducer>,
    relation: &str,
    consumer: &str,
    role: PhysicalScriptSourceRole,
) -> Result<(), String> {
    let producer = producers
        .get(relation)
        .ok_or_else(|| format!("missing attested producer for relation {relation}"))?;
    edges.push(PhysicalScriptStateEdge {
        relation: relation.to_string(),
        producer: producer.clone(),
        consumer_layer_id: consumer.to_string(),
        role,
    });
    Ok(())
}

/// Build a typed state graph from the canonical sequential SQL program.
///
/// Exactness requires the underlying complete-bag write/read interpreter,
/// per-step schema evidence, and one unambiguous physical producer per read.
/// Unsupported SQL or transaction/session semantics remain Residual; impossible
/// writes and known reads of dropped relations remain Impossible. The graph
/// never reparses SQL or accepts separately supplied intermediate states.
pub fn physical_sequential_state_graph_plan(
    bundle: &AnalysisBundle,
    initial_states: &[PhysicalWriteState],
) -> PhysicalSequentialStateGraphProof {
    let exact = match physical_ordered_lifecycle_plan(bundle, initial_states) {
        PhysicalOrderedLifecycleProof::Exact(exact) => exact,
        PhysicalOrderedLifecycleProof::Impossible { reason } => {
            return PhysicalSequentialStateGraphProof::Impossible { reason };
        }
        PhysicalOrderedLifecycleProof::Residual { reason } => return residual(reason),
    };
    let PhysicalOrderedLifecycleExact {
        transitions,
        reads,
        checkpoints,
        final_states,
    } = *exact;
    if checkpoints.len() != bundle.layers().len() {
        return residual("ordered lifecycle checkpoint count differs from SQL layer count");
    }

    let initial_schemas = initial_states
        .iter()
        .map(|state| {
            bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == state.relation())
                .cloned()
        })
        .collect::<Option<Vec<_>>>();
    let Some(initial_schemas) = initial_schemas else {
        return residual("initial physical state has no authoritative typed schema");
    };
    let mut before = match catalog(initial_states.to_vec(), initial_schemas) {
        Some(before) => before,
        None => return residual("initial complete catalog disagrees with typed schema"),
    };
    // Canonical initial states may arrive unsorted. Normalize without changing
    // the row order within any relation's complete physical bag.
    let mut pairs = before
        .states
        .into_iter()
        .zip(before.schemas)
        .collect::<Vec<_>>();
    pairs.sort_by(|a, b| a.0.relation().cmp(b.0.relation()));
    before = PhysicalScriptCatalog {
        states: pairs.iter().map(|(state, _)| state.clone()).collect(),
        schemas: pairs.into_iter().map(|(_, schema)| schema).collect(),
    };
    let mut producers = before
        .states
        .iter()
        .map(|state| {
            (
                state.relation().to_string(),
                PhysicalScriptProducer::Initial,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut writes = transitions.into_iter();
    let mut reads = reads.into_iter();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for (layer, checkpoint) in bundle.layers().iter().zip(checkpoints) {
        if checkpoint.layer_id != layer.id() {
            return residual("lifecycle checkpoint is not in canonical statement order");
        }
        let query = bundle
            .inputs()
            .iter()
            .find(|input| input.id() == layer.input_id())
            .and_then(|input| input.statements().get(layer.statement_index()))
            .and_then(|statement| match statement {
                crate::protocol::ProtocolStatement::Query(query) => Some(query),
                crate::protocol::ProtocolStatement::Unsupported(_) => None,
            });
        let Some(query) = query else {
            return residual("statement has no normalized canonical query evidence");
        };
        let (write, read) = if layer.write_kind().is_some() {
            let Some(write) = writes.next() else {
                return residual("missing canonical write transition in graph");
            };
            if write.layer_id() != layer.id() {
                return residual("ordered write layer disagrees with graph statement identity");
            }
            let target = write.before().relation();
            if write.before_exists() {
                if before.states.iter().find(|s| s.relation() == target) != Some(write.before()) {
                    return residual(
                        "target prestate is not the preceding complete catalog version",
                    );
                }
                if let Err(reason) = connect(
                    &mut edges,
                    &producers,
                    target,
                    layer.id(),
                    PhysicalScriptSourceRole::TargetPrestate,
                ) {
                    return residual(reason);
                }
            } else if before.states.iter().any(|s| s.relation() == target) {
                return residual("a CREATE/definition has a previously live target");
            }
            for source in query.sources() {
                // A CTE or derived relation is not an independently controlled
                // physical bag. Without a certified path to its producer, this
                // graph must not silently omit the read dependency.
                if !before.states.iter().any(|s| s.relation() == source.name()) {
                    return residual(format!(
                        "write reads an unresolved physical or local producer: {}",
                        source.name()
                    ));
                }
                if let Err(reason) = connect(
                    &mut edges,
                    &producers,
                    source.name(),
                    layer.id(),
                    PhysicalScriptSourceRole::ReadSource,
                ) {
                    return residual(reason);
                }
            }
            (Some(write), None)
        } else {
            let Some(read) = reads.next() else {
                return residual("missing canonical SELECT checkpoint in graph");
            };
            if read.layer_id != layer.id()
                || before
                    .states
                    .iter()
                    .find(|s| s.relation() == read.source.relation())
                    != Some(&read.source)
            {
                return residual("SELECT reads a different complete physical catalog version");
            }
            if let Err(reason) = connect(
                &mut edges,
                &producers,
                read.source.relation(),
                layer.id(),
                PhysicalScriptSourceRole::ReadSource,
            ) {
                return residual(reason);
            }
            (None, Some(read))
        };

        let Some(after) = catalog(checkpoint.states, checkpoint.schemas) else {
            return residual("checkpoint does not carry a complete typed live catalog");
        };
        if let Some(transition) = &write {
            // Only the certified write can produce a new table version. RENAME
            // changes relation identity, DROP removes it, and reads never write.
            producers.remove(transition.before().relation());
            if transition.after_exists() {
                producers.insert(
                    transition.after().relation().to_string(),
                    PhysicalScriptProducer::Statement(layer.id().to_string()),
                );
            }
        }
        nodes.push(PhysicalScriptStateNode {
            layer_id: layer.id().to_string(),
            before,
            after: after.clone(),
            write,
            read,
        });
        before = after;
    }
    if writes.next().is_some()
        || reads.next().is_some()
        || before.states != final_states
        || producers.len() != before.states.len()
    {
        return residual("graph does not reconcile with the final canonical state");
    }
    PhysicalSequentialStateGraphProof::Exact(Box::new(PhysicalSequentialStateGraphExact {
        nodes,
        edges,
        final_states,
    }))
}
