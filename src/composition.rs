//! Transitive semantic composition across linked transformation layers.
//!
//! Composition operates only on parser-independent protocol values and the deterministic relation
//! graph. It follows proven producer links, resolves lineage back to physical leaves, and propagates
//! value domains only through direct column identity transformations.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::{
    AnalysisGraph, AnalyzedInput, ComposedJoinColumn, ComposedJoinEquality, ComposedSemantics,
    ComposedSetOperation, CompositionDiagnostic, CompositionFailureReason, GraphEdge,
    RelationResolution, TransformationLayer,
};
use crate::domain::{intersect_case_domain_values, intersect_domains};
use crate::join_witness::JoinWitness;
use crate::protocol::{
    CaseBranch, CaseExpression, CaseSourceDomainAlternative, CaseSourceDomains, ColumnDomain,
    ColumnExpression, ColumnRef, ComparisonOperator, ConditionClause, ConditionExactness,
    Expression, Join, JoinKind, LineageSource, Output, OutputColumn, Predicate, ProtocolStatement,
    QueryStatement, RelationRef, ResidualCondition, ResidualConditionReason, SourceRelation,
    ValueDomain, WriteKind,
};

pub(crate) fn compose_layers(
    inputs: &[AnalyzedInput],
    layers: &[TransformationLayer],
    graph: &AnalysisGraph,
) -> Vec<ComposedSemantics> {
    Composer::new(inputs, layers, graph).compose_all()
}

struct Composer<'a> {
    inputs: &'a [AnalyzedInput],
    layers: &'a [TransformationLayer],
    graph: &'a AnalysisGraph,
    layer_indices: BTreeMap<String, usize>,
    memo: BTreeMap<String, ComposedSemantics>,
}

impl<'a> Composer<'a> {
    fn new(
        inputs: &'a [AnalyzedInput],
        layers: &'a [TransformationLayer],
        graph: &'a AnalysisGraph,
    ) -> Self {
        let layer_indices = layers
            .iter()
            .enumerate()
            .map(|(index, layer)| (layer.id().to_string(), index))
            .collect();

        Self {
            inputs,
            layers,
            graph,
            layer_indices,
            memo: BTreeMap::new(),
        }
    }

    fn compose_all(mut self) -> Vec<ComposedSemantics> {
        let layer_ids = self
            .layers
            .iter()
            .map(|layer| layer.id().to_string())
            .collect::<Vec<_>>();

        layer_ids
            .iter()
            .map(|layer_id| self.compose_layer(layer_id))
            .collect()
    }

