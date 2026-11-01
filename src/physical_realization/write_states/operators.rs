//! Ordered DML/DDL transitions that preserve target and source bag identities.

use super::evaluator::{eval, selected, Context};
use super::projection::{apply_assignments, evaluate_simple_select, schema_for};
use super::*;

fn initial_target<'a>(
    target: &str,
    before: Option<&'a PhysicalWriteState>,
) -> StateResult<&'a PhysicalWriteState> {
    before.ok_or_else(|| {
        residual_gap(
            PhysicalWriteStateGap::MissingPopulation,
            format!("missing complete initial target state: {target}"),
        )
    })
}

pub(super) fn define_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let write = query
        .write()
        .ok_or_else(|| residual("missing definition operation"))?;
    match write.definition_mode() {
        Some(WriteDefinitionMode::CreateTable) if before.is_some() => {
            return Err(impossible(
                "CREATE TABLE conflicts with a prior physical target",
            ));
        }
        Some(WriteDefinitionMode::CreateTable | WriteDefinitionMode::ReplaceTable) => {}
        Some(WriteDefinitionMode::CreateView | WriteDefinitionMode::ReplaceView) => {
            return Err(residual(
                "logical CREATE VIEW is not a writable physical table",
            ));
        }
        None => return Err(residual("missing CREATE/REPLACE lifecycle evidence")),
    }
    let schema = schema_for(bundle, target)?;
    let projected = evaluate_simple_select(bundle, query, states)?;
    let columns = query
        .output()
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect::<Vec<_>>();
    if columns.len() != schema.columns().len()
        || columns.iter().collect::<BTreeSet<_>>().len() != columns.len()
        || schema
            .columns()
            .iter()
            .any(|c| !columns.contains(&c.name().to_string()))
    {
        return Err(residual(
            "definition projection does not determine all target columns",
        ));
    }
    let rows = projected
        .into_iter()
        .map(|values| {
            if values.len() != columns.len() {
                return Err(residual("invalid definition projection arity"));
            }
            Ok(PhysicalWriteRow {
                values: columns.iter().cloned().zip(values).collect(),
            })
        })
        .collect::<StateResult<Vec<_>>>()?;
    let before_rows = before.map_or(0, |state| state.rows.len());
    let deleted = u64::try_from(before_rows).map_err(|_| residual("replaced table overflow"))?;
    let inserted = u64::try_from(rows.len()).map_err(|_| residual("definition result overflow"))?;
    Ok((
        PhysicalWriteState {
            relation: target.to_string(),
            rows,
        },
        WriteRowCounts::new(inserted, 0, deleted),
    ))
}

/// Fill only omitted columns with independently attested scalar defaults.
/// Generated and identity expressions are not interchangeable with defaults.
fn complete_insert_row(
    bundle: &AnalysisBundle,
    relation: &str,
    mut values: BTreeMap<String, LiteralValue>,
    schema: &RelationSchema,
    identity_sequences: &mut BTreeMap<(String, String), PhysicalIdentitySequence>,
) -> StateResult<PhysicalWriteRow> {
    let sources = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == relation)
        .map(|set| set.column_value_sources())
        .unwrap_or(&[]);
    for column in schema.columns() {
        if values.contains_key(column.name()) {
            if sources.iter().any(|source| {
                source.column() == column.name()
                    && source.kind() != crate::column_sources::ColumnValueKind::Default
            }) {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "direct assignment to generated or identity column is unproved",
                ));
            }
            continue;
        }
        let source = sources
            .iter()
            .find(|source| source.column() == column.name())
            .ok_or_else(|| residual("omitted INSERT column has no attested default"))?;
        if source.kind() == crate::column_sources::ColumnValueKind::Generated {
            continue;
        }
        if source.kind() == crate::column_sources::ColumnValueKind::Identity {
            if !super::generated::authoritative(source) {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "identity allocation is not authoritatively declared",
                ));
            }
            let sequence = identity_sequences
                .get_mut(&(relation.to_string(), column.name().to_string()))
                .ok_or_else(|| residual("identity INSERT requires an attested sequence state"))?;
            values.insert(column.name().to_string(), sequence.allocate()?);
            continue;
        }
        if source.kind() != crate::column_sources::ColumnValueKind::Default {
            return Err(residual(
                "unsupported generated column value source in INSERT",
            ));
        }
        if !super::generated::authoritative(source) {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "default metadata lacks authoritative execution evidence",
            ));
        }
        let literal = source
            .value()
            .ok_or_else(|| residual("opaque INSERT default expression cannot be evaluated"))?;
        values.insert(
            column.name().to_string(),
            super::generated::scalar_literal(literal),
        );
    }
    super::generated::complete_generated(sources, &mut values, schema)?;
    if values.len() != schema.columns().len() {
        return Err(residual("INSERT row contains undeclared target columns"));
    }
    Ok(PhysicalWriteRow { values })
}

