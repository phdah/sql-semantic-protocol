//! Deterministic producer discovery, graph edges, cycles and components.

use super::*;

#[derive(Debug)]
pub(super) struct LayerBuildError {
    input_id: String,
    error: RelationResolutionError,
}

pub(super) fn build_layers(
    inputs: &[AnalyzedInput],
    resolver: &dyn RelationResolver,
) -> Result<Vec<TransformationLayer>, LayerBuildError> {
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
                    name: resolve_relation(resolver, input, name)?,
                }],
                None => vec![DatasetRef::Anonymous {
                    layer_id: layer_id.clone(),
                }],
            };

            let mut relation_identities = BTreeMap::new();
            let mut consumes = BTreeSet::new();
            for relation in query.dependencies() {
                let canonical = resolve_relation(resolver, input, relation)?;
                relation_identities.insert(relation.clone(), canonical.clone());
                consumes.insert(canonical);
            }

            let composed_semantics = ComposedSemantics::pending(input.id(), &layer_id);
            layers.push(TransformationLayer {
                id: layer_id,
                input_id: input.id().to_string(),
                statement_index,
                produces,
                consumes: consumes.into_iter().collect(),
                relation_identities,
                write_kind: query.write().map(|write| write.kind()),
                composed_semantics,
            });
        }
    }

    Ok(layers)
}

pub(super) fn resolve_relation(
    resolver: &dyn RelationResolver,
    input: &AnalyzedInput,
    relation: &str,
) -> Result<String, LayerBuildError> {
    resolver
        .resolve_relation(relation, input.dialect(), input.relation_context())
        .map_err(|error| LayerBuildError {
            input_id: input.id().to_string(),
            error,
        })
}

pub(super) fn build_graph(layers: &[TransformationLayer]) -> AnalysisGraph {
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

pub(super) fn collect_producers(layers: &[TransformationLayer]) -> BTreeMap<String, Vec<String>> {
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

pub(super) fn build_edges(
    layers: &[TransformationLayer],
    producers: &BTreeMap<String, Vec<String>>,
) -> Vec<GraphEdge> {
    let mut edges = Vec::new();

    for layer in layers {
        for relation in layer.consumes() {
            let producer_layer_ids = producers.get(relation).cloned().unwrap_or_default();
            let resolution = match producer_layer_ids.len() {
                0 => RelationResolution::External,
                1 => {
                    let producer = layers
                        .iter()
                        .find(|candidate| candidate.id() == producer_layer_ids[0])
                        .expect("producer index is built from existing layers");
                    match producer.write_kind() {
                        Some(kind) if !kind.fully_defines_relation() => RelationResolution::Partial,
                        _ => RelationResolution::Resolved,
                    }
                }
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

pub(super) fn mark_cycle_edges(edges: &mut [GraphEdge]) {
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

pub(super) fn path_exists(start: &str, target: &str, adjacency: &BTreeMap<String, Vec<String>>) -> bool {
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

pub(super) fn build_components(layers: &[TransformationLayer], edges: &[GraphEdge]) -> Vec<GraphComponent> {
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

pub(super) fn build_component(
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
            RelationResolution::Partial => Some(CompositionDiagnostic::warning(
                "partial_relation_producer",
                format!(
                    "relation '{}' is written by a partial mutation and cannot fully define downstream semantics",
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

pub(super) fn topological_layer_order(layer_ids: &BTreeSet<String>, edges: &[GraphEdge]) -> Vec<String> {
    let mut indegree = layer_ids
        .iter()
        .map(|layer_id| (layer_id.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut consumers = BTreeMap::<String, BTreeSet<String>>::new();

    for edge in edges {
        if !matches!(
            edge.resolution(),
            RelationResolution::Resolved | RelationResolution::Partial
        ) {
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

pub(super) fn component_final_outcomes(
    layer_ids: &BTreeSet<String>,
    layers: &[TransformationLayer],
    edges: &[GraphEdge],
) -> Vec<DatasetRef> {
    let consumed_producer_ids = edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.resolution(),
                RelationResolution::Resolved | RelationResolution::Partial
            )
        })
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

