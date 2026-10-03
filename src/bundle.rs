//! Multi-input analysis orchestration.
//!
//! This module owns parser-independent input identities and bundles. It deliberately reuses the
//! existing single-input analyzer for each unit and builds deterministic relation dependency graphs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sqlparser::dialect::Dialect;

use crate::protocol::{
    ColumnDomain, DiagnosticSeverity, Output, Protocol, ProtocolStatement, PROTOCOL_VERSION,
};
use crate::{analyze_sql, Error};

/// Source identity retained for one SQL input unit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SqlInputSource {
    /// SQL supplied directly by the caller.
    Inline,
    /// SQL loaded from a file whose path identifies the source.
    File {
        /// Caller-visible path used to identify this input.
        path: String,
    },
}

/// One SQL text unit supplied to multi-input analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlInput {
    source: SqlInputSource,
    sql: String,
}

/// One SQL input with an explicit stable identity and caller-selected dialect.
///
/// This configured form is useful when input identity and dialect vary per unit, such as when an
/// analysis request is loaded from a manifest. The dialect implementation remains caller-owned.
pub struct ConfiguredSqlInput<'a> {
    id: &'a str,
    input: &'a SqlInput,
    dialect_name: &'a str,
    dialect: &'a dyn Dialect,
}

impl<'a> ConfiguredSqlInput<'a> {
    /// Construct one configured SQL input.
    pub fn new(
        id: &'a str,
        input: &'a SqlInput,
        dialect_name: &'a str,
        dialect: &'a dyn Dialect,
    ) -> Self {
        Self {
            id,
            input,
            dialect_name,
            dialect,
        }
    }

    /// Return the stable input identifier supplied by the caller.
    pub fn id(&self) -> &str {
        self.id
    }

    /// Return the SQL input and source identity.
    pub fn input(&self) -> &SqlInput {
        self.input
    }

    /// Return the caller-visible dialect name.
    pub fn dialect_name(&self) -> &str {
        self.dialect_name
    }

    /// Return the sqlparser dialect implementation used at the parsing boundary.
    pub fn dialect(&self) -> &dyn Dialect {
        self.dialect
    }
}

impl SqlInput {
    /// Construct an inline SQL input.
    pub fn inline(sql: impl Into<String>) -> Self {
        Self {
            source: SqlInputSource::Inline,
            sql: sql.into(),
        }
    }

    /// Construct a file-backed SQL input from its source path and already-read SQL text.
    ///
    /// The library does not perform file I/O. Callers retain control over how file contents are
    /// loaded while the path remains available as stable source identity.
    pub fn file(path: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            source: SqlInputSource::File { path: path.into() },
            sql: sql.into(),
        }
    }

    /// Return the parser-independent source identity for this input.
    pub fn source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the SQL text supplied for this input.
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// One analyzed input in a multi-input protocol bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzedInput {
    id: String,
    source: SqlInputSource,
    dialect: String,
    statements: Vec<ProtocolStatement>,
}

impl AnalyzedInput {
    /// Return the deterministic input identifier generated from caller order.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the source identity associated with this input.
    pub fn source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the normalized dialect name supplied by the caller.
    pub fn dialect(&self) -> &str {
        &self.dialect
    }

    /// Return analyzed statements in source statement order.
    pub fn statements(&self) -> &[ProtocolStatement] {
        &self.statements
    }
}

/// Dataset produced by one transformation layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DatasetRef {
    /// A named relation created by query-backed DDL.
    Relation {
        /// Deterministic SQL relation identity, preserving qualification and quoting.
        name: String,
    },
    /// An anonymous result produced by a bare query.
    Anonymous {
        /// Layer identifier used to address the anonymous result.
        layer_id: String,
    },
}

impl DatasetRef {
    /// Return the named relation identity when this dataset refers to a relation.
    pub fn relation_name(&self) -> Option<&str> {
        match self {
            Self::Relation { name } => Some(name),
            Self::Anonymous { .. } => None,
        }
    }

    /// Return the owning layer identifier when this dataset is anonymous.
    pub fn anonymous_layer_id(&self) -> Option<&str> {
        match self {
            Self::Relation { .. } => None,
            Self::Anonymous { layer_id } => Some(layer_id),
        }
    }
}

