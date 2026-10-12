//! SQL scalar expression and literal normalization.

use super::*;

pub(super) fn analyze_expression_with_scope(
    expression: &Expr,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    match expression {
        Expr::Subquery(query) => Expression::ScalarSubquery(Box::new(
            ScalarSubqueryExpression::new(analyze_subquery_semantics(query, scope)),
        )),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => analyze_case_expression(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            scope,
            named_windows,
            diagnostics,
        ),
        _ if is_boolean_value_expression(expression) => {
            Expression::BooleanPredicate(Box::new(analyze_predicate_with_windows(
                expression,
                named_windows,
                &BTreeMap::new(),
                scope,
                diagnostics,
            )))
        }
        _ => analyze_expression_with_windows(expression, named_windows, diagnostics),
    }
}

pub(super) fn is_boolean_value_expression(expression: &Expr) -> bool {
    match expression {
        Expr::BinaryOp { op, .. } => {
            comparison_operator(op).is_some()
                || matches!(op, SqlBinaryOperator::And | SqlBinaryOperator::Or)
        }
        Expr::IsDistinctFrom(_, _)
        | Expr::IsNotDistinctFrom(_, _)
        | Expr::IsNull(_)
        | Expr::IsNotNull(_)
        | Expr::IsTrue(_)
        | Expr::IsFalse(_)
        | Expr::IsNotTrue(_)
        | Expr::IsNotFalse(_)
        | Expr::InList { .. }
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Between { .. } => true,
        Expr::Nested(inner) => is_boolean_value_expression(inner),
        _ => false,
    }
}

pub(super) fn analyze_case_expression(
    operand: Option<&Expr>,
    conditions: &[sqlparser::ast::CaseWhen],
    else_result: Option<&Expr>,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let normalized_operand = operand
        .map(|value| analyze_expression_with_scope(value, scope, named_windows, diagnostics));
    let normalized_branches = conditions
        .iter()
        .map(|branch| {
            let condition = if operand.is_some() {
                analyze_expression_with_scope(&branch.condition, scope, named_windows, diagnostics)
            } else {
                Expression::BooleanPredicate(Box::new(analyze_predicate_with_windows(
                    &branch.condition,
                    named_windows,
                    &BTreeMap::new(),
                    scope,
                    diagnostics,
                )))
            };
            let result =
                analyze_expression_with_scope(&branch.result, scope, named_windows, diagnostics);
            (condition, result)
        })
        .collect::<Vec<_>>();
    let branch_predicates = normalized_branches
        .iter()
        .map(|(condition, _)| match &normalized_operand {
            Some(operand) => Predicate::Comparison(ComparisonPredicate::new(
                operand.clone(),
                ComparisonOperator::Eq,
                condition.clone(),
            )),
            None => match condition {
                Expression::BooleanPredicate(predicate) => predicate.as_ref().clone(),
                _ => Predicate::BooleanExpression(condition.clone()),
            },
        })
        .collect::<Vec<_>>();
    let case_sources = case_scope_sources(scope);
    let (branch_source_domains, else_source_domains) =
        derive_case_source_domains(&branch_predicates, &case_sources);
    let branch_source_domains = branch_source_domains
        .into_iter()
        .map(|domains| remap_case_source_domains_to_physical(domains, scope))
        .collect::<Vec<_>>();
    let else_source_domains = remap_case_source_domains_to_physical(else_source_domains, scope);
    let branches = normalized_branches
        .into_iter()
        .zip(branch_source_domains)
        .map(|((condition, result), source_domains)| {
            CaseBranch::new(condition, result, source_domains)
        })
        .collect();
    let normalized_else = else_result
        .map(|value| analyze_expression_with_scope(value, scope, named_windows, diagnostics));

    Expression::Case(CaseExpression::new(
        normalized_operand,
        branches,
        normalized_else,
        else_source_domains,
    ))
}

pub(super) fn case_scope_sources(scope: &[OutputRelation]) -> Vec<SourceRelation> {
    scope
        .iter()
        .filter_map(|relation| {
            relation
                .qualifiers
                .first()
                .map(|qualifier| SourceRelation::new(qualifier.clone(), None))
        })
        .collect()
}

