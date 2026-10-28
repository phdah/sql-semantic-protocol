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
        if states.contains_key(cte.name()) {
            return Err(residual(
                "INSERT CTE shadows an existing physical relation identity",
            ));
        }
        let cte_columns = cte
            .producer()
            .output()
            .columns()
            .iter()
            .map(|column| column.name().to_string())
            .collect::<Vec<_>>();
        if cte_columns.is_empty()
            || cte_columns.iter().collect::<BTreeSet<_>>().len() != cte_columns.len()
        {
            return Err(residual(
                "INSERT CTE producer columns are not uniquely named",
            ));
        }
        let cte_rows = evaluate_simple_select(bundle, cte.producer(), states)?
            .into_iter()
            .map(|values| {
                if values.len() != cte_columns.len() {
                    return Err(residual("INSERT CTE producer output arity is unproved"));
                }
                Ok(PhysicalWriteRow {
                    values: cte_columns.iter().cloned().zip(values).collect(),
                })
            })
            .collect::<StateResult<Vec<_>>>()?;
        let mut local_states = states.clone();
        let intermediate = PhysicalWriteState {
            relation: cte.name().to_string(),
            rows: cte_rows,
        };
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
    for assignment in assignments {
        let column = assignment
            .target()
            .rsplit('.')
            .next()
            .unwrap_or(assignment.target());
        if sources.iter().any(|source| {
            source.column() == column
                && source.kind() != crate::column_sources::ColumnValueKind::Default
        }) {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "UPDATE explicitly assigns a generated or identity column",
            ));
        }
    }
    let schema = schema_for(bundle, target)?;
    let mut next = apply_assignments(before, assignments, context, aliases)?;
    for source in sources {
        if source.kind() == crate::column_sources::ColumnValueKind::Generated {
            next.values.remove(source.column());
        }
    }
    super::generated::complete_generated(sources, &mut next.values, schema)?;
    Ok(next)
}

pub(super) fn mutate_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    _states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
    delete: bool,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let write = query
        .write()
        .ok_or_else(|| residual("missing mutation write"))?;
    let mut kept = Vec::new();
    let mut affected = 0_u64;
    for row in &before.rows {
        let context = Context {
            target: Some((target, row)),
            target_alias: write.target_alias(),
            source: None,
        };
        if !selected(write.selection(), &context, &[])? {
            kept.push(row.clone());
            continue;
        }
        affected = affected
            .checked_add(1)
            .ok_or_else(|| residual("affected count overflow"))?;
        if !delete {
            kept.push(apply_target_update(
                bundle,
                target,
                row,
                write.assignments(),
                &context,
                &[],
            )?);
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
    for target_row in &before.rows {
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
                    }
                    MergeAction::Delete => {
                        result = None;
                        deletes += 1;
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
                    }
                    MergeAction::Delete => {
                        result = None;
                        deletes += 1;
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
