//! SELECT output projections and value domains.

use super::*;

pub(super) type LocalOutputMap = BTreeMap<String, Output>;

#[derive(Clone)]
pub(super) struct OutputRelation {
    pub(super) qualifiers: Vec<String>,
    pub(super) source: OutputRelationSource,
}

#[derive(Clone)]
pub(super) enum OutputRelationSource {
    Physical {
        relation: String,
        columns: Option<Vec<String>>,
    },
    Local(Output),
}

pub(super) fn analyze_query_output(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    analyze_query_output_with_outer_scope(
        query,
        inherited_local_outputs,
        &[],
        diagnostics,
        metadata,
    )
}

pub(super) fn analyze_query_output_with_outer_scope(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    let mut local_outputs = inherited_local_outputs.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let mut cte_diagnostics = Vec::new();
            let output = analyze_query_output_with_outer_scope(
                &cte.query,
                &local_outputs,
                &[],
                &mut cte_diagnostics,
                metadata,
            );
            local_outputs.insert(cte.alias.name.to_string(), output);
        }
    }

    analyze_set_expr_output_with_outer_scope(
        query.body.as_ref(),
        &local_outputs,
        outer_scope,
        diagnostics,
        metadata,
    )
}

pub(super) fn analyze_set_expr_output_with_outer_scope(
    expression: &SetExpr,
    local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    match expression {
        SetExpr::Select(select) => analyze_select_output_with_outer_scope(
            select,
            local_outputs,
            outer_scope,
            diagnostics,
            metadata,
        ),
        SetExpr::Query(query) => analyze_query_output_with_outer_scope(
            query,
            local_outputs,
            outer_scope,
            diagnostics,
            metadata,
        ),
        SetExpr::SetOperation {
            left,
            op,
            set_quantifier,
            right,
        } => {
            let quantifier = analyze_set_quantifier(*set_quantifier);
            if quantifier.uses_name_alignment() {
                diagnostics.push(warning(
                    "unsupported_set_operation_alignment",
                    DiagnosticArea::Output,
                    "BY NAME set-operation alignment is represented but output-column composition is not implemented",
                ));
                return Output::new(Vec::new());
            }

            let left_output = analyze_set_expr_output_with_outer_scope(
                left,
                local_outputs,
                outer_scope,
                diagnostics,
                metadata,
            );
            let right_output = analyze_set_expr_output_with_outer_scope(
                right,
                local_outputs,
                outer_scope,
                diagnostics,
                metadata,
            );
            merge_set_operation_output(
                left_output,
                right_output,
                analyze_set_operator(*op),
                diagnostics,
            )
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => Output::new(Vec::new()),
    }
}

// Branch-local output domains include WHERE-derived restrictions. Combining raw
// SELECT AST output domains loses those restrictions, so recompute the final
// domains from the already analyzed leaves in the recursive operation tree.
pub(super) fn refine_set_operation_output_domains(
    output: Output,
    operation: &SetOperation,
) -> Output {
    let mut next_branch = 0;
    let Some(domains) = operation_output_domains(operation, operation.branches(), &mut next_branch)
    else {
        return output;
    };
    if next_branch != operation.branches().len() || domains.len() != output.columns().len() {
        return output;
    }
    Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .zip(domains)
            .map(|(column, domain)| column.with_domain(domain))
            .collect(),
    )
}

pub(super) fn operation_output_domains(
    operation: &SetOperation,
    branches: &[SetBranch],
    next_branch: &mut usize,
) -> Option<Vec<ValueDomain>> {
    let mut operand_domains = |operand: &SetOperand| match operand {
        SetOperand::Query => {
            let branch = branches.get(*next_branch)?;
            *next_branch += 1;
            Some(
                branch
                    .output()
                    .columns()
                    .iter()
                    .map(|column| column.domain().clone())
                    .collect::<Vec<_>>(),
            )
        }
        SetOperand::Operation(nested) => operation_output_domains(nested, branches, next_branch),
    };
    let left = operand_domains(operation.left())?;
    let right = operand_domains(operation.right())?;
    if left.is_empty() || left.len() != right.len() || operation.multiplicity_rule().is_none() {
        return None;
    }
    Some(
        left.iter()
            .zip(&right)
            .map(|(l, r)| match operation.operator() {
                SetOperator::Union => union_domains(l, r),
                SetOperator::Intersect => crate::domain::intersect_set_operation_domains(l, r),
                SetOperator::Except => l.clone(),
            })
            .collect(),
    )
}

