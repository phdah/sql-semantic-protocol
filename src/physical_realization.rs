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
    ConstructiveWitness, CountBounds, ProofStrength, RowQuantifier, WitnessBoundary, WitnessCase,
    WitnessDirection, WitnessFormula, WitnessObligation, WitnessOperator, WitnessTerm,
};
use crate::protocol::{
    Expression, GroupBy, GroupingExpression, Predicate, ProtocolStatement, QueryStatement,
    SetOperand, SetOperation, WriteKind,
};

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

    /// Structural gap, if the dependency graph cannot be fully resolved.
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
    let outcome = if let Some(reason) = gap {
        residual(reason)
    } else {
        let pairs = requested
            .iter()
            .map(|target| (target.layer_id(), target.rows()))
            .collect::<Vec<_>>();
        physical_joint_row_count_plan(bundle, &pairs)
    };
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

/// A regular, non-empty grouping key list cannot form a group from no input
/// rows. ROLLUP, CUBE and GROUPING SETS may contain the empty grouping set,
/// which emits a global group even when every input table is empty.
/// Conservative expressions that evaluate once per input row without
/// arithmetic overflow, casts, opaque functions or row-generating behavior.
/// A CASE of simple source comparisons and literal/copy results is safe for
/// cardinality, even though its produced value is not invertible lineage.
fn total_scalar_projection(expression: &Expression) -> bool {
    match expression {
        Expression::Column(_) | Expression::Literal(_) => true,
        Expression::Case(case) => {
            case.operand().is_none_or(total_scalar_projection)
                && case.branches().iter().all(|branch| {
                    let condition_is_total = match branch.condition() {
                        Expression::BooleanPredicate(predicate) => {
                            total_scalar_predicate(predicate)
                        }
                        condition => total_scalar_projection(condition),
                    };
                    condition_is_total && total_scalar_projection(branch.result())
                })
                && case.else_result().is_none_or(total_scalar_projection)
        }
        Expression::BooleanPredicate(predicate) => total_scalar_predicate(predicate),
        Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::Function(_)
        | Expression::ScalarSubquery(_)
        | Expression::SignedIntegerCast(_)
        | Expression::Unary(_)
        | Expression::Binary(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => false,
    }
}

fn total_scalar_predicate(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Comparison(comparison) => {
            matches!(
                comparison.left(),
                Expression::Column(_) | Expression::Literal(_)
            ) && matches!(
                comparison.right(),
                Expression::Column(_) | Expression::Literal(_)
            )
        }
        Predicate::IsNull(test) => total_scalar_projection(test.expression()),
        Predicate::And(logical) | Predicate::Or(logical) => {
            logical.operands().iter().all(total_scalar_predicate)
        }
        Predicate::Not(negated) => total_scalar_predicate(negated.operand()),
        Predicate::LikePrefix(like) => total_scalar_projection(like.expression()),
        Predicate::Between(_)
        | Predicate::In(_)
        | Predicate::BooleanExpression(_)
        | Predicate::Exists(_)
        | Predicate::InSubquery(_)
        | Predicate::Unknown(_)
        | Predicate::Unsupported(_) => false,
    }
}

fn empty_input_eliminates_groups(query: &QueryStatement) -> bool {
    matches!(
        query.aggregation().and_then(|aggregation| aggregation.group_by()),
        Some(GroupBy::Expressions(groups))
            if !groups.is_empty()
                && groups.iter().all(|group| matches!(group, GroupingExpression::Expression(_)))
    )
}

fn row_local_predicate(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Comparison(comparison) => {
            row_local_expression(comparison.left()) && row_local_expression(comparison.right())
        }
        Predicate::LikePrefix(like) => row_local_expression(like.expression()),
        Predicate::And(logical) | Predicate::Or(logical) => {
            logical.operands().iter().all(row_local_predicate)
        }
        Predicate::Not(not) => row_local_predicate(not.operand()),
        Predicate::IsNull(null) => row_local_expression(null.expression()),
        Predicate::In(values) => {
            row_local_expression(values.expression())
                && values.values().iter().all(row_local_expression)
        }
        Predicate::Between(between) => {
            row_local_expression(between.expression())
                && row_local_expression(between.lower())
                && row_local_expression(between.upper())
        }
        Predicate::BooleanExpression(expression) => row_local_expression(expression),
        Predicate::Exists(_)
        | Predicate::InSubquery(_)
        | Predicate::Unknown(_)
        | Predicate::Unsupported(_) => false,
    }
}

/// In the absence of grouping, expressions must be scalar on each input row.
/// An aggregate nested inside COALESCE, arithmetic or a cast can synthesize a
/// single result on an empty input, even if the query's projection/WHERE shape
/// otherwise looks row-preserving.
fn row_local_expression(expression: &Expression) -> bool {
    match expression {
        Expression::Column(_) | Expression::Literal(_) => true,
        Expression::Function(function) => function.arguments().iter().all(row_local_expression),
        Expression::SignedIntegerCast(cast) => row_local_expression(cast.expression()),
        Expression::Unary(unary) => row_local_expression(unary.operand()),
        Expression::Binary(binary) => {
            row_local_expression(binary.left()) && row_local_expression(binary.right())
        }
        Expression::Case(case) => {
            case.operand().is_none_or(row_local_expression)
                && case.branches().iter().all(|branch| {
                    row_local_expression(branch.condition())
                        && row_local_expression(branch.result())
                })
                && case.else_result().is_none_or(row_local_expression)
        }
        Expression::BooleanPredicate(predicate) => row_local_predicate(predicate),
        Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => false,
    }
}

