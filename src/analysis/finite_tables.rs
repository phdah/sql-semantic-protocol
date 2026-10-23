//! TASK-137 finite source-free VALUES and literal array table constructors.
//!
//! This is the only AST-to-canonical conversion for literal table expansion.
//! Unsupported expressions never become invented finite input rows.

use super::*;
use crate::protocol::{FiniteTable, FiniteTableKind};

fn ordinary_query_shape(query: &SqlQuery) -> bool {
    query.with.is_none()
        && query.order_by.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && query.locks.is_empty()
        && query.for_clause.is_none()
        && query.settings.is_none()
        && query.format_clause.is_none()
        && query.pipe_operators.is_empty()
}

fn ordinary_select_shape(select: &Select) -> bool {
    select.distinct.is_none()
        && select.top.is_none()
        && select.selection.is_none()
        && select.having.is_none()
        && select.qualify.is_none()
        && select.prewhere.is_none()
        && select.into.is_none()
        && select.exclude.is_none()
        && select.connect_by.is_none()
        && select.lateral_views.is_empty()
        && select.named_window.is_empty()
        && select.cluster_by.is_empty()
        && select.distribute_by.is_empty()
        && select.sort_by.is_empty()
        && select.value_table_mode.is_none()
        && matches!(&select.group_by, GroupByExpr::Expressions(keys, modifiers)
            if keys.is_empty() && modifiers.is_empty())
}

fn integer_literal(expr: &Expr) -> Option<LiteralExpression> {
    let literal = literal_expression(expr)?;
    match (literal.literal_type(), literal.value()) {
        (LiteralType::Integer, LiteralValue::Number(number)) if number.parse::<i128>().is_ok() => {
            Some(literal)
        }
        (LiteralType::Null, LiteralValue::Null) => Some(literal),
        _ => None,
    }
}

fn function_arguments(function: &Function) -> Option<Vec<&Expr>> {
    if function.over.is_some()
        || function.filter.is_some()
        || function.null_treatment.is_some()
        || !function.within_group.is_empty()
    {
        return None;
    }
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    if !list.clauses.is_empty() || list.duplicate_treatment.is_some() {
        return None;
    }
    list.args
        .iter()
        .map(|arg| match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr),
            _ => None,
        })
        .collect()
}

fn array_items(expression: &Expr, dialect_name: &str) -> Option<Vec<LiteralExpression>> {
    match expression {
        Expr::Array(array) => {
            let literals = array
                .elem
                .iter()
                .map(integer_literal)
                .collect::<Option<Vec<_>>>()?;
            (!literals.is_empty()).then_some(literals)
        }
        Expr::Function(function) if function.name.to_string().eq_ignore_ascii_case("array") => {
            let literals = function_arguments(function)?
                .into_iter()
                .map(integer_literal)
                .collect::<Option<Vec<_>>>()?;
            (!literals.is_empty()).then_some(literals)
        }
        Expr::Cast {
            expr, data_type, ..
        } => {
            let parsed =
                crate::data_type::parse_data_type(&data_type.to_string(), dialect_name).ok()?;
            let crate::data_type::DataType::Array {
                element: Some(element),
                ..
            } = parsed
            else {
                return None;
            };
            let element = match element.as_ref() {
                crate::data_type::DataType::Nullable(inner) => inner.as_ref(),
                value => value,
            };
            if !matches!(element, crate::data_type::DataType::SignedInteger { .. }) {
                return None;
            }
            if matches!(expr.as_ref(), Expr::Value(value) if value.to_string().eq_ignore_ascii_case("NULL"))
            {
                return Some(Vec::new());
            }
            match expr.as_ref() {
                Expr::Array(array) => array.elem.iter().map(integer_literal).collect(),
                _ => None,
            }
        }
        _ => None,
    }
}

fn unnest_table(
    alias: &Option<sqlparser::ast::TableAlias>,
    expressions: &[Expr],
    offset: bool,
    ordinality: bool,
    kind: FiniteTableKind,
    dialect_name: &str,
) -> Option<FiniteTable> {
    if offset && ordinality {
        return None;
    }
    let [array] = expressions else { return None };
    let literals = array_items(array, dialect_name)?;
    let alias = alias.as_ref()?;
    let expected = if offset || ordinality { 2 } else { 1 };
    if alias.columns.len() != expected {
        return None;
    }
    let columns = alias
        .columns
        .iter()
        .map(|column| column.name.value.clone())
        .collect();
    let mut rows = Vec::new();
    for (index, value) in literals.into_iter().enumerate() {
        let mut row = vec![value];
        if offset || ordinality {
            let index = i128::try_from(index).ok()?;
            let position = if ordinality {
                index.checked_add(1)?
            } else {
                index
            };
            row.push(LiteralExpression::new(
                LiteralType::Integer,
                LiteralValue::Number(position.to_string()),
            ));
        }
        rows.push(row);
    }
    FiniteTable::new(kind, columns, rows)
}