pub(super) fn remap_case_source_domains_to_physical(
    source_domains: CaseSourceDomains,
    scope: &[OutputRelation],
) -> CaseSourceDomains {
    let alternatives = match source_domains {
        CaseSourceDomains::Reachable { alternatives } => alternatives,
        other => return other,
    };

    let mut mapped_alternatives = Vec::new();
    for alternative in alternatives {
        let mut mapped_domains = BTreeMap::<ColumnRef, ValueDomain>::new();
        for column_domain in alternative.column_domains() {
            let source = match resolve_plain_source_column(column_domain.column(), scope) {
                Ok(source) => source,
                Err(reason) => {
                    return CaseSourceDomains::unknown(format!("CASE branch source {reason}"));
                }
            };
            let column = ColumnRef::new(
                Some(source.relation().to_string()),
                source.column().to_string(),
            );
            let domain = column_domain.domain().clone();
            mapped_domains
                .entry(column)
                .and_modify(|existing| {
                    *existing = intersect_case_domain_values(existing, &domain);
                })
                .or_insert(domain);
        }

        if mapped_domains
            .values()
            .any(|domain| matches!(domain, ValueDomain::Empty))
        {
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

pub(super) fn resolve_plain_source_column(
    column: &ColumnRef,
    scope: &[OutputRelation],
) -> Result<LineageSource, String> {
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
                relation,
                columns: Some(columns),
            } if !columns.iter().any(|candidate| candidate == column.name()) => None,
            OutputRelationSource::Physical { relation, .. } => Some(Ok(LineageSource::new(
                relation.clone(),
                column.name().to_string(),
            ))),
            OutputRelationSource::Local(output) => {
                let mut matches = output
                    .columns()
                    .iter()
                    .filter(|candidate| candidate.name() == column.name());
                let candidate = matches.next()?;
                if matches.next().is_some() {
                    return Some(Err(format!(
                        "column {} is ambiguous within the local relation",
                        qualified_column_name(column.relation(), column.name())
                    )));
                }
                Some(candidate.plain_copy_source().cloned().ok_or_else(|| {
                    format!(
                        "column {} is produced by a computed local expression and cannot be mapped safely to physical lineage",
                        qualified_column_name(column.relation(), column.name())
                    )
                }))
            }
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [Ok(source)] => Ok(source.clone()),
        [Err(reason)] => Err(reason.clone()),
        [] => Err(format!(
            "column {} could not be resolved to physical lineage",
            qualified_column_name(column.relation(), column.name())
        )),
        _ => Err(format!(
            "column {} is ambiguous without source schema information",
            qualified_column_name(column.relation(), column.name())
        )),
    }
}

pub(super) fn analyze_expression_with_windows(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    match expression {
        Expr::Identifier(identifier) => {
            Expression::Column(ColumnExpression::new(None, identifier.value.clone()))
        }
        Expr::CompoundIdentifier(identifiers) => analyze_compound_identifier(identifiers),
        Expr::Value(value) => analyze_value(&value.value, expression, diagnostics),
        Expr::TypedString { data_type, value } => analyze_typed_string(
            data_type.to_string().as_str(),
            &value.value,
            expression,
            diagnostics,
        ),
        Expr::Function(function) => {
            analyze_function(function, expression, named_windows, diagnostics)
        }
        Expr::UnaryOp { op, expr } => {
            analyze_unary_expression(op, expr, expression, named_windows, diagnostics)
        }
        Expr::BinaryOp { left, op, right } => {
            analyze_binary_expression(left, op, right, expression, named_windows, diagnostics)
        }
        Expr::Nested(inner) => analyze_expression_with_windows(inner, named_windows, diagnostics),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => analyze_case_expression(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            &[],
            named_windows,
            diagnostics,
        ),
        Expr::Subquery(query) => Expression::ScalarSubquery(Box::new(
            ScalarSubqueryExpression::new(analyze_subquery_semantics(query, &[])),
        )),
        _ => unsupported_expression(
            "expression",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

pub(super) fn analyze_compound_identifier(identifiers: &[sqlparser::ast::Ident]) -> Expression {
    match identifiers.split_last() {
        Some((column, relation_parts)) => {
            let relation = if relation_parts.is_empty() {
                None
            } else {
                Some(
                    relation_parts
                        .iter()
                        .map(|identifier| identifier.value.as_str())
                        .collect::<Vec<_>>()
                        .join("."),
                )
            };
            Expression::Column(ColumnExpression::new(relation, column.value.clone()))
        }
        None => Expression::Unsupported(UnsupportedSemantic::new(
            "column_reference".to_string(),
            Some("empty compound identifier cannot be resolved".to_string()),
        )),
    }
}

pub(super) fn analyze_value(
    value: &Value,
    expression: &Expr,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    match value {
        Value::Number(value, _) => analyze_number(value, expression, diagnostics),
        Value::SingleQuotedString(value)
        | Value::TripleSingleQuotedString(value)
        | Value::TripleDoubleQuotedString(value)
        | Value::EscapedStringLiteral(value)
        | Value::UnicodeStringLiteral(value)
        | Value::SingleQuotedRawStringLiteral(value)
        | Value::DoubleQuotedRawStringLiteral(value)
        | Value::TripleSingleQuotedRawStringLiteral(value)
        | Value::TripleDoubleQuotedRawStringLiteral(value)
        | Value::NationalStringLiteral(value)
        | Value::DoubleQuotedString(value) => Expression::Literal(LiteralExpression::new(
            LiteralType::String,
            LiteralValue::Text(value.clone()),
        )),
        Value::Boolean(value) => Expression::Literal(LiteralExpression::new(
            LiteralType::Boolean,
            LiteralValue::Boolean(*value),
        )),
        Value::Null => Expression::Literal(LiteralExpression::new(
            LiteralType::Null,
            LiteralValue::Null,
        )),
        _ => unsupported_expression(
            "literal",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

pub(super) fn analyze_number(value: &str, expression: &Expr, diagnostics: &mut Vec<Diagnostic>) -> Expression {
    match Number::from_str(value) {
        Ok(number) => {
            let literal_type = if value.contains('.') || value.contains('e') || value.contains('E')
            {
                LiteralType::Decimal
            } else {
                LiteralType::Integer
            };

            Expression::Literal(LiteralExpression::new(
                literal_type,
                LiteralValue::Number(number.to_string()),
            ))
        }
        Err(_) => unsupported_expression(
            "numeric_literal",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

pub(super) fn analyze_typed_string(
    data_type: &str,
    value: &Value,
    expression: &Expr,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let data_type = data_type.to_ascii_uppercase();
    let literal_type = match data_type.as_str() {
        "DATE" => Some(LiteralType::Date),
        data_type
            if data_type.starts_with("TIMESTAMP")
                || matches!(data_type, "TIMESTAMPTZ" | "TIMESTAMP_TZ") =>
        {
            Some(LiteralType::Timestamp)
        }
        data_type if data_type.starts_with("TIME") => Some(LiteralType::Time),
        data_type if data_type.starts_with("INTERVAL") => Some(LiteralType::Interval),
        _ => None,
    };

    match (literal_type, string_literal_value(value)) {
        (Some(literal_type), Some(value)) => Expression::Literal(LiteralExpression::new(
            literal_type,
            LiteralValue::Text(value.to_string()),
        )),
        _ => unsupported_expression(
            "typed_literal",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

pub(super) fn string_literal_value(value: &Value) -> Option<&str> {
    match value {
        Value::SingleQuotedString(value)
        | Value::TripleSingleQuotedString(value)
        | Value::TripleDoubleQuotedString(value)
        | Value::EscapedStringLiteral(value)
        | Value::UnicodeStringLiteral(value)
        | Value::SingleQuotedRawStringLiteral(value)
        | Value::DoubleQuotedRawStringLiteral(value)
        | Value::TripleSingleQuotedRawStringLiteral(value)
        | Value::TripleDoubleQuotedRawStringLiteral(value)
        | Value::NationalStringLiteral(value)
        | Value::DoubleQuotedString(value) => Some(value),
        _ => None,
    }
}