pub(super) fn merge_set_operation_output(
    left: Output,
    right: Output,
    operator: SetOperator,
    diagnostics: &mut Vec<Diagnostic>,
) -> Output {
    if left.columns().is_empty() || right.columns().is_empty() {
        diagnostics.push(warning(
            "unresolved_set_operation_output",
            DiagnosticArea::Output,
            "set-operation output cannot be resolved because at least one branch has no resolved output columns",
        ));
        return Output::new(Vec::new());
    }

    if left.columns().len() != right.columns().len() {
        diagnostics.push(warning(
            "set_operation_arity_mismatch",
            DiagnosticArea::Output,
            &format!(
                "set-operation branches expose different column counts: left has {}, right has {}",
                left.columns().len(),
                right.columns().len()
            ),
        ));
        return Output::new(Vec::new());
    }

    let columns = left
        .columns()
        .iter()
        .zip(right.columns())
        .map(|(left_column, right_column)| {
            let mut lineage = left_column.lineage().to_vec();
            lineage.extend_from_slice(right_column.lineage());

            OutputColumn::new(
                left_column.name().to_string(),
                Expression::Unknown(UnknownSemantic::new(
                    "set-operation output value is determined positionally by multiple query branches"
                        .to_string(),
                )),
                match operator {
                    SetOperator::Union => union_domains(left_column.domain(), right_column.domain()),
                    SetOperator::Intersect => crate::domain::intersect_set_operation_domains(left_column.domain(), right_column.domain()),
                    SetOperator::Except => left_column.domain().clone(),
                },
                lineage,
            )
        })
        .collect();

    Output::new(columns)
}

pub(super) fn analyze_select_output_with_outer_scope(
    select: &Select,
    local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    let scope = build_output_scope(select, local_outputs, outer_scope, diagnostics, metadata);
    let mut columns = Vec::new();
    for item in &select.projection {
        columns.extend(analyze_output_item(
            item,
            &scope,
            &select.named_window,
            diagnostics,
        ));
    }

    if let Some(qualify) = &select.qualify {
        refine_output_domains_from_predicate(&mut columns, qualify);
    }

    Output::new(columns)
}

