//! TASK-160: prove ordered read checkpoints from the same complete physical
//! states as the canonical write interpreter. No SQL reparsing or fabricated
//! intermediate table populations are permitted.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::AnalysisBundle;
use crate::data_type::DataType;
use crate::protocol::{LiteralValue, ProtocolStatement};
use crate::relation::{RelationSchema, SchemaColumn};

use super::write_states::evaluate_ordered_read;
use super::{
    physical_ordered_write_state_plan, PhysicalWriteColumnDomain, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, PhysicalWriteTransition,
};

/// A complete, source-backed SELECT, evaluated at its actual SQL statement position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalLifecycleRead {
    /// Canonical identifier of the SELECT layer.
    pub layer_id: String,
    /// Exact, typed source rows observed by this SELECT.
    pub source: PhysicalWriteState,
    /// Source schema at the SELECT checkpoint, including earlier ALTER changes.
    pub source_schema: RelationSchema,
    /// Exact SELECT result rows, including SQL NULL and duplicate occurrences.
    pub output: PhysicalWriteState,
    /// Output schema derived from source column evidence, never inferred from values.
    pub output_schema: RelationSchema,
    /// Selected original source occurrence ordinals, in output order.
    pub surviving_source_ordinals: Vec<usize>,
    /// Original source occurrences that were NOT TRUE for this SELECT.
    pub rejected_source_ordinals: Vec<usize>,
}

impl PhysicalLifecycleRead {
    /// Exact column frequencies and SQL NULLs of the complete read bag.
    pub fn exact_domains(&self) -> Option<Vec<PhysicalWriteColumnDomain>> {
        self.output.exact_domains(&self.output_schema)
    }
}

/// Complete live relation catalog and bags after one ordered statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalLifecycleCheckpoint {
    /// Canonical layer responsible for this checkpoint.
    pub layer_id: String,
    /// Complete live physical relation populations, in deterministic relation order.
    pub states: Vec<PhysicalWriteState>,
    /// Current typed schemas for the witnessed physical relations.
    pub schemas: Vec<RelationSchema>,
}

/// Joint proof of each write, intermediate SELECT, and final physical catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalOrderedLifecycleExact {
    /// Every exact write transition, in SQL order.
    pub transitions: Vec<PhysicalWriteTransition>,
    /// Every exact read checkpoint, in SQL order.
    pub reads: Vec<PhysicalLifecycleRead>,
    /// One complete live catalog snapshot after each statement layer.
    pub checkpoints: Vec<PhysicalLifecycleCheckpoint>,
    /// Complete final live physical states after the last statement.
    pub final_states: Vec<PhysicalWriteState>,
}

/// Proof strength for a complete mixed DML/DDL/read lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalOrderedLifecycleProof {
    /// All statements, physical bags and SELECTs have exact evidence.
    Exact(Box<PhysicalOrderedLifecycleExact>),
    /// A proven schema, existence or row-state contradiction.
    Impossible { reason: String },
    /// A missing source, unsupported SELECT or unproved dialect/session law.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalOrderedLifecycleProof {
    PhysicalOrderedLifecycleProof::Residual {
        reason: reason.into(),
    }
}

fn integer_type(data_type: &DataType) -> bool {
    match data_type {
        DataType::SignedInteger { bits: Some(bits) } => *bits > 0 && *bits <= 128,
        DataType::Nullable(inner) => integer_type(inner),
        _ => false,
    }
}

fn read_at_checkpoint(
    bundle: &AnalysisBundle,
    layer_id: &str,
    query: &crate::protocol::QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    schemas: &BTreeMap<String, RelationSchema>,
) -> Result<PhysicalLifecycleRead, String> {
    let [source] = query.sources() else {
        return Err("ordered SELECT needs exactly one physical source".to_string());
    };
    if query.dependencies().len() != 1
        || query.dependencies()[0] != source.name()
        || query.cte_program().is_some()
    {
        return Err("ordered SELECT has an unproved physical producer".to_string());
    }
    let state = states
        .get(source.name())
        .ok_or_else(|| format!("missing complete SELECT population: {}", source.name()))?;
    let schema = schemas
        .get(source.name())
        .ok_or_else(|| format!("missing typed SELECT schema: {}", source.name()))?;
    let mut names = BTreeSet::new();
    let mut projected_schema = Vec::new();
    for column in query.output().columns() {
        if !names.insert(column.name()) {
            return Err("ambiguous SELECT output column identity".to_string());
        }
        let [lineage] = column.lineage() else {
            return Err("SELECT output has no unique physical column evidence".to_string());
        };
        if lineage.relation() != source.name() {
            return Err("SELECT output lineage resolves to a different relation".to_string());
        }
        let Some(physical) = schema
            .columns()
            .iter()
            .find(|col| col.name() == lineage.column())
        else {
            return Err("SELECT output references an absent physical column".to_string());
        };
        if !integer_type(physical.data_type()) {
            return Err("SELECT output type needs an independently proven scalar law".to_string());
        }
        // The row evaluator supports more expressions, but only identity
        // projections retain authoritative physical datatype evidence here.
        if !matches!(column.expression(), crate::protocol::Expression::Column(_)) {
            return Err("computed SELECT output requires a typed projection proof".to_string());
        }
        projected_schema.push(
            SchemaColumn::new(column.name(), physical.data_type().clone())
                .map_err(|error| format!("invalid SELECT column: {error}"))?,
        );
    }
    let selected = evaluate_ordered_read(bundle, query, states)?;
    let mut output_rows = Vec::new();
    let mut surviving = Vec::new();
    for (ordinal, values) in selected {
        if values.len() != projected_schema.len() {
            return Err("SELECT projection arity differs from physical schema".to_string());
        }
        let row = query
            .output()
            .columns()
            .iter()
            .map(|col| col.name().to_string())
            .zip(values)
            .collect::<BTreeMap<String, LiteralValue>>();
        let Some(row) = PhysicalWriteRow::new(row) else {
            return Err("invalid SELECT row projection".to_string());
        };
        surviving.push(ordinal);
        output_rows.push(row);
    }
    let output = PhysicalWriteState::new(layer_id, output_rows)
        .ok_or_else(|| "SELECT layer has no valid identity".to_string())?;
    let output_schema = RelationSchema::new(layer_id.to_string(), projected_schema)
        .map_err(|error| format!("invalid SELECT typed output: {error}"))?;
    if output.exact_domains(&output_schema).is_none() {
        return Err("SELECT output cannot be matched to its typed schema".to_string());
    }
    let survivor_set = surviving.iter().copied().collect::<BTreeSet<_>>();
    let rejected_source_ordinals = (0..state.rows().len())
        .filter(|ordinal| !survivor_set.contains(ordinal))
        .collect();
    Ok(PhysicalLifecycleRead {
        layer_id: layer_id.to_string(),
        source: state.clone(),
        source_schema: schema.clone(),
        output,
        output_schema,
        surviving_source_ordinals: surviving,
        rejected_source_ordinals,
    })
}