/// Resolution state for one consumed relation in the bundle dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelationResolution {
    /// Exactly one in-bundle producer matches the consumed relation.
    Resolved,
    /// No in-bundle producer exists, so the relation is an external dependency.
    External,
    /// A required producer is unavailable.
    Missing,
    /// Multiple in-bundle producers match and no producer can be selected safely.
    Ambiguous,
    /// The resolved producer relationship participates in a dependency cycle.
    Cycle,
    /// Resolution could not be represented safely for another explicit reason.
    Unsupported,
}

impl RelationResolution {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::External => "external",
            Self::Missing => "missing",
            Self::Ambiguous => "ambiguous",
            Self::Cycle => "cycle",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One relation dependency edge from a consumer layer to zero or more candidate producers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    consumer_layer_id: String,
    relation: String,
    resolution: RelationResolution,
    producer_layer_ids: Vec<String>,
}

impl GraphEdge {
    /// Return the layer consuming this relation.
    pub fn consumer_layer_id(&self) -> &str {
        &self.consumer_layer_id
    }

    /// Return the normalized consumed relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return how this relation was resolved.
    pub fn resolution(&self) -> RelationResolution {
        self.resolution
    }

    /// Return matching producer layer IDs in deterministic order.
    pub fn producer_layer_ids(&self) -> &[String] {
        &self.producer_layer_ids
    }
}

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
    /// Known semantics cannot be propagated safely.
    Unsupported,
}