/// Every query-shaped leaf must be a single-row-boundary read, without
/// GROUPING SETS, global aggregates or opaque producers. Set operators,
/// including DISTINCT and nested bag operations, cannot create a tuple from
/// entirely empty leaf inputs.
fn empty_input_eliminates_set(operation: &SetOperation, layer: &TransformationLayer) -> bool {
    fn leaf_count(operation: &SetOperation) -> usize {
        let count = |operand: &SetOperand| match operand {
            SetOperand::Query => 1,
            SetOperand::Operation(nested) => leaf_count(nested),
        };
        count(operation.left()) + count(operation.right())
    }
    if operation.branches().len() != leaf_count(operation) {
        return false;
    }
    operation.branches().iter().all(|branch| {
        branch.sources().len() == 1
            && branch.empty_input_preserving()
            && branch.predicates().having_predicate().is_none()
            && branch.sources().iter().all(|source| {
                layer
                    .consumes()
                    .contains(&layer.canonical_relation(source.name()))
            })
    })
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
        let regular_grouping = empty_input_eliminates_groups(query);
        let duplicate_elimination = query
            .aggregation()
            .is_some_and(|aggregation| aggregation.distinct() && aggregation.group_by().is_none());
        let ranked_window = query.ranked_goal_output_shape() && query.window_witness().is_some();
        let set_empty = query
            .set_operation()
            .is_some_and(|operation| empty_input_eliminates_set(operation, layer));
        if (query.sources().is_empty() && !set_empty)
            || (query.aggregation().is_some() && !regular_grouping && !duplicate_elimination)
            || (query.set_operation().is_some() && !set_empty)
            || query.proven_single_row_output()
            || (query.predicates().having_predicate().is_some() && !regular_grouping)
            // Unsupported ORDER BY value/order expressions may affect which
            // nonempty rows survive LIMIT, but cannot synthesize rows from
            // an empty input. All other diagnostics remain blocking.
            || (!regular_grouping
                && !set_empty
                && query
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code() != "unsupported_order_by"))
            || (!regular_grouping
                && !ranked_window
                && !set_empty
                && !query
                    .output()
                    .columns()
                    .iter()
                    .all(|column| row_local_expression(column.expression())))
        {
            return residual(PhysicalProofGap::NonInvertibleTransformation);
        }
        if set_empty {
            // Individual leaves already establish a row-preserving, named
            // boundary. Any bag or set operator on zero leaves returns zero.
            continue;
        }
        if query.joins().is_empty() {
            // Single-source transparent copies and filter-only queries are
            // zero-preserving. No assumption about predicate satisfiability is
            // needed when the entire physical source is controlled as empty.
            if query.sources().len() != 1
                || !(query.row_preserving_projection()
                    || query.filter_only_row_shape()
                    || regular_grouping
                    || duplicate_elimination
                    || ranked_window)
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

/// Follow an identity-only column through an intermediate producer. The
/// producer is permitted to filter its own rows because the *joint* terminal
/// proof later requires all filters, but the intermediate's local witness is
/// never independently relabeled as a physical-source sufficient proof.
fn resolve_filter_column(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    consumer_id: &str,
    column: &crate::protocol::ColumnRef,
    depth: usize,
) -> Option<crate::protocol::ColumnRef> {
    resolve_filter_column_with_evidence(bundle, walker, consumer_id, column, depth, false)
}

/// Trace the same identity-only reference, optionally proving equal declared
/// datatypes across *every* materialization along the producer path. A
/// matching endpoint alone cannot justify unseen intermediate coercions.
fn resolve_filter_column_with_evidence(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    consumer_id: &str,
    column: &crate::protocol::ColumnRef,
    depth: usize,
    certified_types: bool,
) -> Option<crate::protocol::ColumnRef> {
    if depth > walker.layers.len() {
        return None;
    }
    let layer = walker.layers.get(consumer_id).copied()?;
    let canonical = layer.canonical_relation(column.relation()?);
    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.consumer_layer_id() == consumer_id && edge.relation() == canonical)?;
    match edge.resolution() {
        RelationResolution::External => Some(crate::protocol::ColumnRef::new(
            Some(edge.relation().to_string()),
            column.name().to_string(),
        )),
        RelationResolution::Resolved => {
            let [producer_id] = edge.producer_layer_ids() else {
                return None;
            };
            let producer = walker.layers.get(producer_id.as_str()).copied()?;
            let query = query_for(bundle, producer)?;
            if query.sources().len() != 1
                || !(query.row_preserving_projection() || query.filter_only_row_shape())
                || !query.joins().is_empty()
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || !query.diagnostics().is_empty()
            {
                return None;
            }
            let mut output = query
                .output()
                .columns()
                .iter()
                .filter(|item| item.name() == column.name());
            let actual = output.next()?.plain_copy_source()?;
            if output.next().is_some() {
                return None;
            }
            let upstream = crate::protocol::ColumnRef::new(
                Some(actual.relation().to_string()),
                actual.column().to_string(),
            );
            if certified_types {
                let consumed_type = bundle
                    .source_schemas()
                    .iter()
                    .find(|schema| schema.relation() == canonical)?
                    .columns()
                    .iter()
                    .find(|item| item.name() == column.name())?
                    .data_type();
                let upstream_type = bundle
                    .source_schemas()
                    .iter()
                    .find(|schema| schema.relation() == upstream.relation()?)?
                    .columns()
                    .iter()
                    .find(|item| item.name() == upstream.name())?
                    .data_type();
                if consumed_type != upstream_type {
                    return None;
                }
            }
            resolve_filter_column_with_evidence(
                bundle,
                walker,
                producer.id(),
                &upstream,
                depth + 1,
                certified_types,
            )
        }
        _ => None,
    }
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
    if filters
        .iter()
        .any(|filter| filter.boundary_kind() == GroupBoundaryKind::Unresolved)
    {
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
        .map(|filter| match filter.boundary_kind() {
            GroupBoundaryKind::Physical => {
                (filter.witness().source_relation() == source).then(|| filter.witness().clone())
            }
            GroupBoundaryKind::Intermediate => filter.witness().mapped_to_physical(|column| {
                resolve_filter_column(bundle, walker, filter.origin_layer_id(), column, 0)
            }),
            GroupBoundaryKind::Unresolved => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if physical_witnesses
        .iter()
        .any(|witness| witness.source_relation() != source)
    {
        return None;
    }
    let physical_references = physical_witnesses.iter().collect::<Vec<_>>();
    let joint =
        crate::boolean_witness::BooleanWitness::conjoin_physical_filters(&physical_references)?;
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

/// Promote a physical-row TRUE or NOT TRUE classification to a
/// repeatable, schema-backed predicate only when the complete source is
/// controllable. An individually feasible candidate cannot be duplicated
/// arbitrarily when uniqueness or other source constraints are unknown.
fn repeatable_row_truth(
    bundle: &AnalysisBundle,
    physical: &PhysicalSourcePlan,
    direction: &WitnessDirection,
    required_truth: crate::boolean_witness::BooleanTruthCase,
) -> Option<WitnessFormula> {
    let [source] = physical.sources() else {
        return None;
    };
    if !bundle
        .source_schemas()
        .iter()
        .any(|schema| schema.relation() == source)
        || bundle.relation_constraints().iter().any(|set| {
            set.relation() == source
                && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
        })
    {
        return None;
    }
    let WitnessDirection::Feasible(cases) = direction else {
        return None;
    };
    let [case] = cases.as_slice() else {
        return None;
    };
    let [WitnessObligation::Predicate(
        formula @ WitnessFormula::RowTruth {
            row,
            predicate,
            truth,
        },
    )] = case.obligations()
    else {
        return None;
    };
    (row.relation() == source && predicate.is_exact() && *truth == required_truth)
        .then(|| formula.clone())
}

fn repeatable_filter_predicate(
    bundle: &AnalysisBundle,
    physical: &PhysicalSourcePlan,
) -> Option<WitnessFormula> {
    repeatable_row_truth(
        bundle,
        physical,
        physical.qualifying(),
        crate::boolean_witness::BooleanTruthCase::True,
    )
}

/// Supplement a physical count with a standalone scalar WHERE predicate
/// without modifying the emitted operator-local Boolean witness contract.
/// Only one direct, typed physical-source filter and subsequent transparent
/// producers are supported; competing filters require joint truth solving.
/// Only completely row-preserving, single-parent materialization chains
/// can transport physical predicates. Each node retains its producer kind,
/// and typed columns are separately checked at every reference boundary.
fn transparent_materialized_source(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    node: &PhysicalPlanNode,
    source: &str,
) -> bool {
    let [PhysicalPlanRef::Layer(initial_id)] = node.inputs() else {
        return false;
    };
    let mut next_id = initial_id.as_str();
    for _ in 0..walker.layers.len() {
        let Some(producer) = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(next_id.to_string()))
        else {
            return false;
        };
        let valid = walker
            .layers
            .get(next_id)
            .and_then(|layer| query_for(bundle, layer))
            .is_some_and(transparent_projection);
        if !valid || !matches!(producer.write_kind(), None | Some(WriteKind::Definition)) {
            return false;
        }
        match producer.inputs() {
            [PhysicalPlanRef::Source(actual)] => return actual == source,
            [PhysicalPlanRef::Layer(upstream)] => next_id = upstream,
            _ => return false,
        }
    }
    false
}

/// Type-verified scalar WHERE witness through either a direct source or one
/// complete materialization. Every row still refers to the canonical
/// physical source; the intermediate producer is never treated as writable.
fn scalar_physical_row_truth(
    bundle: &AnalysisBundle,
    physical: &PhysicalSourcePlan,
    target_layer_id: &str,
    required_truth: crate::boolean_witness::BooleanTruthCase,
) -> Option<WitnessFormula> {
    let [source] = physical.sources() else {
        return None;
    };
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source)?;
    if bundle.relation_constraints().iter().any(|set| {
        set.relation() == source && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
    }) {
        return None;
    }

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let mut filter = None;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if query.filter_only_row_shape() {
            let [input_source] = query.sources() else {
                return None;
            };
            let direct = node.inputs() == [PhysicalPlanRef::Source(source.clone())]
                && input_source.name() == source;
            let through_producer = transparent_materialized_source(bundle, &walker, node, source)
                && input_source.name() != source;
            if filter.is_some()
                || (!direct && !through_producer)
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || query.window_witness().is_some()
                || !query.subquery_witnesses().is_empty()
                || !query.condition_exactness().is_exact()
                || !query.diagnostics().is_empty()
                || query
                    .output()
                    .columns()
                    .iter()
                    .any(|column| !total_scalar_projection(column.expression()))
            {
                return None;
            }
            filter = Some((query, layer.id(), input_source.name()));
        } else if !transparent_projection(query) {
            return None;
        }
    }
    let (filter, origin_layer, filter_source) = filter?;
    let input_schema = bundle
        .source_schemas()
        .iter()
        .find(|candidate| candidate.relation() == filter_source)?;
    if bundle.relation_constraints().iter().any(|set| {
        set.relation() == filter_source
            && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
    }) {
        return None;
    }
    let witness = crate::boolean_witness::analyze_physical_scalar(
        filter.predicates().where_predicate(),
        filter.sources(),
        |column| {
            (column.relation() == Some(filter_source))
                .then(|| physical_integer_evidence(input_schema, column))
                .flatten()
        },
        |_column| None,
    )?;
    let witness = if filter_source == source {
        witness
    } else {
        witness.mapped_to_physical_certified(
            |column| {
                resolve_filter_column_with_evidence(
                    bundle,
                    &walker,
                    origin_layer,
                    column,
                    0,
                    true,
                )
            },
            |original, mapped| {
                let original_type = input_schema
                    .columns()
                    .iter()
                    .find(|known| known.name() == original.name())
                    .map(|known| known.data_type());
                let mapped_type = schema
                    .columns()
                    .iter()
                    .find(|known| known.name() == mapped.name())
                    .map(|known| known.data_type());
                original_type.is_some() && original_type == mapped_type
            },
        )?
    };
    let direction = match required_truth {
        crate::boolean_witness::BooleanTruthCase::True => witness.qualifying(),
        crate::boolean_witness::BooleanTruthCase::NotTrue => witness.rejected(),
    };
    if !matches!(
        direction,
        crate::boolean_witness::BooleanWitnessDirection::Exact(truth)
            if *truth == required_truth
    ) {
        return None;
    }
    let row = crate::constructive::RowVariable::new(source, source, "candidate")?;
    Some(WitnessFormula::RowTruth {
        row,
        predicate: witness.condition().clone(),
        truth: required_truth,
    })
}

/// Produce typed, whole-physical-source obligations for an exact terminal
/// row-count request. This is an independent constructive plan; it never
/// upgrades the single-candidate row classification returned by
/// [`physical_source_plan`].
///
/// Nonzero cardinality is constructive for source-free singletons,
/// single-source unfiltered row-preserving producer chains, and transparent
/// filter chains with a jointly proven, repeatable TRUE source-row predicate.
/// Physical leaves must have a compatible schema without unproved uniqueness
/// or source constraints.
/// A filter, join, aggregate, set, partial write or ambiguous source remains
/// residual rather than silently assuming independently sampled rows.
pub fn physical_row_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> WitnessDirection {
    let physical = physical_source_plan(bundle, target_layer_id);
    if rows == 0 {
        if let WitnessDirection::Feasible(cases) = physical.zero_output() {
            let Some(bounds) = CountBounds::new(0, Some(0)) else {
                return residual(PhysicalProofGap::NoWitness);
            };
            let cases = cases
                .iter()
                .filter_map(|case| {
                    let mut obligations = case.obligations().to_vec();
                    obligations.push(WitnessObligation::OutputRows {
                        layer_id: target_layer_id.to_string(),
                        bounds,
                    });
                    WitnessCase::new(obligations, ProofStrength::Sufficient)
                })
                .collect::<Vec<_>>();
            return WitnessDirection::feasible(cases)
                .unwrap_or_else(|| residual(PhysicalProofGap::NoWitness));
        }
    }
    if physical.nodes().is_empty() {
        return residual(physical.gap().unwrap_or(PhysicalProofGap::NoWitness));
    }
    let Some(target) = bundle.layers().iter().find(|l| l.id() == target_layer_id) else {
        return residual(PhysicalProofGap::UnknownTarget);
    };
    let Some(query) = query_for(bundle, target) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    if query.proven_single_row_output() && physical.sources().is_empty() {
        if rows != 1 {
            return WitnessDirection::Impossible;
        }
        let Some(bounds) = CountBounds::new(1, Some(1)) else {
            return residual(PhysicalProofGap::NoWitness);
        };
        let case = WitnessCase::new(
            vec![WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds,
            }],
            ProofStrength::Sufficient,
        );
        return case
            .and_then(|case| WitnessDirection::feasible(vec![case]))
            .unwrap_or_else(|| residual(PhysicalProofGap::NoWitness));
    }
    if physical.sources().len() != 1 {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    }
    // A complete physical input containing exactly 'rows' identical
    // qualifying rows produces exactly 'rows' terminal rows through the
    // already-proved identity-preserving filter/projection chain. This is
    // constructive only for unconstrained, typed source schemas; it does
    // not infer cardinalities from an isolated membership example.
    if rows > 0 {
        if let Some(predicate) = repeatable_filter_predicate(bundle, &physical).or_else(|| {
            scalar_physical_row_truth(
                bundle,
                &physical,
                target_layer_id,
                crate::boolean_witness::BooleanTruthCase::True,
            )
        }) {
            let source = &physical.sources()[0];
            let Some(boundary) =
                WitnessBoundary::new(source, GroupBoundaryKind::Physical, target_layer_id)
            else {
                return residual(PhysicalProofGap::UnboundPhysicalSource);
            };
            let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            };
            let Some(case) = WitnessCase::new(
                vec![
                    WitnessObligation::Rows {
                        boundary: boundary.clone(),
                        quantifier: RowQuantifier::ForAll,
                        bounds,
                        predicate,
                        closed_world: true,
                    },
                    WitnessObligation::ClosedWorld {
                        boundary,
                        coverage: ClosedWorldCoverage::EntireRelation,
                    },
                    WitnessObligation::OutputRows {
                        layer_id: target_layer_id.to_string(),
                        bounds,
                    },
                ],
                ProofStrength::Sufficient,
            ) else {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            };
            return WitnessDirection::feasible(vec![case])
                .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven));
        }
    }
    if physical.nodes().iter().any(|node| {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            return false;
        };
        let Some(layer) = bundle.layers().iter().find(|layer| layer.id() == id) else {
            return true;
        };
        let Some(query) = query_for(bundle, layer) else {
            return true;
        };
        !query.row_preserving_projection()
            || query.sources().len() != 1
            || !query.joins().is_empty()
            || query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.window_witness().is_some()
            || !query.subquery_witnesses().is_empty()
            || !query.diagnostics().is_empty()
    }) {
        return residual(PhysicalProofGap::NonInvertibleTransformation);
    }
    let Some(first_layer) = physical.nodes().iter().find_map(|node| {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            return None;
        };
        bundle.layers().iter().find(|layer| layer.id() == id)
    }) else {
        return residual(PhysicalProofGap::MissingProducer);
    };
    let Some(first_query) = query_for(bundle, first_layer) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    let goal = match crate::outcome_goals::OutcomeGoal::new(
        first_layer.id(),
        Some(rows),
        None,
        Vec::new(),
    ) {
        Ok(goal) => goal,
        Err(_) => return residual(PhysicalProofGap::NoWitness),
    };
    let Some(crate::outcome_proofs::OutcomeWitness::SourceRows {
        relation,
        rows: witness_rows,
        ..
    }) = crate::outcome_proofs::construct(bundle, first_layer, first_query, &goal)
    else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    if physical.sources().first() != Some(&relation) || witness_rows != rows {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    }
    let Some(boundary) =
        WitnessBoundary::new(&relation, GroupBoundaryKind::Physical, target_layer_id)
    else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
        return residual(PhysicalProofGap::NoWitness);
    };
    let Some(case) = WitnessCase::new(
        vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate: WitnessFormula::IsNull {
                    term: WitnessTerm::Integer(1),
                    negated: true,
                },
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds,
            },
        ],
        ProofStrength::Sufficient,
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}

