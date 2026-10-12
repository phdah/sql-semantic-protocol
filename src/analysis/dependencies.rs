//! Expression, relation, function and window dependency tracking.

use super::*;

pub(super) fn collect_expression_dependencies(
    expression: &Expr,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    collect_expression_dependencies_with_windows(
        expression,
        &[],
        local_relations,
        diagnostics,
        derived_index,
        dependencies,
    );
}

pub(super) fn collect_expression_dependencies_with_windows(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match expression {
        Expr::Subquery(query)
        | Expr::Exists {
            subquery: query, ..
        } => {
            let nested =
                analyze_query_relations(query, local_relations, diagnostics, derived_index, None);
            dependencies.extend(nested.dependencies);
        }
        Expr::InSubquery { expr, subquery, .. } => {
            collect_expression_dependencies_with_windows(
                expr,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            let nested = analyze_query_relations(
                subquery,
                local_relations,
                diagnostics,
                derived_index,
                None,
            );
            dependencies.extend(nested.dependencies);
        }
        Expr::BinaryOp { left, right, .. }
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. } => {
            collect_expression_dependencies_with_windows(
                left,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_expression_dependencies_with_windows(
                right,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) => {
            collect_expression_dependencies_with_windows(
                expr,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            for expression in [expr.as_ref(), low.as_ref(), high.as_ref()] {
                collect_expression_dependencies_with_windows(
                    expression,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        Expr::InList { expr, list, .. } => {
            collect_expression_dependencies_with_windows(
                expr,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            for value in list {
                collect_expression_dependencies_with_windows(
                    value,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
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
                collect_expression_dependencies_with_windows(
                    operand,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
            for branch in conditions {
                collect_expression_dependencies_with_windows(
                    &branch.condition,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
                collect_expression_dependencies_with_windows(
                    &branch.result,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
            if let Some(else_result) = else_result {
                collect_expression_dependencies_with_windows(
                    else_result,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        Expr::Function(function) => {
            collect_function_argument_dependencies(
                &function.parameters,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_function_argument_dependencies(
                &function.args,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            if let Some(filter) = &function.filter {
                collect_expression_dependencies_with_windows(
                    filter,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
            if let Some(window) = &function.over {
                collect_window_dependencies(
                    window,
                    named_windows,
                    &mut BTreeSet::new(),
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn collect_function_argument_dependencies(
    arguments: &FunctionArguments,
    named_windows: &[NamedWindowDefinition],
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match arguments {
        FunctionArguments::None => {}
        FunctionArguments::Subquery(query) => {
            let nested =
                analyze_query_relations(query, local_relations, diagnostics, derived_index, None);
            dependencies.extend(nested.dependencies);
        }
        FunctionArguments::List(arguments) => {
            for argument in &arguments.args {
                if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expression)) = argument {
                    collect_expression_dependencies_with_windows(
                        expression,
                        named_windows,
                        local_relations,
                        diagnostics,
                        derived_index,
                        dependencies,
                    );
                }
            }
        }
    }
}

pub(super) fn collect_window_dependencies(
    window: &WindowType,
    named_windows: &[NamedWindowDefinition],
    visited: &mut BTreeSet<String>,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match window {
        WindowType::WindowSpec(spec) => collect_window_spec_dependencies(
            spec,
            named_windows,
            visited,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        ),
        WindowType::NamedWindow(name) => collect_named_window_dependencies(
            &name.value,
            named_windows,
            visited,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        ),
    }
}

pub(super) fn collect_named_window_dependencies(
    name: &str,
    named_windows: &[NamedWindowDefinition],
    visited: &mut BTreeSet<String>,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    if !visited.insert(name.to_string()) {
        return;
    }

    if let Some(NamedWindowDefinition(_, definition)) = named_windows
        .iter()
        .find(|definition| definition.0.value == name)
    {
        match definition {
            NamedWindowExpr::NamedWindow(base) => collect_named_window_dependencies(
                &base.value,
                named_windows,
                visited,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            ),
            NamedWindowExpr::WindowSpec(spec) => collect_window_spec_dependencies(
                spec,
                named_windows,
                visited,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            ),
        }
    }

    visited.remove(name);
}

pub(super) fn collect_window_spec_dependencies(
    spec: &SqlWindowSpec,
    named_windows: &[NamedWindowDefinition],
    visited: &mut BTreeSet<String>,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    if let Some(name) = &spec.window_name {
        collect_named_window_dependencies(
            &name.value,
            named_windows,
            visited,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }

    for expression in &spec.partition_by {
        collect_expression_dependencies_with_windows(
            expression,
            named_windows,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }
    for order in &spec.order_by {
        collect_expression_dependencies_with_windows(
            &order.expr,
            named_windows,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }
    if let Some(frame) = &spec.window_frame {
        for bound in [
            &frame.start_bound,
            frame
                .end_bound
                .as_ref()
                .unwrap_or(&SqlWindowFrameBound::CurrentRow),
        ] {
            match bound {
                SqlWindowFrameBound::Preceding(Some(expression))
                | SqlWindowFrameBound::Following(Some(expression)) => {
                    collect_expression_dependencies_with_windows(
                        expression,
                        named_windows,
                        local_relations,
                        diagnostics,
                        derived_index,
                        dependencies,
                    );
                }
                _ => {}
            }
        }
    }
}

pub(super) fn merge_relation_analysis(target: &mut RelationAnalysis, source: RelationAnalysis) {
    for relation in source.sources {
        if !target.sources.iter().any(|existing| {
            existing.name() == relation.name() && existing.alias() == relation.alias()
        }) {
            target.sources.push(relation);
        }
    }
    target.dependencies.extend(source.dependencies);
    target.joins.extend(source.joins);
    target
        .residual_conditions
        .extend(source.residual_conditions);
}
