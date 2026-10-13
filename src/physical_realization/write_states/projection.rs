//! Project complete physical inputs and atomically apply SQL assignments.

use super::evaluator::{eval, selected, Context};
use super::*;

pub(super) fn schema_for<'a>(
    bundle: &'a AnalysisBundle,
    relation: &str,
) -> StateResult<&'a RelationSchema> {
    bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == relation)
        .ok_or_else(|| {
            residual_gap(
                PhysicalWriteStateGap::MissingSchema,
                format!("missing schema for {relation}"),
            )
        })
}

pub(super) fn evaluate_simple_select(
    _bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
) -> StateResult<Vec<Vec<LiteralValue>>> {
    if !(query.filter_only_row_shape() || query.row_preserving_projection())
        || query.set_operation().is_some()
        || query.aggregation().is_some()
        || !query.joins().is_empty()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
    {
        return Err(residual(
            "INSERT source is not a proven row-local, one-source SELECT",
        ));
    }
    if query.filter_only_row_shape() && query.predicates().where_predicate().is_none() {
        return Err(residual(
            "filtered SELECT lacks a canonically normalized WHERE predicate",
        ));
    }
    let [source] = query.sources() else {
        return Err(residual("INSERT SELECT must read one physical relation"));
    };
    let rows = states.get(source.name()).ok_or_else(|| {
        residual_gap(
            PhysicalWriteStateGap::MissingPopulation,
            "missing complete INSERT source population",
        )
    })?;
    let aliases = [source.alias().unwrap_or(source.name()), source.name()];
    let mut result = Vec::new();
    for row in &rows.rows {
        let context = Context {
            target: None,
            target_alias: None,
            source: Some((source.name(), row)),
        };
        if !selected(query.predicates().where_predicate(), &context, &aliases)? {
            continue;
        }
        let values = query
            .output()
            .columns()
            .iter()
            .map(|column| eval(column.expression(), &context, &aliases))
            .collect::<StateResult<Vec<_>>>()?;
        result.push(values);
    }
    Ok(result)
}

pub(super) fn apply_assignments(
    before: &PhysicalWriteRow,
    assignments: &[crate::protocol::MergeAssignment],
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<PhysicalWriteRow> {
    let mut next = before.clone();
    let mut written = BTreeSet::new();
    for assignment in assignments {
        let target = assignment.target();
        let name = if let Some((qualifier, column)) = target.rsplit_once('.') {
            let Some((canonical, _)) = context.target else {
                return Err(residual("update assignment has no target"));
            };
            if qualifier != canonical && context.target_alias != Some(qualifier) {
                return Err(residual("unresolved target column alias"));
            }
            column
        } else {
            target
        };
        if !before.values.contains_key(name) {
            return Err(residual("assignment targets an unknown physical column"));
        }
        if !written.insert(name) {
            return Err(residual("overlapping UPDATE assignments"));
        }
        let value = eval(assignment.value().expression(), context, aliases)?;
        // SQL evaluates all SET expressions against the original row, not
        // the previous SET value in the same statement.
        next.values.insert(name.to_string(), value);
    }
    Ok(next)
}
