//! Output scope, wildcard resolution and lineage provenance.

use super::*;

pub(super) fn build_output_scope(
    select: &Select,
    local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Vec<OutputRelation> {
    let local_qualifiers = select_local_qualifiers(select);
    let mut scope = outer_scope
        .iter()
        .filter_map(|relation| {
            let qualifiers = relation
                .qualifiers
                .iter()
                .filter(|qualifier| !local_qualifiers.contains(*qualifier))
                .cloned()
                .collect::<Vec<_>>();
            (!qualifiers.is_empty()).then(|| OutputRelation {
                qualifiers,
                source: relation.source.clone(),
            })
        })
        .collect::<Vec<_>>();

    for source in &select.from {
        register_output_table_factor(
            &source.relation,
            local_outputs,
            diagnostics,
            &mut scope,
            metadata,
        );
        for join in &source.joins {
            register_output_table_factor(
                &join.relation,
                local_outputs,
                diagnostics,
                &mut scope,
                metadata,
            );
        }
    }

    scope
}

pub(super) fn register_output_table_factor(
    factor: &TableFactor,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    scope: &mut Vec<OutputRelation>,
    metadata: Option<&AnalysisMetadata<'_>>,
) {
    match factor {
        TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } => {
            let relation_name = name.to_string();
            let qualifiers =
                relation_qualifiers(&relation_name, alias.as_ref().map(|a| a.name.to_string()));
            let source = match local_outputs.get(&relation_name) {
                Some(output) => OutputRelationSource::Local(alias.as_ref().map_or_else(
                    || output.clone(),
                    |alias| apply_output_column_aliases(output.clone(), alias),
                )),
                None => OutputRelationSource::Physical {
                    columns: metadata.and_then(|metadata| metadata.schema_columns(&relation_name)),
                    relation: relation_name,
                },
            };
            scope.push(OutputRelation { qualifiers, source });
        }
        TableFactor::Derived {
            lateral,
            subquery,
            alias,
            ..
        } => {
            let visible_outer_scope = if *lateral { scope.clone() } else { Vec::new() };
            let output = analyze_query_output_with_outer_scope(
                subquery,
                local_outputs,
                &visible_outer_scope,
                diagnostics,
                metadata,
            );
            let output = match alias {
                Some(alias) => apply_output_column_aliases(output, alias),
                None => output,
            };
            let qualifiers = alias
                .as_ref()
                .map(|alias| vec![alias.name.to_string()])
                .unwrap_or_default();
            scope.push(OutputRelation {
                qualifiers,
                source: OutputRelationSource::Local(output),
            });
        }
        _ => {}
    }
}

pub(super) fn relation_qualifiers(relation: &str, alias: Option<String>) -> Vec<String> {
    match alias {
        Some(alias) => vec![alias],
        None => {
            let mut qualifiers = vec![relation.to_string()];
            if let Some(last) = relation.rsplit('.').next() {
                if last != relation {
                    qualifiers.push(last.to_string());
                }
            }
            qualifiers
        }
    }
}

