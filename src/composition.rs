//! Transitive semantic composition across linked transformation layers.
//!
//! Composition operates only on parser-independent protocol values and the deterministic relation
//! graph. It follows proven producer links, resolves lineage back to physical leaves, and propagates
//! value domains only through direct column identity transformations.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::{
    AnalysisGraph, AnalyzedInput, ComposedSemantics, CompositionDiagnostic,
    CompositionFailureReason, GraphEdge, RelationResolution, ResolvedComposedSemantics,
    TransformationLayer,
};
use crate::domain::intersect_domains;
use crate::protocol::{
    ColumnDomain, ColumnRef, Expression, LineageSource, Output, OutputColumn, ProtocolStatement,
    QueryStatement, ValueDomain,
};

pub(crate) fn compose_layers(
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
) -> Vec<ComposedSemantics> {
    let layer_indices = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id().to_string(), index))
        .collect::<BTreeMap<_, _>>();
    let mut memo = BTreeMap::<String, ComposedSemantics>::new();

    layers
        .iter()
        .map(|layer| {
            compose_layer(
                layer.id(),
                inputs,
                layers,
                graph,
                &layer_indices,
                &mut memo,
            )
        })
        .collect()
}

fn compose_layer(
    layer_id: &str,
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
    layer_indices: &BTreeMap<String, usize>,
    memo: &mut BTreeMap<String, ComposedSemantics>,
) -> ComposedSemantics {
    if let Some(composed) = memo.get(layer_id) {
        return composed.clone();
    }

    let Some(layer) = layer_indices
        .get(layer_id)
        .and_then(|index| layers.get(*index))
    else {
        return unresolved_internal_layer(layer_id);
    };
    let Some(query) = query_for_layer(inputs, layer) else {
        let composed = ComposedSemantics::unresolved(
            CompositionFailureReason::Unsupported,
            vec![CompositionDiagnostic::layer_warning(
                layer.input_id(),
                layer.id(),
                "missing_layer_query",
                "transformation layer does not reference an analyzable query statement",
                None,
            )],
        );
        memo.insert(layer_id.to_string(), composed.clone());
        return composed;
    };

    let edges = graph
        .edges()
        .iter()
        .filter(|edge| edge.consumer_layer_id() == layer.id())
        .collect::<Vec<_>>();

    if let Some(reason) = graph_failure_reason(&edges) {
        let diagnostics = graph_failure_diagnostics(layer, &edges);
        let composed = ComposedSemantics::unresolved(reason, diagnostics);
        memo.insert(layer_id.to_string(), composed.clone());
        return composed;
    }

    let mut dependencies = BTreeSet::<String>::new();
    let mut domain_map = BTreeMap::<ColumnRef, ValueDomain>::new();
    let mut diagnostics = Vec::<CompositionDiagnostic>::new();

    for edge in &edges {
        match edge.resolution() {
            RelationResolution::External => {
                dependencies.insert(edge.relation().to_string());
            }
            RelationResolution::Resolved => {
                let Some(producer_id) = edge.producer_layer_ids().first() else {
                    let composed = ComposedSemantics::unresolved(
                        CompositionFailureReason::MissingProducer,
                        vec![CompositionDiagnostic::layer_warning(
                            layer.input_id(),
                            layer.id(),
                            "missing_relation_producer",
                            format!(
                                "relation '{}' is marked resolved without a producer",
                                edge.relation()
                            ),
                            Some(edge.relation().to_string()),
                        )],
                    );
                    memo.insert(layer_id.to_string(), composed.clone());
                    return composed;
                };

                match compose_layer(
                    producer_id,
                    inputs,
                    layers,
                    graph,
                    layer_indices,
                    memo,
                ) {
                    ComposedSemantics::Resolved(upstream) => {
                        dependencies.extend(upstream.dependencies().iter().cloned());
                        merge_column_domains(&mut domain_map, upstream.column_domains());
                    }
                    ComposedSemantics::Unresolved(upstream) => {
                        let mut upstream_diagnostics = upstream.diagnostics().to_vec();
                        upstream_diagnostics.push(CompositionDiagnostic::layer_warning(
                            layer.input_id(),
                            layer.id(),
                            "upstream_composition_unresolved",
                            format!(
                                "relation '{}' depends on unresolved producer '{}'",
                                edge.relation(),
                                producer_id
                            ),
                            Some(edge.relation().to_string()),
                        ));
                        let composed =
                            ComposedSemantics::unresolved(upstream.reason(), upstream_diagnostics);
                        memo.insert(layer_id.to_string(), composed.clone());
                        return composed;
                    }
                }
            }
            RelationResolution::Missing
            | RelationResolution::Ambiguous
            | RelationResolution::Cycle
            | RelationResolution::Unsupported => {
                // These states are handled before upstream traversal.
            }
        }
    }

    for column_domain in query.column_domains() {
        match resolve_column_identity(
            layer,
            column_domain.column(),
            inputs,
            layers,
            graph,
            layer_indices,
        ) {
            Ok(source) => {
                let column = ColumnRef::new(
                    Some(source.relation().to_string()),
                    source.column().to_string(),
                );
                merge_domain(
                    &mut domain_map,
                    column,
                    column_domain.domain().clone(),
                );
            }
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    let output = compose_output(
        layer,
        query,
        inputs,
        layers,
        graph,
        layer_indices,
        memo,
        &mut diagnostics,
    );

    let column_domains = domain_map
        .into_iter()
        .map(|(column, domain)| ColumnDomain::new(column, domain))
        .collect::<Vec<_>>();
    let composed = ComposedSemantics::resolved(
        dependencies.into_iter().collect(),
        column_domains,
        output,
        diagnostics,
    );
    memo.insert(layer_id.to_string(), composed.clone());
    composed
}

fn query_for_layer<'a>(
    inputs: &'a [AnalyzedInput],
    layer: &TransformationLayer,
) -> Option<&'a QueryStatement> {
    let input = inputs.iter().find(|input| input.id() == layer.input_id())?;
    match input.statements().get(layer.statement_index())? {
        ProtocolStatement::Query(query) => Some(query),
        ProtocolStatement::Unsupported(_) => None,
    }
}