/// Construct a deliberately rejected, nonempty physical source for a
/// terminal with exactly zero output rows. Every physical row must fail the
/// entire jointly proved filter path with SQL FALSE or UNKNOWN, and the
/// physical relation is closed to exclude otherwise qualifying rows.
///
/// This is stronger than a single rejected candidate witness: the caller
/// requests a complete physical input of `source_rows` rejected rows.
/// It is deliberately limited to unconstrained one-source Boolean filter
/// chains and never assumes row absence from an open-world sample.
pub fn physical_rejected_row_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    source_rows: u64,
) -> WitnessDirection {
    if source_rows == 0 {
        return physical_row_count_plan(bundle, target_layer_id, 0);
    }
    let physical = physical_source_plan(bundle, target_layer_id);
    if !matches!(physical.zero_output(), WitnessDirection::Feasible(_)) {
        return residual(
            physical
                .gap()
                .unwrap_or(PhysicalProofGap::NonInvertibleTransformation),
        );
    }
    let Some(predicate) = repeatable_row_truth(
        bundle,
        &physical,
        physical.rejected(),
        crate::boolean_witness::BooleanTruthCase::NotTrue,
    )
    .or_else(|| {
        scalar_physical_row_truth(
            bundle,
            &physical,
            target_layer_id,
            crate::boolean_witness::BooleanTruthCase::NotTrue,
        )
    }) else {
        return residual(
            physical
                .gap()
                .unwrap_or(PhysicalProofGap::LocalWitnessUnproven),
        );
    };
    let [source] = physical.sources() else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let Some(boundary) = WitnessBoundary::new(source, GroupBoundaryKind::Physical, target_layer_id)
    else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let (Some(source_bounds), Some(output_bounds)) = (
        CountBounds::new(source_rows, Some(source_rows)),
        CountBounds::new(0, Some(0)),
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    let Some(case) = WitnessCase::new(
        vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: source_bounds,
                predicate,
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds: output_bounds,
            },
        ],
        ProofStrength::Sufficient,
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}

