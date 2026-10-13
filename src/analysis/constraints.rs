//! SQL DDL primary, foreign, uniqueness and check constraints.

use super::*;
use crate::column_sources::{ColumnValueKind, ColumnValueSource, GeneratedValueExpression};
use crate::computed_boolean_checks::{
    ComputedBooleanCheckConstraint, ComputedBooleanCheckPredicate,
};
use crate::computed_checks::ComputedCheckConstraint;
use crate::computed_keys::ComputedKeyConstraint;

/// Normalize a complete CREATE TABLE declaration into canonical typed schema evidence.
/// Unknown/unsupported declarations must not produce a partial authoritative schema.
pub(super) fn analyze_create_table_schema(
    create_table: &SqlCreateTable,
    dialect_name: &str,
) -> Result<Option<crate::relation::RelationSchema>, ConstraintDiagnostic> {
    if create_table.columns.is_empty() {
        return Ok(None);
    }
    let relation = create_table.name.to_string();
    let mut columns = Vec::with_capacity(create_table.columns.len());
    for declared in &create_table.columns {
        let mut column = SchemaColumn::from_sql_type(
            declared.name.to_string(),
            &declared.data_type.to_string(),
            dialect_name,
        )
        .map_err(|error| {
            ConstraintDiagnostic::new(
                "unsupported_declared_type",
                format!(
                    "SQL DDL '{}.{}' cannot normalize datatype: {error}",
                    relation, declared.name
                ),
            )
        })?;
        for option in &declared.options {
            if let ColumnOption::Collation(collation) = &option.option {
                column = column
                    .with_collation(collation.to_string())
                    .map_err(|error| {
                        ConstraintDiagnostic::new(
                            "unsupported_declared_collation",
                            format!(
                                "SQL DDL '{}.{}' has invalid collation: {error}",
                                relation, declared.name
                            ),
                        )
                    })?;
            }
        }
        columns.push(column);
    }
    let schema =
        crate::relation::RelationSchema::new(relation.clone(), columns).map_err(|error| {
            ConstraintDiagnostic::new(
                "invalid_ddl_schema",
                format!("SQL DDL '{relation}' has invalid columns: {error}"),
            )
        })?;
    Ok(Some(schema.with_source_kind(
        crate::relation::SchemaSourceKind::SqlDdl,
    )))
}

pub(super) fn analyze_relation_constraints(
    statement: &SqlStatement,
) -> Result<Option<RelationConstraintSet>, ConstraintMetadataError> {
    let SqlStatement::CreateTable(create_table) = statement else {
        return Ok(None);
    };
    analyze_create_table_constraints(create_table)
}

