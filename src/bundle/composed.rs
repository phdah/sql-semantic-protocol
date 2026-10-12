//! Composed join, set, grouping, window and Boolean semantics.

use super::*;

/// Diagnostic attached to graph construction or one graph component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionDiagnostic {
    severity: DiagnosticSeverity,
    code: String,
    message: String,
    input_id: Option<String>,
    layer_id: Option<String>,
    relation: Option<String>,
}

impl CompositionDiagnostic {
    fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        layer_id: Option<String>,
        relation: Option<String>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            input_id: None,
            layer_id,
            relation,
        }
    }

    pub(crate) fn layer_warning(
        input_id: impl Into<String>,
        layer_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        relation: Option<String>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            input_id: Some(input_id.into()),
            layer_id: Some(layer_id.into()),
            relation,
        }
    }

    /// Return the diagnostic severity.
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Return the stable diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Return the human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Return the affected input ID when the diagnostic is input-specific.
    pub fn input_id(&self) -> Option<&str> {
        self.input_id.as_deref()
    }

    /// Return the affected layer ID when the diagnostic is layer-specific.
    pub fn layer_id(&self) -> Option<&str> {
        self.layer_id.as_deref()
    }

    /// Return the affected relation when the diagnostic is relation-specific.
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }
}

/// Why transitive semantic composition could not be completed safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompositionFailureReason {
    /// A required producer is unavailable.
    MissingProducer,
    /// More than one producer could satisfy a consumed relation.
    AmbiguousProducer,
    /// The dependency graph contains a cycle.
    Cycle,
    /// A linked producer only partially defines the written relation.
    PartialProducer,
    /// Known semantics cannot be propagated safely.
    Unsupported,
}

impl CompositionFailureReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MissingProducer => "missing_producer",
            Self::AmbiguousProducer => "ambiguous_producer",
            Self::Cycle => "cycle",
            Self::PartialProducer => "partial_producer",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Local and transitive source lineage retained independently per producer.
pub(crate) struct ComposedSourceEvidence {
    pub(crate) dependencies: Vec<String>,
    pub(crate) producer_sources: BTreeMap<String, Vec<String>>,
}

/// Transitive semantics for one transformation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComposedSemantics {
    /// Transitive dependencies, domains, and output lineage were composed safely.
    Resolved(Box<ResolvedComposedSemantics>),
    /// Composition stopped rather than inventing semantics that cannot be proven.
    Unresolved(UnresolvedComposedSemantics),
}

impl ComposedSemantics {
    pub(crate) fn pending(input_id: &str, layer_id: &str) -> Self {
        Self::Unresolved(UnresolvedComposedSemantics {
            reason: CompositionFailureReason::Unsupported,
            diagnostics: vec![CompositionDiagnostic::layer_warning(
                input_id,
                layer_id,
                "semantic_composition_pending",
                "cross-input semantic composition has not been evaluated",
                None,
            )],
        })
    }

    pub(crate) fn resolved(
        sources: ComposedSourceEvidence,
        column_domains: Vec<ColumnDomain>,
        mut join_equalities: Vec<ComposedJoinEquality>,
        witnesses: ComposedWitnessEvidence,
        condition_exactness: ConditionExactness,
        output: Output,
        mut diagnostics: Vec<CompositionDiagnostic>,
    ) -> Self {
        let ComposedSourceEvidence {
            dependencies,
            producer_sources,
        } = sources;
        let ComposedWitnessEvidence {
            mut set_operations,
            mut group_witnesses,
            mut window_witnesses,
            mut subquery_witnesses,
            mut boolean_witnesses,
            mut join_witnesses,
        } = witnesses;
        join_equalities.sort_by(composed_join_equality_cmp);
        join_witnesses.sort_by(|a, b| a.origin_layer_id().cmp(b.origin_layer_id()));
        join_witnesses.dedup();
        join_equalities.dedup();
        set_operations.sort_by(|a, b| a.origin_layer_id.cmp(&b.origin_layer_id));
        set_operations.dedup();
        group_witnesses.sort_by(|a, b| a.origin_layer_id.cmp(&b.origin_layer_id));
        group_witnesses.dedup();
        window_witnesses.sort_by(|a, b| a.origin_layer_id.cmp(&b.origin_layer_id));
        window_witnesses.dedup();
        subquery_witnesses.sort_by(|a, b| a.origin_layer_id.cmp(&b.origin_layer_id));
        subquery_witnesses.dedup();
        boolean_witnesses.sort_by(|a, b| a.origin_layer_id.cmp(&b.origin_layer_id));
        boolean_witnesses.dedup();
        diagnostics.sort_by(diagnostic_cmp);
        diagnostics.dedup();
        Self::Resolved(Box::new(ResolvedComposedSemantics {
            dependencies,
            producer_sources,
            column_domains,
            join_equalities,
            join_witnesses: join_witnesses.into_boxed_slice(),
            set_operations: set_operations.into_boxed_slice(),
            group_witnesses: group_witnesses.into_boxed_slice(),
            window_witnesses: window_witnesses.into_boxed_slice(),
            subquery_witnesses: subquery_witnesses.into_boxed_slice(),
            boolean_witnesses: boolean_witnesses.into_boxed_slice(),
            condition_exactness,
            output,
            diagnostics,
        }))
    }