/// Construct an exact before/after physical state for an unconditional
/// DELETE with a caller-controlled initial target. This is a *complete*
/// relation-state construction, not the affected-row cardinality estimate
/// from the standalone write-effect contract.
///
/// Other mutations require source/target collision, predicate and branch
/// proofs and deliberately remain residual. An in-bundle earlier producer
/// of the target invalidates an independent physical initial assignment.
pub fn physical_unconditional_delete_plan(
    bundle: &AnalysisBundle,
    layer_id: &str,
    initial_rows: u64,
) -> WitnessDirection {
    let Some(write) = bundle
        .write_state_effects()
        .into_iter()
        .find(|write| write.layer_id() == layer_id)
    else {
        return residual(PhysicalProofGap::UnknownTarget);
    };
    let effect = write.effect();
    if effect.post_state() != crate::protocol::WritePostState::Empty
        || effect.cardinality_rule() != crate::protocol::WriteCardinalityRule::SubtractDeletes
        || !write.sources().is_empty()
        || bundle
            .write_state_effects()
            .iter()
            .any(|other| other.layer_id() != layer_id && other.target() == write.target())
        || bundle.layers().iter().any(|layer| {
            layer.id() != layer_id
                && layer
                    .produces()
                    .iter()
                    .any(|relation| relation.relation_name() == Some(write.target()))
        })
        || !bundle
            .source_schemas()
            .iter()
            .any(|schema| schema.relation() == write.target())
        || bundle.relation_constraints().iter().any(|set| {
            set.relation() == write.target()
                && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
        })
        || effect.resulting_rows(
            initial_rows,
            crate::protocol::WriteRowCounts::new(0, 0, initial_rows),
        ) != Ok(0)
    {
        return residual(PhysicalProofGap::PartialProducer);
    }
    let Some(boundary) =
        WitnessBoundary::new(write.target(), GroupBoundaryKind::Physical, layer_id)
    else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let (Some(initial), Some(empty)) = (
        CountBounds::new(initial_rows, Some(initial_rows)),
        CountBounds::new(0, Some(0)),
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    let Some(case) = WitnessCase::new(
        vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: initial,
                predicate: count_tautology(),
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::StateRows {
                relation: write.target().to_string(),
                before: initial,
                after: empty,
            },
        ],
        ProofStrength::Sufficient,
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}

/// Lift complete scalar distributions across exact identity-only producer
/// chains without reparsing the SQL or fabricating writable intermediate
/// tables. Source histograms are safe only when every named producer preserves
/// row count and every requested column resolves to an unchanged physical
/// source column. Filters, calculations, joins and other row shaping are not
/// silently treated as transparent.
pub(crate) fn physical_distribution_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &crate::outcome_goals::OutcomeGoal,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    let rows = goal.rows()?;
    if rows == 0 || goal.groups().is_some() || goal.distributions().is_empty() {
        return None;
    }
    let physical = physical_source_plan(bundle, target_layer_id);
    let [source] = physical.sources() else {
        return None;
    };
    if !matches!(
        physical_row_count_plan(bundle, target_layer_id, rows),
        WitnessDirection::Feasible(_)
    ) {
        return None;
    }

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    // Source-column histograms require value identity *and* row identity.
    // A computed projection, even one with a row-preserving count, cannot be
    // inverted to source values by a consumer.
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if !transparent_projection(query)
            || query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.window_witness().is_some()
            || !query.subquery_witnesses().is_empty()
        {
            return None;
        }
    }
    let target = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, target)?;
    let mut mappings = Vec::new();
    for distribution in goal.distributions() {
        let mut matches = query
            .output()
            .columns()
            .iter()
            .filter(|column| column.name() == distribution.column());
        let copied = matches.next()?.plain_copy_source()?;
        if matches.next().is_some() {
            return None;
        }
        let column = crate::protocol::ColumnRef::new(
            Some(copied.relation().to_string()),
            copied.column().to_string(),
        );
        // Column lineage may already have been composed to a physical leaf
        // by the analyzer. Otherwise walk only proven copy-producer edges.
        // Both paths require the complete row/value-transparent DAG above.
        let physical_column = if copied.relation() == source {
            column
        } else {
            resolve_filter_column(bundle, &walker, target_layer_id, &column, 0)?
        };
        if physical_column.relation() != Some(source.as_str()) {
            return None;
        }
        mappings.push((
            physical_column.name().to_string(),
            distribution.values().to_vec(),
        ));
    }
    crate::outcome_proofs::construct_mapped_source(bundle, source, rows, mappings)
}

