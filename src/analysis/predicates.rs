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
        Expr::Like {
            negated,
            any: false,
            expr,
            pattern,
            escape_char,
        } if safe_escaped_underscore_like_prefix(pattern, escape_char.as_ref()).is_some() => {
            let prefix = safe_escaped_underscore_like_prefix(pattern, escape_char.as_ref())
                .expect("guarded explicitly escaped underscore LIKE prefix");
            Predicate::LikePrefix(LikePrefixPredicate::new(
                analyze_predicate_expression(
                    expr,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                prefix,
                *negated,
            ))
        }
        Expr::Like {
            negated,
            any: false,
            expr,
            pattern,
            escape_char,
        } if safe_like_exact(pattern, escape_char.as_ref()).is_some() => {
            let literal = safe_like_exact(pattern, escape_char.as_ref())
                .expect("guarded wildcard-free LIKE literal");
            normalize_comparison(
                analyze_predicate_expression(
                    expr,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                if *negated {
                    ComparisonOperator::Neq
                } else {
                    ComparisonOperator::Eq
                },
                Expression::Literal(LiteralExpression::new(
                    LiteralType::String,
                    LiteralValue::Text(literal),
                )),
            )
        }
        Expr::IsNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_predicate_expression(inner, named_windows, output_aliases, scope, diagnostics),
            false,
        )),
        Expr::IsNotNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_predicate_expression(inner, named_windows, output_aliases, scope, diagnostics),
            true,
        )),
        // At a WHERE/HAVING predicate seam, E IS TRUE admits exactly the
        // rows for which E evaluates TRUE; E IS FALSE admits exactly the
        // rows for which NOT E evaluates TRUE. Both reject UNKNOWN, while the
        // NOT TRUE/rejected source direction still includes UNKNOWN.
        // Preserve the older scalar Boolean-column comparison contract:
        // only computed Boolean predicate trees use this transformation.
        // An ordinary SQL comparison with a literal NULL is UNKNOWN for
        // every input row in the standard three-valued model. Consequently,
        // (a = NULL) IS UNKNOWN is TRUE for every row, including NULL a.
        // Do not generalize this law to NULL-safe comparison operators or
        // engine-specific session behaviors.
        Expr::IsUnknown(inner) if is_null_rejecting_literal_comparison(inner) => {
            Predicate::BooleanExpression(Expression::Literal(LiteralExpression::new(
                LiteralType::Boolean,
                LiteralValue::Boolean(true),
            )))
        }
        Expr::IsTrue(inner) if !is_plain_boolean_column(inner) => {
            analyze_predicate_with_windows(inner, named_windows, output_aliases, scope, diagnostics)
        }
        Expr::IsFalse(inner) if !is_plain_boolean_column(inner) => {
            Predicate::Not(NotPredicate::new(analyze_predicate_with_windows(
                inner,
                named_windows,
                output_aliases,
                scope,
                diagnostics,
            )))
        }
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
        } => {
            let operand = analyze_predicate_with_windows(
                expr,
                named_windows,
                output_aliases,
                scope,
                diagnostics,
            );
            // A validated DATE threshold has an order-preserving scalar
            // complement. Negating the operator keeps SQL NULL UNKNOWN and
            // allows the existing calendar-domain contract to own this fact.
            if let Predicate::Comparison(comparison) = &operand {
                if matches!(comparison.left(), Expression::Column(_))
                    && matches!(comparison.right(), Expression::Literal(literal) if literal.literal_type() == LiteralType::Date)
                {
                    return normalize_comparison(
                        comparison.left().clone(),
                        crate::boolean_witness::negated_comparison_operator(comparison.operator()),
                        comparison.right().clone(),
                    );
                }
            }
            Predicate::Not(NotPredicate::new(operand))
        }
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
pub(super) fn is_null_rejecting_literal_comparison(expression: &Expr) -> bool {
    let expression = match expression {
        Expr::Nested(inner) => inner.as_ref(),
        other => other,
    };
    let Expr::BinaryOp { left, op, right } = expression else {
        return false;
    };
    let ordinary = matches!(
        op,
        SqlBinaryOperator::Eq
            | SqlBinaryOperator::NotEq
            | SqlBinaryOperator::Lt
            | SqlBinaryOperator::LtEq
            | SqlBinaryOperator::Gt
            | SqlBinaryOperator::GtEq
    );
    let is_null_literal = |expression: &Expr| {
        matches!(
            expression,
            Expr::Value(value) if matches!(&value.value, sqlparser::ast::Value::Null)
        )
    };
    ordinary && (is_null_literal(left) || is_null_literal(right))
}

