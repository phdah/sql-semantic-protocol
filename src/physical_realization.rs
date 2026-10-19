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
    local_constructive_witnesses, local_pending_producers, ClosedWorldCoverage,
    ConstructiveWitness, CountBounds, JoinPopulationPattern, ProofStrength, RowQuantifier,
    RowVariable, WitnessBoundary, WitnessCase, WitnessDirection, WitnessFormula, WitnessObligation,
    WitnessOperator, WitnessTerm,
};
use crate::protocol::{
    Expression, GroupBy, GroupingExpression, Predicate, ProtocolStatement, QueryStatement,
    SetOperand, SetOperation, WriteKind,
};

/// Whether a relation carries an unproven constraint that blocks closed-world
/// physical plans. Centralizes the repeated relation-constraint check so all
/// physical engines fail closed on the same evidence.
pub(super) fn blocks_unconstrained_physical_plans(bundle: &AnalysisBundle, relation: &str) -> bool {
    bundle
        .relation_constraints()
        .iter()
        .any(|set| set.relation() == relation && set.blocks_unconstrained_physical_plans())
}

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
    operator_witnesses: Vec<ConstructiveWitness>,
    pending_producers: Vec<WitnessObligation>,
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

    /// Operator-local facts originating in this node, not claims that
    /// distinct sufficient cases can be satisfied simultaneously.
    pub fn operator_witnesses(&self) -> &[ConstructiveWitness] {
        &self.operator_witnesses
    }

    /// Intermediate boundaries requiring upstream producer realization.
    pub fn pending_producers(&self) -> &[WitnessObligation] {
        &self.pending_producers
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
    /// Correlated physical row identities or multiplicities lack a common complete assignment.
    UnprovedCrossRowCorrelation,
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
            Self::UnprovedCrossRowCorrelation => "unproved_cross_row_correlation",
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

/// One exact terminal output-cardinality target in a joint proof.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalRowTarget {
    layer_id: String,
    rows: u64,
}

impl PhysicalRowTarget {
    /// Unique producer/query layer whose complete row count is requested.
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }

    /// Exact requested number of terminal output rows.
    pub fn rows(&self) -> u64 {
        self.rows
    }
}

/// One canonical multi-terminal physical graph and its *joint* proof status.
///
/// Producer-first nodes have stable typed identities and are defined once
/// even when multiple terminals reuse an upstream layer. A residual retains
/// the available graph, but never licenses a partial producer as writable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalJointSourcePlan {
    targets: Vec<PhysicalRowTarget>,
    nodes: Vec<PhysicalPlanNode>,
    sources: Vec<String>,
    outcome: WitnessDirection,
    gap: Option<PhysicalProofGap>,
}

impl PhysicalJointSourcePlan {
    /// Sorted terminal goals, deduplicated by layer identity.
    pub fn targets(&self) -> &[PhysicalRowTarget] {
        &self.targets
    }

    /// Producer-first canonical graph with each shared node defined once.
    pub fn nodes(&self) -> &[PhysicalPlanNode] {
        &self.nodes
    }

    /// Independent, uniquely identified controllable physical sources.
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Constructive typed obligations for the entire set of terminal goals.
    pub fn outcome(&self) -> &WitnessDirection {
        &self.outcome
    }

    /// A structural or cross-row proof gap, if detected.
    /// Other semantic limitations are retained in the residual outcome.
    pub fn gap(&self) -> Option<PhysicalProofGap> {
        self.gap
    }
}