/// Reuse an independently proved source-level operator construction when
/// all later materialized producers are exact row-preserving projections.
/// The first transformation must consume only physical leaves: a join,
/// grouped aggregate, window rank or set across already-transformed inputs
/// needs a separate multi-operator proof and remains residual.
pub(crate) fn physical_operator_count_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    if rows == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let layer_ids = walker.nodes.iter().filter_map(|node| match node.id() {
        PhysicalPlanRef::Layer(id) => Some(id.as_str()),
        PhysicalPlanRef::Source(_) => None,
    });
    let mut layer_ids = layer_ids.peekable();
    let first_id = layer_ids.next()?;
    if first_id == target_layer_id {
        return None;
    }
    let first = walker.layers.get(first_id).copied()?;
    let first_query = query_for(bundle, first)?;
    let goal =
        crate::outcome_goals::OutcomeGoal::new(first_id, Some(rows), None, Vec::new()).ok()?;
    let witness = crate::outcome_proofs::construct(bundle, first, first_query, &goal)?;
    if !matches!(
        witness,
        crate::outcome_proofs::OutcomeWitness::JoinPairs { .. }
            | crate::outcome_proofs::OutcomeWitness::Groups { .. }
            | crate::outcome_proofs::OutcomeWitness::Ranked { .. }
            | crate::outcome_proofs::OutcomeWitness::SetTuples { .. }
    ) {
        return None;
    }
    let mut previous = first_id;
    for id in layer_ids {
        let layer = walker.layers.get(id).copied()?;
        let query = query_for(bundle, layer)?;
        if !transparent_projection(query)
            || query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.window_witness().is_some()
            || !query.subquery_witnesses().is_empty()
        {
            return None;
        }
        // Every downstream layer must consume exactly its immediate
        // predecessor, not silently fork or duplicate the operator output.
        let inputs = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(id.to_string()))?
            .inputs();
        let [PhysicalPlanRef::Layer(input_id)] = inputs else {
            return None;
        };
        if input_id != previous {
            return None;
        }
        previous = id;
    }
    (previous == target_layer_id).then_some(witness)
}