fn safe_escaped_underscore_like_prefix(
    pattern: &Expr,
    escape_char: Option<&sqlparser::ast::Value>,
) -> Option<String> {
    let Some(sqlparser::ast::Value::SingleQuotedString(escape)) = escape_char else {
        return None;
    };
    if escape != "!" {
        return None;
    }
    let Expr::Value(value) = pattern else {
        return None;
    };
    let sqlparser::ast::Value::SingleQuotedString(text) = &value.value else {
        return None;
    };
    let body = text.strip_suffix('%')?;
    let mut chars = body.chars();
    let mut prefix = String::new();
    let mut found_escape = false;
    while let Some(character) = chars.next() {
        if character == '!' {
            if chars.next()? != '_' {
                return None;
            }
            prefix.push('_');
            found_escape = true;
        } else if character.is_ascii_alphanumeric() {
            prefix.push(character);
        } else {
            return None;
        }
    }
    (found_escape && !prefix.is_empty()).then_some(prefix)
}

fn safe_like_prefix(pattern: &Expr) -> Option<String> {
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
pub(super) fn safe_like_escape(prefix: &str, escape_char: Option<&sqlparser::ast::Value>) -> bool {
    match escape_char {
        None => true,
        Some(sqlparser::ast::Value::SingleQuotedString(character)) => {
            character.chars().count() == 1 && character != "%" && !prefix.contains(character)
        }
        _ => false,
    }
}

// A wildcard-free LIKE pattern is a portable equality only when no
// dialect can read a character as a wildcard, character class or escape.
// Standard wildcards (`%`, `_`), T-SQL classes (`[`, `]`), the MySQL
// implicit backslash escape and non-ASCII tokenization are all rejected,
// mirroring the ASCII-alphanumeric law of safe LIKE prefixes. An explicit
// ESCAPE is inert exactly when its single character never occurs in the
// pattern, so it escapes nothing. Unlike a safe prefix, a wildcard-free
// pattern cannot contain the `%` wildcard, so `ESCAPE '%'` is inert here.
pub(super) fn safe_like_exact(
    pattern: &Expr,
    escape_char: Option<&sqlparser::ast::Value>,
) -> Option<String> {
    let Expr::Value(value) = pattern else {
        return None;
    };
    let sqlparser::ast::Value::SingleQuotedString(text) = &value.value else {
        return None;
    };
    if text.contains('%')
        || text.contains('_')
        || text.contains('[')
        || text.contains(']')
        || text.contains('\\')
        || !text.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return None;
    }
    let inert = match escape_char {
        None => true,
        Some(sqlparser::ast::Value::SingleQuotedString(character)) => {
            character.chars().count() == 1 && !text.contains(character.as_str())
        }
        _ => false,
    };
    if !inert {
        return None;
    }
    Some(text.clone())
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
            SqlBinaryOperator::Multiply => Some(BinaryOperator::Multiply),
            SqlBinaryOperator::Divide => Some(BinaryOperator::Divide),
            SqlBinaryOperator::Modulo => Some(BinaryOperator::Modulo),
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

    if let Expr::UnaryOp { op, expr } = expression {
        let operator = match op {
            SqlUnaryOperator::Plus => Some(UnaryOperator::Plus),
            SqlUnaryOperator::Minus => Some(UnaryOperator::Minus),
            _ => None,
        };
        if let Some(operator) = operator {
            return Expression::Unary(UnaryExpression::new(
                operator,
                analyze_predicate_expression(
                    expr,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
            ));
        }
    }

    if let Expr::Cast {
        kind,
        expr,
        data_type,
        format: None,
    } = expression
    {
        let operand =
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics);
        if let Some(cast) = normalize_signed_integer_cast(kind, data_type, operand) {
            return cast;
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
        // MySQL `<=>` is a two-valued null-safe equality: TRUE when both
        // sides are NULL or equal, FALSE otherwise. It coincides with
        // `IS NOT DISTINCT FROM` and inherits the same typed proof laws:
        // exact for catalog-typed integers and for attested binary strings,
        // residual everywhere else. No vendor default is assumed.
        SqlBinaryOperator::Spaceship => Some(ComparisonOperator::IsNotDistinctFrom),
        _ => None,
    }
}