fn unnest_select(select: &Select, dialect_name: &str) -> Option<FiniteTable> {
    if !ordinary_select_shape(select)
        || !matches!(select.projection.as_slice(), [item] if item.to_string() == "*")
    {
        return None;
    }
    let [from] = select.from.as_slice() else {
        return None;
    };
    if !from.joins.is_empty() {
        return None;
    }
    match &from.relation {
        TableFactor::UNNEST {
            alias,
            array_exprs,
            with_offset,
            with_ordinality,
            ..
        } => unnest_table(
            alias,
            array_exprs,
            *with_offset,
            *with_ordinality,
            FiniteTableKind::Unnest,
            dialect_name,
        ),
        TableFactor::Function {
            name,
            args,
            alias,
            lateral,
            ..
        } if !*lateral && name.to_string().eq_ignore_ascii_case("unnest") => {
            let arguments = args
                .iter()
                .map(|arg| match arg {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            unnest_table(alias, &arguments, false, false, FiniteTableKind::Unnest, dialect_name)
        }
        _ => None,
    }
}

fn explode_select(select: &Select, dialect_name: &str) -> Option<FiniteTable> {
    if !ordinary_select_shape(select) || !select.from.is_empty() {
        return None;
    }
    let [SelectItem::ExprWithAlias {
        expr: Expr::Function(function),
        alias,
    }] = select.projection.as_slice()
    else {
        return None;
    };
    if !function.name.to_string().eq_ignore_ascii_case("explode") {
        return None;
    }
    let arguments = function_arguments(function)?;
    let [argument] = arguments.as_slice() else {
        return None;
    };
    let rows = array_items(argument, dialect_name)?
        .into_iter()
        .map(|value| vec![value])
        .collect();
    FiniteTable::new(FiniteTableKind::Explode, vec![alias.value.clone()], rows)
}

fn values_table(values: &sqlparser::ast::Values) -> Option<FiniteTable> {
    let [first, ..] = values.rows.as_slice() else {
        return None;
    };
    if first.is_empty() {
        return None;
    }
    let columns = (1..=first.len())
        .map(|index| format!("column{index}"))
        .collect();
    let rows = values
        .rows
        .iter()
        .map(|row| row.iter().map(integer_literal).collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>()?;
    FiniteTable::new(FiniteTableKind::Values, columns, rows)
}

/// Classify the tightly bounded, source-free SQL constructors into canonical
/// complete finite relations. The caller retains normal unknown diagnostics for
/// all other parsed table and collection syntax.
pub(super) fn source_free_finite_query(
    query: &SqlQuery,
    produced_relation: Option<String>,
    dialect_name: &str,
) -> Option<QueryStatement> {
    if !ordinary_query_shape(query) {
        return None;
    }
    let table = match query.body.as_ref() {
        SetExpr::Values(values) => values_table(values),
        SetExpr::Select(select) => unnest_select(select, dialect_name).or_else(|| explode_select(select, dialect_name)),
        _ => None,
    }?;
    let output = Output::new(
        table
            .columns()
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let values = table
                    .rows()
                    .iter()
                    .filter_map(|row| row.get(index).cloned())
                    .collect();
                let domain = ValueDomain::set(SetMode::Include, values);
                OutputColumn::new(
                    name.clone(),
                    Expression::Unknown(UnknownSemantic::new(
                        "finite table output is defined positionally by canonical literal rows"
                            .to_string(),
                    )),
                    domain,
                    Vec::new(),
                )
            })
            .collect(),
    );
    Some(
        QueryStatement::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            RowConditions::new(
                Predicates::new(None, None, None),
                Vec::new(),
                ConditionExactness::from_residuals(Vec::new()),
            ),
            output,
            Vec::new(),
        )
        .with_finite_table(Some(table))
        .with_produced_relation(produced_relation),
    )
}