/// Construct an exact positive equijoin cardinality through two
/// independently materialized, identity-only producer branches. Local join
/// evidence proves matching-pair semantics; tracing both join keys through
/// their producers proves that the *physical* rows can realize those pairs.
/// This deliberately does not admit filtered, grouped or computed branches.
pub(crate) fn physical_materialized_join_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    if rows == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if walker.sources.len() != 2 {
        return None;
    }
    let candidates = walker
        .nodes
        .iter()
        .filter_map(|node| match node.id() {
            PhysicalPlanRef::Layer(id) => {
                let layer = walker.layers.get(id.as_str()).copied()?;
                (query_for(bundle, layer)?.joins().len() == 1).then_some(layer)
            }
            PhysicalPlanRef::Source(_) => None,
        })
        .collect::<Vec<_>>();
    let [join_layer] = candidates.as_slice() else {
        return None;
    };
    let query = query_for(bundle, join_layer)?;
    if !query.plain_goal_output_shape()
        || query.joins().len() != 1
        || query.sources().len() != 2
        || query.dependencies().len() != 2
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || !query.diagnostics().is_empty()
        || query.output().columns().iter().any(|column| {
            !matches!(
                column.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
    {
        return None;
    }
    let ComposedSemantics::Resolved(semantics) = join_layer.composed_semantics() else {
        return None;
    };
    let [join] = semantics.join_witnesses() else {
        return None;
    };
    if join.origin_layer_id() != join_layer.id()
        || !matches!(
            join.kind(),
            crate::protocol::JoinKind::Inner
                | crate::protocol::JoinKind::Left
                | crate::protocol::JoinKind::Right
                | crate::protocol::JoinKind::Full
        )
        || join.comparison() != Some(crate::protocol::ComparisonOperator::Eq)
        || !matches!(
            join.qualifying(),
            crate::join_witness::JoinWitnessDirection::Exact(cases)
                if cases.iter().any(|case|
                    case.shape() == crate::join_witness::JoinWitnessShape::Matched)
        )
    {
        return None;
    }

    // Before the join, only independent value- and row-preserving source
    // copies are permitted. After it, every producer must consume exactly
    // the preceding join result, with no additional row shaping.
    let mut after_join = false;
    let mut previous = None::<String>;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if id == join_layer.id() {
            after_join = true;
            previous = Some(id.clone());
            continue;
        }
        let layer = walker.layers.get(id.as_str()).copied()?;
        if !transparent_projection(query_for(bundle, layer)?) {
            return None;
        }
        if after_join {
            let [PhysicalPlanRef::Layer(input_id)] = node.inputs() else {
                return None;
            };
            if previous.as_deref() != Some(input_id.as_str()) {
                return None;
            }
            previous = Some(id.clone());
        }
    }
    if previous.as_deref() != Some(target_layer_id) {
        return None;
    }

    let map = |endpoint: &crate::bundle::ComposedJoinColumn| {
        let column = crate::protocol::ColumnRef::new(
            Some(endpoint.relation().to_string()),
            endpoint.column().to_string(),
        );
        let mapped = if walker.sources.contains(endpoint.relation()) {
            column
        } else {
            resolve_filter_column(bundle, &walker, join_layer.id(), &column, 0)?
        };
        let relation = mapped.relation()?;
        walker.sources.contains(relation).then(|| {
            crate::bundle::ComposedJoinColumn::new(
                relation.to_string(),
                mapped.name().to_string(),
                endpoint.relation_instance().to_string(),
            )
        })
    };
    let left = map(join.left()?)?;
    let right = map(join.right()?)?;
    if left.relation() == right.relation() {
        return None;
    }
    crate::outcome_proofs::construct_mapped_join_pairs(bundle, &left, &right, rows)
}

/// One jointly controlled physical input with an optional qualifying
/// restriction. Equality of physical counts is necessary only for a genuinely
/// row-preserving (unfiltered) terminal, not for a sufficient filter plan.
struct SourceCountRequirement {
    rows: u64,
    necessary: bool,
    predicate: WitnessFormula,
}

fn count_tautology() -> WitnessFormula {
    WitnessFormula::IsNull {
        term: WitnessTerm::Integer(1),
        negated: true,
    }
}

fn count_predicate_for_source(
    direction: &WitnessDirection,
    source: &str,
    rows: u64,
) -> Option<WitnessFormula> {
    let WitnessDirection::Feasible(cases) = direction else {
        return None;
    };
    let [case] = cases.as_slice() else {
        return None;
    };
    case.obligations()
        .iter()
        .find_map(|obligation| match obligation {
            WitnessObligation::Rows {
                boundary,
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate,
                closed_world: true,
            } if boundary.kind() == GroupBoundaryKind::Physical
                && boundary.relation() == source
                && bounds.minimum() == rows
                && bounds.maximum() == Some(rows) =>
            {
                Some(predicate.clone())
            }
            _ => None,
        })
}

/// Reconcile positive and deliberately empty terminals over one schema-backed
/// source. All-positive constructions use N rows selected by every positive
/// filter; a zero terminal must reject every one of those same rows. This is
/// one sufficient closed-world assignment, not a claim that independent
/// sufficient witnesses can be concatenated or freely multiplied.
fn joint_positive_and_rejected_pair(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> Option<WitnessDirection> {
    let mut outputs = BTreeMap::<String, u64>::new();
    let mut positive = Vec::new();
    let mut zero = Vec::new();
    for &(layer_id, rows) in targets {
        if outputs
            .insert(layer_id.to_string(), rows)
            .is_some_and(|existing| existing != rows)
        {
            return Some(WitnessDirection::Impossible);
        }
        if rows == 0 {
            zero.push(layer_id);
        } else {
            positive.push((layer_id, rows));
        }
    }
    positive.sort_by_key(|(layer_id, _)| *layer_id);
    zero.sort();
    let &(first_positive, rows) = positive.first()?;
    if zero.is_empty() || positive.iter().any(|(_, count)| *count != rows) {
        return None;
    }
    let first_physical = physical_source_plan(bundle, first_positive);
    let [source] = first_physical.sources() else {
        return None;
    };

    let mut qualifying = Vec::new();
    for &(layer_id, _) in &positive {
        let physical = physical_source_plan(bundle, layer_id);
        if physical.sources() != [source.clone()] {
            return None;
        }
        let predicate = count_predicate_for_source(
            &physical_row_count_plan(bundle, layer_id, rows),
            source,
            rows,
        )?;
        if predicate == count_tautology() {
            continue;
        }
        if !matches!(
            predicate,
            WitnessFormula::RowTruth {
                truth: crate::boolean_witness::BooleanTruthCase::True,
                ..
            }
        ) {
            return None;
        }
        qualifying.push(predicate);
    }

    let mut rejecting = Vec::new();
    for layer_id in zero {
        let physical = physical_source_plan(bundle, layer_id);
        if physical.sources() != [source.clone()] {
            return None;
        }
        let predicate = count_predicate_for_source(
            &physical_rejected_row_count_plan(bundle, layer_id, rows),
            source,
            rows,
        )?;
        if !matches!(
            predicate,
            WitnessFormula::RowTruth {
                truth: crate::boolean_witness::BooleanTruthCase::NotTrue,
                ..
            }
        ) {
            return None;
        }
        if !rejecting.contains(&predicate) {
            rejecting.push(predicate);
        }
    }

    let mut requirements = qualifying.clone();
    requirements.extend(rejecting.iter().cloned());
    let satisfiable = joint_source_truths_satisfiable(bundle, source, &requirements)?;
    if !satisfiable {
        // Two mutually exclusive positive filters can use disjoint source
        // subsets when no exact whole-source count is demanded. By contrast,
        // no positive row can survive a negative filter that every row must
        // reject, independent of the total physical source cardinality.
        if qualifying.is_empty()
            || qualifying.iter().any(|predicate| {
                let mut necessary = vec![predicate.clone()];
                necessary.extend(rejecting.iter().cloned());
                joint_source_truths_satisfiable(bundle, source, &necessary) == Some(false)
            })
        {
            return Some(WitnessDirection::Impossible);
        }
        return None;
    }

    let row_predicate = match requirements.as_slice() {
        [predicate] => predicate.clone(),
        predicates => WitnessFormula::All(predicates.to_vec()),
    };
    let boundary = WitnessBoundary::new(source, GroupBoundaryKind::Physical, first_positive)?;
    let rows_bounds = CountBounds::new(rows, Some(rows))?;
    let mut obligations = vec![
        WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: rows_bounds,
            predicate: row_predicate,
            closed_world: true,
        },
        WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        },
    ];
    for (layer_id, count) in outputs {
        obligations.push(WitnessObligation::OutputRows {
            layer_id,
            bounds: CountBounds::new(count, Some(count))?,
        });
    }
    let case = WitnessCase::new(obligations, ProofStrength::Sufficient)?;
    WitnessDirection::feasible(vec![case])
}

