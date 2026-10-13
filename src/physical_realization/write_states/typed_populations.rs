//! Schema-backed complete physical populations with Arrow-like nested values.
//!
//! Reuse the canonical scalar constraint verifier for independently checked
//! scalar columns. Nested values cannot be silently cast to SQL literals, and
//! constraints requiring nested comparison remain residual.

use super::typed_values::{physical_typed_value_plan, PhysicalTypeProof, PhysicalTypedValue};
use super::*;

/// A complete physical row retaining binary, array, map and struct values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTypedWriteRow {
    values: BTreeMap<String, PhysicalTypedValue>,
}

impl PhysicalTypedWriteRow {
    /// Construct a physical row with distinct, nonempty column identities.
    pub fn new(values: BTreeMap<String, PhysicalTypedValue>) -> Option<Self> {
        if values.keys().any(|name| name.trim().is_empty()) {
            None
        } else {
            Some(Self { values })
        }
    }

    /// Exact physical typed values indexed by canonical column.
    pub fn values(&self) -> &BTreeMap<String, PhysicalTypedValue> {
        &self.values
    }
}

/// A complete physical bag for a single schema-backed relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalTypedWriteState {
    relation: String,
    rows: Vec<PhysicalTypedWriteRow>,
}

impl PhysicalTypedWriteState {
    /// Construct a population retaining row order and duplicate multiplicity.
    pub fn new(relation: impl Into<String>, rows: Vec<PhysicalTypedWriteRow>) -> Option<Self> {
        let relation = relation.into();
        if relation.trim().is_empty() {
            None
        } else {
            Some(Self { relation, rows })
        }
    }

    /// Canonical relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Complete typed physical bag, including duplicates.
    pub fn rows(&self) -> &[PhysicalTypedWriteRow] {
        &self.rows
    }
}

/// Proof classification for a complete multi-relation typed population.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalTypedPopulationPlan {
    /// All supplied physical populations satisfy their provable schema laws.
    Feasible {
        final_states: Vec<PhysicalTypedWriteState>,
    },
    /// A supplied physical row contradicts an attested necessary law.
    Impossible {
        relation: Option<String>,
        reason: String,
    },
    /// An unproved cast, comparison or schema obligation prevents certification.
    Residual {
        relation: Option<String>,
        reason: String,
    },
}

fn classify(
    relation: Option<String>,
    proof: PhysicalTypeProof,
) -> Option<PhysicalTypedPopulationPlan> {
    match proof {
        PhysicalTypeProof::Feasible => None,
        PhysicalTypeProof::Impossible { reason } => {
            Some(PhysicalTypedPopulationPlan::Impossible { relation, reason })
        }
        PhysicalTypeProof::Residual { reason } => {
            Some(PhysicalTypedPopulationPlan::Residual { relation, reason })
        }
    }
}

