//! Relational join predicates, keys and join-kind analysis.

use super::*;

pub(super) fn analyze_join(
    join: &SqlJoin,
    left: &RelationRef,
    right: &RelationRef,
    scope: &RelationAnalysisScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) -> ProtocolJoin {
    if join.global {
        diagnostics.push(warning(
            "unsupported_global_join",
            DiagnosticArea::Join,
            "GLOBAL join modifier semantics are not implemented",
        ));
    }

    let (kind, constraint, exact_kind) = analyze_join_operator(&join.join_operator);

    if !exact_kind {
        diagnostics.push(warning(
            "unsupported_join_form",
            DiagnosticArea::Join,
            "join kind is parsed but is not represented precisely by protocol v0",
        ));
    }

    if let Some(JoinConstraint::On(expression)) = constraint {
        collect_expression_dependencies(
            expression,
            scope.local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }

    let condition = constraint.and_then(|constraint| {
        analyze_join_constraint(constraint, left, right, scope.output_scope, diagnostics)
    });

    ProtocolJoin::new(kind, left.clone(), right.clone(), condition)
}

pub(super) fn analyze_join_operator(operator: &JoinOperator) -> (JoinKind, Option<&JoinConstraint>, bool) {
    match operator {
        JoinOperator::Join(constraint) | JoinOperator::Inner(constraint) => {
            (JoinKind::Inner, Some(constraint), true)
        }
        JoinOperator::Left(constraint) | JoinOperator::LeftOuter(constraint) => {
            (JoinKind::Left, Some(constraint), true)
        }
        JoinOperator::Right(constraint) | JoinOperator::RightOuter(constraint) => {
            (JoinKind::Right, Some(constraint), true)
        }
        JoinOperator::FullOuter(constraint) => (JoinKind::Full, Some(constraint), true),
        JoinOperator::CrossJoin => (JoinKind::Cross, None, true),
        JoinOperator::Semi(constraint) | JoinOperator::LeftSemi(constraint) => {
            (JoinKind::LeftSemi, Some(constraint), true)
        }
        JoinOperator::RightSemi(constraint) => (JoinKind::RightSemi, Some(constraint), true),
        JoinOperator::Anti(constraint) | JoinOperator::LeftAnti(constraint) => {
            (JoinKind::LeftAnti, Some(constraint), true)
        }
        JoinOperator::RightAnti(constraint) => (JoinKind::RightAnti, Some(constraint), true),
        JoinOperator::StraightJoin(constraint) => (JoinKind::Unknown, Some(constraint), false),
        JoinOperator::AsOf { constraint, .. } => (JoinKind::Unknown, Some(constraint), false),
        JoinOperator::CrossApply | JoinOperator::OuterApply => (JoinKind::Unknown, None, false),
    }
}

pub(super) fn analyze_join_constraint(
    constraint: &JoinConstraint,
    left: &RelationRef,
    right: &RelationRef,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Predicate> {
    let predicate = match constraint {
        JoinConstraint::On(expression) => Some(analyze_predicate_with_windows(
            expression,
            &[],
            &BTreeMap::new(),
            scope,
            diagnostics,
        )),
        JoinConstraint::Using(columns) => {
            let left_name = left.alias().unwrap_or(left.relation()).to_string();
            let right_name = right.alias().unwrap_or(right.relation()).to_string();
            let predicates = columns.iter().map(|column| {
                Predicate::Comparison(ComparisonPredicate::new(
                    Expression::Column(ColumnExpression::new(
                        Some(left_name.clone()),
                        column.to_string(),
                    )),
                    ComparisonOperator::Eq,
                    Expression::Column(ColumnExpression::new(
                        Some(right_name.clone()),
                        column.to_string(),
                    )),
                ))
            });
            combine_conjunction(predicates)
        }
        JoinConstraint::Natural => {
            diagnostics.push(warning(
                "unsupported_natural_join_condition",
                DiagnosticArea::Join,
                "NATURAL JOIN columns cannot be resolved without source schema information",
            ));
            None
        }
        JoinConstraint::None => None,
    };

    // Self-join aliases are distinct input instances. Remapping both sides to the same
    // physical relation would erase that identity before composition.
    predicate.map(|predicate| {
        if left.relation() == right.relation() && left.alias() != right.alias() {
            predicate
        } else {
            remap_join_equality_columns(predicate, scope, diagnostics)
        }
    })
}

pub(super) fn remap_join_equality_columns(
    predicate: Predicate,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicate {
    match predicate {
        Predicate::Comparison(comparison) if comparison.operator() == ComparisonOperator::Eq => {
            Predicate::Comparison(ComparisonPredicate::new(
                remap_join_equality_expression(comparison.left(), scope, diagnostics),
                comparison.operator(),
                remap_join_equality_expression(comparison.right(), scope, diagnostics),
            ))
        }
        Predicate::And(logical) => {
            remap_join_logical_predicate(logical.operands(), true, scope, diagnostics)
        }
        Predicate::Or(logical) => {
            remap_join_logical_predicate(logical.operands(), false, scope, diagnostics)
        }
        Predicate::Not(not) => Predicate::Not(NotPredicate::new(remap_join_equality_columns(
            not.operand().clone(),
            scope,
            diagnostics,
        ))),
        other => other,
    }
}

pub(super) fn remap_join_logical_predicate(
    operands: &[Predicate],
    conjunction: bool,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicate {
    let mut mapped = operands
        .iter()
        .cloned()
        .map(|predicate| remap_join_equality_columns(predicate, scope, diagnostics));
    let Some(first) = mapped.next() else {
        return Predicate::Unknown(UnknownSemantic::new(
            "join logical predicate has no operands".to_string(),
        ));
    };

    mapped.fold(first, |left, right| {
        if conjunction {
            Predicate::And(LogicalPredicate::pair(left, right))
        } else {
            Predicate::Or(LogicalPredicate::pair(left, right))
        }
    })
}

pub(super) fn remap_join_equality_expression(
    expression: &Expression,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let Expression::Column(column) = expression else {
        return expression.clone();
    };

    let column_ref = ColumnRef::new(
        column.relation().map(str::to_string),
        column.name().to_string(),
    );
    match resolve_plain_source_column(&column_ref, scope) {
        Ok(source) => Expression::Column(ColumnExpression::new(
            Some(source.relation().to_string()),
            source.column().to_string(),
        )),
        Err(reason) => {
            let reason = format!("join equality column {reason}");
            let diagnostic = warning(
                "unresolved_join_column_lineage",
                DiagnosticArea::Join,
                &reason,
            );
            if !diagnostics.contains(&diagnostic) {
                diagnostics.push(diagnostic);
            }
            Expression::Unknown(UnknownSemantic::new(reason))
        }
    }
}

pub(super) fn combine_conjunction(predicates: impl IntoIterator<Item = Predicate>) -> Option<Predicate> {
    let mut predicates = predicates.into_iter();
    let first = predicates.next()?;
    Some(predicates.fold(first, |left, right| {
        Predicate::And(LogicalPredicate::pair(left, right))
    }))
}
