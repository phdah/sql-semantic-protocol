//! SQL function, aggregate, window and expression operator analysis.

use super::*;

pub(super) fn analyze_function(
    function: &Function,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if function.over.is_none() && is_aggregate_function(function) {
        return analyze_aggregate_function(function, expression, named_windows, diagnostics);
    }

    if function.uses_odbc_syntax
        || !matches!(&function.parameters, FunctionArguments::None)
        || function.filter.is_some()
        || function.null_treatment.is_some()
        || !function.within_group.is_empty()
    {
        return unsupported_expression(
            "function",
            expression,
            DiagnosticArea::Function,
            diagnostics,
        );
    }

    let (arguments, distinct) = match &function.args {
        FunctionArguments::None => (Vec::new(), false),
        FunctionArguments::List(arguments) if arguments.clauses.is_empty() => {
            let distinct = matches!(
                arguments.duplicate_treatment,
                Some(DuplicateTreatment::Distinct)
            );
            let mut normalized_arguments = Vec::with_capacity(arguments.args.len());

            for argument in &arguments.args {
                match argument {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) => {
                        normalized_arguments.push(analyze_expression_with_windows(
                            argument,
                            named_windows,
                            diagnostics,
                        ));
                    }
                    FunctionArg::Unnamed(FunctionArgExpr::Wildcard)
                        if function.over.is_some()
                            && function.name.to_string().eq_ignore_ascii_case("count") =>
                    {
                        // COUNT(*) and COUNT(1) are semantically identical for a
                        // window row bag: the normalized argument is non-NULL.
                        normalized_arguments.push(Expression::Literal(
                            crate::protocol::LiteralExpression::new(
                                crate::protocol::LiteralType::Integer,
                                crate::protocol::LiteralValue::Number("1".to_string()),
                            ),
                        ));
                    }
                    _ => {
                        return unsupported_expression(
                            "function",
                            expression,
                            DiagnosticArea::Function,
                            diagnostics,
                        )
                    }
                }
            }

            (normalized_arguments, distinct)
        }
        FunctionArguments::List(_) | FunctionArguments::Subquery(_) => {
            return unsupported_expression(
                "function",
                expression,
                DiagnosticArea::Function,
                diagnostics,
            );
        }
    };

    let function_expression =
        FunctionExpression::new(function.name.to_string(), arguments, distinct);

    match &function.over {
        Some(window) => match analyze_window_type(window, named_windows, diagnostics) {
            Some(window) => Expression::WindowFunction(WindowFunctionExpression::new(
                function_expression,
                window,
            )),
            None => Expression::Unsupported(UnsupportedSemantic::new(
                "window_function".to_string(),
                Some("window specification could not be resolved safely".to_string()),
            )),
        },
        None => Expression::Function(function_expression),
    }
}

pub(super) fn analyze_aggregate_function(
    function: &Function,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if function.uses_odbc_syntax
        || !matches!(&function.parameters, FunctionArguments::None)
        || function.null_treatment.is_some()
        || !function.within_group.is_empty()
    {
        return unsupported_expression(
            "aggregate_function",
            expression,
            DiagnosticArea::Function,
            diagnostics,
        );
    }

    let (arguments, distinct) = match &function.args {
        FunctionArguments::None => (Vec::new(), false),
        FunctionArguments::List(arguments) if arguments.clauses.is_empty() => {
            let distinct = matches!(
                arguments.duplicate_treatment,
                Some(DuplicateTreatment::Distinct)
            );
            let mut normalized_arguments = Vec::with_capacity(arguments.args.len());

            for argument in &arguments.args {
                match argument {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) => {
                        normalized_arguments.push(AggregateArgument::Expression(
                            analyze_expression_with_windows(argument, named_windows, diagnostics),
                        ));
                    }
                    FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => {
                        normalized_arguments.push(AggregateArgument::Wildcard);
                    }
                    FunctionArg::Unnamed(FunctionArgExpr::QualifiedWildcard(qualifier)) => {
                        normalized_arguments
                            .push(AggregateArgument::QualifiedWildcard(qualifier.to_string()));
                    }
                    _ => {
                        return unsupported_expression(
                            "aggregate_function",
                            expression,
                            DiagnosticArea::Function,
                            diagnostics,
                        )
                    }
                }
            }

            (normalized_arguments, distinct)
        }
        FunctionArguments::List(_) | FunctionArguments::Subquery(_) => {
            return unsupported_expression(
                "aggregate_function",
                expression,
                DiagnosticArea::Function,
                diagnostics,
            );
        }
    };

    let empty_aliases = BTreeMap::new();
    let filter = function.filter.as_ref().map(|filter| {
        analyze_predicate_with_windows(filter, named_windows, &empty_aliases, &[], diagnostics)
    });

    Expression::AggregateFunction(AggregateFunctionExpression::new(
        function.name.to_string(),
        arguments,
        distinct,
        filter,
    ))
}