/// Materialize one local CTE from its complete original physical inputs.
fn materialize_local_cte(
    bundle: &AnalysisBundle,
    name: &str,
    producer: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
) -> StateResult<PhysicalWriteState> {
    if states.contains_key(name) {
        return Err(residual(
            "local CTE shadows an existing physical relation identity",
        ));
    }
    let columns = producer
        .output()
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if columns.is_empty() || columns.iter().collect::<BTreeSet<_>>().len() != columns.len() {
        return Err(residual(
            "local CTE producer columns are not uniquely named",
        ));
    }
    let rows = evaluate_simple_select(bundle, producer, states)?
        .into_iter()
        .map(|values| {
            if values.len() != columns.len() {
                return Err(residual("local CTE producer output arity is unproved"));
            }
            // A local CTE is evaluated before INSERT target coercion. In
            // particular, INTEGER arithmetic cannot borrow a wider target's
            // capacity to make an overflowing producer expression exact.
            // The bounded ordinary-CTE proof permits checked signed i32
            // arithmetic. Until an expression-width law is represented on
            // the canonical output, refuse wider computed values here too.
            for (column, value) in producer.output().columns().iter().zip(&values) {
                if matches!(
                    column.expression(),
                    Expression::Binary(_) | Expression::Unary(_)
                ) {
                    match value {
                        LiteralValue::Null => {}
                        LiteralValue::Number(number)
                            if number
                                .parse::<i128>()
                                .ok()
                                .is_some_and(|number| i32::try_from(number).is_ok()) => {}
                        _ => {
                            return Err(residual(
                                "computed local CTE expression needs a portable integer width proof",
                            ));
                        }
                    }
                }
            }
            Ok(PhysicalWriteRow {
                values: columns.iter().cloned().zip(values).collect(),
            })
        })
        .collect::<StateResult<Vec<_>>>()?;
    Ok(PhysicalWriteState {
        relation: name.to_string(),
        rows,
    })
}