/// Prove a finite mixed CREATE/INSERT/ALTER/SELECT/DROP script from original
/// source populations. An Exact proof includes all intermediate catalogs and
/// SELECT bags, not just the final write-state count.
///
/// Unsupported SQL, ambiguous identity or missing typed metadata remains
/// Residual; proven contradictions are Impossible.
pub fn physical_ordered_lifecycle_plan(
    bundle: &AnalysisBundle,
    initial_states: &[PhysicalWriteState],
) -> PhysicalOrderedLifecycleProof {
    let (transitions, final_states) =
        match physical_ordered_write_state_plan(bundle, initial_states) {
            PhysicalWriteStatePlan::Feasible {
                transitions,
                final_states,
            } => (transitions, final_states),
            PhysicalWriteStatePlan::Impossible { reason, .. } => {
                return PhysicalOrderedLifecycleProof::Impossible { reason };
            }
            PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        };
    let mut states = BTreeMap::new();
    for state in initial_states {
        if states
            .insert(state.relation().to_string(), state.clone())
            .is_some()
        {
            return PhysicalOrderedLifecycleProof::Impossible {
                reason: "duplicate initial physical relation identity".to_string(),
            };
        }
    }
    let mut schemas = bundle
        .source_schemas()
        .iter()
        .map(|schema| (schema.relation().to_string(), schema.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut next_write = transitions.iter();
    let mut reads = Vec::new();
    let mut checkpoints = Vec::new();
    for layer in bundle.layers() {
        if layer.write_kind().is_some() {
            let Some(transition) = next_write.next() else {
                return residual("missing ordered write transition");
            };
            if transition.layer_id() != layer.id() {
                return residual("write transition identity does not match SQL statement order");
            }
            let target = transition.before().relation();
            // A previously attested complete poststate must agree with the
            // next operation's preimage, never merely with its cardinality.
            if transition.before_exists() && states.get(target) != Some(transition.before()) {
                return residual(
                    "ordered source preimage disagrees with the canonical write proof",
                );
            }
            if !transition.before_exists() && states.contains_key(target) {
                return residual("unexpected live relation before CREATE");
            }
            if transition.after_exists() {
                let after = transition.after();
                if after.relation() != target {
                    states.remove(target);
                    schemas.remove(target);
                }
                states.insert(after.relation().to_string(), after.clone());
                if let Some(schema) = transition.after_schema() {
                    schemas.insert(after.relation().to_string(), schema.clone());
                }
            } else {
                states.remove(target);
                schemas.remove(target);
            }
        } else {
            let Some(query) = bundle
                .inputs()
                .iter()
                .find(|input| input.id() == layer.input_id())
                .and_then(|input| input.statements().get(layer.statement_index()))
                .and_then(|statement| match statement {
                    ProtocolStatement::Query(query) if query.write().is_none() => Some(query),
                    _ => None,
                })
            else {
                return residual("non-write statement has no canonical SELECT semantics");
            };
            let read = match read_at_checkpoint(bundle, layer.id(), query, &states, &schemas) {
                Ok(read) => read,
                Err(reason) => return residual(reason),
            };
            reads.push(read);
        }
        // Catalog membership is from actual, live states, not every source
        // schema mentioned elsewhere in this multi-statement bundle.
        let live_schemas = states
            .keys()
            .map(|name| schemas.get(name).cloned())
            .collect::<Option<Vec<_>>>();
        let Some(live_schemas) = live_schemas else {
            return residual("live catalog has a table without typed schema");
        };
        checkpoints.push(PhysicalLifecycleCheckpoint {
            layer_id: layer.id().to_string(),
            states: states.values().cloned().collect(),
            schemas: live_schemas,
        });
    }
    if next_write.next().is_some() || states.values().cloned().collect::<Vec<_>>() != final_states {
        return residual("lifecycle replay disagrees with canonical final state");
    }
    PhysicalOrderedLifecycleProof::Exact(Box::new(PhysicalOrderedLifecycleExact {
        transitions,
        reads,
        checkpoints,
        final_states,
    }))
}