    fn compose_layer(&mut self, layer_id: &str) -> ComposedSemantics {
        if let Some(composed) = self.memo.get(layer_id) {
            return composed.clone();
        }

        let Some(layer) = self.layer_by_id(layer_id).cloned() else {
            return unresolved_internal_layer(layer_id);
        };
        let Some(query) = self.query_for_layer(&layer).cloned() else {
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
            self.memo.insert(layer_id.to_string(), composed.clone());
            return composed;
        };

        if matches!(
            layer.write_kind(),
            Some(WriteKind::ConditionalMutation | WriteKind::Update | WriteKind::Delete)
        ) {
            let composed = ComposedSemantics::unresolved(
                CompositionFailureReason::PartialProducer,
                vec![CompositionDiagnostic::layer_warning(
                    layer.input_id(),
                    layer.id(),
                    "conditional_mutation_partial_semantics",
                    "MERGE mutates existing target state, so complete target semantics cannot be composed",
                    layer
                        .produces()
                        .iter()
                        .find_map(|dataset| dataset.relation_name())
                        .map(str::to_string),
                )],
            );
            self.memo.insert(layer_id.to_string(), composed.clone());
            return composed;
        }

        let edges = self
            .graph
            .edges()
            .iter()
            .filter(|edge| edge.consumer_layer_id() == layer.id())
            .cloned()
            .collect::<Vec<_>>();

        if let Some(reason) = graph_failure_reason(&edges) {
            let diagnostics = graph_failure_diagnostics(&layer, &edges);
            let composed = ComposedSemantics::unresolved(reason, diagnostics);
            self.memo.insert(layer_id.to_string(), composed.clone());
            return composed;
        }

        let mut dependencies = BTreeSet::<String>::new();
        let mut producer_sources = BTreeMap::<String, Vec<String>>::new();
        let mut domain_map = BTreeMap::<ColumnRef, ValueDomain>::new();
        let mut join_equalities = Vec::<ComposedJoinEquality>::new();
        let mut join_witnesses = self.compose_query_join_witnesses(&layer, &query);
        let mut set_operations = query
            .set_operation()
            .map(|operation| {
                vec![ComposedSetOperation::new(
                    layer.id().to_string(),
                    operation.clone(),
                )]
            })
            .unwrap_or_default();
        let mut group_witnesses = query
            .group_witness()
            .map(|witness| {
                let boundary_kind = witness
                    .boundary()
                    .and_then(|boundary| edges.iter().find(|edge| edge.relation() == boundary))
                    .map_or(
                        crate::bundle::GroupBoundaryKind::Unresolved,
                        |edge| match edge.resolution() {
                            RelationResolution::External => {
                                crate::bundle::GroupBoundaryKind::Physical
                            }
                            RelationResolution::Resolved => {
                                crate::bundle::GroupBoundaryKind::Intermediate
                            }
                            _ => crate::bundle::GroupBoundaryKind::Unresolved,
                        },
                    );
                vec![crate::bundle::ComposedGroupWitness::new(
                    layer.id().to_string(),
                    witness.clone(),
                    boundary_kind,
                )]
            })
            .unwrap_or_default();
        let mut window_witnesses = query
            .window_witness()
            .map(|witness| {
                let boundary_kind = witness
                    .boundary()
                    .and_then(|boundary| edges.iter().find(|edge| edge.relation() == boundary))
                    .map_or(
                        crate::bundle::GroupBoundaryKind::Unresolved,
                        |edge| match edge.resolution() {
                            RelationResolution::External => {
                                crate::bundle::GroupBoundaryKind::Physical
                            }
                            RelationResolution::Resolved => {
                                crate::bundle::GroupBoundaryKind::Intermediate
                            }
                            _ => crate::bundle::GroupBoundaryKind::Unresolved,
                        },
                    );
                vec![crate::bundle::ComposedWindowWitness::new(
                    layer.id().to_string(),
                    witness.clone(),
                    boundary_kind,
                )]
            })
            .unwrap_or_default();
        let mut boolean_witnesses = query
            .boolean_witness()
            .map(|witness| {
                let boundary_kind = edges
                    .iter()
                    .find(|edge| edge.relation() == witness.source_relation())
                    .map_or(
                        crate::bundle::GroupBoundaryKind::Unresolved,
                        |edge| match edge.resolution() {
                            RelationResolution::External => {
                                crate::bundle::GroupBoundaryKind::Physical
                            }
                            RelationResolution::Resolved => {
                                crate::bundle::GroupBoundaryKind::Intermediate
                            }
                            _ => crate::bundle::GroupBoundaryKind::Unresolved,
                        },
                    );
                let (witness, boundary_kind) = if boundary_kind
                    == crate::bundle::GroupBoundaryKind::Intermediate
                    && self.boolean_source_has_passthrough_path(&layer, witness.source_relation())
                {
                    match witness.mapped_to_physical(|column| {
                        self.resolve_column_identity(&layer, &query, column)
                            .ok()
                            .map(|source| {
                                ColumnRef::new(
                                    Some(source.relation().to_string()),
                                    source.column().to_string(),
                                )
                            })
                    }) {
                        Some(mapped) => (mapped, crate::bundle::GroupBoundaryKind::Physical),
                        None => (witness.clone(), boundary_kind),
                    }
                } else {
                    (witness.clone(), boundary_kind)
                };
                vec![crate::bundle::ComposedBooleanWitness::new(
                    layer.id().to_string(),
                    witness,
                    boundary_kind,
                )]
            })
            .unwrap_or_default();
        let mut subquery_witnesses = query
            .subquery_witnesses()
            .iter()
            .cloned()
            .map(|witness| {
                let boundary_kind = witness
                    .inner_relation()
                    .and_then(|inner| edges.iter().find(|edge| edge.relation() == inner))
                    .map_or(
                        crate::bundle::GroupBoundaryKind::Unresolved,
                        |edge| match edge.resolution() {
                            RelationResolution::External => {
                                crate::bundle::GroupBoundaryKind::Physical
                            }
                            RelationResolution::Resolved => {
                                crate::bundle::GroupBoundaryKind::Intermediate
                            }
                            _ => crate::bundle::GroupBoundaryKind::Unresolved,
                        },
                    );
                crate::bundle::ComposedSubqueryWitness::new(
                    layer.id().to_string(),
                    witness,
                    boundary_kind,
                )
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::<CompositionDiagnostic>::new();
        let mut condition_exactness: ConditionExactness = query
            .condition_exactness()
            .with_layer_origin(layer.id().to_string());

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
                        self.memo.insert(layer_id.to_string(), composed.clone());
                        return composed;
                    };

                    match self.compose_layer(producer_id) {
                        ComposedSemantics::Resolved(upstream) => {
                            producer_sources.extend(upstream.producer_sources().iter().map(
                                |(relation, sources)| (relation.clone(), sources.clone()),
                            ));
                            producer_sources.insert(
                                edge.relation().to_string(),
                                upstream.dependencies().to_vec(),
                            );
                            dependencies.extend(upstream.dependencies().iter().cloned());
                            merge_column_domains(&mut domain_map, upstream.column_domains());
                            join_equalities.extend(upstream.join_equalities().iter().cloned());
                            join_witnesses.extend(upstream.join_witnesses().iter().cloned());
                            set_operations.extend(upstream.set_operations().iter().cloned());
                            group_witnesses.extend(upstream.group_witnesses().iter().cloned());
                            window_witnesses.extend(upstream.window_witnesses().iter().cloned());
                            subquery_witnesses
                                .extend(upstream.subquery_witnesses().iter().cloned());
                            boolean_witnesses.extend(upstream.boolean_witnesses().iter().cloned());
                            condition_exactness =
                                condition_exactness.merged_with(upstream.condition_exactness());
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
                            let composed = ComposedSemantics::unresolved(
                                upstream.reason(),
                                upstream_diagnostics,
                            );
                            self.memo.insert(layer_id.to_string(), composed.clone());
                            return composed;
                        }
                    }
                }
                RelationResolution::Missing
                | RelationResolution::Ambiguous
                | RelationResolution::Cycle
                | RelationResolution::Partial
                | RelationResolution::Unsupported => {
                    // These states are handled before upstream traversal.
                }
            }
        }

        for column_domain in query.column_domains() {
            match self.resolve_column_identity(&layer, &query, column_domain.column()) {
                Ok(source) => {
                    let column = ColumnRef::new(
                        Some(source.relation().to_string()),
                        source.column().to_string(),
                    );
                    merge_domain(&mut domain_map, column, column_domain.domain().clone());
                }
                Err(diagnostic) => {
                    let identity = format!(
                        "composition_domain:{}",
                        column_domain.column().relation().map_or_else(
                            || column_domain.column().name().to_string(),
                            |relation| format!("{relation}.{}", column_domain.column().name()),
                        )
                    );
                    diagnostics.push(*diagnostic);
                    condition_exactness =
                        condition_exactness.merged_with(&ConditionExactness::from_residuals(vec![
                            ResidualCondition::new(
                                ResidualConditionReason::ComputedExpression,
                                ConditionClause::Where,
                                identity,
                            )
                            .with_layer_origin(layer.id().to_string()),
                        ]));
                }
            }
        }

        let (local_join_equalities, equality_residuals) =
            self.compose_query_join_equalities(&layer, &query);
        join_equalities.extend(local_join_equalities);
        condition_exactness = condition_exactness
            .merged_with(&ConditionExactness::from_residuals(equality_residuals));

        let output = self.compose_output(&layer, &query, &mut diagnostics);
        let column_domains = domain_map
            .into_iter()
            .map(|(column, domain)| ColumnDomain::new(column, domain))
            .collect::<Vec<_>>();
        let composed = ComposedSemantics::resolved(
            dependencies.into_iter().collect(),
            producer_sources,
            column_domains,
            join_equalities,
            crate::bundle::ComposedWitnessEvidence {
                set_operations,
                group_witnesses,
                window_witnesses,
                subquery_witnesses,
                boolean_witnesses,
                join_witnesses,
            },
            condition_exactness,
            output,
            diagnostics,
        );
        self.memo.insert(layer_id.to_string(), composed.clone());
        composed
    }

    fn layer_by_id(&self, layer_id: &str) -> Option<&TransformationLayer> {
        self.layer_indices
            .get(layer_id)
            .and_then(|index| self.layers.get(*index))
    }

    fn query_for_layer(&self, layer: &TransformationLayer) -> Option<&QueryStatement> {
        let input = self
            .inputs
            .iter()
            .find(|input| input.id() == layer.input_id())?;
        match input.statements().get(layer.statement_index())? {
            ProtocolStatement::Query(query) => Some(query),
            ProtocolStatement::Unsupported(_) => None,
        }
    }

    /// Only a single, unambiguous binary join permits source-level row witnesses.
    /// Chained joins have composite left inputs and must not be flattened into independent
    /// pairwise obligations (in particular when an earlier join null-extends a row).
    fn compose_query_join_witnesses(
        &self,
        layer: &TransformationLayer,
        query: &QueryStatement,
    ) -> Vec<JoinWitness> {
        query
            .joins()
            .iter()
            .map(|join| {
                let residual = |reason: &str| {
                    JoinWitness::residual(join.kind(), layer.id().to_string(), reason)
                };
                if query.joins().len() != 1 {
                    return residual("composite_join_tree");
                }
                if matches!(join.kind(), JoinKind::Unknown | JoinKind::Cross) {
                    return residual("unsupported_join_kind");
                }
                let Some(Predicate::Comparison(comparison)) = join.condition() else {
                    return residual("join_condition_not_single_column_comparison");
                };
                let (Expression::Column(left), Expression::Column(right)) =
                    (comparison.left(), comparison.right())
                else {
                    return residual("computed_join_comparison");
                };
                if matches!(
                    comparison.operator(),
                    ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
                ) {
                    return residual("null_safe_comparison_not_supported");
                }
                // A physical-row witness is valid only when any named input contains
                // exactly its source rows. A filtered producer can remove a matching
                // partner and make a physical left_unmatched claim false.
                if !self.join_input_preserves_source_rows(layer, join.left().relation())
                    || !self.join_input_preserves_source_rows(layer, join.right().relation())
                {
                    return residual("upstream_row_membership_not_preserved");
                }
                let Ok(left_endpoint) = self.compose_join_column(layer, query, left, Some(join))
                else {
                    return residual("unresolved_left_physical_lineage");
                };
                let Ok(right_endpoint) = self.compose_join_column(layer, query, right, Some(join))
                else {
                    return residual("unresolved_right_physical_lineage");
                };
                let left_name = join.left().alias().unwrap_or(join.left().relation());
                let right_name = join.right().alias().unwrap_or(join.right().relation());
                if left_name == right_name {
                    return residual("ambiguous_relation_instances");
                }
                let (left_endpoint, right_endpoint, operator) = if left_endpoint.relation_instance()
                    == left_name
                    && right_endpoint.relation_instance() == right_name
                {
                    (left_endpoint, right_endpoint, comparison.operator())
                } else if left_endpoint.relation_instance() == right_name
                    && right_endpoint.relation_instance() == left_name
                {
                    (
                        right_endpoint,
                        left_endpoint,
                        comparison.operator().reversed(),
                    )
                } else {
                    return residual("comparison_does_not_link_join_participants");
                };
                JoinWitness::exact(
                    join.kind(),
                    left_endpoint,
                    right_endpoint,
                    operator,
                    layer.id().to_string(),
                )
            })
            .collect()
    }

    /// A join-local witness is expressed against physical source rows. Only plain,
    /// row-preserving projections may be traced through named producer layers:
    /// predicates or row-shaping operators can change which partners exist.
    fn join_input_preserves_source_rows(
        &self,
        layer: &TransformationLayer,
        relation: &str,
    ) -> bool {
        let Some(edge) = self.edge_for_source(layer.id(), relation) else {
            return false;
        };
        match edge.resolution() {
            RelationResolution::External => true,
            RelationResolution::Resolved => {
                let [producer_id] = edge.producer_layer_ids() else {
                    return false;
                };
                let Some(producer) = self.layer_by_id(producer_id) else {
                    return false;
                };
                let Some(query) = self.query_for_layer(producer) else {
                    return false;
                };
                let [source] = query.sources() else {
                    return false;
                };
                if producer.write_kind() != Some(WriteKind::Definition)
                    || !query.joins().is_empty()
                    || query.aggregation().is_some()
                    || query.set_operation().is_some()
                    || query.window_witness().is_some()
                    || query.predicates().where_predicate().is_some()
                    || query.predicates().having_predicate().is_some()
                    || query.predicates().qualify_predicate().is_some()
                    || !query.condition_exactness().is_exact()
                    || !query.diagnostics().is_empty()
                    || query
                        .output()
                        .columns()
                        .iter()
                        .any(|column| column.plain_copy_source().is_none())
                {
                    return false;
                }
                self.join_input_preserves_source_rows(producer, source.name())
            }
            RelationResolution::Missing
            | RelationResolution::Ambiguous
            | RelationResolution::Cycle
            | RelationResolution::Partial
            | RelationResolution::Unsupported => false,
        }
    }

    fn compose_query_join_equalities(
        &self,
        layer: &TransformationLayer,
        query: &QueryStatement,
    ) -> (Vec<ComposedJoinEquality>, Vec<ResidualCondition>) {
        let mut equalities = Vec::new();
        let mut residuals = Vec::new();

        for (join_index, join) in query.joins().iter().enumerate() {
            let Some(condition) = join.condition() else {
                continue;
            };
            let mut pairs = Vec::new();
            collect_conjunctive_column_equalities(condition, &mut pairs);
            for (equality_index, (left, right)) in pairs.into_iter().enumerate() {
                match self.compose_join_equality(layer, query, left, right, join.kind(), Some(join))
                {
                    Ok(equality) => equalities.push(equality),
                    Err(()) => residuals.push(
                        ResidualCondition::new(
                            ResidualConditionReason::ComputedExpression,
                            ConditionClause::JoinOn,
                            format!("join_equality:join:{join_index}:{equality_index}"),
                        )
                        .with_layer_origin(layer.id().to_string()),
                    ),
                }
            }
        }

        if let Some(predicate) = query.predicates().where_predicate() {
            let mut pairs = Vec::new();
            collect_conjunctive_column_equalities(predicate, &mut pairs);
            for (equality_index, (left, right)) in pairs.into_iter().enumerate() {
                if !columns_reference_distinct_query_sources(left, right, query.sources()) {
                    continue;
                }
                match self.compose_join_equality(layer, query, left, right, JoinKind::Inner, None) {
                    Ok(equality) => equalities.push(equality),
                    Err(()) => residuals.push(
                        ResidualCondition::new(
                            ResidualConditionReason::ComputedExpression,
                            ConditionClause::Where,
                            format!("join_equality:where:{equality_index}"),
                        )
                        .with_layer_origin(layer.id().to_string()),
                    ),
                }
            }
        }

        (equalities, residuals)
    }

    fn compose_join_equality(
        &self,
        layer: &TransformationLayer,
        query: &QueryStatement,
        left: &ColumnExpression,
        right: &ColumnExpression,
        kind: JoinKind,
        join: Option<&Join>,
    ) -> Result<ComposedJoinEquality, ()> {
        let left = self.compose_join_column(layer, query, left, join)?;
        let right = self.compose_join_column(layer, query, right, join)?;

        if left.relation() == right.relation()
            && left.relation_instance() == right.relation_instance()
        {
            return Err(());
        }

        Ok(ComposedJoinEquality::new(
            left,
            right,
            kind,
            layer.id().to_string(),
        ))
    }

    fn compose_join_column(
        &self,
        layer: &TransformationLayer,
        query: &QueryStatement,
        column: &ColumnExpression,
        join: Option<&Join>,
    ) -> Result<ComposedJoinColumn, ()> {
        let (source_relation, relation_instance) =
            resolve_query_column_instance(column, query.sources(), join)?;
        let physical = self
            .resolve_source_identity(
                layer,
                &LineageSource::new(source_relation, column.name().to_string()),
            )
            .map_err(|_| ())?;

        Ok(ComposedJoinColumn::new(
            physical.relation().to_string(),
            physical.column().to_string(),
            relation_instance,
        ))
    }

    // A column may have exact identity lineage without a row surviving an
    // intermediate filter. A physical witness requires row-set identity too.
    fn boolean_source_has_passthrough_path(
        &self,
        consumer: &TransformationLayer,
        relation: &str,
    ) -> bool {
        let Some(edge) = self.edge_for_source(consumer.id(), relation) else {
            return false;
        };
        match edge.resolution() {
            RelationResolution::External => true,
            RelationResolution::Resolved => {
                let [producer_id] = edge.producer_layer_ids() else {
                    return false;
                };
                let Some(producer) = self.layer_by_id(producer_id) else {
                    return false;
                };
                let Some(query) = self.query_for_layer(producer) else {
                    return false;
                };
                let [source] = query.sources() else {
                    return false;
                };
                query.row_preserving_projection()
                    && self.boolean_source_has_passthrough_path(producer, source.name())
            }
            RelationResolution::Missing
            | RelationResolution::Ambiguous
            | RelationResolution::Cycle
            | RelationResolution::Partial
            | RelationResolution::Unsupported => false,
        }
    }

    fn resolve_column_identity(
        &self,
        layer: &TransformationLayer,
        query: &QueryStatement,
        column: &ColumnRef,
    ) -> Result<LineageSource, Box<CompositionDiagnostic>> {
        let Some(relation) = column.relation() else {
            return Err(composition_error(
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

        let direct = self.resolve_source_identity(
            layer,
            &LineageSource::new(relation.to_string(), column.name().to_string()),
        );
        if direct.is_ok() {
            return direct;
        }

        // Domains carried from a nested joined relation may retain an inner
        // table alias. Only recover that alias when the join participant is
        // unique and proven to be a physical dependency of this layer.
        let candidates = query
            .joins()
            .iter()
            .flat_map(|join| [join.left(), join.right()])
            .filter(|participant| participant.alias() == Some(relation))
            .map(|participant| participant.relation())
            .collect::<BTreeSet<_>>();
        if let Some(physical_relation) = candidates.iter().next().filter(|_| candidates.len() == 1)
        {
            if query
                .dependencies()
                .iter()
                .any(|dependency| dependency == physical_relation)
            {
                return self.resolve_source_identity(
                    layer,
                    &LineageSource::new(
                        (*physical_relation).to_string(),
                        column.name().to_string(),
                    ),
                );
            }
        }
        direct
    }

    fn resolve_source_identity(
        &self,
        consumer: &TransformationLayer,
        source: &LineageSource,
    ) -> Result<LineageSource, Box<CompositionDiagnostic>> {
        let Some(edge) = self.edge_for_source(consumer.id(), source.relation()) else {
            return Err(composition_error(
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
            RelationResolution::External => Ok(LineageSource::new(
                edge.relation().to_string(),
                source.column().to_string(),
            )),
            RelationResolution::Resolved => {
                let Some(producer_id) = edge.producer_layer_ids().first() else {
                    return Err(composition_error(
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
                let Some(producer) = self.layer_by_id(producer_id) else {
                    return Err(composition_error(
                        consumer.input_id(),
                        consumer.id(),
                        "missing_relation_producer",
                        format!("producer layer '{}' does not exist", producer_id),
                        Some(source.relation().to_string()),
                    ));
                };
                let Some(query) = self.query_for_layer(producer) else {
                    return Err(composition_error(
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
                    return Err(composition_error(
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
                    return Err(composition_error(
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
                    return Err(composition_error(
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

                self.resolve_source_identity(producer, upstream)
            }
            RelationResolution::Missing
            | RelationResolution::Ambiguous
            | RelationResolution::Cycle
            | RelationResolution::Partial
            | RelationResolution::Unsupported => Err(composition_error(
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

    fn compose_output(
        &mut self,
        layer: &TransformationLayer,
        query: &QueryStatement,
        diagnostics: &mut Vec<CompositionDiagnostic>,
    ) -> Output {
        let mut columns = Vec::with_capacity(query.output().columns().len());

        for column in query.output().columns() {
            let mut lineage = BTreeSet::<LineageSource>::new();

            for source in column.lineage() {
                match self.expand_lineage_source(layer, source) {
                    Ok(sources) => lineage.extend(sources),
                    Err(diagnostic) => diagnostics.push(*diagnostic),
                }
            }

            let domain = self.compose_output_domain(layer, column);
            let expression = self.compose_output_expression(layer, column);
            columns.push(OutputColumn::new(
                column.name().to_string(),
                expression,
                domain,
                lineage.into_iter().collect(),
            ));
        }

        Output::new(columns)
    }

    fn compose_output_expression(
        &mut self,
        consumer: &TransformationLayer,
        column: &OutputColumn,
    ) -> Expression {
        if let Some(source) = column.plain_copy_source() {
            if let Some(expression) = self.inherited_composed_case_expression(consumer, source) {
                return expression;
            }
        }

        match column.expression() {
            Expression::Case(case_expression) => {
                Expression::Case(self.compose_case_expression(consumer, case_expression))
            }
            expression => expression.clone(),
        }
    }

    fn inherited_composed_case_expression(
        &mut self,
        consumer: &TransformationLayer,
        source: &LineageSource,
    ) -> Option<Expression> {
        let edge = self
            .edge_for_source(consumer.id(), source.relation())
            .cloned()?;
        if edge.resolution() != RelationResolution::Resolved {
            return None;
        }
        let producer_id = edge.producer_layer_ids().first()?.clone();
        let ComposedSemantics::Resolved(producer) = self.compose_layer(&producer_id) else {
            return None;
        };
        let matches = producer
            .output()
            .columns()
            .iter()
            .filter(|candidate| candidate.name() == source.column())
            .collect::<Vec<_>>();
        let [producer_column] = matches.as_slice() else {
            return None;
        };

        matches!(producer_column.expression(), Expression::Case(_))
            .then(|| producer_column.expression().clone())
    }

    fn compose_case_expression(
        &self,
        consumer: &TransformationLayer,
        case_expression: &CaseExpression,
    ) -> CaseExpression {
        let branches = case_expression
            .branches()
            .iter()
            .map(|branch| {
                CaseBranch::new(
                    branch.condition().clone(),
                    branch.result().clone(),
                    self.compose_case_source_domains(consumer, branch.source_domains()),
                )
            })
            .collect();

        CaseExpression::new(
            case_expression.operand().cloned(),
            branches,
            case_expression.else_result().cloned(),
            self.compose_case_source_domains(consumer, case_expression.else_source_domains()),
        )
    }

    fn compose_case_source_domains(
        &self,
        consumer: &TransformationLayer,
        source_domains: &CaseSourceDomains,
    ) -> CaseSourceDomains {
        let CaseSourceDomains::Reachable { alternatives } = source_domains else {
            return source_domains.clone();
        };

        let mut mapped_alternatives = Vec::new();
        for alternative in alternatives {
            let mut mapped_domains = BTreeMap::<ColumnRef, ValueDomain>::new();
            let mut impossible = false;

            for column_domain in alternative.column_domains() {
                let Some(relation) = column_domain.column().relation() else {
                    return CaseSourceDomains::unknown(format!(
                        "CASE branch source column '{}' has no relation and cannot be mapped through composition",
                        column_domain.column().name()
                    ));
                };
                let source = LineageSource::new(
                    relation.to_string(),
                    column_domain.column().name().to_string(),
                );
                let mapped_source = match self.resolve_source_identity(consumer, &source) {
                    Ok(source) => source,
                    Err(diagnostic) => {
                        return CaseSourceDomains::unknown(format!(
                            "CASE branch source '{}.{}' cannot be mapped through composition: {}",
                            relation,
                            column_domain.column().name(),
                            diagnostic.message()
                        ));
                    }
                };
                let column = ColumnRef::new(
                    Some(mapped_source.relation().to_string()),
                    mapped_source.column().to_string(),
                );
                let domain = column_domain.domain().clone();

                mapped_domains
                    .entry(column)
                    .and_modify(|existing| {
                        *existing = intersect_case_domain_values(existing, &domain);
                    })
                    .or_insert(domain);

                if mapped_domains
                    .values()
                    .any(|domain| matches!(domain, ValueDomain::Empty))
                {
                    impossible = true;
                    break;
                }
            }

            if impossible {
                continue;
            }

            mapped_domains.retain(|_, domain| !matches!(domain, ValueDomain::Unbounded));
            let mapped = CaseSourceDomainAlternative::new(
                mapped_domains
                    .into_iter()
                    .map(|(column, domain)| ColumnDomain::new(column, domain))
                    .collect(),
            );
            if !mapped_alternatives.contains(&mapped) {
                mapped_alternatives.push(mapped);
            }
        }

        CaseSourceDomains::reachable(mapped_alternatives)
    }

    fn compose_output_domain(
        &mut self,
        consumer: &TransformationLayer,
        column: &OutputColumn,
    ) -> ValueDomain {
        let domain = column.domain().clone();
        let Some(source) = column.plain_copy_source() else {
            return domain;
        };
        let Some(edge) = self
            .edge_for_source(consumer.id(), source.relation())
            .cloned()
        else {
            return domain;
        };
        if edge.resolution() != RelationResolution::Resolved {
            return domain;
        }
        let Some(producer_id) = edge.producer_layer_ids().first() else {
            return domain;
        };

        let ComposedSemantics::Resolved(producer) = self.compose_layer(producer_id) else {
            return domain;
        };
        let matches = producer
            .output()
            .columns()
            .iter()
            .filter(|producer_column| producer_column.name() == source.column())
            .collect::<Vec<_>>();
        let [producer_column] = matches.as_slice() else {
            return domain;
        };

        intersect_domains(&domain, producer_column.domain())
    }

    fn expand_lineage_source(
        &mut self,
        consumer: &TransformationLayer,
        source: &LineageSource,
    ) -> Result<Vec<LineageSource>, Box<CompositionDiagnostic>> {
        let Some(edge) = self
            .edge_for_source(consumer.id(), source.relation())
            .cloned()
        else {
            return Err(composition_error(
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
            RelationResolution::External => Ok(vec![LineageSource::new(
                edge.relation().to_string(),
                source.column().to_string(),
            )]),
            RelationResolution::Resolved => {
                let Some(producer_id) = edge.producer_layer_ids().first() else {
                    return Err(composition_error(
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

                match self.compose_layer(producer_id) {
                    ComposedSemantics::Resolved(producer) => {
                        let matches = producer
                            .output()
                            .columns()
                            .iter()
                            .filter(|column| column.name() == source.column())
                            .collect::<Vec<_>>();
                        let [column] = matches.as_slice() else {
                            return Err(composition_error(
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
                    ComposedSemantics::Unresolved(_) => Err(composition_error(
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
            | RelationResolution::Partial
            | RelationResolution::Unsupported => Err(composition_error(
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

    fn edge_for_source(&self, consumer_layer_id: &str, relation: &str) -> Option<&GraphEdge> {
        let consumer = self.layer_by_id(consumer_layer_id)?;
        let canonical_relation = consumer.canonical_relation(relation);
        self.graph.edges().iter().find(|edge| {
            edge.consumer_layer_id() == consumer_layer_id
                && edge.relation() == canonical_relation.as_str()
        })
    }
}

fn collect_conjunctive_column_equalities<'a>(
    predicate: &'a Predicate,
    equalities: &mut Vec<(&'a ColumnExpression, &'a ColumnExpression)>,
) {
    match predicate {
        Predicate::Comparison(comparison) if comparison.operator() == ComparisonOperator::Eq => {
            if let (Expression::Column(left), Expression::Column(right)) =
                (comparison.left(), comparison.right())
            {
                equalities.push((left, right));
            }
        }
        Predicate::And(logical) => {
            for operand in logical.operands() {
                collect_conjunctive_column_equalities(operand, equalities);
            }
        }
        _ => {}
    }
}

fn columns_reference_distinct_query_sources(
    left: &ColumnExpression,
    right: &ColumnExpression,
    sources: &[SourceRelation],
) -> bool {
    let left = resolve_query_source(left, sources);
    let right = resolve_query_source(right, sources);
    matches!((left, right), (Some((left_index, _)), Some((right_index, _))) if left_index != right_index)
}

fn resolve_query_column_instance(
    column: &ColumnExpression,
    sources: &[SourceRelation],
    join: Option<&Join>,
) -> Result<(String, String), ()> {
    if let Some((_, source)) = resolve_query_source(column, sources) {
        return Ok((
            source.name().to_string(),
            source.alias().unwrap_or(source.name()).to_string(),
        ));
    }

    let qualifier = column.relation().ok_or(())?;
    let join = join.ok_or(())?;
    let matches = [join.left(), join.right()]
        .into_iter()
        .filter(|participant| relation_ref_matches(participant, qualifier))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [participant] => Ok((
            participant.relation().to_string(),
            participant
                .alias()
                .unwrap_or(participant.relation())
                .to_string(),
        )),
        [] => Ok((qualifier.to_string(), qualifier.to_string())),
        _ => Err(()),
    }
}

fn resolve_query_source<'a>(
    column: &ColumnExpression,
    sources: &'a [SourceRelation],
) -> Option<(usize, &'a SourceRelation)> {
    match column.relation() {
        Some(qualifier) => {
            let matches = sources
                .iter()
                .enumerate()
                .filter(|(_, source)| source_relation_matches(source, qualifier))
                .collect::<Vec<_>>();
            let [source] = matches.as_slice() else {
                return None;
            };
            Some(*source)
        }
        None => match sources {
            [source] => Some((0, source)),
            _ => None,
        },
    }
}

fn source_relation_matches(source: &SourceRelation, qualifier: &str) -> bool {
    source.alias() == Some(qualifier)
        || source.name() == qualifier
        || (source.alias().is_none() && source.name().rsplit('.').next() == Some(qualifier))
}

fn relation_ref_matches(relation: &RelationRef, qualifier: &str) -> bool {
    relation.alias() == Some(qualifier)
        || relation.relation() == qualifier
        || (relation.alias().is_none() && relation.relation().rsplit('.').next() == Some(qualifier))
}

fn composition_error(
    input_id: impl Into<String>,
    layer_id: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
    relation: Option<String>,
) -> Box<CompositionDiagnostic> {
    Box::new(CompositionDiagnostic::layer_warning(
        input_id, layer_id, code, message, relation,
    ))
}

fn graph_failure_reason(edges: &[GraphEdge]) -> Option<CompositionFailureReason> {
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
        .any(|edge| edge.resolution() == RelationResolution::Partial)
    {
        return Some(CompositionFailureReason::PartialProducer);
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
    edges: &[GraphEdge],
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
                    format!(
                        "relation '{}' requires a producer that is unavailable",
                        edge.relation()
                    ),
                ),
                RelationResolution::Partial => (
                    "partial_relation_producer",
                    format!(
                        "relation '{}' is only partially defined by its in-bundle writer",
                        edge.relation()
                    ),
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

fn merge_column_domains(target: &mut BTreeMap<ColumnRef, ValueDomain>, domains: &[ColumnDomain]) {
    for domain in domains {
        merge_domain(target, domain.column().clone(), domain.domain().clone());
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