pub(super) fn analyze_output_item(
    item: &SelectItem,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<OutputColumn> {
    match item {
        SelectItem::UnnamedExpr(expression) => {
            let normalized = inherit_local_case_expression(
                analyze_expression_with_scope(expression, scope, named_windows, diagnostics),
                scope,
            );
            let domain = derive_output_domain(&normalized);
            vec![OutputColumn::new(
                output_name_for_expression(expression),
                normalized,
                domain,
                lineage_for_expression(expression, scope, named_windows, diagnostics),
            )]
        }
        SelectItem::ExprWithAlias { expr, alias } => {
            let normalized = inherit_local_case_expression(
                analyze_expression_with_scope(expr, scope, named_windows, diagnostics),
                scope,
            );
            let domain = derive_output_domain(&normalized);
            vec![OutputColumn::new(
                alias.value.clone(),
                normalized,
                domain,
                lineage_for_expression(expr, scope, named_windows, diagnostics),
            )]
        }
        SelectItem::Wildcard(_) => expand_wildcard(None, scope, diagnostics),
        SelectItem::QualifiedWildcard(prefix, _) => {
            let qualifier = prefix.to_string();
            expand_wildcard(Some(&qualifier), scope, diagnostics)
        }
    }
}

pub(super) fn inherit_local_case_expression(
    expression: Expression,
    scope: &[OutputRelation],
) -> Expression {
    let Expression::Column(column) = &expression else {
        return expression;
    };

    let candidates = scope
        .iter()
        .filter(|relation| {
            column.relation().is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .filter_map(|relation| match &relation.source {
            OutputRelationSource::Physical {
                columns: Some(columns),
                ..
            } if !columns.iter().any(|candidate| candidate == column.name()) => None,
            OutputRelationSource::Physical { .. } => Some(None),
            OutputRelationSource::Local(output) => {
                let mut matches = output
                    .columns()
                    .iter()
                    .filter(|candidate| candidate.name() == column.name());
                let candidate = matches.next()?;
                if matches.next().is_some() {
                    return Some(None);
                }

                Some(
                    matches!(candidate.expression(), Expression::Case(_))
                        .then(|| candidate.expression().clone()),
                )
            }
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [Some(case_expression)] => case_expression.clone(),
        _ => expression,
    }
}

pub(super) fn expand_wildcard(
    qualifier: Option<&str>,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<OutputColumn> {
    let relations = scope
        .iter()
        .filter(|relation| {
            qualifier.is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .collect::<Vec<_>>();

    if relations.is_empty() || (qualifier.is_some() && relations.len() != 1) {
        return vec![unresolved_wildcard_column(
            qualifier.map_or_else(|| "*".to_string(), |value| format!("{value}.*")),
            diagnostics,
        )];
    }

    let mut columns = Vec::new();
    for relation in relations {
        let expression_qualifier = relation.qualifiers.first().cloned();
        match &relation.source {
            OutputRelationSource::Physical {
                relation,
                columns: Some(schema_columns),
            } => {
                columns.extend(schema_columns.iter().map(|column| {
                    OutputColumn::new(
                        column.clone(),
                        Expression::Column(ColumnExpression::new(
                            expression_qualifier.clone(),
                            column.clone(),
                        )),
                        ValueDomain::Unbounded,
                        vec![LineageSource::new(relation.clone(), column.clone())],
                    )
                }));
            }
            OutputRelationSource::Physical { columns: None, .. } => {
                return vec![unresolved_wildcard_column(
                    qualifier.map_or_else(|| "*".to_string(), |value| format!("{value}.*")),
                    diagnostics,
                )];
            }
            OutputRelationSource::Local(output) => {
                columns.extend(output.columns().iter().map(|column| {
                    let expression = match column.expression() {
                        Expression::Case(_) => column.expression().clone(),
                        _ => Expression::Column(ColumnExpression::new(
                            expression_qualifier.clone(),
                            column.name().to_string(),
                        )),
                    };
                    OutputColumn::new(
                        column.name().to_string(),
                        expression,
                        column.domain().clone(),
                        column.lineage().to_vec(),
                    )
                }));
            }
        }
    }

    if columns.is_empty() {
        vec![unresolved_wildcard_column(
            qualifier.map_or_else(|| "*".to_string(), |value| format!("{value}.*")),
            diagnostics,
        )]
    } else {
        columns
    }
}

pub(super) fn unresolved_wildcard_column(
    name: String,
    diagnostics: &mut Vec<Diagnostic>,
) -> OutputColumn {
    let reason = "wildcard output cannot be resolved without source schema information";
    diagnostics.push(warning(
        "unresolved_wildcard",
        DiagnosticArea::Output,
        reason,
    ));
    OutputColumn::new(
        name,
        Expression::Unknown(UnknownSemantic::new(reason.to_string())),
        ValueDomain::unknown(reason),
        Vec::new(),
    )
}

pub(super) fn refine_output_domains_from_column_domains(
    output: Output,
    column_domains: &[ColumnDomain],
    sources: &[SourceRelation],
) -> Output {
    Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .map(|column| {
                let derived = derive_expression_domain_with_column_domains(
                    column.expression(),
                    column_domains,
                    sources,
                );
                let lineage_domain =
                    column
                        .plain_copy_source()
                        .map_or(ValueDomain::Unbounded, |source| {
                            column_domains
                                .iter()
                                .find(|candidate| {
                                    candidate.column().relation() == Some(source.relation())
                                        && candidate.column().name() == source.column()
                                })
                                .map_or(ValueDomain::Unbounded, |candidate| {
                                    candidate.domain().clone()
                                })
                        });
                let domain = intersect_domains(column.domain(), &derived);
                let domain = intersect_domains(&domain, &lineage_domain);
                column.with_domain(domain)
            })
            .collect(),
    )
}

pub(super) fn derive_expression_domain_with_column_domains(
    expression: &Expression,
    column_domains: &[ColumnDomain],
    sources: &[SourceRelation],
) -> ValueDomain {
    if let Expression::Column(column) = expression {
        let resolved = resolve_column(column, sources);
        if let Some(domain) = column_domains
            .iter()
            .find(|candidate| candidate.column() == &resolved)
        {
            return domain.domain().clone();
        }
    }

    derive_output_domain(expression)
}

pub(super) fn derive_output_domain(expression: &Expression) -> ValueDomain {
    match expression {
        Expression::Column(_) => ValueDomain::Unbounded,
        Expression::Literal(literal) => ValueDomain::set(SetMode::Include, vec![literal.clone()]),
        Expression::BooleanPredicate(_) => boolean_domain(),
        Expression::WindowFunction(window)
            if window.function().name().eq_ignore_ascii_case("row_number") =>
        {
            integer_lower_bound_domain(1)
        }
        Expression::WindowFunction(_) => {
            ValueDomain::unknown("window-function output domain is not known safely")
        }
        Expression::AggregateFunction(function)
            if function.name().eq_ignore_ascii_case("count") =>
        {
            integer_lower_bound_domain(0)
        }
        Expression::AggregateFunction(_) => {
            ValueDomain::unknown("aggregate output domain is not known safely")
        }
        Expression::Case(case_expression) => {
            let mut domain = ValueDomain::Empty;
            for branch in case_expression.branches() {
                domain = union_domains(&domain, &derive_output_domain(branch.result()));
            }
            let else_domain = case_expression.else_result().map_or_else(
                || {
                    ValueDomain::set(
                        SetMode::Include,
                        vec![LiteralExpression::new(
                            LiteralType::Null,
                            LiteralValue::Null,
                        )],
                    )
                },
                derive_output_domain,
            );
            union_domains(&domain, &else_domain)
        }
        Expression::SignedIntegerCast(cast) => derive_signed_integer_cast_output_domain(cast),
        Expression::Unary(unary) => derive_unary_output_domain(unary),
        Expression::Binary(binary) => derive_binary_output_domain(binary),
        Expression::ScalarSubquery(subquery) => {
            let columns = subquery.subquery().output().columns();
            match columns {
                [column] => column.domain().clone(),
                _ => ValueDomain::unknown(
                    "scalar-subquery output domain requires exactly one resolved column",
                ),
            }
        }
        Expression::Function(_) => {
            ValueDomain::unknown("scalar-function output domain is not known safely")
        }
        Expression::Unknown(semantic) => ValueDomain::unknown(semantic.reason()),
        Expression::Unsupported(_) => {
            ValueDomain::unknown("unsupported expression has no safely known output domain")
        }
    }
}

pub(super) fn boolean_domain() -> ValueDomain {
    ValueDomain::set(
        SetMode::Include,
        vec![
            LiteralExpression::new(LiteralType::Boolean, LiteralValue::Boolean(false)),
            LiteralExpression::new(LiteralType::Boolean, LiteralValue::Boolean(true)),
        ],
    )
}

pub(super) fn integer_lower_bound_domain(value: i128) -> ValueDomain {
    ValueDomain::ranges(vec![ValueRange::new(
        Some(Bound::new(integer_literal(value), true)),
        None,
    )])
}

pub(super) fn integer_literal(value: i128) -> LiteralExpression {
    LiteralExpression::new(
        LiteralType::Integer,
        LiteralValue::Number(value.to_string()),
    )
}

pub(super) fn singleton_integer(domain: &ValueDomain) -> Option<i128> {
    let ValueDomain::Set(set) = domain else {
        return None;
    };
    if set.mode() != SetMode::Include {
        return None;
    }
    let [literal] = set.values() else {
        return None;
    };
    if literal.literal_type() != LiteralType::Integer {
        return None;
    }
    let LiteralValue::Number(value) = literal.value() else {
        return None;
    };
    value.parse().ok()
}

fn explicit_signed_integer_width(expression: &Expression) -> Option<SignedIntegerWidth> {
    match expression {
        Expression::SignedIntegerCast(cast) => Some(cast.target_width()),
        Expression::Unary(unary) => explicit_signed_integer_width(unary.operand()),
        _ => None,
    }
}

pub(super) fn derive_unary_output_domain(unary: &UnaryExpression) -> ValueDomain {
    let operand = derive_output_domain(unary.operand());
    match unary.operator() {
        UnaryOperator::Plus => operand,
        UnaryOperator::Minus => exact_decimal_unary_domain(unary).unwrap_or_else(|| {
            let width = explicit_signed_integer_width(unary.operand())
                .unwrap_or(SignedIntegerWidth::Bits32);
            singleton_integer(&operand)
                .filter(|value| width.fits(*value))
                .and_then(i128::checked_neg)
                .filter(|value| width.fits(*value))
                .map_or_else(
                    || ValueDomain::unknown("unary minus cannot be bounded safely"),
                    |value| ValueDomain::set(SetMode::Include, vec![integer_literal(value)]),
                )
        }),
        UnaryOperator::BitwiseNot => {
            ValueDomain::unknown("bitwise output domain is not bounded safely")
        }
    }
}

pub(super) fn derive_binary_output_domain(binary: &BinaryExpression) -> ValueDomain {
    if let Some(exact_decimal) = exact_decimal_binary_domain(binary) {
        return exact_decimal;
    }
    // Only certified integer operands can propagate a known SQL NULL result.
    // In particular do not turn NULL / 0 or untyped NULL coercions into an
    // exact value-domain proof.
    let null_literal = |expression: &Expression| {
        matches!(
            expression,
            Expression::Literal(literal) if literal.literal_type() == LiteralType::Null
        )
    };
    if matches!(
        binary.operator(),
        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
    ) && ((null_literal(binary.left())
        && singleton_integer(&derive_output_domain(binary.right())).is_some())
        || (null_literal(binary.right())
            && singleton_integer(&derive_output_domain(binary.left())).is_some()))
    {
        return ValueDomain::set(
            SetMode::Include,
            vec![LiteralExpression::new(
                LiteralType::Null,
                LiteralValue::Null,
            )],
        );
    }
    let left = singleton_integer(&derive_output_domain(binary.left()));
    let right = singleton_integer(&derive_output_domain(binary.right()));
    let (Some(left), Some(right)) = (left, right) else {
        return ValueDomain::unknown(format!(
            "arithmetic {} on {} and {} requires safely bounded, typed operands",
            binary.operator().as_str(),
            crate::protocol::arithmetic_operand_kind(binary.left()),
            crate::protocol::arithmetic_operand_kind(binary.right())
        ));
    };
    let left_width = explicit_signed_integer_width(binary.left());
    let right_width = explicit_signed_integer_width(binary.right());
    let result_width = match (left_width, right_width) {
        (Some(left), Some(right)) => left.min(right),
        (Some(width), None) | (None, Some(width)) => width,
        // Portable implicit integer literal arithmetic is limited to the
        // narrowest conventional SQL integer width. Wider results require an
        // explicit cast instead of assuming dialect-specific promotion.
        (None, None) => SignedIntegerWidth::Bits32,
    };

    let value = match binary.operator() {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::Multiply => left.checked_mul(right),
        // SQL integer division may return a decimal/floating result in
        // some dialects and a truncated integer in others. Do not silently
        // encode Rust integer division as a portable SQL result domain.
        BinaryOperator::Divide => None,
        // Modulo by +1 has an exact zero result for every signed integer,
        // with no divide-by-zero or MIN / -1 edge. Other remainder signs,
        // argument coercions and zero divisors are not universally attested.
        BinaryOperator::Modulo if right == 1 => Some(0),
        BinaryOperator::Modulo => None,
        BinaryOperator::StringConcat
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor => None,
    };

    // Literal calculations with implicit arithmetic widening, or
    // dialect-dependent division and remainder laws, are not portable exact
    // integer domains.
    value
        .filter(|value| {
            result_width.fits(left)
                && result_width.fits(right)
                && result_width.fits(*value)
        })
        .map_or_else(
            || {
                ValueDomain::unknown(format!(
                    "arithmetic {} on {} and {} requires attested result width, zero-divisor and rounding laws",
                    binary.operator().as_str(),
                    crate::protocol::arithmetic_operand_kind(binary.left()),
                    crate::protocol::arithmetic_operand_kind(binary.right())
                ))
            },
            |value| ValueDomain::set(SetMode::Include, vec![integer_literal(value)]),
        )
}

pub(super) fn refine_output_domains_from_predicate(columns: &mut [OutputColumn], predicate: &Expr) {
    for column in columns {
        if let Some(refinement) = output_alias_domain(predicate, column.name()) {
            let domain = intersect_domains(column.domain(), &refinement);
            *column = column.clone().with_domain(domain);
        }
    }
}

pub(super) fn output_alias_domain(expression: &Expr, alias: &str) -> Option<ValueDomain> {
    match expression {
        Expr::Nested(inner) => output_alias_domain(inner, alias),
        Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::And,
            right,
        } => match (
            output_alias_domain(left, alias),
            output_alias_domain(right, alias),
        ) {
            (Some(left), Some(right)) => Some(intersect_domains(&left, &right)),
            (Some(domain), None) | (None, Some(domain)) => Some(domain),
            (None, None) => None,
        },
        Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::Or,
            right,
        } => match (
            output_alias_domain(left, alias),
            output_alias_domain(right, alias),
        ) {
            (Some(left), Some(right)) => Some(union_domains(&left, &right)),
            _ => None,
        },
        Expr::BinaryOp { left, op, right } => {
            let operator = comparison_operator(op)?;
            if expression_is_alias(left, alias) {
                literal_domain_for_comparison(right, operator)
            } else if expression_is_alias(right, alias) {
                literal_domain_for_comparison(left, operator.reversed())
            } else {
                None
            }
        }
        Expr::Between {
            expr,
            negated: false,
            low,
            high,
        } if expression_is_alias(expr, alias) => {
            let low = literal_expression(low)?;
            let high = literal_expression(high)?;
            Some(ValueDomain::ranges(vec![ValueRange::new(
                Some(Bound::new(low, true)),
                Some(Bound::new(high, true)),
            )]))
        }
        Expr::InList {
            expr,
            list,
            negated,
        } if expression_is_alias(expr, alias) => {
            let values = list
                .iter()
                .map(literal_expression)
                .collect::<Option<Vec<_>>>()?;
            Some(ValueDomain::set(
                if *negated {
                    SetMode::Exclude
                } else {
                    SetMode::Include
                },
                values,
            ))
        }
        _ => None,
    }
}

pub(super) fn expression_is_alias(expression: &Expr, alias: &str) -> bool {
    matches!(expression, Expr::Identifier(identifier) if identifier.value == alias)
}

pub(super) fn literal_expression(expression: &Expr) -> Option<LiteralExpression> {
    let mut diagnostics = Vec::new();
    match analyze_expression_with_windows(expression, &[], &mut diagnostics) {
        Expression::Literal(literal) => Some(literal),
        _ => None,
    }
}

pub(super) fn literal_domain_for_comparison(
    expression: &Expr,
    operator: ComparisonOperator,
) -> Option<ValueDomain> {
    let literal = literal_expression(expression)?;
    match operator {
        ComparisonOperator::Eq => Some(ValueDomain::set(SetMode::Include, vec![literal])),
        ComparisonOperator::Neq => Some(ValueDomain::set(SetMode::Exclude, vec![literal])),
        ComparisonOperator::Gt => Some(ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(literal, false)),
            None,
        )])),
        ComparisonOperator::Gte => Some(ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(literal, true)),
            None,
        )])),
        ComparisonOperator::Lt => Some(ValueDomain::ranges(vec![ValueRange::new(
            None,
            Some(Bound::new(literal, false)),
        )])),
        ComparisonOperator::Lte => Some(ValueDomain::ranges(vec![ValueRange::new(
            None,
            Some(Bound::new(literal, true)),
        )])),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => None,
    }
}