pub(super) fn lineage_for_expression(
    expression: &Expr,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LineageSource> {
    let mut lineage = BTreeSet::new();
    collect_output_lineage(
        expression,
        scope,
        named_windows,
        &mut BTreeSet::new(),
        diagnostics,
        &mut lineage,
    );
    lineage
        .into_iter()
        .map(|(relation, column)| LineageSource::new(relation, column))
        .collect()
}

pub(super) fn collect_output_lineage(
    expression: &Expr,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    match expression {
        Expr::Identifier(identifier) => {
            resolve_output_column(None, &identifier.value, scope, diagnostics, lineage);
        }
        Expr::CompoundIdentifier(identifiers) => {
            if let Some((column, relation_parts)) = identifiers.split_last() {
                let relation = relation_parts
                    .iter()
                    .map(|identifier| identifier.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                resolve_output_column(
                    Some(relation.as_str()),
                    &column.value,
                    scope,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::Subquery(query) => {
            let output = analyze_query_output_with_outer_scope(
                query,
                &BTreeMap::new(),
                scope,
                diagnostics,
                None,
            );
            for column in output.columns() {
                lineage.extend(
                    column
                        .lineage()
                        .iter()
                        .map(|source| (source.relation().to_string(), source.column().to_string())),
                );
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(operand) = operand {
                collect_output_lineage(
                    operand,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
            for branch in conditions {
                collect_output_lineage(
                    &branch.condition,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
                collect_output_lineage(
                    &branch.result,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
            if let Some(else_result) = else_result {
                collect_output_lineage(
                    else_result,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::Function(function) => {
            collect_function_argument_lineage(
                &function.parameters,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            collect_function_argument_lineage(
                &function.args,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            if let Some(filter) = &function.filter {
                collect_output_lineage(
                    filter,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
            if let Some(window) = &function.over {
                collect_window_lineage(
                    window,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::UnaryOp { expr, .. }
        | Expr::Nested(expr)
        | Expr::Cast { expr, .. }
        | Expr::IsNull(expr)
        | Expr::IsNotNull(expr) => {
            collect_output_lineage(
                expr,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
        Expr::BinaryOp { left, right, .. }
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. }
        | Expr::IsDistinctFrom(left, right)
        | Expr::IsNotDistinctFrom(left, right) => {
            collect_output_lineage(
                left,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            collect_output_lineage(
                right,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            for expression in [expr.as_ref(), low.as_ref(), high.as_ref()] {
                collect_output_lineage(
                    expression,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::InList { expr, list, .. } => {
            collect_output_lineage(
                expr,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            for value in list {
                collect_output_lineage(
                    value,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn collect_function_argument_lineage(
    arguments: &FunctionArguments,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if let FunctionArguments::List(arguments) = arguments {
        for argument in &arguments.args {
            if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expression)) = argument {
                collect_output_lineage(
                    expression,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
    }
}

pub(super) fn collect_window_lineage(
    window: &WindowType,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    match window {
        WindowType::WindowSpec(spec) => collect_window_spec_lineage(
            spec,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        ),
        WindowType::NamedWindow(name) => collect_named_window_lineage(
            &name.value,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        ),
    }
}

pub(super) fn collect_named_window_lineage(
    name: &str,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if !visited_windows.insert(name.to_string()) {
        return;
    }

    if let Some(NamedWindowDefinition(_, definition)) = named_windows
        .iter()
        .find(|definition| definition.0.value == name)
    {
        match definition {
            NamedWindowExpr::NamedWindow(base) => collect_named_window_lineage(
                &base.value,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            ),
            NamedWindowExpr::WindowSpec(spec) => collect_window_spec_lineage(
                spec,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            ),
        }
    }

    visited_windows.remove(name);
}

pub(super) fn collect_window_spec_lineage(
    spec: &SqlWindowSpec,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if let Some(name) = &spec.window_name {
        collect_named_window_lineage(
            &name.value,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        );
    }
    for expression in &spec.partition_by {
        collect_output_lineage(
            expression,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        );
    }
    for order in &spec.order_by {
        collect_output_lineage(
            &order.expr,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        );
    }
    if let Some(frame) = &spec.window_frame {
        if let SqlWindowFrameBound::Preceding(Some(expression))
        | SqlWindowFrameBound::Following(Some(expression)) = &frame.start_bound
        {
            collect_output_lineage(
                expression,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
        if let Some(
            SqlWindowFrameBound::Preceding(Some(expression))
            | SqlWindowFrameBound::Following(Some(expression)),
        ) = &frame.end_bound
        {
            collect_output_lineage(
                expression,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
    }
}

pub(super) fn output_column_candidates(
    qualifier: Option<&str>,
    column: &str,
    scope: &[OutputRelation],
) -> Vec<Vec<LineageSource>> {
    scope
        .iter()
        .filter(|relation| {
            qualifier.is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .filter_map(|relation| match &relation.source {
            OutputRelationSource::Physical {
                relation,
                columns: Some(columns),
            } if !columns.iter().any(|candidate| candidate == column) => None,
            OutputRelationSource::Physical { relation, .. } => Some(vec![LineageSource::new(
                relation.clone(),
                column.to_string(),
            )]),
            OutputRelationSource::Local(output) => output
                .columns()
                .iter()
                .find(|candidate| candidate.name() == column)
                .map(|candidate| candidate.lineage().to_vec()),
        })
        .collect()
}

pub(super) fn resolve_output_column(
    qualifier: Option<&str>,
    column: &str,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    let candidates = output_column_candidates(qualifier, column, scope);

    match candidates.as_slice() {
        [candidate] => {
            lineage.extend(
                candidate
                    .iter()
                    .map(|source| (source.relation().to_string(), source.column().to_string())),
            );
        }
        [] => diagnostics.push(warning(
            "unresolved_output_lineage",
            DiagnosticArea::Output,
            &format!(
                "source lineage for column {} could not be resolved",
                qualified_column_name(qualifier, column)
            ),
        )),
        _ => diagnostics.push(warning(
            "ambiguous_output_lineage",
            DiagnosticArea::Output,
            &format!(
                "source lineage for column {} is ambiguous without source schema information",
                qualified_column_name(qualifier, column)
            ),
        )),
    }
}

pub(super) fn qualified_column_name(qualifier: Option<&str>, column: &str) -> String {
    qualifier.map_or_else(
        || column.to_string(),
        |qualifier| format!("{qualifier}.{column}"),
    )
}