pub(super) fn append_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
    identity_sequences: &mut BTreeMap<(String, String), PhysicalIdentitySequence>,
    intermediate_states: &mut Vec<PhysicalWriteState>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let Some(write) = query.write() else {
        return Err(residual("missing INSERT write"));
    };
    let schema = schema_for(bundle, target)?;
    let columns = if write.default_values() {
        Vec::new()
    } else if write.target_columns().is_empty() {
        schema
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect::<Vec<_>>()
    } else {
        write.target_columns().to_vec()
    };
    if columns.iter().collect::<BTreeSet<_>>().len() != columns.len()
        || columns
            .iter()
            .any(|column| !schema.columns().iter().any(|c| c.name() == column))
    {
        return Err(residual("INSERT targets duplicate or undeclared columns"));
    }
    let produced = if write.default_values() {
        vec![Vec::new()]
    } else if let Some(cte) = write.insert_cte_source() {
        let intermediate = materialize_local_cte(bundle, cte.name(), cte.producer(), states)?;
        let mut local_states = states.clone();
        local_states.insert(cte.name().to_string(), intermediate.clone());
        let result = evaluate_simple_select(bundle, cte.consumer(), &local_states)?;
        intermediate_states.push(intermediate);
        result
    } else if let Some(table) = query.finite_table() {
        if table.kind() != crate::protocol::FiniteTableKind::Values {
            return Err(residual(
                "INSERT finite source is not a literal VALUES table",
            ));
        }
        table
            .rows()
            .iter()
            .map(|row| row.iter().map(|value| value.value().clone()).collect())
            .collect()
    } else {
        evaluate_simple_select(bundle, query, states)?
    };
    if write.insert_conflict().is_some()
        && bundle.relation_constraints().iter().any(|set| {
            set.relation() == target
                && set
                    .column_value_sources()
                    .iter()
                    .any(|source| source.kind() == crate::column_sources::ColumnValueKind::Identity)
        })
    {
        return Err(residual(
            "ON CONFLICT with identity allocation needs a proven sequence consumption law",
        ));
    }
    let mut next = before.clone();
    for output_row in produced {
        if output_row.len() != columns.len() {
            return Err(residual("INSERT projection arity is not proved"));
        }
        let values = columns.iter().cloned().zip(output_row).collect();
        let candidate = complete_insert_row(bundle, target, values, schema, identity_sequences)?;
        if write.insert_conflict().is_some() {
            // A skipped tuple is not necessarily exempt from datatype checks.
            // Reject unclear validation precedence as Residual, not a false
            // successful conflict suppression or an unconditional Impossible.
            let candidate_state = PhysicalWriteState {
                relation: target.to_string(),
                rows: vec![candidate.clone()],
            };
            if super::constraints::validate_state(bundle, &candidate_state).is_err() {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "ON CONFLICT candidate has unproved datatype validation precedence",
                ));
            }
        }
        if let Some(policy) = write.insert_conflict() {
            // Validate the named key even for an empty initial target bag.
            // A collision may also be against a preceding row in this INSERT.
            super::constraints::validate_insert_conflict_target(bundle, target, policy.columns())?;
            // SQL UNIQUE NULL semantics vary by engine and declaration.
            // A single nullable candidate without a competing row does not
            // establish which native NULLS DISTINCT law applies.
            if policy
                .columns()
                .iter()
                .any(|column| matches!(candidate.values.get(column), Some(LiteralValue::Null)))
            {
                let primary = bundle.relation_constraints().iter().any(|set| {
                    set.relation() == target
                        && set.constraints().iter().any(|constraint| {
                            matches!(constraint, RelationConstraint::PrimaryKey(key)
                            if key.columns() == policy.columns())
                        })
                });
                if primary {
                    return Err(impossible("enforced PRIMARY KEY cannot contain NULL"));
                }
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "nullable ON CONFLICT UNIQUE key needs a native NULLS DISTINCT law",
                ));
            }
            let mut conflict = false;
            for (index, prior) in next.rows.iter().enumerate() {
                if super::constraints::targeted_insert_conflict(
                    bundle,
                    target,
                    policy.columns(),
                    &candidate,
                    prior,
                )? {
                    // SELECT does not certify source iteration order. Two
                    // colliding newly inserted rows with different payloads
                    // may select different winners on a native engine.
                    if index >= before.rows.len() && prior != &candidate {
                        return Err(residual(
                            "unordered INSERT conflict has different candidate payloads",
                        ));
                    }
                    conflict = true;
                    break;
                }
            }
            if conflict {
                continue;
            }
        }
        next.rows.push(candidate);
    }
    let inserted = u64::try_from(next.rows.len() - before.rows.len())
        .map_err(|_| residual("inserted count overflow"))?;
    Ok((next, WriteRowCounts::new(inserted, 0, 0)))
}

