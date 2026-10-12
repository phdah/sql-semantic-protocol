//! Full analysis-bundle metadata, accessors, and composition.

use super::*;

pub struct AnalysisBundle {
    protocol_version: &'static str,
    inputs: Vec<AnalyzedInput>,
    layers: Vec<TransformationLayer>,
    graph: AnalysisGraph,
    source_schemas: Vec<RelationSchema>,
    relation_constraints: Vec<RelationConstraintSet>,
    constraint_diagnostics: Vec<ConstraintDiagnostic>,
    comparison_declarations: Vec<ComparisonAssumption>,
    outcome_goals: Vec<crate::outcome_goals::EvaluatedOutcomeGoal>,
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

    /// Return caller-supplied typed schemas for source relations.
    pub fn source_schemas(&self) -> &[RelationSchema] {
        &self.source_schemas
    }

    /// Return canonical relation constraints from SQL and metadata adapters.
    pub fn relation_constraints(&self) -> &[RelationConstraintSet] {
        &self.relation_constraints
    }

    /// Resolve partial writes against canonical target identities and constraint evidence.
    ///
    /// Missing constraint evidence is reported as None, not as an empty set of
    /// guaranteed collision-free keys. Source relations exclude the initial target.
    pub fn write_state_effects(&self) -> Vec<LayerWriteStateEffect> {
        self.layers
            .iter()
            .filter_map(|layer| {
                let input = self
                    .inputs
                    .iter()
                    .find(|input| input.id() == layer.input_id())?;
                let ProtocolStatement::Query(query) =
                    input.statements().get(layer.statement_index())?
                else {
                    return None;
                };
                let effect = query.write()?.state_effect()?;
                let target = layer
                    .produces()
                    .iter()
                    .find_map(|dataset| dataset.relation_name())?;
                let constraints = self
                    .relation_constraints
                    .iter()
                    .find(|set| set.relation() == target)
                    .cloned();
                Some(LayerWriteStateEffect {
                    layer_id: layer.id().to_string(),
                    target: target.to_string(),
                    sources: layer.consumes().to_vec(),
                    effect,
                    target_constraints: constraints,
                })
            })
            .collect()
    }

    /// Return constraint diagnostics that cannot be scoped to one canonical relation.
    pub fn constraint_diagnostics(&self) -> &[ConstraintDiagnostic] {
        &self.constraint_diagnostics
    }

    /// Return the explicitly declared comparison settings, in deterministic order.
    pub fn comparison_declarations(&self) -> &[ComparisonAssumption] {
        &self.comparison_declarations
    }

    /// Return optional caller goals and their conservative feasibility assessments.
    pub fn outcome_goals(&self) -> &[crate::outcome_goals::EvaluatedOutcomeGoal] {
        &self.outcome_goals
    }

    /// Replace requested outcome goals. Requests are evaluated only against proved semantics;
    /// this does not modify source predicates or any SQL-derived value domain.
    pub fn set_outcome_goals(
        &mut self,
        goals: &[crate::outcome_goals::OutcomeGoal],
    ) -> Result<(), crate::outcome_goals::OutcomeGoalError> {
        let evaluated = crate::outcome_goals::evaluate(self, goals)?;
        self.outcome_goals = evaluated;
        Ok(())
    }

    /// Attest warehouse comparison settings and apply them to local and composed scopes.
    /// No assumption is silently supplied by default.
    pub fn declare_comparison_assumptions(&mut self, declared: &[ComparisonAssumption]) {
        self.comparison_declarations
            .extend(declared.iter().copied());
        self.comparison_declarations.sort();
        self.comparison_declarations.dedup();
        for input in &mut self.inputs {
            for statement in &mut input.statements {
                if let ProtocolStatement::Query(query) = statement {
                    query.declare_comparison_assumptions(&self.comparison_declarations);
                }
            }
        }
        for layer in &mut self.layers {
            if let ComposedSemantics::Resolved(ref mut semantics) = layer.composed_semantics {
                semantics.condition_exactness = semantics
                    .condition_exactness
                    .clone()
                    .with_declarations(&self.comparison_declarations);
                for witness in &mut semantics.boolean_witnesses {
                    witness
                        .witness
                        .declare_comparison_assumptions(&self.comparison_declarations);
                }
            }
        }
        self.refresh_outcome_goals();
    }