/// Verify complete, named Arrow-like populations against canonical schemas and
/// cross-relation constraints without converting nested values to SQL literals.
///
/// Scalar columns are validated with the same laws as the ordered DML engine;
/// nested values are validated structurally, and only *unconstrained* nested
/// columns are replaced by private NULL sentinels for the scalar constraint
/// verifier. Those sentinels are never exposed as constructed output. Key, FK,
/// CHECK or accepted-values rules on nested columns always remain residual
/// rather than inheriting fabricated equality or ordering semantics.
pub fn physical_typed_closed_world_schema_plan(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalTypedWriteState],
) -> PhysicalTypedPopulationPlan {
    let mut typed = BTreeMap::<String, PhysicalTypedWriteState>::new();
    let mut scalar_snapshots = Vec::new();
    let mut non_null_nested = BTreeSet::new();
    for state in snapshots {
        let relation = Some(state.relation.clone());
        if typed
            .insert(state.relation.clone(), state.clone())
            .is_some()
        {
            return PhysicalTypedPopulationPlan::Impossible {
                relation,
                reason: "duplicate complete typed physical source identity".to_string(),
            };
        }
        let Some(schema) = bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation() == state.relation)
        else {
            return PhysicalTypedPopulationPlan::Residual {
                relation,
                reason: "missing canonical typed source schema".to_string(),
            };
        };
        let mut projected = Vec::new();
        for row in &state.rows {
            if row.values.len() != schema.columns().len() {
                return PhysicalTypedPopulationPlan::Impossible {
                    relation,
                    reason: "typed physical row arity contradicts declared schema".to_string(),
                };
            }
            let mut scalar_values = BTreeMap::new();
            for column in schema.columns() {
                let Some(value) = row.values.get(column.name()) else {
                    return PhysicalTypedPopulationPlan::Impossible {
                        relation,
                        reason: format!("typed physical row omits column {}", column.name()),
                    };
                };
                let proof = match value {
                    PhysicalTypedValue::Scalar(literal) => {
                        match super::type_validation::validate_column_value(literal, column) {
                            Ok(()) => PhysicalTypeProof::Feasible,
                            Err(StateFailure::Impossible(reason)) => {
                                PhysicalTypeProof::Impossible { reason }
                            }
                            Err(StateFailure::Residual(_, reason)) => {
                                PhysicalTypeProof::Residual { reason }
                            }
                        }
                    }
                    _ => physical_typed_value_plan(value, column.data_type()),
                };
                if let Some(outcome) = classify(Some(state.relation.clone()), proof) {
                    return outcome;
                }
                // A concrete nested value is not SQL NULL. Preserve NOT NULL
                // independently from the scalar verifier, which cannot encode
                // nested physical values and otherwise sees a private NULL.
                let not_null_rules = bundle
                    .relation_constraints()
                    .iter()
                    .filter(|set| set.relation() == state.relation)
                    .flat_map(|set| set.constraints())
                    .filter(|constraint| {
                        matches!(constraint,
                        RelationConstraint::NotNull(rule) if rule.column() == column.name())
                    })
                    .collect::<Vec<_>>();
                if matches!(value, PhysicalTypedValue::Scalar(LiteralValue::Null))
                    && not_null_rules.iter().any(|constraint| {
                        constraint
                            .evidence()
                            .iter()
                            .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
                    })
                {
                    return PhysicalTypedPopulationPlan::Impossible {
                        relation,
                        reason: format!("enforced NOT NULL violated for {}", column.name()),
                    };
                }
                if !matches!(value, PhysicalTypedValue::Scalar(_)) && !not_null_rules.is_empty() {
                    non_null_nested.insert((state.relation.clone(), column.name().to_string()));
                }
                let scalar = match value {
                    PhysicalTypedValue::Scalar(literal) => literal.clone(),
                    _ => {
                        // A nested constrained key cannot masquerade as NULL in the
                        // existing scalar equality/ordering and FK verifier.
                        let constrained = bundle.relation_constraints().iter().any(|set| {
                            (set.relation() == state.relation && set.computed_checks().iter().any(|check| {
                                check.left().referenced_columns().contains(&column.name())
                                    || check.right().referenced_columns().contains(&column.name())
                            })) || (set.relation() == state.relation && set.computed_boolean_checks().iter().any(|check| {
                                check.predicate().referenced_columns().contains(&column.name())
                            })) || (set.relation() == state.relation && set.computed_keys().iter().any(|key| {
                                key.expressions().iter().any(|expr| expr.referenced_columns().contains(&column.name()))
                            })) || set.constraints().iter().any(|constraint| {
                                (set.relation() == state.relation
                                    && !matches!(constraint, RelationConstraint::NotNull(_))
                                    && constraint.columns().iter().any(|name| name == column.name()))
                                    || matches!(constraint, RelationConstraint::ForeignKey(fk)
                                        if fk.referenced_relation() == state.relation
                                        && fk.referenced_columns().iter().any(|name| name == column.name()))
                            })
                        });
                        if constrained {
                            return PhysicalTypedPopulationPlan::Residual {
                                relation,
                                reason: format!(
                                    "nested column {} has no canonical key or CHECK comparison law",
                                    column.name()
                                ),
                            };
                        }
                        LiteralValue::Null
                    }
                };
                scalar_values.insert(column.name().to_string(), scalar);
            }
            projected.push(PhysicalWriteRow {
                values: scalar_values,
            });
        }
        scalar_snapshots.push(PhysicalWriteState {
            relation: state.relation.clone(),
            rows: projected,
        });
    }

    match physical_closed_world_schema_plan_with_nested(bundle, &scalar_snapshots, &non_null_nested)
    {
        PhysicalWriteStatePlan::Feasible { .. } => PhysicalTypedPopulationPlan::Feasible {
            final_states: typed.into_values().collect(),
        },
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            PhysicalTypedPopulationPlan::Impossible {
                relation: None,
                reason,
            }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => PhysicalTypedPopulationPlan::Residual {
            relation: None,
            reason,
        },
    }
}