/// Evidence for direct physical signed integer comparisons. Kept identical
/// across the positive-only and mixed-truth physical row solvers.
fn physical_integer_evidence(
    schema: &crate::relation::RelationSchema,
    column: &crate::protocol::ColumnRef,
) -> Option<crate::boolean_witness::SignedIntegerEvidence> {
    let known = schema
        .columns()
        .iter()
        .find(|known| known.name() == column.name())?;
    let data_type = match known.data_type() {
        crate::data_type::DataType::Nullable(inner) => inner.as_ref(),
        other => other,
    };
    match data_type {
        crate::data_type::DataType::SignedInteger { bits: Some(bits) }
            if *bits > 0 && *bits <= 64 =>
        {
            let magnitude = 1_i128 << (u32::from(*bits) - 1);
            Some(crate::boolean_witness::SignedIntegerEvidence {
                minimum: -magnitude,
                maximum: magnitude - 1,
            })
        }
        _ => None,
    }
}

/// Validate mixed SQL TRUE and NOT TRUE conditions on one *identical* row.
/// The witnessed RowVariable must have the same source, instance and name,
/// rather than merely the same relation string.
fn joint_source_truths_satisfiable(
    bundle: &AnalysisBundle,
    source: &str,
    predicates: &[WitnessFormula],
) -> Option<bool> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source)?;
    let mut row_identity = None;
    let mut requirements = Vec::new();
    for predicate in predicates {
        let WitnessFormula::RowTruth {
            row,
            predicate,
            truth,
        } = predicate
        else {
            return None;
        };
        if row.relation() != source || row_identity.is_some_and(|identity| identity != row) {
            return None;
        }
        row_identity = Some(row);
        requirements.push((predicate, *truth));
    }
    crate::boolean_witness::jointly_satisfiable_physical_truths(
        source,
        &requirements,
        |column| {
            schema
                .columns()
                .iter()
                .any(|known| known.name() == column.name())
        },
        |column| physical_integer_evidence(schema, column),
    )
}