    pub(crate) fn unresolved(
        reason: CompositionFailureReason,
        mut diagnostics: Vec<CompositionDiagnostic>,
    ) -> Self {
        diagnostics.sort_by(diagnostic_cmp);
        diagnostics.dedup();
        Self::Unresolved(UnresolvedComposedSemantics {
            reason,
            diagnostics,
        })
    }
}

/// One physical column endpoint of a composed join equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComposedJoinColumn {
    relation: String,
    column: String,
    relation_instance: String,
}

impl ComposedJoinColumn {
    pub(crate) fn new(relation: String, column: String, relation_instance: String) -> Self {
        Self {
            relation,
            column,
            relation_instance,
        }
    }

    /// Return the physical leaf relation.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the physical leaf column.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Return the stable relation-instance identity within the originating layer.
    pub fn relation_instance(&self) -> &str {
        &self.relation_instance
    }
}

/// A physical column equality required by a composed row-condition contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedJoinEquality {
    left: ComposedJoinColumn,
    right: ComposedJoinColumn,
    join_kind: JoinKind,
    origin_layer_id: String,
}

impl ComposedJoinEquality {
    pub(crate) fn new(
        left: ComposedJoinColumn,
        right: ComposedJoinColumn,
        join_kind: JoinKind,
        origin_layer_id: String,
    ) -> Self {
        Self {
            left,
            right,
            join_kind,
            origin_layer_id,
        }
    }

    /// Return the left physical equality endpoint.
    pub fn left(&self) -> &ComposedJoinColumn {
        &self.left
    }

    /// Return the right physical equality endpoint.
    pub fn right(&self) -> &ComposedJoinColumn {
        &self.right
    }

    /// Return the join kind that introduced the equality.
    pub fn join_kind(&self) -> JoinKind {
        self.join_kind
    }

    /// Return the transformation layer where this equality originated.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }
}

pub(super) fn composed_join_equality_cmp(
    left: &ComposedJoinEquality,
    right: &ComposedJoinEquality,
) -> std::cmp::Ordering {
    (
        left.origin_layer_id(),
        left.join_kind().as_str(),
        left.left(),
        left.right(),
    )
        .cmp(&(
            right.origin_layer_id(),
            right.join_kind().as_str(),
            right.left(),
            right.right(),
        ))
}

/// One set-operation tree retained from an input or an upstream producer layer.
/// This is evidence with provenance, not a guarantee of generatable source witnesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedSetOperation {
    origin_layer_id: String,
    operation: SetOperation,
}

impl ComposedSetOperation {
    pub(crate) fn new(origin_layer_id: String, operation: SetOperation) -> Self {
        Self {
            origin_layer_id,
            operation,
        }
    }

    /// Layer where this SQL set operation was introduced.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }

    /// Parser-independent operation and its branch-local evidence.
    pub fn operation(&self) -> &SetOperation {
        &self.operation
    }
}

