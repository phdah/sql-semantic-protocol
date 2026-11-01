//! Normalize supported ALTER TABLE column operations at the SQL AST boundary.
//!
//! A statement is accepted only when every operation has a portable, typed
//! representation. Unsupported clauses cannot be partially executed.

use super::*;
use sqlparser::ast::{AlterColumnOperation, AlterTableOperation, ObjectName};

fn scalar_default(expr: &Expr) -> Option<LiteralValue> {
    match expr {
        Expr::Value(value) => match &value.value {
            Value::Null => Some(LiteralValue::Null),
            Value::Boolean(value) => Some(LiteralValue::Boolean(*value)),
            Value::Number(value, _) => Number::from_str(value).ok().map(|n| LiteralValue::Number(n.to_string())),
            Value::SingleQuotedString(value) => Some(LiteralValue::Text(value.clone())),
            _ => None,
        },
        Expr::UnaryOp { op: SqlUnaryOperator::Minus, expr } => {
            let Expr::Value(value) = expr.as_ref() else {
                return None;
            };
            let Value::Number(number, _) = &value.value else {
                return None;
            };
            Number::from_str(&format!("-{number}"))
                .ok()
                .map(|number| LiteralValue::Number(number.to_string()))
        }
        _ => None,
    }
}

fn add_column(
    declared: &sqlparser::ast::ColumnDef,
    dialect_name: &str,
) -> Option<crate::protocol::ColumnAlterStep> {
    let column = SchemaColumn::from_sql_type(
        declared.name.to_string(),
        &declared.data_type.to_string(),
        dialect_name,
    ).ok()?;
    let mut default = None;
    for option in &declared.options {
        match &option.option {
            ColumnOption::Default(expr) if default.is_none() => {
                default = Some(scalar_default(expr)?);
            }
            ColumnOption::Null => {}
            // NOT NULL, keys, generated values, collation and vendor options
            // have no portable, independently attested migration law here.
            _ => return None,
        }
    }
    Some(crate::protocol::ColumnAlterStep::Add { column, default })
}

pub(super) fn analyze_alter_columns(
    name: &ObjectName,
    operations: &[AlterTableOperation],
    dialect_name: &str,
) -> ProtocolStatement {
    use crate::protocol::ColumnAlterStep;
    if operations.is_empty() {
        return unsupported_write_statement(
            "alter_table", "unsupported_alter_table",
            "ALTER TABLE requires at least one supported column operation",
        );
    }
    let mut steps = Vec::with_capacity(operations.len());
    for operation in operations {
        let step = match operation {
            AlterTableOperation::AddColumn {
                if_not_exists: false,
                column_def,
                column_position: None,
                ..
            } => add_column(column_def, dialect_name),
            AlterTableOperation::DropColumn {
                if_exists: false,
                column_names,
                drop_behavior: None,
                ..
            } if column_names.len() == 1 => Some(ColumnAlterStep::Drop {
                column: column_names[0].to_string(),
            }),
            AlterTableOperation::RenameColumn {
                old_column_name,
                new_column_name,
            } => Some(ColumnAlterStep::Rename {
                old: old_column_name.to_string(),
                new: new_column_name.to_string(),
            }),
            AlterTableOperation::AlterColumn {
                column_name,
                op: AlterColumnOperation::SetDataType { data_type, using: None, .. },
            } => SchemaColumn::from_sql_type(
                column_name.to_string(),
                &data_type.to_string(),
                dialect_name,
            ).ok().map(|column| ColumnAlterStep::SetType { column }),
            _ => None,
        };
        let Some(step) = step else {
            return unsupported_write_statement(
                "alter_table", "unsupported_alter_column",
                "ALTER TABLE contains an unproved column option, cast, rename or vendor law",
            );
        };
        steps.push(step);
    }
    let target = name.to_string();
    ProtocolStatement::Query(
        QueryStatement::new(
            Vec::new(),
            vec![target.clone()],
            Vec::new(),
            RowConditions::new(
                Predicates::new(None, None, None),
                Vec::new(),
                ConditionExactness::from_residuals(vec![ResidualCondition::new(
                    ResidualConditionReason::AnalysisDiagnostic,
                    ConditionClause::RowSetOperator,
                    "alter_requires_complete_prior_schema_and_rows",
                )]),
            ),
            Output::new(Vec::new()),
            Vec::new(),
        )
        .with_produced_relation(Some(target.clone()))
        .with_write(Some(WriteOperation::alter_columns(target, steps))),
    )
}