/// Check write-time schema obligations before WHERE filters away all rows.
/// SQL validates target names and DEFAULT declarations even for a zero-row write.
fn validate_update_assignments(
    bundle: &AnalysisBundle,
    target: &str,
    alias: Option<&str>,
    assignments: &[crate::protocol::MergeAssignment],
) -> StateResult<()> {
    let schema = schema_for(bundle, target)?;
    let sources = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == target)
        .map(|set| set.column_value_sources())
        .unwrap_or(&[]);
    let mut seen = BTreeSet::new();
    for assignment in assignments {
        let column = if let Some((qualifier, column)) = assignment.target().rsplit_once('.') {
            if qualifier != target && alias != Some(qualifier) {
                return Err(residual("UPDATE has an unresolved target assignment alias"));
            }
            column
        } else {
            assignment.target()
        };
        if !seen.insert(column) || !schema.columns().iter().any(|item| item.name() == column) {
            return Err(residual("unknown or overlapping UPDATE assignment"));
        }
        if sources.iter().any(|source| {
            source.column() == column
                && source.kind() != crate::column_sources::ColumnValueKind::Default
        }) {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "UPDATE explicitly assigns a generated or identity column",
            ));
        }
        if assignment.is_default() {
            let source = sources
                .iter()
                .find(|source| {
                    source.column() == column
                        && source.kind() == crate::column_sources::ColumnValueKind::Default
                })
                .ok_or_else(|| residual("UPDATE DEFAULT requires declared target default"))?;
            if !super::generated::authoritative(source) {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "UPDATE DEFAULT lacks authoritative execution evidence",
                ));
            }
            if source.value().is_none() {
                return Err(residual("opaque UPDATE DEFAULT cannot be evaluated"));
            }
        }
    }
    Ok(())
}

/// Apply an UPDATE atomically, then recompute attested generated columns.
///
/// SQL does not permit assigning an ALWAYS-generated column directly. Preserve
/// this distinction even when a direct assignment would equal the generated
/// value, and recompute from the post-assignment base-column values.
pub(super) fn apply_target_update(
    bundle: &AnalysisBundle,
    target: &str,
    before: &PhysicalWriteRow,
    assignments: &[crate::protocol::MergeAssignment],
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<PhysicalWriteRow> {
    let sources = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == target)
        .map(|set| set.column_value_sources())
        .unwrap_or(&[]);
    validate_update_assignments(bundle, target, context.target_alias, assignments)?;
    let mut normal = Vec::new();
    let mut defaults = Vec::new();
    for assignment in assignments {
        let column = assignment
            .target()
            .rsplit('.')
            .next()
            .unwrap_or(assignment.target());
        if assignment.is_default() {
            defaults.push(column);
        } else {
            normal.push(assignment.clone());
        }
    }
    let schema = schema_for(bundle, target)?;
    let mut next = apply_assignments(before, &normal, context, aliases)?;
    for column in defaults {
        let source = sources
            .iter()
            .find(|source| {
                source.column() == column
                    && source.kind() == crate::column_sources::ColumnValueKind::Default
            })
            .ok_or_else(|| residual("UPDATE DEFAULT requires declared target default"))?;
        if !super::generated::authoritative(source) {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "UPDATE DEFAULT lacks authoritative execution evidence",
            ));
        }
        let value = source
            .value()
            .ok_or_else(|| residual("opaque UPDATE DEFAULT cannot be evaluated"))?;
        next.values
            .insert(column.to_string(), super::generated::scalar_literal(value));
    }
    for source in sources {
        if source.kind() == crate::column_sources::ColumnValueKind::Generated {
            next.values.remove(source.column());
        }
    }
    super::generated::complete_generated(sources, &mut next.values, schema)?;
    Ok(next)
}