/// Locally and transitively composed generator-facing witness evidence.
/// Each witness retains the layer that introduced it and its own proof boundary.
pub(crate) struct ComposedWitnessEvidence {
    pub(crate) join_witnesses: Vec<crate::join_witness::JoinWitness>,
    pub(crate) set_operations: Vec<ComposedSetOperation>,
    pub(crate) group_witnesses: Vec<ComposedGroupWitness>,
    pub(crate) window_witnesses: Vec<ComposedWindowWitness>,
    pub(crate) subquery_witnesses: Vec<ComposedSubqueryWitness>,
    pub(crate) boolean_witnesses: Vec<ComposedBooleanWitness>,
}

/// A coupled boolean source-row witness retained at its originating layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedBooleanWitness {
    origin_layer_id: String,
    witness: crate::boolean_witness::BooleanWitness,
    boundary_kind: GroupBoundaryKind,
}

impl ComposedBooleanWitness {
    pub(crate) fn new(
        origin_layer_id: String,
        witness: crate::boolean_witness::BooleanWitness,
        boundary_kind: GroupBoundaryKind,
    ) -> Self {
        Self {
            origin_layer_id,
            witness,
            boundary_kind,
        }
    }

    /// Layer where the correlated predicate originated.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }

    /// Coupled source-row proof at that layer.
    pub fn witness(&self) -> &crate::boolean_witness::BooleanWitness {
        &self.witness
    }

    /// Physical, intermediate or unresolved source boundary.
    pub fn boundary_kind(&self) -> GroupBoundaryKind {
        self.boundary_kind
    }
}

/// One source-membership witness retained at its introducing layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedSubqueryWitness {
    origin_layer_id: String,
    witness: crate::subquery_witness::SubqueryMembershipWitness,
    boundary_kind: GroupBoundaryKind,
}

impl ComposedSubqueryWitness {
    pub(crate) fn new(
        origin_layer_id: String,
        witness: crate::subquery_witness::SubqueryMembershipWitness,
        boundary_kind: GroupBoundaryKind,
    ) -> Self {
        Self {
            origin_layer_id,
            witness,
            boundary_kind,
        }
    }

    /// Layer that introduced the subquery membership predicate.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }

    /// Source-level operator witness at its originating boundary.
    pub fn witness(&self) -> &crate::subquery_witness::SubqueryMembershipWitness {
        &self.witness
    }

    /// Whether the inner boundary is a physical or intermediate relation.
    pub fn boundary_kind(&self) -> GroupBoundaryKind {
        self.boundary_kind
    }
}

/// Relation class of an originating grouped witness boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBoundaryKind {
    /// The grouped relation is an external physical input.
    Physical,
    /// The grouped relation is an upstream producer's output.
    Intermediate,
    /// A single controllable boundary could not be established.
    Unresolved,
}

impl GroupBoundaryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::Intermediate => "intermediate",
            Self::Unresolved => "unresolved",
        }
    }
}

/// A grouped HAVING witness introduced at one SQL or dbt transformation layer.
///
/// This is provenance-bearing local evidence, not a claim that an arbitrary
/// downstream transformation preserves the group membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedGroupWitness {
    origin_layer_id: String,
    witness: crate::group_witness::GroupWitness,
    boundary_kind: GroupBoundaryKind,
}

impl ComposedGroupWitness {
    pub(crate) fn new(
        origin_layer_id: String,
        witness: crate::group_witness::GroupWitness,
        boundary_kind: GroupBoundaryKind,
    ) -> Self {
        Self {
            origin_layer_id,
            witness,
            boundary_kind,
        }
    }

    /// Layer introducing these HAVING obligations.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }

    /// Local HAVING proof at its origin boundary.
    pub fn witness(&self) -> &crate::group_witness::GroupWitness {
        &self.witness
    }

    /// Whether that boundary is physical, intermediate, or unresolved.
    pub fn boundary_kind(&self) -> GroupBoundaryKind {
        self.boundary_kind
    }
}

/// One ranked window witness at the layer that introduced its rank filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedWindowWitness {
    origin_layer_id: String,
    witness: crate::window_witness::WindowWitness,
    boundary_kind: GroupBoundaryKind,
}

