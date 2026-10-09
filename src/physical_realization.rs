//! Conservative physical-source witness composition over the canonical dependency graph.
//!
//! A proof of one row's membership is different from a proof of an entire
//! output's cardinality. Only a one-source filter followed by transparent
//! projections can currently be lifted to physical leaves. All other graph
//! shapes retain their typed topology and explicitly fail closed.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::{
    AnalysisBundle, ComposedSemantics, RelationResolution, TransformationLayer,
};
use crate::constructive::{
    local_constructive_witnesses, WitnessDirection, WitnessObligation, WitnessOperator,
};
use crate::protocol::{ProtocolStatement, QueryStatement, WriteKind};

/// Stable reference to a physical source or an in-bundle producer layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PhysicalPlanRef {
    /// Independent table whose rows can be controlled by a consumer.
    Source(String),
    /// Named transformation whose output must be materialized or evaluated.
    Layer(String),
}

/// One canonical producer node. Inputs reference other nodes rather than
/// copying their semantics or pretending that derived tables are writable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalPlanNode {
    id: PhysicalPlanRef,
    inputs: Vec<PhysicalPlanRef>,
    produced_relations: Vec<String>,
    write_kind: Option<WriteKind>,
}

impl PhysicalPlanNode {
    /// Identity shared by all users of a source or producer.
    pub fn id(&self) -> &PhysicalPlanRef {
        &self.id
    }

    /// Direct dependencies in deterministic relation order.
    pub fn inputs(&self) -> &[PhysicalPlanRef] {
        &self.inputs
    }

    /// Named datasets defined by this node.
    pub fn produced_relations(&self) -> &[String] {
        &self.produced_relations
    }

    /// None for query results and external physical sources.
    pub fn write_kind(&self) -> Option<WriteKind> {
        self.write_kind
    }
}

/// Why no end-to-end physical-source row proof was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalProofGap {
    /// The caller's layer ID is absent.
    UnknownTarget,
    /// A producer reference points to no layer.
    MissingProducer,
    /// A producer or relation is ambiguous.
    AmbiguousProducer,
    /// Cyclic dependencies cannot be physically solved by this composer.
    Cycle,
    /// A producer is partial or depends on prior target state.
    PartialProducer,
    /// A dependency cannot be represented safely.
    UnsupportedDependency,
    /// The layer's transitive semantics remain unresolved.
    UnresolvedSemantics,
    /// No source-level membership classification is proved.
    NoWitness,
    /// Local witnesses for different operators cannot be assumed jointly satisfiable.
    MultipleWitnesses,
    /// Only one-source row-preserving projection chains are currently invertible.
    NonInvertibleTransformation,
    /// One local operator's witness cannot yet be lifted through the graph.
    UnsupportedOperator,
    /// The witness still targets an intermediate, not a controlled physical leaf.
    IntermediateBoundary,
    /// The identified source differs from the leaf proved by the witness.
    UnboundPhysicalSource,
}

impl PhysicalProofGap {
    /// Stable identifier intended for diagnostics and downstream routing.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownTarget => "unknown_target",
            Self::MissingProducer => "missing_producer",
            Self::AmbiguousProducer => "ambiguous_producer",
            Self::Cycle => "cycle",
            Self::PartialProducer => "partial_producer",
            Self::UnsupportedDependency => "unsupported_dependency",
            Self::UnresolvedSemantics => "unresolved_semantics",
            Self::NoWitness => "no_witness",
            Self::MultipleWitnesses => "multiple_witnesses",
            Self::NonInvertibleTransformation => "non_invertible_transformation",
            Self::UnsupportedOperator => "unsupported_operator",
            Self::IntermediateBoundary => "intermediate_boundary",
            Self::UnboundPhysicalSource => "unbound_physical_source",
        }
    }
}

/// Proven physical-row classification, or a traceable unproved boundary.
///
/// These directions classify a chosen source row as surviving or rejected by
/// the terminal query. They do not promise whole-relation output cardinality,
/// final write state, or joint satisfiability across independent candidate rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSourcePlan {
    target_layer_id: String,
    nodes: Vec<PhysicalPlanNode>,
    sources: Vec<String>,
    qualifying: WitnessDirection,
    rejected: WitnessDirection,
    gap: Option<PhysicalProofGap>,
}

impl PhysicalSourcePlan {
    /// Target selected for the construction.
    pub fn target_layer_id(&self) -> &str {
        &self.target_layer_id
    }

    /// Reference-based, producer-first nodes; shared producers occur once.
    pub fn nodes(&self) -> &[PhysicalPlanNode] {
        &self.nodes
    }