/// Validate caller-materialized nested target insertions against an already
/// complete physical source/target population.
///
/// This is a schema-and-constraint proof for exact typed rows, not an SQL
/// INSERT expression interpreter. Both the prior and combined populations
/// must be feasible: new rows cannot evade existing UNIQUE/PK, NOT NULL,
/// CHECK or FK obligations, nor fabricate referenced relations. The returned
/// final states retain the actual nested Arrow-like values and multiplicity.
pub fn physical_typed_append_schema_plan(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalTypedWriteState],
    target: &str,
    inserted: &[PhysicalTypedWriteRow],
) -> PhysicalTypedPopulationPlan {
    if target.trim().is_empty() {
        return PhysicalTypedPopulationPlan::Residual {
            relation: None,
            reason: "target relation identity is empty".to_string(),
        };
    }

    let mut final_states = match physical_typed_closed_world_schema_plan(bundle, snapshots) {
        PhysicalTypedPopulationPlan::Feasible { final_states } => final_states,
        other => return other,
    };
    let Some(state) = final_states
        .iter_mut()
        .find(|state| state.relation == target)
    else {
        return PhysicalTypedPopulationPlan::Residual {
            relation: Some(target.to_string()),
            reason: "missing complete prior typed target population".to_string(),
        };
    };
    state.rows.extend_from_slice(inserted);
    physical_typed_closed_world_schema_plan(bundle, &final_states)
}

/// Convert only attested scalar values, never nested Arrow values, to a
/// temporary row for the existing canonical scalar predicate interpreter.
fn scalar_shadow(row: &PhysicalTypedWriteRow) -> PhysicalWriteRow {
    PhysicalWriteRow {
        values: row
            .values
            .iter()
            .filter_map(|(name, value)| match value {
                PhysicalTypedValue::Scalar(literal) => Some((name.clone(), literal.clone())),
                _ => None,
            })
            .collect(),
    }
}

/// Fill omitted typed INSERT columns only from canonical authoritative scalar
/// defaults. Neither a nested placeholder nor an inferred nullable SQL NULL is
/// a physical value-production proof.
fn complete_typed_insert_row(
    bundle: &AnalysisBundle,
    target: &str,
    schema: &RelationSchema,
    mut row: PhysicalTypedWriteRow,
) -> Result<PhysicalTypedWriteRow, String> {
    let sources = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == target)
        .map(|set| set.column_value_sources())
        .unwrap_or(&[]);
    for column in schema.columns() {
        let source = sources.iter().find(|source| source.column() == column.name());
        if row.values.contains_key(column.name()) {
            // Database-managed columns cannot be assigned explicitly even
            // when their supplied bytes happen to equal the produced value.
            if source.is_some_and(|source| source.kind() != crate::column_sources::ColumnValueKind::Default) {
                return Err("explicit assignment to a generated or identity column is unproved".into());
            }
            continue;
        }
        let Some(source) = source else {
            return Err(format!("omitted typed column {} has no attested default", column.name()));
        };
        if source.kind() != crate::column_sources::ColumnValueKind::Default
            || !super::generated::authoritative(source)
        {
            return Err("omitted generated, identity or unattested typed default is unproved".into());
        }
        let Some(value) = source.value() else {
            return Err("opaque typed default cannot be evaluated".into());
        };
        row.values.insert(
            column.name().to_string(),
            PhysicalTypedValue::Scalar(super::generated::scalar_literal(value)),
        );
    }
    Ok(row)
}