/// Conjoin independently proven SQL truth directions on one physical row.
/// Independent terminal examples cannot prove a common source assignment.
fn conjoin_source_row_truths(
    bundle: &AnalysisBundle,
    source: &str,
    left: &WitnessFormula,
    right: &WitnessFormula,
) -> Option<(WitnessFormula, bool)> {
    let WitnessFormula::RowTruth {
        row: left_row,
        predicate: left_condition,
        truth: left_truth,
    } = left
    else {
        return None;
    };
    let WitnessFormula::RowTruth {
        row: right_row,
        predicate: right_condition,
        truth: right_truth,
    } = right
    else {
        return None;
    };
    if left_row != right_row || left_row.relation() != source || left_truth != right_truth {
        return None;
    }
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source)?;
    let (predicate, satisfiable) = crate::boolean_witness::conjoin_physical_row_truths(
        source,
        *left_truth,
        &[left_condition, right_condition],
        |column| {
            schema
                .columns()
                .iter()
                .any(|known| known.name() == column.name())
        },
        |column| physical_integer_evidence(schema, column),
    )?;
    Some((
        WitnessFormula::RowTruth {
            row: left_row.clone(),
            predicate,
            truth: *left_truth,
        },
        satisfiable,
    ))
}

/// Construct a single complete physical source assignment for several
/// terminal row-count goals. Independent sufficient cases are composed only
/// after reconciling their *shared physical source identities*.
///
/// Transparent paths require exactly the terminal count at their one source.
/// Empty-input proofs for filters, joins and ordinary grouping are sufficient
/// but not necessary, so conflicts with nonzero source requirements are
/// residual rather than (incorrectly) proved impossible.
pub fn physical_joint_row_count_plan(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> WitnessDirection {
    if targets.is_empty() {
        return residual(PhysicalProofGap::NoWitness);
    }
    let mut ordered = targets.to_vec();
    ordered.sort();
    if let Some(witness) = joint_positive_and_rejected_pair(bundle, &ordered) {
        return witness;
    }

    let mut outputs = BTreeMap::<String, u64>::new();
    let mut sources = BTreeMap::<String, SourceCountRequirement>::new();
    for &(layer_id, rows) in &ordered {
        if outputs
            .insert(layer_id.to_string(), rows)
            .is_some_and(|existing| existing != rows)
        {
            return WitnessDirection::Impossible;
        }

        let row_plan = physical_row_count_plan(bundle, layer_id, rows);
        match &row_plan {
            WitnessDirection::Impossible => return WitnessDirection::Impossible,
            WitnessDirection::Residual { .. } => {
                return residual(PhysicalProofGap::LocalWitnessUnproven)
            }
            WitnessDirection::Feasible(_) => {}
        }

        let physical = physical_source_plan(bundle, layer_id);
        if physical.sources().is_empty() {
            // Source-free singletons are handled by the individual proof.
            continue;
        }

        // An all-TRUE filter construction uses exactly 'rows' physical rows,
        // but that count is sufficient, not necessary: an alternative physical
        // assignment could contain rejected rows as well.
        let one_row_plan = physical_row_count_plan(bundle, layer_id, 1);
        let has_filter = physical.sources().iter().any(|source| {
            matches!(
                count_predicate_for_source(&one_row_plan, source, 1),
                Some(WitnessFormula::RowTruth { .. })
            )
        });
        let requires_exact_count =
            !has_filter && matches!(one_row_plan, WitnessDirection::Feasible(_));
        if requires_exact_count && physical.sources().len() != 1 {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        }

        for relation in physical.sources() {
            let Some(predicate) = count_predicate_for_source(&row_plan, relation, rows) else {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            };
            match sources.get(relation) {
                Some(existing) if existing.rows != rows => {
                    if existing.necessary && requires_exact_count {
                        return WitnessDirection::Impossible;
                    }
                    return residual(PhysicalProofGap::MultipleWitnesses);
                }
                Some(existing) => {
                    let joined_predicate = if existing.predicate == count_tautology() {
                        predicate
                    } else if predicate == count_tautology() || predicate == existing.predicate {
                        existing.predicate.clone()
                    } else {
                        let Some((joint, satisfiable)) = conjoin_source_row_truths(
                            bundle,
                            relation,
                            &existing.predicate,
                            &predicate,
                        ) else {
                            return residual(PhysicalProofGap::MultipleWitnesses);
                        };
                        if !satisfiable {
                            // Only a necessary exact source count makes a
                            // disjoint pair impossible. Otherwise extra
                            // physical rows could satisfy each output separately.
                            if existing.necessary || requires_exact_count {
                                return WitnessDirection::Impossible;
                            }
                            return residual(PhysicalProofGap::MultipleWitnesses);
                        }
                        joint
                    };
                    sources.insert(
                        relation.clone(),
                        SourceCountRequirement {
                            rows,
                            necessary: existing.necessary || requires_exact_count,
                            predicate: joined_predicate,
                        },
                    );
                }
                None => {
                    sources.insert(
                        relation.clone(),
                        SourceCountRequirement {
                            rows,
                            necessary: requires_exact_count,
                            predicate,
                        },
                    );
                }
            }
        }
    }

    let origin = outputs.keys().next().map(String::as_str).unwrap_or("");
    let mut obligations = Vec::new();
    for (relation, requirement) in sources {
        let Some(boundary) = WitnessBoundary::new(&relation, GroupBoundaryKind::Physical, origin)
        else {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        };
        let Some(bounds) = CountBounds::new(requirement.rows, Some(requirement.rows)) else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds,
            predicate: requirement.predicate,
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
    }
    for (layer_id, rows) in outputs {
        let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        obligations.push(WitnessObligation::OutputRows { layer_id, bounds });
    }
    let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}