pub(super) fn is_aggregate_function(function: &Function) -> bool {
    let name = function.name.to_string();
    let unqualified = name.rsplit('.').next().unwrap_or(name.as_str());
    let normalized = unqualified
        .trim_matches(|character| matches!(character, '"' | '`' | '[' | ']'))
        .to_ascii_uppercase();

    matches!(
        normalized.as_str(),
        "ANY_VALUE"
            | "ARRAY_AGG"
            | "AVG"
            | "BIT_AND"
            | "BIT_OR"
            | "BIT_XOR"
            | "BOOL_AND"
            | "BOOL_OR"
            | "CORR"
            | "COUNT"
            | "COVAR_POP"
            | "COVAR_SAMP"
            | "EVERY"
            | "MAX"
            | "MIN"
            | "STDDEV"
            | "STDDEV_POP"
            | "STDDEV_SAMP"
            | "STRING_AGG"
            | "SUM"
            | "VAR_POP"
            | "VAR_SAMP"
            | "VARIANCE"
    )
}

pub(super) fn analyze_window_type(
    window: &WindowType,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WindowSpecification> {
    match window {
        WindowType::WindowSpec(spec) => {
            analyze_window_spec(spec, named_windows, &mut BTreeSet::new(), diagnostics)
        }
        WindowType::NamedWindow(name) => {
            let resolved = resolve_named_window(
                &name.value,
                named_windows,
                &mut BTreeSet::new(),
                diagnostics,
            )?;
            Some(WindowSpecification::new(
                Some(name.value.clone()),
                resolved.partition_by().to_vec(),
                resolved.order_by().to_vec(),
                resolved.frame().cloned(),
            ))
        }
    }
}

pub(super) fn resolve_named_window(
    name: &str,
    named_windows: &[NamedWindowDefinition],
    resolving: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WindowSpecification> {
    let matches = named_windows
        .iter()
        .filter(|definition| definition.0.value == name)
        .collect::<Vec<_>>();

    let definition = match matches.as_slice() {
        [definition] => *definition,
        [] => {
            diagnostics.push(warning(
                "unresolved_named_window",
                DiagnosticArea::Function,
                &format!("named window {name} is not defined in the local query scope"),
            ));
            return None;
        }
        _ => {
            diagnostics.push(warning(
                "ambiguous_named_window",
                DiagnosticArea::Function,
                &format!("named window {name} is defined more than once in the local query scope"),
            ));
            return None;
        }
    };

    if !resolving.insert(name.to_string()) {
        diagnostics.push(warning(
            "cyclic_named_window",
            DiagnosticArea::Function,
            &format!("named window {name} participates in a reference cycle"),
        ));
        return None;
    }

    let resolved = match &definition.1 {
        NamedWindowExpr::NamedWindow(base) => {
            resolve_named_window(&base.value, named_windows, resolving, diagnostics)
        }
        NamedWindowExpr::WindowSpec(spec) => {
            analyze_window_spec(spec, named_windows, resolving, diagnostics)
        }
    };
    resolving.remove(name);

    resolved.map(|resolved| {
        WindowSpecification::new(
            Some(name.to_string()),
            resolved.partition_by().to_vec(),
            resolved.order_by().to_vec(),
            resolved.frame().cloned(),
        )
    })
}

pub(super) fn analyze_window_spec(
    spec: &SqlWindowSpec,
    named_windows: &[NamedWindowDefinition],
    resolving: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WindowSpecification> {
    let mut name = None;
    let mut partition_by = Vec::new();
    let mut order_by = Vec::new();
    let mut frame = None;

    if let Some(base_name) = &spec.window_name {
        let base = resolve_named_window(&base_name.value, named_windows, resolving, diagnostics)?;
        name = Some(base_name.value.clone());
        partition_by = base.partition_by().to_vec();
        order_by = base.order_by().to_vec();
        frame = base.frame().cloned();
    }

    if !spec.partition_by.is_empty() {
        if !partition_by.is_empty() {
            diagnostics.push(warning(
                "unsupported_window_override",
                DiagnosticArea::Function,
                "window PARTITION BY cannot safely override an inherited PARTITION BY clause",
            ));
            return None;
        }
        partition_by = spec
            .partition_by
            .iter()
            .map(|expression| {
                analyze_expression_with_windows(expression, named_windows, diagnostics)
            })
            .collect();
    }

    if !spec.order_by.is_empty() {
        if !order_by.is_empty() {
            diagnostics.push(warning(
                "unsupported_window_override",
                DiagnosticArea::Function,
                "window ORDER BY cannot safely override an inherited ORDER BY clause",
            ));
            return None;
        }
        order_by = spec
            .order_by
            .iter()
            .map(|order| {
                if order.with_fill.is_some() {
                    diagnostics.push(warning(
                        "unsupported_window_order_option",
                        DiagnosticArea::Function,
                        "window ORDER BY WITH FILL semantics are not represented",
                    ));
                }
                WindowOrderExpression::new(
                    analyze_expression_with_windows(&order.expr, named_windows, diagnostics),
                    order.options.asc,
                    order.options.nulls_first,
                )
            })
            .collect();
    }

    if let Some(window_frame) = &spec.window_frame {
        if frame.is_some() {
            diagnostics.push(warning(
                "unsupported_window_override",
                DiagnosticArea::Function,
                "window frame cannot safely override an inherited frame",
            ));
            return None;
        }
        frame = Some(analyze_window_frame(
            window_frame,
            named_windows,
            diagnostics,
        ));
    }

    Some(WindowSpecification::new(
        name,
        partition_by,
        order_by,
        frame,
    ))
}

pub(super) fn analyze_window_frame(
    frame: &SqlWindowFrame,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> WindowFrame {
    WindowFrame::new(
        match frame.units {
            SqlWindowFrameUnits::Rows => WindowFrameUnits::Rows,
            SqlWindowFrameUnits::Range => WindowFrameUnits::Range,
            SqlWindowFrameUnits::Groups => WindowFrameUnits::Groups,
        },
        analyze_window_frame_bound(&frame.start_bound, named_windows, diagnostics),
        frame
            .end_bound
            .as_ref()
            .map_or(WindowFrameBound::CurrentRow, |bound| {
                analyze_window_frame_bound(bound, named_windows, diagnostics)
            }),
    )
}

pub(super) fn analyze_window_frame_bound(
    bound: &SqlWindowFrameBound,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> WindowFrameBound {
    match bound {
        SqlWindowFrameBound::CurrentRow => WindowFrameBound::CurrentRow,
        SqlWindowFrameBound::Preceding(None) => WindowFrameBound::UnboundedPreceding,
        SqlWindowFrameBound::Following(None) => WindowFrameBound::UnboundedFollowing,
        SqlWindowFrameBound::Preceding(Some(expression)) => WindowFrameBound::Preceding(Box::new(
            analyze_expression_with_windows(expression, named_windows, diagnostics),
        )),
        SqlWindowFrameBound::Following(Some(expression)) => WindowFrameBound::Following(Box::new(
            analyze_expression_with_windows(expression, named_windows, diagnostics),
        )),
    }
}

pub(super) fn analyze_unary_expression(
    operator: &SqlUnaryOperator,
    operand: &Expr,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let operator = match operator {
        SqlUnaryOperator::Plus => Some(UnaryOperator::Plus),
        SqlUnaryOperator::Minus => Some(UnaryOperator::Minus),
        SqlUnaryOperator::PGBitwiseNot => Some(UnaryOperator::BitwiseNot),
        _ => None,
    };

    match operator {
        Some(operator) => Expression::Unary(UnaryExpression::new(
            operator,
            analyze_expression_with_windows(operand, named_windows, diagnostics),
        )),
        None => unsupported_expression(
            "unary_expression",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

pub(super) fn analyze_binary_expression(
    left: &Expr,
    operator: &SqlBinaryOperator,
    right: &Expr,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let operator = match operator {
        SqlBinaryOperator::Plus => Some(BinaryOperator::Add),
        SqlBinaryOperator::Minus => Some(BinaryOperator::Subtract),
        SqlBinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        SqlBinaryOperator::Divide => Some(BinaryOperator::Divide),
        SqlBinaryOperator::Modulo => Some(BinaryOperator::Modulo),
        SqlBinaryOperator::StringConcat => Some(BinaryOperator::StringConcat),
        SqlBinaryOperator::BitwiseAnd => Some(BinaryOperator::BitwiseAnd),
        SqlBinaryOperator::BitwiseOr => Some(BinaryOperator::BitwiseOr),
        SqlBinaryOperator::BitwiseXor | SqlBinaryOperator::PGBitwiseXor => {
            Some(BinaryOperator::BitwiseXor)
        }
        _ => None,
    };

    match operator {
        Some(operator) => Expression::Binary(BinaryExpression::new(
            operator,
            analyze_expression_with_windows(left, named_windows, diagnostics),
            analyze_expression_with_windows(right, named_windows, diagnostics),
        )),
        None => unsupported_expression(
            "binary_expression",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

pub(super) fn unsupported_expression(
    feature: &str,
    expression: &Expr,
    area: DiagnosticArea,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    unsupported_expression_with_reason(
        feature,
        expression,
        area,
        diagnostics,
        "its semantics are not implemented",
    )
}

pub(super) fn unsupported_expression_with_reason(
    feature: &str,
    expression: &Expr,
    area: DiagnosticArea,
    diagnostics: &mut Vec<Diagnostic>,
    missing_law: &str,
) -> Expression {
    let code = if area == DiagnosticArea::Function {
        "unsupported_function"
    } else {
        "unsupported_expression"
    };
    let reason = format!("{feature} {expression} is parsed but {missing_law}");

    diagnostics.push(warning(code, area, &reason));

    Expression::Unsupported(UnsupportedSemantic::new(feature.to_string(), Some(reason)))
}