impl ComposedWindowWitness {
    pub(crate) fn new(
        origin_layer_id: String,
        witness: crate::window_witness::WindowWitness,
        boundary_kind: GroupBoundaryKind,
    ) -> Self {
        Self {
            origin_layer_id,
            witness,
            boundary_kind,
        }
    }
    /// Layer introducing this ranked window filter.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }
    /// Input-partition and ordered-row obligations at that layer.
    pub fn witness(&self) -> &crate::window_witness::WindowWitness {
        &self.witness
    }
    /// Physical, intermediate or unresolved input relation boundary.
    pub fn boundary_kind(&self) -> GroupBoundaryKind {
        self.boundary_kind
    }
}

/// Successfully composed transitive semantics for a transformation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedComposedSemantics {
    dependencies: Vec<String>,
    producer_sources: BTreeMap<String, Vec<String>>,
    column_domains: Vec<ColumnDomain>,
    join_equalities: Vec<ComposedJoinEquality>,
    join_witnesses: Box<[crate::join_witness::JoinWitness]>,
    set_operations: Box<[ComposedSetOperation]>,
    group_witnesses: Box<[ComposedGroupWitness]>,
    window_witnesses: Box<[ComposedWindowWitness]>,
    subquery_witnesses: Box<[ComposedSubqueryWitness]>,
    boolean_witnesses: Box<[ComposedBooleanWitness]>,
    condition_exactness: ConditionExactness,
    output: Output,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl ResolvedComposedSemantics {
    /// Return physical leaf dependencies in deterministic relation order.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Proven physical leaves for a particular upstream relation producer.
    /// None means the producer's specific physical dependencies are unresolved.
    pub fn producer_physical_sources(&self, relation: &str) -> Option<&[String]> {
        self.producer_sources.get(relation).map(Vec::as_slice)
    }

    pub(crate) fn producer_sources(&self) -> &BTreeMap<String, Vec<String>> {
        &self.producer_sources
    }

    /// Return value domains mapped back to physical source columns.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.column_domains
    }

    /// Return physical join equalities required by the composed row-condition contract.
    pub fn join_equalities(&self) -> &[ComposedJoinEquality] {
        &self.join_equalities
    }

    /// Matched, unmatched, and null-extension obligations carried across producer layers.
    pub fn join_witnesses(&self) -> &[crate::join_witness::JoinWitness] {
        &self.join_witnesses
    }

    /// Set-operation evidence introduced locally or inherited from upstream layers.
    /// This does not grant exactness or a proven physical-source witness direction.
    pub fn set_operations(&self) -> &[ComposedSetOperation] {
        &self.set_operations
    }

    /// Upstream and local group witness evidence, each identified by its originating layer.
    pub fn group_witnesses(&self) -> &[ComposedGroupWitness] {
        &self.group_witnesses
    }

    /// Ranked-window witness evidence retained with the originating boundary.
    pub fn window_witnesses(&self) -> &[ComposedWindowWitness] {
        &self.window_witnesses
    }

    /// Subquery membership evidence, with its original layer and boundary.
    pub fn subquery_witnesses(&self) -> &[ComposedSubqueryWitness] {
        &self.subquery_witnesses
    }

    /// Coupled boolean conditions, each retained at the layer where it was proven.
    pub fn boolean_witnesses(&self) -> &[ComposedBooleanWitness] {
        &self.boolean_witnesses
    }

    /// Return transitive row-condition exactness for this resolved layer.
    ///
    /// Residual conditions include every referenced local scope and resolved ancestor layer,
    /// with origin metadata identifying where each residual was introduced.
    pub fn condition_exactness(&self) -> &ConditionExactness {
        &self.condition_exactness
    }

    /// Return final output columns with transitive physical lineage.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Return diagnostics for semantics that could not be propagated precisely.
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

/// Explicit failure to compose one transformation layer safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedComposedSemantics {
    reason: CompositionFailureReason,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl UnresolvedComposedSemantics {
    /// Return why composition stopped.
    pub fn reason(&self) -> CompositionFailureReason {
        self.reason
    }

    /// Return diagnostics explaining the unresolved composition.
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