/// Prove one UPDATE/DELETE against a complete target bag and, when present,
/// one independent complete physical source bag.
///
/// UPDATE FROM requires no more than one matching source row per target row;
/// otherwise the selected assignment is engine-dependent and must fail closed.
/// DELETE USING has existential semantics and deletes each target occurrence
/// only once even when several source rows satisfy WHERE.
/// A grouped mutation result sink avoids multiple independent mutable
/// parameters and keeps the complete local producer bags in the transition.
pub(super) struct MutationTracking<'a> {
    pub(super) affected_rows: &'a mut Vec<PhysicalWriteRow>,
    pub(super) affected_target_ordinals: &'a mut Vec<usize>,
    pub(super) intermediate_states: &'a mut Vec<PhysicalWriteState>,
}

pub(super) fn mutate_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
    tracking: MutationTracking<'_>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let write = query
        .write()
        .ok_or_else(|| residual("missing mutation write"))?;
    let delete = write.kind() == WriteKind::Delete;
    if !delete {
        validate_update_assignments(bundle, target, write.target_alias(), write.assignments())?;
    }

    // A joined CTE reads only the physical source bag present at this exact
    // statement boundary. It is never a free initial or intermediate source.
    let local_states = if let Some(cte) = write.joined_cte_source() {
        if write
            .joined_source()
            .is_none_or(|source| source.name() != cte.name())
        {
            return Err(residual("joined CTE producer/source identity mismatch"));
        }
        let intermediate = materialize_local_cte(bundle, cte.name(), cte.producer(), states)?;
        let mut scoped = states.clone();
        scoped.insert(cte.name().to_string(), intermediate.clone());
        tracking.intermediate_states.push(intermediate);
        Some(scoped)
    } else {
        None
    };
    let states = local_states.as_ref().unwrap_or(states);
    let joined = if let Some(source) = write.joined_source() {
        if query.sources() != std::slice::from_ref(source) || source.name() == target {
            return Err(residual(
                "joined mutation has unproved source identity or target self-source",
            ));
        }
        if write.joined_cte_source().is_none() {
            schema_for(bundle, source.name())?;
        }
        let source_rows = states.get(source.name()).ok_or_else(|| {
            residual_gap(
                PhysicalWriteStateGap::MissingPopulation,
                "missing complete UPDATE FROM / DELETE USING source population",
            )
        })?;
        Some((source, source_rows))
    } else {
        None
    };
    // A correlated EXISTS is an independent read-only source, not a
    // relational join or a synthetic producer. All negative membership
    // proofs inspect the same complete source bag for every target preimage.
    let correlated = if joined.is_none() && !query.sources().is_empty() {
        let [source] = query.sources() else {
            return Err(residual("mutation has unaccounted physical sources"));
        };
        let Some(Predicate::Exists(exists)) = write.selection() else {
            return Err(residual("mutation source lacks canonical EXISTS owner"));
        };
        let nested = exists.subquery();
        if nested.sources() != query.sources()
            || !nested.row_shape_preserves_candidates()
            || !nested.joins().is_empty()
            || !nested.diagnostics().is_empty()
            || nested.predicates().where_predicate().is_none()
            || nested.correlations().is_empty()
            || nested
                .correlations()
                .iter()
                .any(|column| column.relation() != target)
            || source.name() == target
        {
            return Err(residual("correlated mutation has unproved producer shape"));
        }
        schema_for(bundle, source.name())?;
        let population = states.get(source.name()).ok_or_else(|| {
            residual_gap(
                PhysicalWriteStateGap::MissingPopulation,
                "correlated mutation needs complete independent source population",
            )
        })?;
        Some((source, nested, exists.negated(), population))
    } else {
        None
    };
    let aliases: Vec<&str> = joined.map_or_else(Vec::new, |(source, _)| {
        vec![source.alias().unwrap_or(source.name()), source.name()]
    });
    let mut kept = Vec::new();
    let mut affected = 0_u64;
    for (ordinal, row) in before.rows.iter().enumerate() {
        // All source matches must be evaluated against the *same* original target
        // preimage, never a previously modified row or a guessed producer bag.
        let matched_source = if let Some((source, population)) = joined {
            let mut matched = None;
            for candidate in &population.rows {
                let context = Context {
                    target: Some((target, row)),
                    target_alias: write.target_alias(),
                    source: Some((source.name(), candidate)),
                };
                if selected(write.selection(), &context, &aliases)? {
                    if matched.is_some() && !delete {
                        return Err(residual(
                            "multiple UPDATE FROM matches require an attested deterministic source-selection law",
                        ));
                    }
                    matched = Some(candidate);
                }
            }
            matched
        } else {
            None
        };
        let context = Context {
            target: Some((target, row)),
            target_alias: write.target_alias(),
            source: matched_source
                .and_then(|matched| joined.map(|(source, _)| (source.name(), matched))),
        };
        let matches = if joined.is_some() {
            matched_source.is_some()
        } else if let Some((source, nested, negated, population)) = correlated {
            let aliases = [source.alias().unwrap_or(source.name()), source.name()];
            let mut has_match = false;
            for candidate in &population.rows {
                let inner_context = Context {
                    target: Some((target, row)),
                    target_alias: write.target_alias(),
                    source: Some((source.name(), candidate)),
                };
                if selected(
                    nested.predicates().where_predicate(),
                    &inner_context,
                    &aliases,
                )? {
                    has_match = true;
                    break;
                }
            }
            has_match != negated
        } else {
            selected(write.selection(), &context, &[])?
        };
        if !matches {
            kept.push(row.clone());
            continue;
        }
        affected = affected
            .checked_add(1)
            .ok_or_else(|| residual("affected count overflow"))?;
        tracking.affected_target_ordinals.push(ordinal);
        if delete {
            // DELETE RETURNING contains target preimages, never source rows.
            tracking.affected_rows.push(row.clone());
        } else {
            let updated =
                apply_target_update(bundle, target, row, write.assignments(), &context, &aliases)?;
            tracking.affected_rows.push(updated.clone());
            kept.push(updated);
        }
    }
    let counts = if delete {
        WriteRowCounts::new(0, 0, affected)
    } else {
        WriteRowCounts::new(0, affected, 0)
    };
    Ok((
        PhysicalWriteState {
            relation: target.to_string(),
            rows: kept,
        },
        counts,
    ))
}