impl CompositionFailureReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MissingProducer => "missing_producer",
            Self::AmbiguousProducer => "ambiguous_producer",
            Self::Cycle => "cycle",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Transitive semantics for one transformation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComposedSemantics {
    /// Transitive dependencies, domains, and output lineage were composed safely.
    Resolved(ResolvedComposedSemantics),
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
        dependencies: Vec<String>,
        column_domains: Vec<ColumnDomain>,
        output: Output,
        mut diagnostics: Vec<CompositionDiagnostic>,
    ) -> Self {
        diagnostics.sort_by(diagnostic_cmp);
        diagnostics.dedup();
        Self::Resolved(ResolvedComposedSemantics {
            dependencies,
            column_domains,
            output,
            diagnostics,
        })
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

/// Successfully composed transitive semantics for a transformation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedComposedSemantics {
    dependencies: Vec<String>,
    column_domains: Vec<ColumnDomain>,
    output: Output,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl ResolvedComposedSemantics {
    /// Return physical leaf dependencies in deterministic relation order.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Return value domains mapped back to physical source columns.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.column_domains
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

/// One connected component of transformation layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphComponent {
    id: String,
    layer_ids: Vec<String>,
    final_outcomes: Vec<DatasetRef>,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl GraphComponent {
    /// Return the deterministic component identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return component layers in deterministic dependency order when acyclic.
    pub fn layer_ids(&self) -> &[String] {
        &self.layer_ids
    }

    /// Return terminal datasets when the component has an unambiguous acyclic outcome.
    pub fn final_outcomes(&self) -> &[DatasetRef] {
        &self.final_outcomes
    }

    /// Return diagnostics affecting this component.
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

/// Deterministic dependency graph across all transformation layers in an analysis bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisGraph {
    edges: Vec<GraphEdge>,
    components: Vec<GraphComponent>,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl AnalysisGraph {
    /// Return dependency edges in deterministic consumer/relation order.
    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }

    /// Return disconnected graph components ordered by their earliest layer.
    pub fn components(&self) -> &[GraphComponent] {
        &self.components
    }

    /// Return graph-level ambiguity and cycle diagnostics.
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

/// One local transformation layer derived from an analyzed query statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformationLayer {
    id: String,
    input_id: String,
    statement_index: usize,
    produces: Vec<DatasetRef>,
    consumes: Vec<String>,
    composed_semantics: ComposedSemantics,
}

impl TransformationLayer {
    /// Return the deterministic layer identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the input containing the statement represented by this layer.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Return the zero-based statement index within the input.
    pub fn statement_index(&self) -> usize {
        self.statement_index
    }

    /// Return datasets produced by this layer.
    pub fn produces(&self) -> &[DatasetRef] {
        &self.produces
    }

    /// Return normalized physical relations consumed directly by this layer.
    pub fn consumes(&self) -> &[String] {
        &self.consumes
    }

    /// Return transitive semantics composed through in-bundle producers.
    pub fn composed_semantics(&self) -> &ComposedSemantics {
        &self.composed_semantics
    }
}

/// Multi-input analysis result with deterministic local layers and relation dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisBundle {
    protocol_version: &'static str,
    inputs: Vec<AnalyzedInput>,
    layers: Vec<TransformationLayer>,
    graph: AnalysisGraph,
}

impl AnalysisBundle {
    /// Return the multi-input protocol contract version.
    pub fn protocol_version(&self) -> &str {
        self.protocol_version
    }

    /// Return analyzed inputs in caller-provided order.
    pub fn inputs(&self) -> &[AnalyzedInput] {
        &self.inputs
    }

    /// Return query-backed transformation layers in deterministic statement order.
    pub fn layers(&self) -> &[TransformationLayer] {
        &self.layers
    }

    /// Return the deterministic relation dependency graph across all layers.
    pub fn graph(&self) -> &AnalysisGraph {
        &self.graph
    }

    pub(crate) fn from_protocol(protocol: &Protocol) -> Self {
        let inputs = vec![AnalyzedInput {
            id: "input-0001".to_string(),
            source: SqlInputSource::Inline,
            dialect: protocol.source().dialect().to_string(),
            statements: protocol.statements().to_vec(),
        }];

        Self::from_inputs(inputs)
    }

    fn from_inputs(inputs: Vec<AnalyzedInput>) -> Self {
        let mut layers = build_layers(&inputs);
        let graph = build_graph(&layers);
        let composed = crate::composition::compose_layers(&inputs, &layers, &graph);
        for (layer, semantics) in layers.iter_mut().zip(composed) {
            layer.composed_semantics = semantics;
        }

        Self {
            protocol_version: PROTOCOL_VERSION,
            inputs,
            layers,
            graph,
        }
    }
}

fn build_layers(inputs: &[AnalyzedInput]) -> Vec<TransformationLayer> {
    let layer_count = inputs
        .iter()
        .flat_map(|input| input.statements())
        .filter(|statement| matches!(statement, ProtocolStatement::Query(_)))
        .count();
    let width = layer_count.max(1).to_string().len().max(4);
    let mut layers = Vec::with_capacity(layer_count);

    for input in inputs {
        for (statement_index, statement) in input.statements().iter().enumerate() {
            let ProtocolStatement::Query(query) = statement else {
                continue;
            };

            let layer_number = layers.len() + 1;
            let layer_id = format!("layer-{:0width$}", layer_number, width = width);
            let produces = match query.produced_relation() {
                Some(name) => vec![DatasetRef::Relation {
                    name: name.to_string(),
                }],
                None => vec![DatasetRef::Anonymous {
                    layer_id: layer_id.clone(),
                }],
            };

            let composed_semantics = ComposedSemantics::pending(input.id(), &layer_id);
            layers.push(TransformationLayer {
                id: layer_id,
                input_id: input.id().to_string(),
                statement_index,
                produces,
                consumes: query.dependencies().to_vec(),
                composed_semantics,
            });
        }
    }

    layers
}

fn build_graph(layers: &[TransformationLayer]) -> AnalysisGraph {
    let producers = collect_producers(layers);
    let mut edges = build_edges(layers, &producers);
    mark_cycle_edges(&mut edges);
    let components = build_components(layers, &edges);
    let mut diagnostics = components
        .iter()
        .flat_map(|component| component.diagnostics().iter().cloned())
        .collect::<Vec<_>>();
    diagnostics.sort_by(diagnostic_cmp);

    AnalysisGraph {
        edges,
        components,
        diagnostics,
    }
}

fn collect_producers(layers: &[TransformationLayer]) -> BTreeMap<String, Vec<String>> {
    let mut producers = BTreeMap::<String, Vec<String>>::new();

    for layer in layers {
        for dataset in layer.produces() {
            let Some(relation) = dataset.relation_name() else {
                continue;
            };
            producers
                .entry(relation.to_string())
                .or_default()
                .push(layer.id().to_string());
        }
    }

    for producer_ids in producers.values_mut() {
        producer_ids.sort();
        producer_ids.dedup();
    }

    producers
}

fn build_edges(
    layers: &[TransformationLayer],
    producers: &BTreeMap<String, Vec<String>>,
) -> Vec<GraphEdge> {
    let mut edges = Vec::new();

    for layer in layers {
        for relation in layer.consumes() {
            let producer_layer_ids = producers.get(relation).cloned().unwrap_or_default();
            let resolution = match producer_layer_ids.len() {
                0 => RelationResolution::External,
                1 => RelationResolution::Resolved,
                _ => RelationResolution::Ambiguous,
            };

            edges.push(GraphEdge {
                consumer_layer_id: layer.id().to_string(),
                relation: relation.clone(),
                resolution,
                producer_layer_ids,
            });
        }
    }

    edges.sort_by(|left, right| {
        (
            left.consumer_layer_id.as_str(),
            left.relation.as_str(),
            left.producer_layer_ids.as_slice(),
        )
            .cmp(&(
                right.consumer_layer_id.as_str(),
                right.relation.as_str(),
                right.producer_layer_ids.as_slice(),
            ))
    });
    edges
}

fn mark_cycle_edges(edges: &mut [GraphEdge]) {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();

    for edge in edges.iter() {
        if edge.resolution != RelationResolution::Resolved {
            continue;
        }
        if let Some(producer_id) = edge.producer_layer_ids.first() {
            adjacency
                .entry(edge.consumer_layer_id.clone())
                .or_default()
                .push(producer_id.clone());
        }
    }

    for neighbors in adjacency.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }

    let cycle_edges = edges
        .iter()
        .filter(|edge| edge.resolution == RelationResolution::Resolved)
        .filter_map(|edge| {
            let producer_id = edge.producer_layer_ids.first()?;
            path_exists(producer_id, &edge.consumer_layer_id, &adjacency).then(|| {
                (
                    edge.consumer_layer_id.clone(),
                    edge.relation.clone(),
                    producer_id.clone(),
                )
            })
        })
        .collect::<BTreeSet<_>>();

    for edge in edges {
        let Some(producer_id) = edge.producer_layer_ids.first() else {
            continue;
        };
        if cycle_edges.contains(&(
            edge.consumer_layer_id.clone(),
            edge.relation.clone(),
            producer_id.clone(),
        )) {
            edge.resolution = RelationResolution::Cycle;
        }
    }
}

