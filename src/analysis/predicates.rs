//! SQL boolean predicate normalization.

use super::*;

pub(super) fn analyze_predicate(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) -> Predicate {
    analyze_predicate_with_windows(expression, &[], &BTreeMap::new(), &[], diagnostics)
}

pub(super) fn analyze_predicate_with_windows(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    output_aliases: &BTreeMap<String, &Expr>,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicate {
    match expression {
        Expr::Nested(inner) => {
            analyze_predicate_with_windows(inner, named_windows, output_aliases, scope, diagnostics)
        }
        Expr::BinaryOp { left, op, right } => match op {
            SqlBinaryOperator::And => Predicate::And(LogicalPredicate::pair(
                analyze_predicate_with_windows(
                    left,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                analyze_predicate_with_windows(
                    right,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
            )),
            SqlBinaryOperator::Or => Predicate::Or(LogicalPredicate::pair(
                analyze_predicate_with_windows(
                    left,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                analyze_predicate_with_windows(
                    right,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
            )),
            _ => match comparison_operator(op) {
                Some(operator) => normalize_comparison(
                    analyze_predicate_expression(
                        left,
                        named_windows,
                        output_aliases,
                        scope,
                        diagnostics,
                    ),
                    operator,
                    analyze_predicate_expression(
                        right,
                        named_windows,
                        output_aliases,
                        scope,
                        diagnostics,
                    ),
                ),
                None => Predicate::BooleanExpression(analyze_expression_with_scope(
                    expression,
                    scope,
                    named_windows,
                    diagnostics,
                )),
            },
        },
        Expr::IsDistinctFrom(left, right) => normalize_comparison(
            analyze_predicate_expression(left, named_windows, output_aliases, scope, diagnostics),
            ComparisonOperator::IsDistinctFrom,
            analyze_predicate_expression(right, named_windows, output_aliases, scope, diagnostics),
        ),
        Expr::IsNotDistinctFrom(left, right) => normalize_comparison(
            analyze_predicate_expression(left, named_windows, output_aliases, scope, diagnostics),
            ComparisonOperator::IsNotDistinctFrom,
            analyze_predicate_expression(right, named_windows, output_aliases, scope, diagnostics),
        ),
        Expr::Like {
            negated,
            any: false,
            expr,
            pattern,
            escape_char,
        } if safe_like_prefix(pattern)
            .is_some_and(|prefix| safe_like_escape(&prefix, escape_char.as_ref())) =>
        {
            Predicate::LikePrefix(LikePrefixPredicate::new(
                analyze_predicate_expression(
                    expr,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                safe_like_prefix(pattern).expect("guarded LIKE prefix"),
                *negated,
            ))
        }
        Expr::IsNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_predicate_expression(inner, named_windows, output_aliases, scope, diagnostics),
            false,
        )),
        Expr::IsNotNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_predicate_expression(inner, named_windows, output_aliases, scope, diagnostics),
            true,
        )),
        Expr::IsTrue(inner)
        | Expr::IsFalse(inner)
        | Expr::IsNotTrue(inner)
        | Expr::IsNotFalse(inner) => {
            let (value, include_null) = match expression {
                Expr::IsTrue(_) => (true, false),
                Expr::IsFalse(_) => (false, false),
                Expr::IsNotTrue(_) => (true, true),
                Expr::IsNotFalse(_) => (false, true),
                _ => unreachable!("matched boolean truth-test variants"),
            };
            normalize_boolean_test(
                analyze_predicate_expression(
                    inner,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                value,
                include_null,
            )
        }
        Expr::InList {
            expr,
            list,
            negated,
        } => Predicate::In(InPredicate::new(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            list.iter()
                .map(|value| {
                    analyze_predicate_expression(
                        value,
                        named_windows,
                        output_aliases,
                        scope,
                        diagnostics,
                    )
                })
                .collect(),
            *negated,
        )),
        Expr::Exists { subquery, negated } => Predicate::Exists(ExistsPredicate::new(
            analyze_subquery_semantics(subquery, scope),
            *negated,
        )),
        Expr::InSubquery {
            expr,
            subquery,
            negated,
        } => Predicate::InSubquery(InSubqueryPredicate::new(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            analyze_subquery_semantics(subquery, scope),
            *negated,
        )),
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Predicate::Between(BetweenPredicate::new(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            analyze_predicate_expression(low, named_windows, output_aliases, scope, diagnostics),
            analyze_predicate_expression(high, named_windows, output_aliases, scope, diagnostics),
            *negated,
        )),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Not | SqlUnaryOperator::BangNot,
            expr,
        } if is_plain_boolean_column(expr) => normalize_boolean_test(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            false,
            false,
        ),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Not | SqlUnaryOperator::BangNot,
            expr,
        } => Predicate::Not(NotPredicate::new(analyze_predicate_with_windows(
            expr,
            named_windows,
            output_aliases,
            scope,
            diagnostics,
        ))),
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => normalize_boolean_test(
            analyze_predicate_expression(
                expression,
                named_windows,
                output_aliases,
                scope,
                diagnostics,
            ),
            true,
            false,
        ),
        _ => Predicate::BooleanExpression(analyze_predicate_expression(
            expression,
            named_windows,
            output_aliases,
            scope,
            diagnostics,
        )),
    }
}

// Only an unescaped alphabetic/digit prefix and one final wildcard is
// independent of dialect-specific LIKE escape, collation and tokenization rules.
// Non-ASCII and embedded wildcards remain regular unsupported SQL expressions.
pub(super) fn safe_like_prefix(pattern: &Expr) -> Option<String> {
    let Expr::Value(value) = pattern else {
        return None;
    };
    let sqlparser::ast::Value::SingleQuotedString(text) = &value.value else {
        return None;
    };
    let prefix = text.strip_suffix('%')?;
    (!prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .then(|| prefix.to_string())
}

// An explicit ESCAPE character is harmless for a safe trailing-wildcard
// prefix only when it cannot escape any character in the pattern. In
// particular ESCAPE '%' would turn the final wildcard into a literal.
pub(super) fn safe_like_escape(
    prefix: &str,
    escape_char: Option<&sqlparser::ast::Value>,
) -> bool {
    match escape_char {
        None => true,
        Some(sqlparser::ast::Value::SingleQuotedString(character)) => {
            character.chars().count() == 1
                && character != "%"
                && !prefix.contains(character)
        }
        _ => false,
    }
}

pub(super) fn is_plain_boolean_column(expression: &Expr) -> bool {
    match expression {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => true,
        Expr::Nested(inner) => is_plain_boolean_column(inner),
        _ => false,
    }
}

pub(super) fn normalize_boolean_test(
    expression: Expression,
    value: bool,
    include_null: bool,
) -> Predicate {
    normalize_comparison(
        expression,
        if include_null {
            ComparisonOperator::IsDistinctFrom
        } else {
            ComparisonOperator::Eq
        },
        Expression::Literal(LiteralExpression::new(
            LiteralType::Boolean,
            LiteralValue::Boolean(value),
        )),
    )
}

pub(super) fn analyze_predicate_expression(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    output_aliases: &BTreeMap<String, &Expr>,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if let Expr::Identifier(identifier) = expression {
        if let Some(aliased_expression) = output_aliases.get(&identifier.value) {
            return analyze_expression_with_scope(
                aliased_expression,
                scope,
                named_windows,
                diagnostics,
            );
        }
    }

    // Keep the operands of scoped integer arithmetic in the predicate model.
    // In particular, the cast in CAST(a AS BIGINT) + 1 must not be lost
    // when normalizing the enclosing arithmetic expression.
    if let Expr::Nested(inner) = expression {
        return analyze_predicate_expression(
            inner,
            named_windows,
            output_aliases,
            scope,
            diagnostics,
        );
    }
    if let Expr::BinaryOp { left, op, right } = expression {
        let operator = match op {
            SqlBinaryOperator::Plus => Some(BinaryOperator::Add),
            SqlBinaryOperator::Minus => Some(BinaryOperator::Subtract),
            _ => None,
        };
        if let Some(operator) = operator {
            return Expression::Binary(BinaryExpression::new(
                operator,
                analyze_predicate_expression(
                    left,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                analyze_predicate_expression(
                    right,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
            ));
        }
    }

    // Normalize only ordinary, formatting-free signed integer casts in
    // predicates. TRY/SAFE_CAST, string/decimal casts, ambiguous targets and
    // narrowing remain unsupported rather than gaining spurious exactness.
    if let Expr::Cast {
        kind: SqlCastKind::Cast,
        expr,
        data_type,
        format: None,
    } = expression
    {
        use sqlparser::ast::DataType as SqlDataType;
        let bits = match data_type {
            SqlDataType::SmallInt(_) | SqlDataType::Int2(_) | SqlDataType::Int16 => Some(16),
            SqlDataType::Int(_)
            | SqlDataType::Integer(_)
            | SqlDataType::Int4(_)
            | SqlDataType::Int32 => Some(32),
            SqlDataType::BigInt(_) | SqlDataType::Int8(_) | SqlDataType::Int64 => Some(64),
            _ => None,
        };
        if let Some(bits) = bits {
            return Expression::SignedIntegerCast(SignedIntegerCastExpression::new(
                analyze_predicate_expression(
                    expr,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                bits,
            ));
        }
    }

    analyze_expression_with_scope(expression, scope, named_windows, diagnostics)
}

pub(super) fn normalize_comparison(
    left: Expression,
    operator: ComparisonOperator,
    right: Expression,
) -> Predicate {
    if matches!(left, Expression::Literal(_)) && matches!(right, Expression::Column(_)) {
        Predicate::Comparison(ComparisonPredicate::new(right, operator.reversed(), left))
    } else {
        Predicate::Comparison(ComparisonPredicate::new(left, operator, right))
    }
}

pub(super) fn comparison_operator(operator: &SqlBinaryOperator) -> Option<ComparisonOperator> {
    match operator {
        SqlBinaryOperator::Eq => Some(ComparisonOperator::Eq),
        SqlBinaryOperator::NotEq => Some(ComparisonOperator::Neq),
        SqlBinaryOperator::Lt => Some(ComparisonOperator::Lt),
        SqlBinaryOperator::LtEq => Some(ComparisonOperator::Lte),
        SqlBinaryOperator::Gt => Some(ComparisonOperator::Gt),
        SqlBinaryOperator::GtEq => Some(ComparisonOperator::Gte),
        _ => None,
    }
}