pub(super) fn merge_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
    identity_sequences: &mut BTreeMap<(String, String), PhysicalIdentitySequence>,
    affected_target_ordinals: &mut Vec<usize>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let write = query
        .write()
        .ok_or_else(|| residual("missing MERGE write"))?;
    let Some(match_condition) = write.match_condition() else {
        return Err(residual("missing MERGE match condition"));
    };
    let [source] = query.sources() else {
        return Err(residual("MERGE must have one independent physical source"));
    };
    if source.name() == target {
        return Err(residual(
            "MERGE self-source needs joint physical alias proof",
        ));
    }
    schema_for(bundle, source.name())?;
    let source_rows = states.get(source.name()).ok_or_else(|| {
        residual_gap(
            PhysicalWriteStateGap::MissingPopulation,
            "missing complete MERGE source population",
        )
    })?;
    let aliases = [source.alias().unwrap_or(source.name()), source.name()];
    let mut matched_sources = BTreeSet::new();
    let mut updates = 0_u64;
    let mut deletes = 0_u64;
    let mut inserts = Vec::new();
    let mut retained = Vec::new();
    for (target_ordinal, target_row) in before.rows.iter().enumerate() {
        let matches = source_rows
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, source_row)| {
                let context = Context {
                    target: Some((target, target_row)),
                    target_alias: write.target_alias(),
                    source: Some((source.name(), source_row)),
                };
                match selected(Some(match_condition), &context, &aliases) {
                    Ok(true) => Some(Ok(index)),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<StateResult<Vec<_>>>()?;
        if matches.len() > 1 {
            // Duplicate matches can produce engine-specific errors or winners.
            // Keep the established TASK-153/154 residual category; no exact
            // poststate can be claimed without an attested native law.
            return Err(residual(
                "multiple MERGE source matches require deterministic branch/multiplicity law",
            ));
        }
        let mut result = Some(target_row.clone());
        if let Some(index) = matches.first().copied() {
            matched_sources.insert(index);
            let source_row = &source_rows.rows[index];
            let context = Context {
                target: Some((target, target_row)),
                target_alias: write.target_alias(),
                source: Some((source.name(), source_row)),
            };
            for clause in write.merge_clauses() {
                if clause.match_kind() != MergeMatchKind::Matched
                    || !selected(clause.predicate(), &context, &aliases)?
                {
                    continue;
                }
                match clause.action() {
                    MergeAction::Update { assignments } => {
                        result = Some(apply_target_update(
                            bundle,
                            target,
                            target_row,
                            assignments,
                            &context,
                            &aliases,
                        )?);
                        updates += 1;
                        affected_target_ordinals.push(target_ordinal);
                    }
                    MergeAction::Delete => {
                        result = None;
                        deletes += 1;
                        affected_target_ordinals.push(target_ordinal);
                    }
                    MergeAction::Insert { .. } | MergeAction::Unsupported(_) => {
                        return Err(residual("unproved MATCHED MERGE action"));
                    }
                }
                break; // SQL's first-applicable branch, not independent actions.
            }
        } else {
            let context = Context {
                target: Some((target, target_row)),
                target_alias: write.target_alias(),
                source: None,
            };
            for clause in write.merge_clauses() {
                if clause.match_kind() != MergeMatchKind::NotMatchedBySource
                    || !selected(clause.predicate(), &context, &aliases)?
                {
                    continue;
                }
                match clause.action() {
                    MergeAction::Update { assignments } => {
                        result = Some(apply_target_update(
                            bundle,
                            target,
                            target_row,
                            assignments,
                            &context,
                            &aliases,
                        )?);
                        updates += 1;
                        affected_target_ordinals.push(target_ordinal);
                    }
                    MergeAction::Delete => {
                        result = None;
                        deletes += 1;
                        affected_target_ordinals.push(target_ordinal);
                    }
                    _ => return Err(residual("unproved BY SOURCE MERGE action")),
                }
                break;
            }
        }
        if let Some(row) = result {
            retained.push(row);
        }
    }
    for (index, source_row) in source_rows.rows.iter().enumerate() {
        if matched_sources.contains(&index) {
            continue;
        }
        let context = Context {
            target: None,
            target_alias: write.target_alias(),
            source: Some((source.name(), source_row)),
        };
        for clause in write.merge_clauses() {
            if !matches!(
                clause.match_kind(),
                MergeMatchKind::NotMatched | MergeMatchKind::NotMatchedByTarget
            ) || !selected(clause.predicate(), &context, &aliases)?
            {
                continue;
            }
            match clause.action() {
                MergeAction::Insert { columns, values } => {
                    let schema = schema_for(bundle, target)?;
                    if values.len() != 1 {
                        return Err(residual("MERGE INSERT requires one proven target row"));
                    }
                    if columns.len() != values[0].len() {
                        return Err(residual("MERGE INSERT target/value arity is not proved"));
                    }
                    let mut row_values = BTreeMap::new();
                    for (column, value) in columns.iter().zip(&values[0]) {
                        if row_values
                            .insert(
                                column.clone(),
                                eval(value.expression(), &context, &aliases)?,
                            )
                            .is_some()
                        {
                            return Err(impossible("duplicate MERGE target column"));
                        }
                    }
                    inserts.push(complete_insert_row(
                        bundle,
                        target,
                        row_values,
                        schema,
                        identity_sequences,
                    )?);
                }
                _ => return Err(residual("unproved NOT MATCHED MERGE action")),
            }
            break;
        }
    }
    let inserted = u64::try_from(inserts.len()).map_err(|_| residual("inserted count overflow"))?;
    retained.extend(inserts);
    Ok((
        PhysicalWriteState {
            relation: target.to_string(),
            rows: retained,
        },
        WriteRowCounts::new(inserted, updates, deletes),
    ))
}