fn path_exists(start: &str, target: &str, adjacency: &BTreeMap<String, Vec<String>>) -> bool {
    if start == target {
        return true;
    }

    let mut pending = vec![start.to_string()];
    let mut visited = BTreeSet::new();

    while let Some(layer_id) = pending.pop() {
        if !visited.insert(layer_id.clone()) {
            continue;
        }
        let Some(neighbors) = adjacency.get(&layer_id) else {
            continue;
        };
        for neighbor in neighbors {
            if neighbor == target {
                return true;
            }
            if !visited.contains(neighbor) {
                pending.push(neighbor.clone());
            }
        }
    }

    false
}

fn build_components(layers: &[TransformationLayer], edges: &[GraphEdge]) -> Vec<GraphComponent> {
    let mut neighbors = layers
        .iter()
        .map(|layer| (layer.id().to_string(), BTreeSet::<String>::new()))
        .collect::<BTreeMap<_, _>>();

    for edge in edges {
        for producer_id in edge.producer_layer_ids() {
            if let Some(consumer_neighbors) = neighbors.get_mut(edge.consumer_layer_id()) {
                consumer_neighbors.insert(producer_id.clone());
            }
            if let Some(producer_neighbors) = neighbors.get_mut(producer_id) {
                producer_neighbors.insert(edge.consumer_layer_id().to_string());
            }
        }
    }

    let mut groups = Vec::<BTreeSet<String>>::new();
    let mut visited = BTreeSet::<String>::new();

    for layer in layers {
        if visited.contains(layer.id()) {
            continue;
        }

        let mut group = BTreeSet::new();
        let mut pending = vec![layer.id().to_string()];
        while let Some(layer_id) = pending.pop() {
            if !visited.insert(layer_id.clone()) {
                continue;
            }
            group.insert(layer_id.clone());
            if let Some(layer_neighbors) = neighbors.get(&layer_id) {
                for neighbor in layer_neighbors.iter().rev() {
                    if !visited.contains(neighbor) {
                        pending.push(neighbor.clone());
                    }
                }
            }
        }
        groups.push(group);
    }

    let width = groups.len().max(1).to_string().len().max(4);
    groups
        .into_iter()
        .enumerate()
        .map(|(index, group)| build_component(index + 1, width, &group, layers, edges))
        .collect()
}

