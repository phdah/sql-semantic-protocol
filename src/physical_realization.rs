//! Conservative physical-source witness composition over the canonical dependency graph.
//!
//! A proof of one row's membership is different from a proof of an entire
//! output's cardinality. Only a one-source filter followed by transparent
//! projections can currently be lifted to physical leaves. All other graph
//! shapes retain their typed topology and explicitly fail closed.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::{
    AnalysisBundle, ComposedSemantics, GroupBoundaryKind, RelationResolution, TransformationLayer,
};
use crate::constructive::{
    local_constructive_witnesses, ClosedWorldCoverage, CountBounds, ProofStrength, RowQuantifier,
    WitnessBoundary, WitnessCase, WitnessDirection, WitnessFormula, WitnessObligation,
    WitnessOperator, WitnessTerm,
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
    /// Contradictory exact physical column domains prevent a joint construction.
    ConflictingDomains,
    /// No source-level membership classification is proved.
    NoWitness,
    /// No safe classification was proved by the local witness.
    LocalWitnessUnproven,
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
            Self::ConflictingDomains => "conflicting_domains",
            Self::NoWitness => "no_witness",
            Self::LocalWitnessUnproven => "local_witness_unproven",
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
    zero_output: WitnessDirection,
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

    /// Sufficient closed-world source obligations for exactly zero terminal rows.
    /// This is independent of the single-row qualifying and rejected directions.
    pub fn zero_output(&self) -> &WitnessDirection {
        &self.zero_output
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
        let layer = self
            .layers
            .get(layer_id)
            .copied()
            .ok_or(PhysicalProofGap::MissingProducer)?;
        if !matches!(layer.write_kind(), None | Some(WriteKind::Definition)) {
            return Err(PhysicalProofGap::PartialProducer);
        }
        let mut inputs = Vec::new();
        for edge in self
            .bundle
            .graph()
            .edges()
            .iter()
            .filter(|e| e.consumer_layer_id() == layer_id)
        {
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
                RelationResolution::Unsupported => {
                    return Err(PhysicalProofGap::UnsupportedDependency)
                }
            }
        }
        // Prefer precise graph-edge failure reasons over generic unresolved semantics.
        if !matches!(layer.composed_semantics(), ComposedSemantics::Resolved(_)) {
            return Err(PhysicalProofGap::UnresolvedSemantics);
        }
        inputs.sort();
        inputs.dedup();
        self.nodes.push(PhysicalPlanNode {
            id: PhysicalPlanRef::Layer(layer.id().to_string()),
            inputs,
            produced_relations: layer
                .produces()
                .iter()
                .filter_map(|p| p.relation_name().map(str::to_string))
                .collect(),
            write_kind: layer.write_kind(),
        });
        self.visited.insert(layer_id.to_string());
        Ok(())
    }
}

fn query_for<'a>(
    bundle: &'a AnalysisBundle,
    layer: &TransformationLayer,
) -> Option<&'a QueryStatement> {
    let input = bundle
        .inputs()
        .iter()
        .find(|input| input.id() == layer.input_id())?;
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
        && query
            .output()
            .columns()
            .iter()
            .all(|column| column.plain_copy_source().is_some())
}

/// A completely empty controllable source ensures zero output only through
/// transformations whose row-shape cannot invent rows.
///
/// An empty source is not a constructive proof of any positive cardinality,
/// and must not be applied to global aggregates or source-free projections.
fn prove_zero_rows(bundle: &AnalysisBundle, walker: &Walker<'_>, target: &str) -> WitnessDirection {
    if walker.sources.is_empty() {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    }
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let Some(layer) = walker.layers.get(id.as_str()).copied() else {
            return residual(PhysicalProofGap::MissingProducer);
        };
        let Some(query) = query_for(bundle, layer) else {
            return residual(PhysicalProofGap::UnresolvedSemantics);
        };
        if query.sources().is_empty()
            || query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.proven_single_row_output()
            || !query.diagnostics().is_empty()
        {
            return residual(PhysicalProofGap::NonInvertibleTransformation);
        }
        if query.joins().is_empty() {
            // Single-source transparent copies and filter-only queries are
            // zero-preserving. No assumption about predicate satisfiability is
            // needed when the entire physical source is controlled as empty.
            if query.sources().len() != 1
                || !(query.row_preserving_projection() || query.filter_only_row_shape())
            {
                return residual(PhysicalProofGap::NonInvertibleTransformation);
            }
        } else {
            // A known binary join of *named, controlled* row sources is empty
            // when all inputs are empty, regardless of INNER/OUTER/SEMI/ANTI
            // multiplicity. An opaque joined relation or row producer may
            // emit rows independently of the named physical sources.
            let names = query
                .sources()
                .iter()
                .map(|source| source.name())
                .collect::<BTreeSet<_>>();
            if query.joins().iter().any(|join| {
                join.kind() == crate::protocol::JoinKind::Unknown
                    || !names.contains(join.left().relation())
                    || !names.contains(join.right().relation())
            }) || names
                .iter()
                .any(|name| !layer.consumes().contains(&layer.canonical_relation(name)))
            {
                return residual(PhysicalProofGap::UnsupportedOperator);
            }
        }
    }
    let Some(bounds) = CountBounds::new(0, Some(0)) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    let mut obligations = Vec::new();
    for source in &walker.sources {
        let Some(boundary) = WitnessBoundary::new(source, GroupBoundaryKind::Physical, target)
        else {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        };
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds,
            predicate: WitnessFormula::IsNull {
                term: WitnessTerm::Integer(1),
                negated: true,
            },
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
    }
    let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::UnresolvedSemantics))
}