fn graph_failure_reason(edges: &[&GraphEdge]) -> Option<CompositionFailureReason> {
    if edges
        .iter()
        .any(|edge| edge.resolution() == RelationResolution::Cycle)
    {
        return Some(CompositionFailureReason::Cycle);
    }
    if edges
        .iter()
        .any(|edge| edge.resolution() == RelationResolution::Ambiguous)
    {
        return Some(CompositionFailureReason::AmbiguousProducer);
    }
    if edges
        .iter()
        .any(|edge| edge.resolution() == RelationResolution::Missing)
    {
        return Some(CompositionFailureReason::MissingProducer);
    }
    if edges
        .iter()
        .any(|edge| edge.resolution() == RelationResolution::Unsupported)
    {
        return Some(CompositionFailureReason::Unsupported);
    }
    None
}

fn graph_failure_diagnostics(
    layer: &TransformationLayer,
    edges: &[&GraphEdge],
) -> Vec<CompositionDiagnostic> {
    edges
        .iter()
        .filter_map(|edge| {
            let (code, message) = match edge.resolution() {
                RelationResolution::Ambiguous => (
                    "ambiguous_relation_producer",
                    format!(
                        "relation '{}' has multiple in-bundle producers: {}",
                        edge.relation(),
                        edge.producer_layer_ids().join(", ")
                    ),
                ),
                RelationResolution::Cycle => (
                    "dependency_cycle",
                    format!(
                        "relation '{}' participates in a dependency cycle",
                        edge.relation()
                    ),
                ),
                RelationResolution::Missing => (
                    "missing_relation_producer",
                    format!("relation '{}' requires a producer that is unavailable", edge.relation()),
                ),
                RelationResolution::Unsupported => (
                    "unsupported_relation_composition",
                    format!("relation '{}' cannot be composed safely", edge.relation()),
                ),
                RelationResolution::Resolved | RelationResolution::External => return None,
            };
            Some(CompositionDiagnostic::layer_warning(
                layer.input_id(),
                layer.id(),
                code,
                message,
                Some(edge.relation().to_string()),
            ))
        })
        .collect()
}

fn merge_column_domains(
    target: &mut BTreeMap<ColumnRef, ValueDomain>,
    domains: &[ColumnDomain],
) {
    for domain in domains {
        merge_domain(
            target,
            domain.column().clone(),
            domain.domain().clone(),
        );
    }
}

