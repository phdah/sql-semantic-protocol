//! SQL DDL primary, foreign, uniqueness and check constraints.

use super::*;
use crate::column_sources::{ColumnValueKind, ColumnValueSource};

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
        ).map_err(|error| ConstraintDiagnostic::new(
            "unsupported_declared_type",
            format!("SQL DDL '{}.{}' cannot normalize datatype: {error}", relation, declared.name),
        ))?;
        for option in &declared.options {
            if let ColumnOption::Collation(collation) = &option.option {
                column = column.with_collation(collation.to_string()).map_err(|error| {
                    ConstraintDiagnostic::new(
                        "unsupported_declared_collation",
                        format!("SQL DDL '{}.{}' has invalid collation: {error}", relation, declared.name),
                    )
                })?;
            }
        }
        columns.push(column);
    }
    let schema = crate::relation::RelationSchema::new(relation.clone(), columns)
        .map_err(|error| ConstraintDiagnostic::new(
            "invalid_ddl_schema",
            format!("SQL DDL '{relation}' has invalid columns: {error}"),
        ))?;
    Ok(Some(schema.with_source_kind(crate::relation::SchemaSourceKind::SqlDdl)))
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
                            name.as_ref().map(ToString::to_string)
                                .or_else(|| index_name.as_ref().map(ToString::to_string)),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?),
                None => diagnostics.push(ConstraintDiagnostic::new(
                    "unsupported_key_expression",
                    format!(
                        "primary key on relation '{relation}' contains a non-column expression"
                    ),
                )),
            },
            TableConstraint::Unique {
                name,
                index_name,
                columns,
                characteristics,
                ..
            } => match index_column_names(columns) {
                Some(columns) => constraints.push(RelationConstraint::unique_key(
                    columns,
                    vec![sql_constraint_evidence(
                        &relation,
                        constraint_identity(
                            "unique",
                            index,
                            name.as_ref().map(ToString::to_string)
                                .or_else(|| index_name.as_ref().map(ToString::to_string)),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?),
                None => diagnostics.push(ConstraintDiagnostic::new(
                    "unsupported_key_expression",
                    format!(
                        "unique constraint on relation '{relation}' contains a non-column expression"
                    ),
                )),
            },
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
            } => match accepted_values_from_check(expr, None) {
                Some((column, values, quote)) => {
                    constraints.push(RelationConstraint::accepted_values(
                        column,
                        values,
                        quote,
                        vec![sql_constraint_evidence_with_enforcement(
                            &relation,
                            constraint_identity(
                                "check",
                                index,
                                name.as_ref().map(ToString::to_string),
                            ),
                            *enforced,
                        )?],
                    )?);
                }
                None => diagnostics.push(ConstraintDiagnostic::new(
                    "unsupported_check_constraint",
                    format!(
                        "check constraint on relation '{relation}' cannot be represented safely as a finite accepted-values constraint"
                    ),
                )),
            },
            _ => {}
        }
    }

    for column in &create_table.columns {
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
                    let value = constraint_value_from_expression(expression).map(|(value, _)| value);
                    let evidence = sql_constraint_evidence(
                        &relation,
                        format!("{}:column:{}:default:{}", relation, column.name, option_index),
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
                    generation_expr,
                    ..
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
                        format!("{}:column:{}:{}:{}", relation, column.name, kind.as_str(), option_index),
                        None,
                    )?;
                    column_value_sources.push(ColumnValueSource::new(
                        column.name.to_string(),
                        kind,
                        None,
                        vec![evidence],
                    )?);
                }
                ColumnOption::Identity(_) => {
                    let evidence = sql_constraint_evidence(
                        &relation,
                        format!("{}:column:{}:identity:{}", relation, column.name, option_index),
                        None,
                    )?;
                    column_value_sources.push(ColumnValueSource::new(
                        column.name.to_string(),
                        ColumnValueKind::Identity,
                        None,
                        vec![evidence],
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
                    match accepted_values_from_check(expr, Some(column.name.value.as_str())) {
                        Some((column_name, values, quote)) => {
                            constraints.push(RelationConstraint::accepted_values(
                                column_name,
                                values,
                                quote,
                                vec![sql_constraint_evidence(
                                    &relation,
                                    format!(
                                        "{}:column:{}:check:{}",
                                        relation, column.name, option_index
                                    ),
                                    None,
                                )?],
                            )?);
                        }
                        None => diagnostics.push(ConstraintDiagnostic::new(
                            "unsupported_check_constraint",
                            format!(
                                "check constraint on '{}.{}' cannot be represented safely as a finite accepted-values constraint",
                                relation, column.name
                            ),
                        )),
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

    if constraints.is_empty() && diagnostics.is_empty() && column_value_sources.is_empty() {
        return Ok(None);
    }

    let mut set = RelationConstraintSet::new(relation, constraints)?
        .with_column_value_sources(column_value_sources);
    for diagnostic in diagnostics {
        set.add_diagnostic(diagnostic);
    }
    Ok(Some(set))
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