pub(super) fn analyze_create_table_constraints(
    create_table: &SqlCreateTable,
) -> Result<Option<RelationConstraintSet>, ConstraintMetadataError> {
    let relation = create_table.name.to_string();
    let mut constraints = Vec::new();
    let mut diagnostics = Vec::new();
    let mut column_value_sources = Vec::new();
    let mut computed_keys = Vec::new();
    let mut computed_checks = Vec::new();
    let mut computed_boolean_checks = Vec::new();

    for (index, constraint) in create_table.constraints.iter().enumerate() {
        match constraint {
            TableConstraint::PrimaryKey {
                name,
                index_name,
                columns,
                characteristics,
                ..
            } => match index_column_names(columns) {
                Some(columns) => constraints.push(RelationConstraint::primary_key(
                    columns,
                    vec![sql_constraint_evidence(
                        &relation,
                        constraint_identity(
                            "primary_key",
                            index,
                            name.as_ref()
                                .map(ToString::to_string)
                                .or_else(|| index_name.as_ref().map(ToString::to_string)),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?),
                None => {
                    if let Some(expressions) = index_computed_key_exprs(columns) {
                        computed_keys.push(ComputedKeyConstraint::new(
                            expressions,
                            true,
                            vec![sql_constraint_evidence(
                                &relation,
                                constraint_identity(
                                    "primary_key",
                                    index,
                                    name.as_ref()
                                        .map(ToString::to_string)
                                        .or_else(|| index_name.as_ref().map(ToString::to_string)),
                                ),
                                characteristics.as_ref(),
                            )?],
                        )?);
                    } else {
                        diagnostics.push(ConstraintDiagnostic::new(
                            "unsupported_key_expression",
                            format!("primary_key on relation '{relation}' contains an unproved expression"),
                        ));
                    }
                }
            },
            TableConstraint::Unique {
                name,
                index_name,
                columns,
                characteristics,
                ..
            } => {
                match index_column_names(columns) {
                    Some(columns) => constraints.push(RelationConstraint::unique_key(
                        columns,
                        vec![sql_constraint_evidence(
                            &relation,
                            constraint_identity(
                                "unique",
                                index,
                                name.as_ref()
                                    .map(ToString::to_string)
                                    .or_else(|| index_name.as_ref().map(ToString::to_string)),
                            ),
                            characteristics.as_ref(),
                        )?],
                    )?),
                    None => {
                        if let Some(expressions) = index_computed_key_exprs(columns) {
                            computed_keys.push(ComputedKeyConstraint::new(
                                expressions,
                                false,
                                vec![sql_constraint_evidence(
                                    &relation,
                                    constraint_identity(
                                        "unique",
                                        index,
                                        name.as_ref().map(ToString::to_string).or_else(|| {
                                            index_name.as_ref().map(ToString::to_string)
                                        }),
                                    ),
                                    characteristics.as_ref(),
                                )?],
                            )?);
                        } else {
                            diagnostics.push(ConstraintDiagnostic::new(
                            "unsupported_key_expression",
                            format!("unique on relation '{relation}' contains an unproved expression"),
                        ));
                        }
                    }
                }
            }
            TableConstraint::ForeignKey {
                name,
                columns,
                foreign_table,
                referred_columns,
                characteristics,
                ..
            } => {
                if referred_columns.is_empty() {
                    diagnostics.push(ConstraintDiagnostic::new(
                        "unresolved_foreign_key_columns",
                        format!(
                            "foreign key on relation '{relation}' does not declare referenced columns"
                        ),
                    ));
                    continue;
                }
                constraints.push(RelationConstraint::foreign_key(
                    columns.iter().map(ToString::to_string).collect(),
                    foreign_table.to_string(),
                    referred_columns.iter().map(ToString::to_string).collect(),
                    vec![sql_constraint_evidence(
                        &relation,
                        constraint_identity(
                            "foreign_key",
                            index,
                            name.as_ref().map(ToString::to_string),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?);
            }
            TableConstraint::Check {
                name,
                expr,
                enforced,
            } => {
                let evidence = vec![sql_constraint_evidence_with_enforcement(
                    &relation,
                    constraint_identity("check", index, name.as_ref().map(ToString::to_string)),
                    *enforced,
                )?];
                if let Some((rules, checks)) = portable_check_constraints(expr, None, &evidence)? {
                    constraints.extend(rules);
                    computed_checks.extend(checks);
                } else if let Some(predicate) = portable_boolean_check(expr, None, &evidence)? {
                    computed_boolean_checks.push(predicate);
                } else {
                    diagnostics.push(ConstraintDiagnostic::new(
                        "unsupported_check_constraint",
                        format!(
                            "check constraint on '{relation}' has unproved expression semantics"
                        ),
                    ));
                }
            }
            _ => {}
        }
    }

    for column in &create_table.columns {
        // Explicit SQL NULL permits an omitted column to receive SQL NULL,
        // unless a separate DEFAULT or database-generated source takes precedence.
        let has_value_source = column.options.iter().any(|option| {
            matches!(
                &option.option,
                ColumnOption::Default(_)
                    | ColumnOption::Generated { .. }
                    | ColumnOption::Identity(_)
            )
        });
        for (option_index, option) in column.options.iter().enumerate() {
            match &option.option {
                ColumnOption::Unique {
                    is_primary,
                    characteristics,
                } => {
                    let source_id = format!(
                        "{}:column:{}:{}:{}",
                        relation,
                        column.name,
                        if *is_primary { "primary_key" } else { "unique" },
                        option_index
                    );
                    let evidence = vec![sql_constraint_evidence(
                        &relation,
                        source_id,
                        characteristics.as_ref(),
                    )?];
                    let constraint = if *is_primary {
                        RelationConstraint::primary_key(vec![column.name.to_string()], evidence)?
                    } else {
                        RelationConstraint::unique_key(vec![column.name.to_string()], evidence)?
                    };
                    constraints.push(constraint);
                }
                ColumnOption::ForeignKey {
                    foreign_table,
                    referred_columns,
                    characteristics,
                    ..
                } => {
                    if referred_columns.is_empty() {
                        diagnostics.push(ConstraintDiagnostic::new(
                            "unresolved_foreign_key_columns",
                            format!(
                                "foreign key on '{}.{}' does not declare referenced columns",
                                relation, column.name
                            ),
                        ));
                        continue;
                    }
                    constraints.push(RelationConstraint::foreign_key(
                        vec![column.name.to_string()],
                        foreign_table.to_string(),
                        referred_columns.iter().map(ToString::to_string).collect(),
                        vec![sql_constraint_evidence(
                            &relation,
                            format!(
                                "{}:column:{}:foreign_key:{}",
                                relation, column.name, option_index
                            ),
                            characteristics.as_ref(),
                        )?],
                    )?);
                }
                ColumnOption::Default(expression) => {
                    let value =
                        constraint_value_from_expression(expression).map(|(value, _)| value);
                    let evidence = sql_constraint_evidence(
                        &relation,
                        format!(
                            "{}:column:{}:default:{}",
                            relation, column.name, option_index
                        ),
                        None,
                    )?;
                    column_value_sources.push(ColumnValueSource::new(
                        column.name.to_string(),
                        ColumnValueKind::Default,
                        value,
                        vec![evidence],
                    )?);
                }
                ColumnOption::Generated {
                    generation_expr, ..
                } => {
                    // A sequence-backed GENERATED AS IDENTITY declaration is not a
                    // computed column expression. Both require independent laws.
                    let kind = if generation_expr.is_some() {
                        ColumnValueKind::Generated
                    } else {
                        ColumnValueKind::Identity
                    };
                    let evidence = sql_constraint_evidence(
                        &relation,
                        format!(
                            "{}:column:{}:{}:{}",
                            relation,
                            column.name,
                            kind.as_str(),
                            option_index
                        ),
                        None,
                    )?;
                    let source = ColumnValueSource::new(
                        column.name.to_string(),
                        kind,
                        None,
                        vec![evidence],
                    )?;
                    let source = match generation_expr
                        .as_ref()
                        .and_then(parse_generated_expression)
                    {
                        Some(expression) => source.with_generated_expression(expression)?,
                        None => source,
                    };
                    column_value_sources.push(source);
                }
                ColumnOption::Identity(_) => {
                    let evidence = sql_constraint_evidence(
                        &relation,
                        format!(
                            "{}:column:{}:identity:{}",
                            relation, column.name, option_index
                        ),
                        None,
                    )?;
                    column_value_sources.push(ColumnValueSource::new(
                        column.name.to_string(),
                        ColumnValueKind::Identity,
                        None,
                        vec![evidence],
                    )?);
                }
                ColumnOption::Null if !has_value_source => {
                    column_value_sources.push(ColumnValueSource::new(
                        column.name.to_string(),
                        ColumnValueKind::Default,
                        Some(ConstraintValue::Null),
                        vec![sql_constraint_evidence(
                            &relation,
                            format!(
                                "{}:column:{}:explicit_nullable_null:{}",
                                relation, column.name, option_index
                            ),
                            None,
                        )?],
                    )?);
                }
                ColumnOption::NotNull => {
                    constraints.push(RelationConstraint::not_null(
                        column.name.to_string(),
                        vec![sql_constraint_evidence(
                            &relation,
                            format!(
                                "{}:column:{}:not_null:{}",
                                relation, column.name, option_index
                            ),
                            None,
                        )?],
                    )?);
                }
                ColumnOption::Check(expr) => {
                    let evidence = vec![sql_constraint_evidence(
                        &relation,
                        format!("{}:column:{}:check:{}", relation, column.name, option_index),
                        None,
                    )?];
                    if let Some((rules, checks)) = portable_check_constraints(
                        expr,
                        Some(column.name.value.as_str()),
                        &evidence,
                    )? {
                        constraints.extend(rules);
                        computed_checks.extend(checks);
                    } else if let Some(predicate) =
                        portable_boolean_check(expr, Some(column.name.value.as_str()), &evidence)?
                    {
                        computed_boolean_checks.push(predicate);
                    } else {
                        diagnostics.push(ConstraintDiagnostic::new(
                            "unsupported_check_constraint",
                            format!(
                                "check constraint on '{}.{}' has unproved expression semantics",
                                relation, column.name
                            ),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(primary_key) = create_table.primary_key.as_deref() {
        match expression_key_columns(primary_key) {
            Some(columns) => constraints.push(RelationConstraint::primary_key(
                columns,
                vec![sql_constraint_evidence(
                    &relation,
                    format!("{relation}:clickhouse_primary_key"),
                    None,
                )?],
            )?),
            None => diagnostics.push(ConstraintDiagnostic::new(
                "unsupported_key_expression",
                format!("primary key on relation '{relation}' contains a non-column expression"),
            )),
        }
    }

    if constraints.is_empty()
        && computed_keys.is_empty()
        && computed_checks.is_empty()
        && computed_boolean_checks.is_empty()
        && diagnostics.is_empty()
        && column_value_sources.is_empty()
    {
        return Ok(None);
    }

    let mut set = RelationConstraintSet::new(relation, constraints)?
        .with_column_value_sources(column_value_sources)
        .with_computed_keys(computed_keys)
        .with_computed_checks(computed_checks)
        .with_computed_boolean_checks(computed_boolean_checks);
    for diagnostic in diagnostics {
        set.add_diagnostic(diagnostic);
    }
    Ok(Some(set))
}

/// Canonical CHECK semantics that may be supplied by a metadata adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PortableCheckExpression {
    /// A single canonical column compared against a typed scalar constant.
    Scalar(
        String,
        crate::constraints::CheckCompareOperator,
        ConstraintValue,
    ),
    /// A comparison of deterministic scalar expressions over the current row.
    Computed(
        GeneratedValueExpression,
        crate::constraints::CheckCompareOperator,
        GeneratedValueExpression,
    ),
    /// A complete same-column equality disjunction, never a partial OR proof.
    AcceptedValues(String, Vec<ConstraintValue>, bool),
    /// A complete portable SQL three-valued Boolean CHECK predicate.
    Boolean(ComputedBooleanCheckPredicate),
}

/// Normalize metadata-authored CHECK SQL once at the producer boundary.
///
/// Only deterministic row-local comparisons, same-column equality disjunctions
/// and non-NULL IN lists have canonical proof laws. Dialect functions, casts,
/// side effects and arbitrary Boolean combinations remain explicit residuals.
/// Normalize one CHECK into an all-or-nothing set of portable conjuncts.
///
/// No fragment of an unsupported AND/OR predicate may be certified in
/// isolation, since that would discard a target-side obligation.
/// A non-negated BETWEEN has two independent, simultaneous lower and upper
/// scalar bounds. Negated BETWEEN is a disjunction and is not equivalent to
/// their conjunction; never silently admit it.
fn portable_between_bounds(
    expr: &Expr,
    expected_column: Option<&str>,
) -> Option<
    [(
        String,
        crate::constraints::CheckCompareOperator,
        ConstraintValue,
    ); 2],
> {
    let Expr::Between {
        expr: value,
        negated: false,
        low,
        high,
    } = unwrap_nested_expression(expr)
    else {
        return None;
    };
    let lower = Expr::BinaryOp {
        left: value.clone(),
        op: SqlBinaryOperator::GtEq,
        right: low.clone(),
    };
    let upper = Expr::BinaryOp {
        left: value.clone(),
        op: SqlBinaryOperator::LtEq,
        right: high.clone(),
    };
    Some([
        comparison_from_check(&lower, expected_column)?,
        comparison_from_check(&upper, expected_column)?,
    ])
}

/// Normalize a disjunction only when every alternative is a same-column
/// equality with a known non-NULL literal. SQL CHECK admits UNKNOWN, so an
/// explicit NULL alternative must never become a restrictive IN constraint.
fn portable_equality_disjunction(
    expr: &Expr,
    expected_column: Option<&str>,
) -> Option<(String, Vec<ConstraintValue>, bool)> {
    fn collect(expr: &Expr, entries: &mut Vec<(String, ConstraintValue, bool)>) -> Option<()> {
        match unwrap_nested_expression(expr) {
            Expr::BinaryOp {
                left,
                op: SqlBinaryOperator::Or,
                right,
            } => {
                collect(left, entries)?;
                collect(right, entries)
            }
            Expr::BinaryOp {
                left,
                op: SqlBinaryOperator::Eq,
                right,
            } => {
                let (column, constant) = if let Some(column) = check_column_name(left) {
                    (column, right.as_ref())
                } else {
                    (check_column_name(right)?, left.as_ref())
                };
                let (value, quote) = constraint_value_from_expression(constant)?;
                if matches!(value, ConstraintValue::Null) {
                    return None;
                }
                entries.push((column.to_string(), value, quote.unwrap_or(false)));
                Some(())
            }
            _ => None,
        }
    }

    if !matches!(
        unwrap_nested_expression(expr),
        Expr::BinaryOp {
            op: SqlBinaryOperator::Or,
            ..
        }
    ) {
        return None;
    }
    let mut entries = Vec::new();
    collect(expr, &mut entries)?;
    let (first_column, _, first_quote) = entries.first()?;
    let first_column = first_column.clone();
    let first_quote = *first_quote;
    if expected_column.is_some_and(|column| column != first_column.as_str())
        || entries
            .iter()
            .any(|(column, _, quote)| column != &first_column || *quote != first_quote)
    {
        return None;
    }
    Some((
        first_column,
        entries.into_iter().map(|(_, value, _)| value).collect(),
        first_quote,
    ))
}

/// Normalize only source-independent Boolean logic over row-local scalar
/// expressions. Unknown functions, subqueries, casts and dialect constructs
/// abort the entire tree rather than being dropped as partial predicates.
fn portable_boolean_predicate(expr: &Expr) -> Option<ComputedBooleanCheckPredicate> {
    use ComputedBooleanCheckPredicate as P;
    match unwrap_nested_expression(expr) {
        Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::And,
            right,
        } => Some(P::And(vec![
            portable_boolean_predicate(left)?,
            portable_boolean_predicate(right)?,
        ])),
        Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::Or,
            right,
        } => Some(P::Or(vec![
            portable_boolean_predicate(left)?,
            portable_boolean_predicate(right)?,
        ])),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Not,
            expr,
        } => Some(P::Not(Box::new(portable_boolean_predicate(expr)?))),
        Expr::IsNull(expr) => Some(P::IsNull {
            expression: parse_generated_expression(expr)?,
            negated: false,
        }),
        Expr::IsNotNull(expr) => Some(P::IsNull {
            expression: parse_generated_expression(expr)?,
            negated: true,
        }),
        expression => {
            let (left, operator, right) = computed_check_from_expr(expression, None)?;
            Some(P::Comparison {
                left,
                operator,
                right,
            })
        }
    }
}

fn portable_boolean_check(
    expr: &Expr,
    expected_column: Option<&str>,
    evidence: &[ConstraintEvidence],
) -> Result<Option<ComputedBooleanCheckConstraint>, ConstraintMetadataError> {
    let Some(predicate) = portable_boolean_predicate(expr) else {
        return Ok(None);
    };
    if expected_column.is_some_and(|column| !predicate.referenced_columns().contains(&column)) {
        return Ok(None);
    }
    Ok(Some(ComputedBooleanCheckConstraint::new(
        predicate,
        evidence.to_vec(),
    )?))
}

type PortableCheckFacts = (Vec<RelationConstraint>, Vec<ComputedCheckConstraint>);

fn portable_check_constraints(
    expr: &Expr,
    expected_column: Option<&str>,
    evidence: &[ConstraintEvidence],
) -> Result<Option<PortableCheckFacts>, ConstraintMetadataError> {
    if let Expr::BinaryOp {
        left,
        op: SqlBinaryOperator::And,
        right,
    } = unwrap_nested_expression(expr)
    {
        let Some((mut constraints, mut computed)) =
            portable_check_constraints(left, expected_column, evidence)?
        else {
            return Ok(None);
        };
        let Some((right_constraints, right_computed)) =
            portable_check_constraints(right, expected_column, evidence)?
        else {
            return Ok(None);
        };
        constraints.extend(right_constraints);
        computed.extend(right_computed);
        return Ok(Some((constraints, computed)));
    }
    if let Some(bounds) = portable_between_bounds(expr, expected_column) {
        let rules = bounds
            .into_iter()
            .map(|(column, operator, value)| {
                RelationConstraint::check_comparison(column, operator, value, evidence.to_vec())
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Some((rules, Vec::new())));
    }
    if let Some((column, values, quote)) = portable_equality_disjunction(expr, expected_column) {
        return Ok(Some((
            vec![RelationConstraint::accepted_values(
                column,
                values,
                quote,
                evidence.to_vec(),
            )?],
            Vec::new(),
        )));
    }
    if let Some((column, values, quote)) = accepted_values_from_check(expr, expected_column) {
        return Ok(Some((
            vec![RelationConstraint::accepted_values(
                column,
                values,
                quote,
                evidence.to_vec(),
            )?],
            Vec::new(),
        )));
    }
    if let Some((column, op, value)) = comparison_from_check(expr, expected_column) {
        return Ok(Some((
            vec![RelationConstraint::check_comparison(
                column,
                op,
                value,
                evidence.to_vec(),
            )?],
            Vec::new(),
        )));
    }
    if let Some((left, op, right)) = computed_check_from_expr(expr, expected_column) {
        return Ok(Some((
            Vec::new(),
            vec![ComputedCheckConstraint::new(
                left,
                op,
                right,
                evidence.to_vec(),
            )?],
        )));
    }
    Ok(None)
}

pub(crate) fn portable_check_expression(expression: &str) -> Option<PortableCheckExpression> {
    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;
    let statements =
        Parser::parse_sql(&GenericDialect {}, &format!("SELECT 1 WHERE {expression}")).ok()?;
    let [SqlStatement::Query(query)] = statements.as_slice() else {
        return None;
    };
    let sqlparser::ast::SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let condition = select.selection.as_ref()?;
    if let Some((column, op, value)) = comparison_from_check(condition, None) {
        return Some(PortableCheckExpression::Scalar(column, op, value));
    }
    let (left, op, right) = computed_check_from_expr(condition, None)?;
    Some(PortableCheckExpression::Computed(left, op, right))
}

/// Parse every conjunct of a portable CHECK without discarding unknown terms.
///
/// The scalar parser remains the single authoritative AST normalization for
/// leaves. A nonportable OR, function or implicit cast aborts the entire list.
pub(crate) fn portable_check_expressions(expression: &str) -> Option<Vec<PortableCheckExpression>> {
    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;

    fn visit(expr: &Expr, output: &mut Vec<PortableCheckExpression>) -> Option<()> {
        if let Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::And,
            right,
        } = unwrap_nested_expression(expr)
        {
            visit(left, output)?;
            visit(right, output)?;
        } else if let Some(bounds) = portable_between_bounds(expr, None) {
            for (column, operator, value) in bounds {
                output.push(PortableCheckExpression::Scalar(column, operator, value));
            }
        } else if let Some((column, values, quote)) = portable_equality_disjunction(expr, None)
            .or_else(|| accepted_values_from_check(expr, None))
        {
            output.push(PortableCheckExpression::AcceptedValues(
                column, values, quote,
            ));
        } else if let Some(simple) = portable_check_expression(&expr.to_string()) {
            output.push(simple);
        } else {
            output.push(PortableCheckExpression::Boolean(
                portable_boolean_predicate(expr)?,
            ));
        }
        Some(())
    }

    let statements =
        Parser::parse_sql(&GenericDialect {}, &format!("SELECT 1 WHERE {expression}")).ok()?;
    let [SqlStatement::Query(query)] = statements.as_slice() else {
        return None;
    };
    let sqlparser::ast::SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let mut output = Vec::new();
    visit(select.selection.as_ref()?, &mut output)?;
    Some(output)
}

/// Normalize only portable, deterministic generated integer arithmetic.
fn parse_generated_expression(expr: &Expr) -> Option<GeneratedValueExpression> {
    use GeneratedValueExpression as G;
    match unwrap_nested_expression(expr) {
        Expr::Identifier(identifier) => Some(G::Column(identifier.to_string())),
        Expr::Value(_)
        | Expr::UnaryOp {
            op: SqlUnaryOperator::Minus | SqlUnaryOperator::Plus,
            ..
        } => constraint_value_from_expression(expr).map(|(value, _)| G::Literal(value)),
        Expr::Function(function) => {
            if function.uses_odbc_syntax
                || !matches!(&function.parameters, FunctionArguments::None)
                || function.filter.is_some()
                || function.null_treatment.is_some()
                || function.over.is_some()
                || !function.within_group.is_empty()
            {
                return None;
            }
            let FunctionArguments::List(arguments) = &function.args else {
                return None;
            };
            if !arguments.clauses.is_empty() || arguments.duplicate_treatment.is_some() {
                return None;
            }
            let parsed = arguments
                .args
                .iter()
                .map(|argument| match argument {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => {
                        parse_generated_expression(expr)
                    }
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let name = function.name.to_string();
            if name.eq_ignore_ascii_case("ABS") {
                let [value] = <[G; 1]>::try_from(parsed).ok()?;
                Some(G::Abs(Box::new(value)))
            } else if name.eq_ignore_ascii_case("COALESCE") && parsed.len() >= 2 {
                Some(G::Coalesce(parsed))
            } else {
                None
            }
        }
        Expr::BinaryOp { left, op, right } => {
            let left = Box::new(parse_generated_expression(left)?);
            let right = Box::new(parse_generated_expression(right)?);
            match op {
                SqlBinaryOperator::Plus => Some(G::Add(left, right)),
                SqlBinaryOperator::Minus => Some(G::Subtract(left, right)),
                SqlBinaryOperator::Multiply => Some(G::Multiply(left, right)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn computed_check_from_expr(
    expression: &Expr,
    expected_column: Option<&str>,
) -> Option<(
    GeneratedValueExpression,
    crate::constraints::CheckCompareOperator,
    GeneratedValueExpression,
)> {
    use crate::constraints::CheckCompareOperator;
    let Expr::BinaryOp { left, op, right } = unwrap_nested_expression(expression) else {
        return None;
    };
    let operator = match op {
        SqlBinaryOperator::Eq => CheckCompareOperator::Eq,
        SqlBinaryOperator::NotEq => CheckCompareOperator::NotEq,
        SqlBinaryOperator::Gt => CheckCompareOperator::Gt,
        SqlBinaryOperator::GtEq => CheckCompareOperator::Gte,
        SqlBinaryOperator::Lt => CheckCompareOperator::Lt,
        SqlBinaryOperator::LtEq => CheckCompareOperator::Lte,
        _ => return None,
    };
    let left = parse_generated_expression(left)?;
    let right = parse_generated_expression(right)?;
    if expected_column.is_some_and(|column| {
        !left.referenced_columns().contains(&column)
            && !right.referenced_columns().contains(&column)
    }) {
        return None;
    }
    Some((left, operator, right))
}

/// A portable scalar CHECK comparison, leaving expressions/functions residual.
pub(super) fn comparison_from_check(
    expression: &Expr,
    expected_column: Option<&str>,
) -> Option<(
    String,
    crate::constraints::CheckCompareOperator,
    ConstraintValue,
)> {
    use crate::constraints::CheckCompareOperator;
    let Expr::BinaryOp { left, op, right } = unwrap_nested_expression(expression) else {
        return None;
    };
    let (column, literal, reversed) = if let Some(column) = check_column_name(left) {
        (column, constraint_value_from_expression(right)?.0, false)
    } else {
        (
            check_column_name(right)?,
            constraint_value_from_expression(left)?.0,
            true,
        )
    };
    if expected_column.is_some_and(|expected| expected != column) {
        return None;
    }
    let operator = match (op, reversed) {
        (SqlBinaryOperator::Eq, _) => CheckCompareOperator::Eq,
        (SqlBinaryOperator::NotEq, _) => CheckCompareOperator::NotEq,
        (SqlBinaryOperator::Gt, false) | (SqlBinaryOperator::Lt, true) => CheckCompareOperator::Gt,
        (SqlBinaryOperator::GtEq, false) | (SqlBinaryOperator::LtEq, true) => {
            CheckCompareOperator::Gte
        }
        (SqlBinaryOperator::Lt, false) | (SqlBinaryOperator::Gt, true) => CheckCompareOperator::Lt,
        (SqlBinaryOperator::LtEq, false) | (SqlBinaryOperator::GtEq, true) => {
            CheckCompareOperator::Lte
        }
        _ => return None,
    };
    Some((column.to_string(), operator, literal))
}

pub(super) fn accepted_values_from_check(
    expression: &Expr,
    expected_column: Option<&str>,
) -> Option<(String, Vec<ConstraintValue>, bool)> {
    let Expr::InList {
        expr,
        list,
        negated: false,
    } = unwrap_nested_expression(expression)
    else {
        return None;
    };

    let column = check_column_name(expr)?;
    if expected_column.is_some_and(|expected| expected != column) {
        return None;
    }

    let mut values = Vec::with_capacity(list.len());
    let mut quote = None;
    for expression in list {
        let (value, value_quote) = constraint_value_from_expression(expression)?;
        if let Some(value_quote) = value_quote {
            match quote {
                Some(current) if current != value_quote => return None,
                None => quote = Some(value_quote),
                _ => {}
            }
        }
        // CHECK (x IN (..., NULL)) admits UNKNOWN for an unmatched x.
        // A strict accepted-values domain would incorrectly call that row
        // impossible. Keep the entire CHECK residual instead.
        if matches!(value, ConstraintValue::Null) {
            return None;
        }
        values.push(value);
    }

    Some((column.to_string(), values, quote.unwrap_or(false)))
}

pub(super) fn unwrap_nested_expression(mut expression: &Expr) -> &Expr {
    while let Expr::Nested(inner) = expression {
        expression = inner;
    }
    expression
}

pub(super) fn check_column_name(expression: &Expr) -> Option<&str> {
    match unwrap_nested_expression(expression) {
        Expr::Identifier(identifier) => Some(identifier.value.as_str()),
        Expr::CompoundIdentifier(identifiers) => identifiers
            .last()
            .map(|identifier| identifier.value.as_str()),
        _ => None,
    }
}

pub(super) fn constraint_value_from_expression(
    expression: &Expr,
) -> Option<(ConstraintValue, Option<bool>)> {
    match unwrap_nested_expression(expression) {
        Expr::Value(value) => constraint_value_from_sql_value(&value.value),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Plus,
            expr,
        } => constraint_number_from_expression(expr, false).map(|value| (value, Some(false))),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Minus,
            expr,
        } => constraint_number_from_expression(expr, true).map(|value| (value, Some(false))),
        _ => None,
    }
}

pub(super) fn constraint_value_from_sql_value(
    value: &Value,
) -> Option<(ConstraintValue, Option<bool>)> {
    match value {
        Value::Null => Some((ConstraintValue::Null, None)),
        Value::Boolean(value) => Some((ConstraintValue::Boolean(*value), Some(false))),
        Value::Number(value, _) => constraint_number(value).map(|value| (value, Some(false))),
        value => string_literal_value(value)
            .map(|value| (ConstraintValue::String(value.to_string()), Some(true))),
    }
}

pub(super) fn constraint_number_from_expression(
    expression: &Expr,
    negative: bool,
) -> Option<ConstraintValue> {
    let Expr::Value(value) = unwrap_nested_expression(expression) else {
        return None;
    };
    let Value::Number(value, _) = &value.value else {
        return None;
    };
    if negative {
        constraint_number(&format!("-{value}"))
    } else {
        constraint_number(value)
    }
}

pub(super) fn constraint_number(value: &str) -> Option<ConstraintValue> {
    let number = Number::from_str(value).ok()?;
    if let Some(value) = number.as_i64() {
        Some(ConstraintValue::Integer(value))
    } else if let Some(value) = number.as_u64() {
        Some(ConstraintValue::UnsignedInteger(value))
    } else {
        Some(ConstraintValue::Number(number.to_string()))
    }
}

pub(super) fn sql_constraint_evidence(
    relation: &str,
    source_id: String,
    characteristics: Option<&ConstraintCharacteristics>,
) -> Result<ConstraintEvidence, ConstraintMetadataError> {
    sql_constraint_evidence_with_enforcement(
        relation,
        source_id,
        characteristics.and_then(|value| value.enforced),
    )
}

pub(super) fn sql_constraint_evidence_with_enforcement(
    relation: &str,
    source_id: String,
    enforced: Option<bool>,
) -> Result<ConstraintEvidence, ConstraintMetadataError> {
    let enforcement = match enforced {
        Some(true) => ConstraintEnforcement::Enforced,
        Some(false) => ConstraintEnforcement::NotEnforced,
        None => ConstraintEnforcement::Unknown,
    };
    let provenance = ConstraintProvenance::new(
        ConstraintSourceKind::SqlDdl,
        if source_id.trim().is_empty() {
            format!("create_table:{relation}")
        } else {
            source_id
        },
    )?;
    Ok(ConstraintEvidence::new(provenance, enforcement))
}

pub(super) fn constraint_identity(kind: &str, index: usize, name: Option<String>) -> String {
    match name {
        Some(name) => format!("{kind}:{name}"),
        None => format!("{kind}:{}", index + 1),
    }
}

fn index_computed_key_exprs(columns: &[IndexColumn]) -> Option<Vec<GeneratedValueExpression>> {
    columns
        .iter()
        .map(|column| parse_generated_expression(&column.column.expr))
        .collect()
}

pub(super) fn index_column_names(columns: &[IndexColumn]) -> Option<Vec<String>> {
    columns
        .iter()
        .map(|column| match &column.column.expr {
            Expr::Identifier(identifier) => Some(identifier.to_string()),
            _ => None,
        })
        .collect()
}

pub(super) fn expression_key_columns(expression: &Expr) -> Option<Vec<String>> {
    match expression {
        Expr::Identifier(identifier) => Some(vec![identifier.to_string()]),
        Expr::Tuple(expressions) => expressions
            .iter()
            .map(|expression| match expression {
                Expr::Identifier(identifier) => Some(identifier.to_string()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}