/// Apply scalar-only UPDATE or DELETE against a complete typed target population.
///
/// The SQL evaluator cannot interpret Arrow-valued expressions. Nested columns
/// remain byte-for-byte unchanged and are never exposed as fabricated NULLs.
/// Each poststate is independently validated after the caller's SQL step.
fn mutate_typed_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    before: &PhysicalTypedWriteState,
    delete: bool,
) -> Result<PhysicalTypedWriteState, String> {
    let write = query
        .write()
        .ok_or("missing canonical mutation operation")?;
    // An unqualified, unconditional mutation can be proved by the scalar
    // interpreter, but that path must not be confused with an unparsed WHERE.
    // Only the explicitly normalized predicate subset is certified here.
    let predicate = write
        .selection()
        .ok_or("typed mutation needs a normalized WHERE predicate")?;
    if !query.sources().is_empty() {
        return Err("typed mutation unexpectedly has independent source relations".into());
    }
    let mut retained = Vec::new();
    for row in &before.rows {
        let scalar = scalar_shadow(row);
        let context = super::evaluator::Context {
            target: Some((before.relation(), &scalar)),
            target_alias: write.target_alias(),
            source: None,
        };
        let selected = super::evaluator::selected(Some(predicate), &context, &[])
            .map_err(|_| "typed mutation WHERE lacks exact scalar semantics")?;
        if !selected {
            retained.push(row.clone());
            continue;
        }
        if delete {
            continue;
        }
        let updated = super::operators::apply_target_update(
            bundle,
            before.relation(),
            &scalar,
            write.assignments(),
            &context,
            &[],
        )
        .map_err(|_| "typed UPDATE assignment or generated expression is unproved")?;
        let mut next = row.clone();
        for (name, literal) in updated.values {
            next.values
                .insert(name, PhysicalTypedValue::Scalar(literal));
        }
        retained.push(next);
    }
    Ok(PhysicalTypedWriteState {
        relation: before.relation.clone(),
        rows: retained,
    })
}