    /// Distinct controllable physical relations, in lexicographic order.
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Sufficient qualifying witness for an individual physical row, if proved.
    pub fn qualifying(&self) -> &WitnessDirection {
        &self.qualifying
    }

    /// Sufficient rejection witness for an individual physical row, if proved.
    pub fn rejected(&self) -> &WitnessDirection {
        &self.rejected
    }

    /// Why physical row classification is unproved. None is not an
    /// assertion that arbitrary terminal row-count goals can be realized.
    pub fn gap(&self) -> Option<PhysicalProofGap> {
        self.gap
    }
}

fn residual(gap: PhysicalProofGap) -> WitnessDirection {
    WitnessDirection::Residual {
        reason: gap.as_str().to_string(),
    }
}

struct Walker<'a> {
    bundle: &'a AnalysisBundle,
    layers: BTreeMap<&'a str, &'a TransformationLayer>,
    visited: BTreeSet<String>,
    visiting: BTreeSet<String>,
    nodes: Vec<PhysicalPlanNode>,
    sources: BTreeSet<String>,
}

impl<'a> Walker<'a> {
    fn new(bundle: &'a AnalysisBundle) -> Self {
        Self {
            bundle,
            layers: bundle.layers().iter().map(|l| (l.id(), l)).collect(),
            visited: BTreeSet::new(),
            visiting: BTreeSet::new(),
            nodes: Vec::new(),
            sources: BTreeSet::new(),
        }
    }

    fn visit(&mut self, layer_id: &str) -> Result<(), PhysicalProofGap> {
        if self.visited.contains(layer_id) {
            return Ok(());
        }
        if !self.visiting.insert(layer_id.to_string()) {
            return Err(PhysicalProofGap::Cycle);
        }
        let result = self.visit_inputs(layer_id);
        self.visiting.remove(layer_id);
        result
    }

    fn visit_inputs(&mut self, layer_id: &str) -> Result<(), PhysicalProofGap> {
        let layer = self.layers.get(layer_id).copied().ok_or(PhysicalProofGap::MissingProducer)?;
        if !matches!(layer.write_kind(), None | Some(WriteKind::Definition)) {
            return Err(PhysicalProofGap::PartialProducer);
        }
        if !matches!(layer.composed_semantics(), ComposedSemantics::Resolved(_)) {
            return Err(PhysicalProofGap::UnresolvedSemantics);
        }
        let mut inputs = Vec::new();
        for edge in self.bundle.graph().edges().iter().filter(|e| e.consumer_layer_id() == layer_id) {
            match edge.resolution() {
                RelationResolution::External => {
                    let relation = edge.relation().to_string();
                    let reference = PhysicalPlanRef::Source(relation.clone());
                    self.sources.insert(relation.clone());
                    if !self.nodes.iter().any(|node| node.id == reference) {
                        self.nodes.push(PhysicalPlanNode {
                            id: reference.clone(),
                            inputs: Vec::new(),
                            produced_relations: vec![relation],
                            write_kind: None,
                        });
                    }
                    inputs.push(reference);
                }
                RelationResolution::Resolved => {
                    let [producer] = edge.producer_layer_ids() else {
                        return Err(PhysicalProofGap::AmbiguousProducer);
                    };
                    self.visit(producer)?;
                    inputs.push(PhysicalPlanRef::Layer(producer.clone()));
                }
                RelationResolution::Missing => return Err(PhysicalProofGap::MissingProducer),
                RelationResolution::Ambiguous => return Err(PhysicalProofGap::AmbiguousProducer),
                RelationResolution::Cycle => return Err(PhysicalProofGap::Cycle),
                RelationResolution::Partial => return Err(PhysicalProofGap::PartialProducer),
                RelationResolution::Unsupported => return Err(PhysicalProofGap::UnsupportedDependency),
            }
        }
        inputs.sort();
        inputs.dedup();
        self.nodes.push(PhysicalPlanNode {
            id: PhysicalPlanRef::Layer(layer.id().to_string()),
            inputs,
            produced_relations: layer.produces().iter().filter_map(|p| p.relation_name().map(str::to_string)).collect(),
            write_kind: layer.write_kind(),
        });
        self.visited.insert(layer_id.to_string());
        Ok(())
    }
}

fn query_for<'a>(bundle: &'a AnalysisBundle, layer: &TransformationLayer) -> Option<&'a QueryStatement> {
    let input = bundle.inputs().iter().find(|input| input.id() == layer.input_id())?;
    match input.statements().get(layer.statement_index())? {
        ProtocolStatement::Query(query) => Some(query),
        ProtocolStatement::Unsupported(_) => None,
    }
}