/// Collect a single topologically ordered physical graph and verify several
/// terminal count goals against the same shared physical assignments.
/// Request order never changes node order or constructive proof identity.
pub fn physical_joint_source_plan(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> PhysicalJointSourcePlan {
    let mut requested = targets
        .iter()
        .map(|&(layer_id, rows)| PhysicalRowTarget {
            layer_id: layer_id.to_string(),
            rows,
        })
        .collect::<Vec<_>>();
    requested.sort();
    requested.dedup();
    let mut walker = Walker::new(bundle);
    let mut gap = None;
    for target in &requested {
        if gap.is_some() {
            break;
        }
        let result = if walker.layers.contains_key(target.layer_id()) {
            walker.visit(target.layer_id())
        } else {
            Err(PhysicalProofGap::UnknownTarget)
        };
        if let Err(reason) = result {
            gap = Some(reason);
        }
    }
    let mut outcome = if let Some(reason) = gap {
        residual(reason)
    } else {
        let pairs = requested
            .iter()
            .map(|target| (target.layer_id(), target.rows()))
            .collect::<Vec<_>>();
        physical_joint_row_count_plan(bundle, &pairs)
    };
    if gap.is_none()
        && matches!(outcome, WitnessDirection::Residual { .. })
        && walker.nodes.iter().any(|node| {
            node.operator_witnesses()
                .iter()
                .any(|witness| witness.operator() != WitnessOperator::Boolean)
        })
    {
        // Operator-local join, group, set, window and subquery examples are
        // never promoted to a shared-row, cross-operator physical proof.
        gap = Some(PhysicalProofGap::UnprovedCrossRowCorrelation);
        outcome = residual(PhysicalProofGap::UnprovedCrossRowCorrelation);
    }
    PhysicalJointSourcePlan {
        targets: requested,
        nodes: walker.nodes,
        sources: walker.sources.into_iter().collect(),
        outcome,
        gap,
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
        let partial_write = !matches!(layer.write_kind(), None | Some(WriteKind::Definition));
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
                            operator_witnesses: Vec::new(),
                            pending_producers: Vec::new(),
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
                RelationResolution::Partial => {
                    // Preserve all known partial producer nodes and write-kind
                    // references for the consumer, but never treat their
                    // result state as a complete generated relation.
                    for producer in edge.producer_layer_ids() {
                        let _ = self.visit(producer);
                    }
                    return Err(PhysicalProofGap::PartialProducer);
                }
                RelationResolution::Unsupported => {
                    return Err(PhysicalProofGap::UnsupportedDependency)
                }
            }
        }
        // Prefer precise graph-edge failure reasons over generic unresolved semantics.
        if !partial_write && !matches!(layer.composed_semantics(), ComposedSemantics::Resolved(_)) {
            return Err(PhysicalProofGap::UnresolvedSemantics);
        }
        inputs.sort();
        inputs.dedup();

        let (operator_witnesses, pending_producers) = match layer.composed_semantics() {
            ComposedSemantics::Resolved(resolved) => (
                local_constructive_witnesses(resolved)
                    .into_iter()
                    .filter(|w| w.origin_layer_id() == layer.id())
                    .collect(),
                local_pending_producers(resolved)
                    .into_iter()
                    .filter(|obligation| {
                        matches!(
                            obligation,
                            WitnessObligation::Producer { boundary, .. }
                                if boundary.origin_layer_id() == layer.id()
                        )
                    })
                    .collect(),
            ),
            ComposedSemantics::Unresolved(_) => (Vec::new(), Vec::new()),
        };
        self.nodes.push(PhysicalPlanNode {
            id: PhysicalPlanRef::Layer(layer.id().to_string()),
            inputs,
            produced_relations: layer
                .produces()
                .iter()
                .filter_map(|p| p.relation_name().map(str::to_string))
                .collect(),
            write_kind: layer.write_kind(),
            operator_witnesses,
            pending_producers,
        });
        self.visited.insert(layer_id.to_string());
        if partial_write {
            // Retain DML provenance and write kind in the canonical graph,
            // but never certify it as a complete physical-source producer.
            Err(PhysicalProofGap::PartialProducer)
        } else {
            Ok(())
        }
    }
}

// Implementations are grouped by physical proof responsibility.
mod aggregate_bags;
mod aggregate_joins;
mod composition;
mod counts;
mod distributions;
mod filters;
mod finite_chains;
mod graph;
mod grouping;
mod join_finite;
mod join_temporal;
mod join_uniform;
mod joins;
mod joint_counts;
mod joint_distributions;
mod membership;
mod operators;
mod ranking;
mod rank_bags;
mod set_bags;
mod set_trees;
mod sets;
mod write_states;

pub use rank_bags::{
    physical_finite_rank_bag_plan, PhysicalFiniteRankExact, PhysicalFiniteRankProof, PhysicalRankRow,
};
pub use aggregate_bags::{
    physical_finite_aggregate_bag_plan, PhysicalAggregateGroup, PhysicalAggregateTruth,
    PhysicalFiniteAggregateExact, PhysicalFiniteAggregateProof,
};
pub use aggregate_joins::{
    physical_finite_join_aggregate_plan, PhysicalJoinedAggregateExact, PhysicalJoinedAggregateProof,
};
pub use counts::{
    physical_rejected_row_count_plan, physical_row_count_plan, physical_unconditional_delete_plan,
};
pub(crate) use distributions::{
    physical_distribution_conflict, physical_distribution_plan,
    physical_inverted_set_distribution_plan,
};
pub use filters::physical_source_plan;
pub use finite_chains::{
    physical_finite_chain_bag_plan, PhysicalFiniteChainColumnDomain, PhysicalFiniteChainExact,
    PhysicalFiniteChainProof,
};
pub use joint_counts::physical_joint_row_count_plan;
pub use joint_distributions::physical_joint_distribution_plan;
pub(crate) use operators::{
    physical_materialized_group_or_rank_witness, physical_materialized_join_witness,
    physical_materialized_set_witness, physical_operator_count_witness,
};
pub use set_bags::{
    physical_finite_set_bag_plan, PhysicalFiniteSetExact, PhysicalFiniteSetProof,
    PhysicalSetColumnDomain, PhysicalSetTuple,
};
pub use set_trees::{
    physical_finite_set_tree_plan, PhysicalFiniteSetTreeExact, PhysicalFiniteSetTreeProof,
    PhysicalSetTreeBranch, PhysicalSetTreeTuple,
};
pub use write_states::{
    physical_closed_world_schema_plan, physical_ordered_write_state_plan,
    physical_ordered_write_state_plan_with_identities, physical_typed_append_schema_plan,
    physical_typed_closed_world_schema_plan, physical_typed_ordered_insert_plan,
    physical_typed_ordered_write_plan, physical_typed_value_plan, PhysicalIdentitySequence,
    PhysicalTypeProof, PhysicalTypedPopulationPlan, PhysicalTypedValue, PhysicalTypedWriteRow,
    PhysicalTypedWriteState, PhysicalWriteColumnDomain, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStateGap, PhysicalWriteStatePlan, PhysicalWriteTransition,
};

use composition::*;
use filters::*;
use graph::*;
use grouping::*;
use joins::*;
use joint_counts::*;
use membership::*;
use ranking::*;
use sets::*;