fn merge_domain(
    target: &mut BTreeMap<ColumnRef, ValueDomain>,
    column: ColumnRef,
    domain: ValueDomain,
) {
    match target.remove(&column) {
        Some(existing) => {
            target.insert(column, intersect_domains(&existing, &domain));
        }
        None => {
            target.insert(column, domain);
        }
    }
}

fn resolve_column_identity(
    layer: &TransformationLayer,
    column: &ColumnRef,
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
    layer_indices: &BTreeMap<String, usize>,
) -> Result<LineageSource, CompositionDiagnostic> {
    let Some(relation) = column.relation() else {
        return Err(CompositionDiagnostic::layer_warning(
            layer.input_id(),
            layer.id(),
            "unresolved_domain_column_relation",
            format!(
                "column '{}' cannot be mapped to one source relation",
                column.name()
            ),
            None,
        ));
    };

    resolve_source_identity(
        layer,
        &LineageSource::new(relation.to_string(), column.name().to_string()),
        inputs,
        layers,
        graph,
        layer_indices,
    )
}

fn resolve_source_identity(
    consumer: &TransformationLayer,
    source: &LineageSource,
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
    layer_indices: &BTreeMap<String, usize>,
) -> Result<LineageSource, CompositionDiagnostic> {
    let Some(edge) = edge_for_source(graph, consumer.id(), source.relation()) else {
        return Err(CompositionDiagnostic::layer_warning(
            consumer.input_id(),
            consumer.id(),
            "missing_lineage_edge",
            format!(
                "source column '{}.{}' has no dependency edge",
                source.relation(),
                source.column()
            ),
            Some(source.relation().to_string()),
        ));
    };

    match edge.resolution() {
        RelationResolution::External => Ok(source.clone()),
        RelationResolution::Resolved => {
            let Some(producer_id) = edge.producer_layer_ids().first() else {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "missing_relation_producer",
                    format!(
                        "source column '{}.{}' has no resolved producer",
                        source.relation(),
                        source.column()
                    ),
                    Some(source.relation().to_string()),
                ));
            };
            let Some(producer) = layer_indices
                .get(producer_id)
                .and_then(|index| layers.get(*index))
            else {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "missing_relation_producer",
                    format!("producer layer '{}' does not exist", producer_id),
                    Some(source.relation().to_string()),
                ));
            };
            let Some(query) = query_for_layer(inputs, producer) else {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "missing_producer_query",
                    format!("producer layer '{}' has no analyzable query", producer_id),
                    Some(source.relation().to_string()),
                ));
            };

            let matches = query
                .output()
                .columns()
                .iter()
                .filter(|column| column.name() == source.column())
                .collect::<Vec<_>>();
            let [column] = matches.as_slice() else {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "unresolved_producer_column",
                    format!(
                        "producer '{}' does not expose one unambiguous column named '{}'",
                        producer_id,
                        source.column()
                    ),
                    Some(source.relation().to_string()),
                ));
            };

            if !matches!(column.expression(), Expression::Column(_)) {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "non_invertible_column_transform",
                    format!(
                        "column '{}.{}' is produced by a non-identity expression",
                        source.relation(),
                        source.column()
                    ),
                    Some(source.relation().to_string()),
                ));
            }

            let [upstream] = column.lineage() else {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "unresolved_column_identity",
                    format!(
                        "column '{}.{}' does not have exactly one identity source",
                        source.relation(),
                        source.column()
                    ),
                    Some(source.relation().to_string()),
                ));
            };

            resolve_source_identity(
                producer,
                upstream,
                inputs,
                layers,
                graph,
                layer_indices,
            )
        }
        RelationResolution::Missing
        | RelationResolution::Ambiguous
        | RelationResolution::Cycle
        | RelationResolution::Unsupported => Err(CompositionDiagnostic::layer_warning(
            consumer.input_id(),
            consumer.id(),
            "unresolved_column_identity",
            format!(
                "column '{}.{}' depends on relation resolution '{}'",
                source.relation(),
                source.column(),
                edge.resolution().as_str()
            ),
            Some(source.relation().to_string()),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn compose_output(
    layer: &TransformationLayer,
    query: &QueryStatement,
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
    layer_indices: &BTreeMap<String, usize>,
    memo: &mut BTreeMap<String, ComposedSemantics>,
    diagnostics: &mut Vec<CompositionDiagnostic>,
) -> Output {
    let mut columns = Vec::with_capacity(query.output().columns().len());

    for column in query.output().columns() {
        let mut lineage = BTreeSet::<LineageSource>::new();

        for source in column.lineage() {
            match expand_lineage_source(
                layer,
                source,
                inputs,
                layers,
                graph,
                layer_indices,
                memo,
            ) {
                Ok(sources) => lineage.extend(sources),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }

        columns.push(OutputColumn::new(
            column.name().to_string(),
            column.expression().clone(),
            lineage.into_iter().collect(),
        ));
    }

    Output::new(columns)
}

#[allow(clippy::too_many_arguments)]
fn expand_lineage_source(
    consumer: &TransformationLayer,
    source: &LineageSource,
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
    layer_indices: &BTreeMap<String, usize>,
    memo: &mut BTreeMap<String, ComposedSemantics>,
) -> Result<Vec<LineageSource>, CompositionDiagnostic> {
    let Some(edge) = edge_for_source(graph, consumer.id(), source.relation()) else {
        return Err(CompositionDiagnostic::layer_warning(
            consumer.input_id(),
            consumer.id(),
            "missing_lineage_edge",
            format!(
                "source column '{}.{}' has no dependency edge",
                source.relation(),
                source.column()
            ),
            Some(source.relation().to_string()),
        ));
    };

    match edge.resolution() {
        RelationResolution::External => Ok(vec![source.clone()]),
        RelationResolution::Resolved => {
            let Some(producer_id) = edge.producer_layer_ids().first() else {
                return Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "missing_relation_producer",
                    format!(
                        "source column '{}.{}' has no resolved producer",
                        source.relation(),
                        source.column()
                    ),
                    Some(source.relation().to_string()),
                ));
            };

            match compose_layer(
                producer_id,
                inputs,
                layers,
                graph,
                layer_indices,
                memo,
            ) {
                ComposedSemantics::Resolved(producer) => {
                    let matches = producer
                        .output()
                        .columns()
                        .iter()
                        .filter(|column| column.name() == source.column())
                        .collect::<Vec<_>>();
                    let [column] = matches.as_slice() else {
                        return Err(CompositionDiagnostic::layer_warning(
                            consumer.input_id(),
                            consumer.id(),
                            "unresolved_producer_column",
                            format!(
                                "producer '{}' does not expose one unambiguous column named '{}'",
                                producer_id,
                                source.column()
                            ),
                            Some(source.relation().to_string()),
                        ));
                    };
                    Ok(column.lineage().to_vec())
                }
                ComposedSemantics::Unresolved(_) => Err(CompositionDiagnostic::layer_warning(
                    consumer.input_id(),
                    consumer.id(),
                    "upstream_lineage_unresolved",
                    format!(
                        "source column '{}.{}' belongs to an unresolved producer",
                        source.relation(),
                        source.column()
                    ),
                    Some(source.relation().to_string()),
                )),
            }
        }
        RelationResolution::Missing
        | RelationResolution::Ambiguous
        | RelationResolution::Cycle
        | RelationResolution::Unsupported => Err(CompositionDiagnostic::layer_warning(
            consumer.input_id(),
            consumer.id(),
            "unresolved_lineage_relation",
            format!(
                "source column '{}.{}' depends on relation resolution '{}'",
                source.relation(),
                source.column(),
                edge.resolution().as_str()
            ),
            Some(source.relation().to_string()),
        )),
    }
}

fn edge_for_source<'a>(
    graph: &'a AnalysisGraph,
    consumer_layer_id: &str,
    relation: &str,
) -> Option<&'a GraphEdge> {
    graph.edges().iter().find(|edge| {
        edge.consumer_layer_id() == consumer_layer_id && edge.relation() == relation
    })
}

fn unresolved_internal_layer(layer_id: &str) -> ComposedSemantics {
    ComposedSemantics::unresolved(
        CompositionFailureReason::Unsupported,
        vec![CompositionDiagnostic::layer_warning(
            "unknown-input",
            layer_id,
            "missing_layer",
            format!("transformation layer '{}' does not exist", layer_id),
            None,
        )],
    )
}