fn build_component(
    component_number: usize,
    width: usize,
    layer_ids: &BTreeSet<String>,
    layers: &[TransformationLayer],
    edges: &[GraphEdge],
) -> GraphComponent {
    let relevant_edges = edges
        .iter()
        .filter(|edge| layer_ids.contains(edge.consumer_layer_id()))
        .collect::<Vec<_>>();
    let has_cycle = relevant_edges
        .iter()
        .any(|edge| edge.resolution() == RelationResolution::Cycle);
    let has_ambiguity = relevant_edges
        .iter()
        .any(|edge| edge.resolution() == RelationResolution::Ambiguous);

    let mut diagnostics = relevant_edges
        .iter()
        .filter_map(|edge| match edge.resolution() {
            RelationResolution::Ambiguous => Some(CompositionDiagnostic::warning(
                "ambiguous_relation_producer",
                format!(
                    "relation '{}' has multiple in-bundle producers: {}",
                    edge.relation(),
                    edge.producer_layer_ids().join(", ")
                ),
                Some(edge.consumer_layer_id().to_string()),
                Some(edge.relation().to_string()),
            )),
            RelationResolution::Cycle => Some(CompositionDiagnostic::warning(
                "dependency_cycle",
                format!(
                    "relation '{}' participates in a dependency cycle",
                    edge.relation()
                ),
                Some(edge.consumer_layer_id().to_string()),
                Some(edge.relation().to_string()),
            )),
            RelationResolution::Resolved
            | RelationResolution::External
            | RelationResolution::Missing
            | RelationResolution::Unsupported => None,
        })
        .collect::<Vec<_>>();
    diagnostics.sort_by(diagnostic_cmp);
    diagnostics.dedup();

    let ordered_layer_ids = if has_cycle {
        layer_ids.iter().cloned().collect()
    } else {
        topological_layer_order(layer_ids, edges)
    };

    let final_outcomes = if has_cycle || has_ambiguity {
        Vec::new()
    } else {
        component_final_outcomes(layer_ids, layers, edges)
    };

    GraphComponent {
        id: format!("component-{:0width$}", component_number, width = width),
        layer_ids: ordered_layer_ids,
        final_outcomes,
        diagnostics,
    }
}

fn topological_layer_order(layer_ids: &BTreeSet<String>, edges: &[GraphEdge]) -> Vec<String> {
    let mut indegree = layer_ids
        .iter()
        .map(|layer_id| (layer_id.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut consumers = BTreeMap::<String, BTreeSet<String>>::new();

    for edge in edges {
        if edge.resolution() != RelationResolution::Resolved {
            continue;
        }
        let Some(producer_id) = edge.producer_layer_ids().first() else {
            continue;
        };
        if !layer_ids.contains(edge.consumer_layer_id()) || !layer_ids.contains(producer_id) {
            continue;
        }

        consumers
            .entry(producer_id.clone())
            .or_default()
            .insert(edge.consumer_layer_id().to_string());
        if let Some(value) = indegree.get_mut(edge.consumer_layer_id()) {
            *value += 1;
        }
    }

    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(layer_id, _)| layer_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(layer_ids.len());

    while let Some(layer_id) = ready.pop_first() {
        ordered.push(layer_id.clone());
        let Some(layer_consumers) = consumers.get(&layer_id) else {
            continue;
        };
        for consumer_id in layer_consumers {
            let Some(degree) = indegree.get_mut(consumer_id) else {
                continue;
            };
            *degree -= 1;
            if *degree == 0 {
                ready.insert(consumer_id.clone());
            }
        }
    }

    if ordered.len() == layer_ids.len() {
        ordered
    } else {
        layer_ids.iter().cloned().collect()
    }
}

fn component_final_outcomes(
    layer_ids: &BTreeSet<String>,
    layers: &[TransformationLayer],
    edges: &[GraphEdge],
) -> Vec<DatasetRef> {
    let consumed_producer_ids = edges
        .iter()
        .filter(|edge| edge.resolution() == RelationResolution::Resolved)
        .flat_map(|edge| edge.producer_layer_ids().iter().cloned())
        .collect::<BTreeSet<_>>();

    let mut outcomes = layers
        .iter()
        .filter(|layer| {
            layer_ids.contains(layer.id()) && !consumed_producer_ids.contains(layer.id())
        })
        .flat_map(|layer| layer.produces().iter().cloned())
        .collect::<Vec<_>>();
    outcomes.sort();
    outcomes.dedup();
    outcomes
}

/// Error returned when an explicit target relation cannot be selected safely.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TargetSelectionError {
    /// No supplied transformation produces the requested relation.
    UnknownTarget {
        /// Requested relation identifier.
        target: String,
    },
    /// More than one supplied transformation produces the requested relation.
    AmbiguousTarget {
        /// Requested relation identifier.
        target: String,
        /// Candidate producer layer identifiers in deterministic order.
        producer_layer_ids: Vec<String>,
    },
}

impl TargetSelectionError {
    /// Return the requested relation identifier.
    pub fn target(&self) -> &str {
        match self {
            Self::UnknownTarget { target } | Self::AmbiguousTarget { target, .. } => target,
        }
    }

    /// Return candidate producer layers when the requested relation is ambiguous.
    pub fn producer_layer_ids(&self) -> &[String] {
        match self {
            Self::UnknownTarget { .. } => &[],
            Self::AmbiguousTarget {
                producer_layer_ids, ..
            } => producer_layer_ids,
        }
    }
}

impl fmt::Display for TargetSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget { target } => write!(
                formatter,
                "target relation '{target}' is not produced by any supplied transformation"
            ),
            Self::AmbiguousTarget {
                target,
                producer_layer_ids,
            } => write!(
                formatter,
                "target relation '{target}' is ambiguous; produced by layers {}",
                producer_layer_ids.join(", ")
            ),
        }
    }
}