    /// Merge adapter-neutral canonical constraint evidence into this bundle.
    ///
    /// Identical facts coalesce evidence. Conflicting primary keys remain explicit diagnostics.
    pub fn enrich_relation_constraints(&mut self, constraints: &[RelationConstraintSet]) {
        let validated = constraints
            .iter()
            .map(|set| set.validated_against_schemas(&self.source_schemas))
            .collect::<Vec<_>>();
        merge_relation_constraint_sets(&mut self.relation_constraints, &validated);
        self.recheck_boolean_witnesses();
    }

    /// Merge adapter diagnostics that cannot be scoped to one canonical relation.
    pub fn enrich_constraint_diagnostics(&mut self, diagnostics: &[ConstraintDiagnostic]) {
        self.constraint_diagnostics
            .extend(diagnostics.iter().cloned());
        self.constraint_diagnostics.sort();
        self.constraint_diagnostics.dedup();
        self.refresh_outcome_goals();
    }

    pub(crate) fn replace_source_schemas(&mut self, mut schemas: Vec<RelationSchema>) {
        schemas.sort_by(|left, right| left.relation().cmp(right.relation()));
        self.source_schemas = schemas;
        self.validate_existing_constraints();
        self.recheck_boolean_witnesses();
    }

    fn recheck_boolean_witnesses(&mut self) {
        for input in &mut self.inputs {
            for statement in &mut input.statements {
                if let ProtocolStatement::Query(query) = statement {
                    query.restrict_boolean_witness(&self.relation_constraints);
                }
            }
        }
        for layer in &mut self.layers {
            if let ComposedSemantics::Resolved(semantics) = &mut layer.composed_semantics {
                for item in semantics.boolean_witnesses.iter_mut() {
                    item.witness
                        .restrict_with_schema_constraints(&self.relation_constraints);
                }
            }
        }
        self.refresh_outcome_goals();
    }

    // Goals are evaluated from mutable evidence, not immutable SQL syntax alone.
    // Keep the caller's requests while replacing any now-invalid witness or bound.
    fn refresh_outcome_goals(&mut self) {
        if self.outcome_goals.is_empty() {
            return;
        }
        let requested = self
            .outcome_goals
            .iter()
            .map(|item| item.goal().clone())
            .collect::<Vec<_>>();
        self.outcome_goals = crate::outcome_goals::evaluate(self, &requested)
            .expect("evidence updates preserve validated output layer and column identities");
    }

    fn validate_existing_constraints(&mut self) {
        self.relation_constraints = self
            .relation_constraints
            .iter()
            .map(|set| set.validated_against_schemas(&self.source_schemas))
            .collect();
    }

    pub(crate) fn from_protocol(protocol: &Protocol) -> Self {
        let inputs = vec![AnalyzedInput {
            id: "input-0001".to_string(),
            source: SqlInputSource::Inline,
            dialect: protocol.source().dialect().to_string(),
            relation_context: None,
            statements: protocol.statements().to_vec(),
            relation_constraints: protocol.relation_constraints().to_vec(),
        }];

        let mut bundle = Self::from_inputs(inputs);
        bundle.declare_comparison_assumptions(protocol.comparison_declarations());
        bundle
    }

    fn from_inputs(inputs: Vec<AnalyzedInput>) -> Self {
        let catalog = RelationCatalog::default();
        Self::from_inputs_with_resolver(inputs, &catalog)
            .expect("empty resolver metadata preserves textual relation identities")
    }

    fn from_inputs_with_resolver(
        inputs: Vec<AnalyzedInput>,
        resolver: &dyn RelationResolver,
    ) -> Result<Self, LayerBuildError> {
        let mut relation_constraints = Vec::new();
        for input in &inputs {
            for constraints in input.relation_constraints() {
                let relation = resolve_relation(resolver, input, constraints.relation())?;
                let resolved = constraints.map_relations(relation, |reference| {
                    resolve_relation(resolver, input, reference)
                })?;
                merge_relation_constraint_sets(&mut relation_constraints, &[resolved]);
            }
        }

        let mut layers = build_layers(&inputs, resolver)?;
        let graph = build_graph(&layers);
        let composed = crate::composition::compose_layers(&inputs, &layers, &graph);
        for (layer, semantics) in layers.iter_mut().zip(composed) {
            layer.composed_semantics = semantics;
        }

        let mut bundle = Self {
            protocol_version: PROTOCOL_VERSION,
            inputs,
            layers,
            graph,
            source_schemas: Vec::new(),
            relation_constraints,
            constraint_diagnostics: Vec::new(),
            comparison_declarations: Vec::new(),
            outcome_goals: Vec::new(),
        };
        bundle.recheck_boolean_witnesses();
        Ok(bundle)
    }
}

#[derive(Debug)]