fn transparent_projection(query: &QueryStatement) -> bool {
    query.row_preserving_projection()
        && query.sources().len() == 1
        && query.diagnostics().is_empty()
        && query.condition_exactness().is_exact()
        && query.output().columns().iter().all(|column| column.plain_copy_source().is_some())
}

/// Construct canonical producer references, and lift an individual physical-row
/// predicate witness only when the entire path is transparently reversible.
///
/// Unknown and multi-operator cases deliberately return independent residual
/// directions. The returned topology is still useful when proof is residual:
/// no consumer should fabricate direct writes into derived tables.
pub fn physical_source_plan(bundle: &AnalysisBundle, target_layer_id: &str) -> PhysicalSourcePlan {
    let mut walker = Walker::new(bundle);
    let walk = if walker.layers.contains_key(target_layer_id) {
        walker.visit(target_layer_id)
    } else {
        Err(PhysicalProofGap::UnknownTarget)
    };
    let mut gap = walk.err();
    if gap.is_none() {
        let target = walker.layers.get(target_layer_id).copied();
        match target.and_then(|layer| match layer.composed_semantics() {
            ComposedSemantics::Resolved(semantics) => Some(semantics.as_ref()),
            ComposedSemantics::Unresolved(_) => None,
        }) {
            Some(semantics) => {
                let proofs = local_constructive_witnesses(semantics);
                let origin_proofs = proofs.iter().filter(|p| matches!(
                    (p.qualifying(), p.rejected()),
                    (WitnessDirection::Feasible(_), _) | (_, WitnessDirection::Feasible(_))
                )).collect::<Vec<_>>();
                gap = if proofs.len() != 1 || origin_proofs.len() != 1 {
                    Some(if proofs.is_empty() { PhysicalProofGap::NoWitness } else { PhysicalProofGap::MultipleWitnesses })
                } else if proofs[0].operator() != WitnessOperator::Boolean {
                    Some(PhysicalProofGap::UnsupportedOperator)
                } else if walker.sources.len() != 1 {
                    Some(PhysicalProofGap::UnboundPhysicalSource)
                } else if !walker.nodes.iter().filter_map(|node| match &node.id {
                    PhysicalPlanRef::Layer(id) => walker.layers.get(id.as_str()).copied(),
                    PhysicalPlanRef::Source(_) => None,
                }).all(|layer| {
                    let Some(query) = query_for(bundle, layer) else { return false; };
                    if layer.id() == proofs[0].origin_layer_id() {
                        query.sources().len() == 1
                            && query.joins().is_empty()
                            && query.aggregation().is_none()
                            && query.set_operation().is_none()
                            && query.window_witness().is_none()
                            && query.subquery_witnesses().is_empty()
                            && query.predicates().where_predicate().is_some()
                            && query.predicates().having_predicate().is_none()
                            && query.predicates().qualify_predicate().is_none()
                            && query.diagnostics().is_empty()
                            && query.output().columns().iter().all(|c| c.plain_copy_source().is_some())
                    } else {
                        transparent_projection(query)
                    }
                }) {
                    Some(PhysicalProofGap::NonInvertibleTransformation)
                } else if proofs[0].qualifying().is_residual()
                    || proofs[0].rejected().is_residual()
                {
                    Some(PhysicalProofGap::IntermediateBoundary)
                } else {
                    let only_source = walker.sources.iter().next().map(String::as_str);
                    let physical = |direction: &WitnessDirection| match direction {
                        WitnessDirection::Feasible(cases) => cases.iter().all(|case| {
                            case.obligations().iter().all(|obligation| match obligation {
                                WitnessObligation::Predicate(crate::constructive::WitnessFormula::RowTruth { row, .. }) =>
                                    Some(row.relation()) == only_source,
                                _ => false,
                            })
                        }),
                        _ => false,
                    };
                    if physical(proofs[0].qualifying()) && physical(proofs[0].rejected()) {
                        None
                    } else {
                        Some(PhysicalProofGap::UnboundPhysicalSource)
                    }
                };
                if gap.is_none() {
                    return PhysicalSourcePlan {
                        target_layer_id: target_layer_id.to_string(),
                        nodes: walker.nodes,
                        sources: walker.sources.into_iter().collect(),
                        qualifying: proofs[0].qualifying().clone(),
                        rejected: proofs[0].rejected().clone(),
                        gap: None,
                    };
                }
            }
            None => gap = Some(PhysicalProofGap::UnresolvedSemantics),
        }
    }
    let gap = gap.unwrap_or(PhysicalProofGap::NoWitness);
    PhysicalSourcePlan {
        target_layer_id: target_layer_id.to_string(),
        nodes: walker.nodes,
        sources: walker.sources.into_iter().collect(),
        qualifying: residual(gap),
        rejected: residual(gap),
        gap: Some(gap),
    }
}