impl std::error::Error for TargetSelectionError {}

/// Project a fully analyzed bundle onto explicit named target relations.
///
/// Target selection is deliberately applied after analysis and composition. The returned bundle
/// keeps the complete analyzed input list, while its transformation layers and dependency graph
/// are limited to the requested target producers plus every in-bundle ancestor needed to describe
/// them. Passing no targets returns the complete bundle unchanged.
pub fn select_targets(
    bundle: &AnalysisBundle,
    targets: &[String],
) -> Result<AnalysisBundle, TargetSelectionError> {
    if targets.is_empty() {
        return Ok(bundle.clone());
    }

    let producers = collect_producers(bundle.layers());
    let mut selected_layer_ids = BTreeSet::new();
    let mut pending = Vec::new();

    for target in targets {
        match producers.get(target) {
            None => {
                return Err(TargetSelectionError::UnknownTarget {
                    target: target.clone(),
                });
            }
            Some(producer_layer_ids) if producer_layer_ids.len() == 1 => {
                pending.push(producer_layer_ids[0].clone());
            }
            Some(producer_layer_ids) => {
                return Err(TargetSelectionError::AmbiguousTarget {
                    target: target.clone(),
                    producer_layer_ids: producer_layer_ids.clone(),
                });
            }
        }
    }

    while let Some(layer_id) = pending.pop() {
        if !selected_layer_ids.insert(layer_id.clone()) {
            continue;
        }

        for edge in bundle
            .graph()
            .edges()
            .iter()
            .filter(|edge| edge.consumer_layer_id() == layer_id)
        {
            for producer_layer_id in edge.producer_layer_ids().iter().rev() {
                if !selected_layer_ids.contains(producer_layer_id) {
                    pending.push(producer_layer_id.clone());
                }
            }
        }
    }

    let layers = bundle
        .layers()
        .iter()
        .filter(|layer| selected_layer_ids.contains(layer.id()))
        .cloned()
        .collect::<Vec<_>>();
    let graph = build_graph(&layers);

    Ok(AnalysisBundle {
        protocol_version: bundle.protocol_version,
        inputs: bundle.inputs.clone(),
        layers,
        graph,
    })
}

fn diagnostic_cmp(
    left: &CompositionDiagnostic,
    right: &CompositionDiagnostic,
) -> std::cmp::Ordering {
    (
        left.severity().as_str(),
        left.code(),
        left.input_id(),
        left.layer_id(),
        left.relation(),
        left.message(),
    )
        .cmp(&(
            right.severity().as_str(),
            right.code(),
            right.input_id(),
            right.layer_id(),
            right.relation(),
            right.message(),
        ))
}

/// Error produced while parsing or analyzing one input in a multi-input invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputAnalysisError {
    input_id: String,
    source: SqlInputSource,
    error: Error,
}