/// Execute provable nested INSERT SELECT, scalar UPDATE and scalar DELETE
/// against complete typed snapshots, in canonical SQL statement order.
///
/// Each projected value must be a direct source-column reference or an exact
/// literal. Nested Arrow values are copied without converting them to SQL
/// scalar literals. Portable WHERE predicates over complete scalar columns
/// obey SQL three-valued logic; nested predicates, joins, grouping, functions,
/// generated/identity omissions, opaque defaults, transaction effects and
/// opaque writes remain residual. Exact attested scalar defaults are executed.
/// Every poststate is checked against all attested target/source constraints, so
/// duplicate rows and FK cycles are never certified by isolated row guesses.
pub fn physical_typed_ordered_write_plan(
    bundle: &AnalysisBundle,
    initial_states: &[PhysicalTypedWriteState],
) -> PhysicalTypedPopulationPlan {
    let final_states = match physical_typed_closed_world_schema_plan(bundle, initial_states) {
        PhysicalTypedPopulationPlan::Feasible { final_states } => final_states,
        other => return other,
    };
    let mut states = final_states
        .into_iter()
        .map(|state| (state.relation.clone(), state))
        .collect::<BTreeMap<_, _>>();
    if bundle.inputs().iter().any(|input| {
        input
            .statements()
            .iter()
            .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_)))
    }) {
        return PhysicalTypedPopulationPlan::Residual {
            relation: None,
            reason: "unsupported SQL statement or transaction in typed write program".into(),
        };
    }

    for layer in bundle.layers() {
        let Some(kind) = layer.write_kind() else {
            continue;
        };
        let query = bundle
            .inputs()
            .iter()
            .find(|input| input.id() == layer.input_id())
            .and_then(|input| input.statements().get(layer.statement_index()))
            .and_then(|statement| match statement {
                ProtocolStatement::Query(query) => Some(query),
                ProtocolStatement::Unsupported(_) => None,
            });
        let Some(query) = query else {
            return PhysicalTypedPopulationPlan::Residual {
                relation: None,
                reason: "typed write layer has no canonical query".into(),
            };
        };
        let Some(write) = query.write() else {
            return PhysicalTypedPopulationPlan::Residual {
                relation: None,
                reason: "typed INSERT has no canonical target operation".into(),
            };
        };
        let target = write.target();
        let gap = |reason: &str| PhysicalTypedPopulationPlan::Residual {
            relation: Some(target.to_string()),
            reason: reason.to_string(),
        };
        if matches!(kind, WriteKind::Update | WriteKind::Delete) {
            let Some(before) = states.get(target).cloned() else {
                return gap("typed mutation requires a complete target population");
            };
            let after = match mutate_typed_rows(bundle, query, &before, kind == WriteKind::Delete) {
                Ok(after) => after,
                Err(reason) => return gap(&reason),
            };
            states.insert(target.to_string(), after);
            let snapshots = states.values().cloned().collect::<Vec<_>>();
            match physical_typed_closed_world_schema_plan(bundle, &snapshots) {
                PhysicalTypedPopulationPlan::Feasible { .. } => {}
                other => return other,
            }
            continue;
        }
        if kind != WriteKind::Append {
            return gap("typed write operation needs additional SQL execution evidence");
        }
        if !(query.row_preserving_projection() || query.filter_only_row_shape())
            || query.set_operation().is_some()
            || query.aggregation().is_some()
            || !query.joins().is_empty()
            || query.predicates().having_predicate().is_some()
            || query.predicates().qualify_predicate().is_some()
        {
            return gap("typed INSERT source is not a provable one-source projection/filter");
        }
        if query.filter_only_row_shape() && query.predicates().where_predicate().is_none() {
            return PhysicalTypedPopulationPlan::Residual {
                relation: None,
                reason: "typed INSERT WHERE was not normalized into a canonical predicate".into(),
            };
        }
        let [source] = query.sources() else {
            return gap("typed INSERT requires exactly one physical source");
        };
        let Some(source_rows) = states.get(source.name()) else {
            return gap("complete typed INSERT source population is missing");
        };
        let Some(schema) = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == target)
        else {
            return gap("typed INSERT target schema is missing");
        };
        let columns = if write.target_columns().is_empty() {
            schema
                .columns()
                .iter()
                .map(|column| column.name().to_string())
                .collect::<Vec<_>>()
        } else {
            write.target_columns().to_vec()
        };
        if columns.iter().collect::<BTreeSet<_>>().len() != columns.len()
            || columns
                .iter()
                .any(|name| !schema.columns().iter().any(|column| column.name() == name))
            || columns.len() != query.output().columns().len()
        {
            return gap("typed INSERT target columns must be declared, unique and match projection arity");
        }
        let aliases = [source.alias().unwrap_or(source.name()), source.name()];
        let mut inserted = Vec::new();
        for row in &source_rows.rows {
            // WHERE can inspect scalar source columns only. Do not expose
            // fabricated scalar NULLs for nested Arrow values to the SQL
            // predicate evaluator, which would turn IS NULL into a false proof.
            let scalar_row = scalar_shadow(row);
            let context = super::evaluator::Context {
                target: None,
                target_alias: None,
                source: Some((source.name(), &scalar_row)),
            };
            match super::evaluator::selected(
                query.predicates().where_predicate(),
                &context,
                &aliases,
            ) {
                Ok(false) => continue,
                Ok(true) => {}
                Err(_) => return gap("typed INSERT filter has no exact scalar predicate proof"),
            }
            let mut projected = BTreeMap::new();
            for (target_column, output) in columns.iter().zip(query.output().columns()) {
                let value = match output.expression() {
                    Expression::Column(column)
                        if column.relation().is_none()
                            || column.relation() == Some(source.name())
                            || column.relation() == source.alias() =>
                    {
                        row.values.get(column.name()).cloned()
                    }
                    Expression::Literal(literal) => {
                        Some(PhysicalTypedValue::Scalar(literal.value().clone()))
                    }
                    _ => None,
                };
                let Some(value) = value else {
                    return gap(
                        "typed INSERT projection needs an exact direct source-column or literal",
                    );
                };
                projected.insert(target_column.clone(), value);
            }
            let completed = match complete_typed_insert_row(
                bundle,
                target,
                schema,
                PhysicalTypedWriteRow { values: projected },
            ) {
                Ok(row) => row,
                Err(reason) => return gap(&reason),
            };
            inserted.push(completed);
        }
        let Some(state) = states.get_mut(target) else {
            return gap("complete initial typed INSERT target population is missing");
        };
        state.rows.extend(inserted);
        let snapshots = states.values().cloned().collect::<Vec<_>>();
        let checked = physical_typed_closed_world_schema_plan(bundle, &snapshots);
        match checked {
            PhysicalTypedPopulationPlan::Feasible { .. } => {}
            other => return other,
        }
    }
    PhysicalTypedPopulationPlan::Feasible {
        final_states: states.into_values().collect(),
    }
}

/// Execute only lossless, row-preserving typed INSERT SELECT statements.
///
/// This narrower compatibility API intentionally rejects UPDATE/DELETE. Use
/// `physical_typed_ordered_write_plan` for attested scalar mutations preserving
/// nested columns.
pub fn physical_typed_ordered_insert_plan(
    bundle: &AnalysisBundle,
    initial_states: &[PhysicalTypedWriteState],
) -> PhysicalTypedPopulationPlan {
    if bundle.layers().iter().any(|layer| {
        layer
            .write_kind()
            .is_some_and(|kind| kind != WriteKind::Append)
    }) {
        return PhysicalTypedPopulationPlan::Residual {
            relation: None,
            reason: "typed INSERT-only plan cannot execute other DML statements".into(),
        };
    }
    physical_typed_ordered_write_plan(bundle, initial_states)
}