/// Normalize sequential physical WHERE filters into one candidate-row
/// formula, then solve TRUE versus FALSE/UNKNOWN jointly. Distinct independent
/// sufficient examples must never simply be conjoined without this proof.
fn joint_physical_filters(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    semantics: &crate::bundle::ResolvedComposedSemantics,
) -> Option<(WitnessDirection, WitnessDirection)> {
    let filters = semantics.boolean_witnesses();
    if filters.len() <= 1 || filters.len() > 12 || walker.sources.len() != 1 {
        return None;
    }
    let source = walker.sources.iter().next()?;
    if filters.iter().any(|filter| {
        filter.boundary_kind() != GroupBoundaryKind::Physical
            || filter.witness().source_relation() != source
    }) {
        return None;
    }
    let origin_ids = filters
        .iter()
        .map(|filter| filter.origin_layer_id())
        .collect::<BTreeSet<_>>();
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if origin_ids.contains(layer.id()) {
            if !query.filter_only_row_shape()
                || query.sources().len() != 1
                || !query.joins().is_empty()
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || query.window_witness().is_some()
                || !query.subquery_witnesses().is_empty()
                || query.predicates().where_predicate().is_none()
                || query.predicates().having_predicate().is_some()
                || query.predicates().qualify_predicate().is_some()
                || !query.diagnostics().is_empty()
                || !query
                    .output()
                    .columns()
                    .iter()
                    .all(|c| c.plain_copy_source().is_some())
            {
                return None;
            }
        } else if !transparent_projection(query) {
            return None;
        }
    }
    let physical_witnesses = filters
        .iter()
        .map(|filter| filter.witness())
        .collect::<Vec<_>>();
    let joint =
        crate::boolean_witness::BooleanWitness::conjoin_physical_filters(&physical_witnesses)?;
    let row = crate::constructive::RowVariable::new(source, source, "candidate")?;
    let translate = |direction: &crate::boolean_witness::BooleanWitnessDirection| match direction {
        crate::boolean_witness::BooleanWitnessDirection::Exact(truth) => WitnessCase::new(
            vec![WitnessObligation::Predicate(WitnessFormula::RowTruth {
                row: row.clone(),
                predicate: joint.condition().clone(),
                truth: *truth,
            })],
            ProofStrength::Sufficient,
        )
        .and_then(|case| WitnessDirection::feasible(vec![case]))
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven)),
        crate::boolean_witness::BooleanWitnessDirection::Residual { .. } => {
            residual(PhysicalProofGap::LocalWitnessUnproven)
        }
    };
    Some((translate(joint.qualifying()), translate(joint.rejected())))
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
    let zero_output = if let Some(reason) = gap {
        residual(reason)
    } else {
        prove_zero_rows(bundle, &walker, target_layer_id)
    };
    if gap.is_none() {
        let target = walker.layers.get(target_layer_id).copied();
        match target.and_then(|layer| match layer.composed_semantics() {
            ComposedSemantics::Resolved(semantics) => Some(semantics.as_ref()),
            ComposedSemantics::Unresolved(_) => None,
        }) {
            Some(semantics) => {
                let proofs = local_constructive_witnesses(semantics);
                if !semantics
                    .column_domains()
                    .iter()
                    .any(|domain| matches!(domain.domain(), crate::protocol::ValueDomain::Empty))
                    && semantics.boolean_witnesses().len() == proofs.len()
                {
                    if let Some((qualifying, rejected)) =
                        joint_physical_filters(bundle, &walker, semantics)
                    {
                        let partial = matches!(qualifying, WitnessDirection::Residual { .. })
                            || matches!(rejected, WitnessDirection::Residual { .. });
                        return PhysicalSourcePlan {
                            target_layer_id: target_layer_id.to_string(),
                            nodes: walker.nodes,
                            sources: walker.sources.into_iter().collect(),
                            qualifying,
                            rejected,
                            zero_output,
                            gap: partial.then_some(PhysicalProofGap::LocalWitnessUnproven),
                        };
                    }
                }

                gap = if semantics
                    .column_domains()
                    .iter()
                    .any(|domain| matches!(domain.domain(), crate::protocol::ValueDomain::Empty))
                {
                    Some(PhysicalProofGap::ConflictingDomains)
                } else if proofs.is_empty() {
                    Some(PhysicalProofGap::NoWitness)
                } else if proofs.len() != 1 {
                    Some(PhysicalProofGap::MultipleWitnesses)
                } else if proofs[0].operator() != WitnessOperator::Boolean {
                    Some(PhysicalProofGap::UnsupportedOperator)
                } else if !matches!(
                    (proofs[0].qualifying(), proofs[0].rejected()),
                    (WitnessDirection::Feasible(_), _) | (_, WitnessDirection::Feasible(_))
                ) {
                    Some(PhysicalProofGap::LocalWitnessUnproven)
                } else if walker.sources.len() != 1 {
                    Some(PhysicalProofGap::UnboundPhysicalSource)
                } else if !walker
                    .nodes
                    .iter()
                    .filter_map(|node| match &node.id {
                        PhysicalPlanRef::Layer(id) => walker.layers.get(id.as_str()).copied(),
                        PhysicalPlanRef::Source(_) => None,
                    })
                    .all(|layer| {
                        let Some(query) = query_for(bundle, layer) else {
                            return false;
                        };
                        if layer.id() == proofs[0].origin_layer_id() {
                            query.filter_only_row_shape()
                                && query.sources().len() == 1
                                && query.joins().is_empty()
                                && query.aggregation().is_none()
                                && query.set_operation().is_none()
                                && query.window_witness().is_none()
                                && query.subquery_witnesses().is_empty()
                                && query.predicates().where_predicate().is_some()
                                && query.predicates().having_predicate().is_none()
                                && query.predicates().qualify_predicate().is_none()
                                && query.diagnostics().is_empty()
                                && query
                                    .output()
                                    .columns()
                                    .iter()
                                    .all(|c| c.plain_copy_source().is_some())
                        } else {
                            transparent_projection(query)
                        }
                    })
                {
                    Some(PhysicalProofGap::NonInvertibleTransformation)
                } else {
                    let only_source = walker.sources.iter().next().map(String::as_str);
                    let physical = |direction: &WitnessDirection| match direction {
                        WitnessDirection::Feasible(cases) => cases.iter().all(|case| {
                            case.obligations()
                                .iter()
                                .all(|obligation| match obligation {
                                    WitnessObligation::Predicate(
                                        crate::constructive::WitnessFormula::RowTruth {
                                            row, ..
                                        },
                                    ) => Some(row.relation()) == only_source,
                                    _ => false,
                                })
                        }),
                        WitnessDirection::Impossible | WitnessDirection::Residual { .. } => true,
                    };
                    if physical(proofs[0].qualifying()) && physical(proofs[0].rejected()) {
                        let still_residual =
                            matches!(proofs[0].qualifying(), WitnessDirection::Residual { .. })
                                || matches!(
                                    proofs[0].rejected(),
                                    WitnessDirection::Residual { .. }
                                );
                        return PhysicalSourcePlan {
                            target_layer_id: target_layer_id.to_string(),
                            nodes: walker.nodes,
                            sources: walker.sources.into_iter().collect(),
                            qualifying: proofs[0].qualifying().clone(),
                            rejected: proofs[0].rejected().clone(),
                            zero_output,
                            gap: still_residual.then_some(PhysicalProofGap::IntermediateBoundary),
                        };
                    }
                    Some(PhysicalProofGap::UnboundPhysicalSource)
                };
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
        zero_output,
        gap: Some(gap),
    }
}