impl InputAnalysisError {
    /// Return the deterministic identifier of the input that failed.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Return the source identity of the input that failed.
    pub fn input_source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the underlying parse or analysis error.
    pub fn error(&self) -> &Error {
        &self.error
    }
}

impl fmt::Display for InputAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            SqlInputSource::Inline => {
                write!(
                    formatter,
                    "input {} (inline): {}",
                    self.input_id, self.error
                )
            }
            SqlInputSource::File { path } => write!(
                formatter,
                "input {} (file '{}'): {}",
                self.input_id, path, self.error
            ),
        }
    }
}

impl std::error::Error for InputAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Error produced before or during configured multi-input analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfiguredInputAnalysisError {
    /// A configured input identifier is empty or whitespace-only.
    InvalidInputId {
        /// One-based input position in caller order.
        position: usize,
    },
    /// More than one configured input uses the same stable identity.
    DuplicateInputId {
        /// Duplicated stable input identifier.
        id: String,
    },
    /// SQL parsing or semantic analysis failed for one configured input.
    Input(InputAnalysisError),
}

impl ConfiguredInputAnalysisError {
    /// Return the underlying per-input analysis error when SQL processing failed.
    pub fn input_error(&self) -> Option<&InputAnalysisError> {
        match self {
            Self::Input(error) => Some(error),
            Self::InvalidInputId { .. } | Self::DuplicateInputId { .. } => None,
        }
    }
}

impl fmt::Display for ConfiguredInputAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInputId { position } => {
                write!(formatter, "configured input {position} has an empty id")
            }
            Self::DuplicateInputId { id } => {
                write!(formatter, "configured input id '{id}' is duplicated")
            }
            Self::Input(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ConfiguredInputAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::InvalidInputId { .. } | Self::DuplicateInputId { .. } => None,
        }
    }
}

fn analyze_input(
    input_id: String,
    input: &SqlInput,
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<AnalyzedInput, InputAnalysisError> {
    let protocol = analyze_sql(input.sql(), dialect_name, dialect).map_err(|error| {
        InputAnalysisError {
            input_id: input_id.clone(),
            source: input.source().clone(),
            error,
        }
    })?;

    Ok(AnalyzedInput {
        id: input_id,
        source: input.source().clone(),
        dialect: dialect_name.to_string(),
        statements: protocol.statements().to_vec(),
    })
}

/// Parse and analyze an arbitrary number of SQL inputs using one caller-selected dialect.
///
/// Inputs are analyzed in caller order. Generated IDs start at `input-0001`; the numeric width
/// expands when necessary rather than imposing a maximum input count.
pub fn analyze_inputs(
    inputs: &[SqlInput],
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<AnalysisBundle, InputAnalysisError> {
    let width = inputs.len().max(1).to_string().len().max(4);
    let mut analyzed_inputs = Vec::with_capacity(inputs.len());

    for (index, input) in inputs.iter().enumerate() {
        let input_id = format!("input-{:0width$}", index + 1, width = width);
        analyzed_inputs.push(analyze_input(input_id, input, dialect_name, dialect)?);
    }

    Ok(AnalysisBundle::from_inputs(analyzed_inputs))
}

/// Analyze inputs that each carry an explicit stable identity and dialect.
///
/// Input order remains significant for deterministic layer ordering. Identifiers must be non-empty
/// and unique. Dialect implementations are supplied by the caller so sqlparser remains confined to
/// the parsing boundary.
pub fn analyze_configured_inputs(
    inputs: &[ConfiguredSqlInput<'_>],
) -> Result<AnalysisBundle, ConfiguredInputAnalysisError> {
    let mut seen_ids = BTreeSet::new();
    let mut analyzed_inputs = Vec::with_capacity(inputs.len());

    for (index, configured) in inputs.iter().enumerate() {
        let id = configured.id().trim();
        if id.is_empty() {
            return Err(ConfiguredInputAnalysisError::InvalidInputId {
                position: index + 1,
            });
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ConfiguredInputAnalysisError::DuplicateInputId {
                id: id.to_string(),
            });
        }

        let analyzed = analyze_input(
            id.to_string(),
            configured.input(),
            configured.dialect_name(),
            configured.dialect(),
        )
        .map_err(ConfiguredInputAnalysisError::Input)?;
        analyzed_inputs.push(analyzed);
    }

    Ok(AnalysisBundle::from_inputs(analyzed_inputs))
}
