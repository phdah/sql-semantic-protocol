//! TASK-137 finite source-free VALUES and literal array table constructors.
//!
//! This is the only AST-to-canonical conversion for literal table expansion.
//! Unsupported expressions never become invented finite input rows.

use super::*;
use crate::protocol::{FiniteColumnType, FiniteTable, FiniteTableKind};

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
            // Untyped [] and [NULL] carry no reliable element datatype.
            // Only an observed integer literal attests this narrow array law.
            literals
                .iter()
                .any(|literal| literal.literal_type() == LiteralType::Integer)
                .then_some(literals)
        }
        Expr::Function(function) if function.name.to_string().eq_ignore_ascii_case("array") => {
            let literals = function_arguments(function)?
                .into_iter()
                .map(integer_literal)
                .collect::<Option<Vec<_>>>()?;
            // Untyped [] and [NULL] carry no reliable element datatype.
            // Only an observed integer literal attests this narrow array law.
            literals
                .iter()
                .any(|literal| literal.literal_type() == LiteralType::Integer)
                .then_some(literals)
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
            let crate::data_type::DataType::SignedInteger { bits: Some(bits) } = element else {
                return None;
            };
            if matches!(expr.as_ref(), Expr::Value(value) if value.to_string().eq_ignore_ascii_case("NULL"))
            {
                return Some(Vec::new());
            }
            match expr.as_ref() {
                Expr::Array(array) => {
                    let literals = array
                        .elem
                        .iter()
                        .map(integer_literal)
                        .collect::<Option<Vec<_>>>()?;
                    literals
                        .iter()
                        .all(|literal| fits_signed_width(literal, *bits))
                        .then_some(literals)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

// An explicit cast only preserves finite literal identities if every value
// fits its declared integer width. Otherwise engine casts may fail or coerce.
fn fits_signed_width(literal: &LiteralExpression, bits: u16) -> bool {
    match literal.value() {
        LiteralValue::Null => true,
        LiteralValue::Number(value) => {
            let Ok(number) = value.parse::<i128>() else {
                return false;
            };
            match bits {
                1..=127 => {
                    let half = 1_i128 << (bits - 1);
                    (-half..half).contains(&number)
                }
                128.. => true,
                _ => false,
            }
        }
        _ => false,
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
    if (offset && ordinality) || (matches!(kind, FiniteTableKind::Explode) && (offset || ordinality)) {
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
    FiniteTable::new(
        kind,
        columns,
        vec![FiniteColumnType::Integer; expected],
        rows,
    )
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
            with_offset_alias: None,
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
        // sqlparser 0.58 may represent a source-free FROM UNNEST/EXPLODE as a
        // Table with function arguments. Preserve the WITH ORDINALITY fact
        // before ever considering this a material physical source.
        TableFactor::Table {
            name,
            alias,
            args: Some(args),
            with_ordinality,
            sample: None,
            with_hints,
            version: None,
            partitions,
            json_path: None,
            index_hints,
            ..
        } if (name.to_string().eq_ignore_ascii_case("unnest")
            || name.to_string().eq_ignore_ascii_case("explode"))
            && with_hints.is_empty()
            && partitions.is_empty()
            && index_hints.is_empty()
            && args.settings.is_none() =>
        {
            let arguments = args
                .args
                .iter()
                .map(|arg| match arg {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let kind = if name.to_string().eq_ignore_ascii_case("explode") {
                FiniteTableKind::Explode
            } else {
                FiniteTableKind::Unnest
            };
            unnest_table(
                alias,
                &arguments,
                false,
                *with_ordinality,
                kind,
                dialect_name,
            )
        }
        TableFactor::Function {
            name,
            args,
            alias,
            lateral,
            ..
        } if !*lateral
            && (name.to_string().eq_ignore_ascii_case("unnest")
                || name.to_string().eq_ignore_ascii_case("explode")) =>
        {
            let arguments = args
                .iter()
                .map(|arg| match arg {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let kind = if name.to_string().eq_ignore_ascii_case("explode") {
                FiniteTableKind::Explode
            } else {
                FiniteTableKind::Unnest
            };
            unnest_table(alias, &arguments, false, false, kind, dialect_name)
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
    FiniteTable::new(
        FiniteTableKind::Explode,
        vec![alias.value.clone()],
        vec![FiniteColumnType::Integer],
        rows,
    )
}

fn values_table(values: &sqlparser::ast::Values) -> Option<FiniteTable> {
    let [first, ..] = values.rows.as_slice() else {
        return None;
    };
    if first.is_empty() {
        return None;
    }
    let columns: Vec<String> = (1..=first.len())
        .map(|index| format!("column{index}"))
        .collect();
    let rows = values
        .rows
        .iter()
        .map(|row| row.iter().map(integer_literal).collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>()?;
    let column_types = (0..columns.len())
        .map(|index| {
            if rows.iter().any(|row: &Vec<LiteralExpression>| {
                row[index].literal_type() == LiteralType::Integer
            }) {
                FiniteColumnType::Integer
            } else {
                FiniteColumnType::Unknown
            }
        })
        .collect();
    FiniteTable::new(FiniteTableKind::Values, columns, column_types, rows)
}

// Closed, typed source-free VALUES snapshots carry positional column aliases
// into deterministic star projections, without consulting an external table.
fn named_values_source(factor: &TableFactor) -> Option<FiniteTable> {
    let TableFactor::Derived {
        lateral: false,
        subquery,
        alias: Some(alias),
        ..
    } = factor
    else {
        return None;
    };
    if !ordinary_query_shape(subquery) {
        return None;
    }
    let SetExpr::Values(values) = subquery.body.as_ref() else {
        return None;
    };
    let table = values_table(values)?;
    if alias.columns.len() != table.columns().len() {
        return None;
    }
    let columns = alias
        .columns
        .iter()
        .map(|column| column.name.value.clone())
        .collect();
    FiniteTable::new(
        FiniteTableKind::Values,
        columns,
        table.column_types().to_vec(),
        table.rows().to_vec(),
    )
}

fn star_select(select: &Select) -> Option<FiniteTable> {
    use super::star_projection::{plan_star_modification, StarModification};

    if !ordinary_select_shape(select) {
        return None;
    }
    let [from] = select.from.as_slice() else {
        return None;
    };
    if !from.joins.is_empty() {
        return None;
    }
    let table = named_values_source(&from.relation)?;
    let [item] = select.projection.as_slice() else {
        return None;
    };
    let options = match item {
        SelectItem::Wildcard(options) => options,
        SelectItem::QualifiedWildcard(qualifier, options)
            if matches!(&from.relation, TableFactor::Derived { alias: Some(alias), .. }
                if qualifier.to_string() == alias.name.value) =>
        {
            options
        }
        _ => return None,
    };
    let mut columns = table.columns().to_vec();
    let mut types = table.column_types().to_vec();
    let mut rows = table.rows().to_vec();
    match plan_star_modification(options, &columns).ok()? {
        StarModification::Identity => return None,
        StarModification::Exclude(positions) => {
            for position in positions.into_iter().rev() {
                columns.remove(position);
                types.remove(position);
                for row in &mut rows {
                    row.remove(position);
                }
            }
        }
        StarModification::Rename(pairs) => {
            for (position, name) in pairs {
                *columns.get_mut(position)? = name;
            }
        }
        StarModification::Replace(pairs) => {
            for (position, expression) in pairs {
                let literal = integer_literal(&expression)?;
                let column_type = match literal.literal_type() {
                    LiteralType::Integer => FiniteColumnType::Integer,
                    LiteralType::Null => FiniteColumnType::Unknown,
                    _ => return None,
                };
                *types.get_mut(position)? = column_type;
                for row in &mut rows {
                    *row.get_mut(position)? = literal.clone();
                }
            }
        }
    }
    let contributors = (0..table.rows().len()).map(|index| vec![index]).collect();
    FiniteTable::new(FiniteTableKind::Star, columns, types, rows)?.with_source(table, contributors)
}

// Fixed-schema UNPIVOT preserves each complete source row and column identity.
// Column labels are exact SQL string values; NULL inclusion is explicit.
fn unpivot_select(select: &Select) -> Option<FiniteTable> {
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
    let TableFactor::Unpivot {
        table,
        value,
        name,
        columns,
        null_inclusion,
        alias,
        ..
    } = &from.relation
    else {
        return None;
    };
    if alias
        .as_ref()
        .is_some_and(|alias| !alias.columns.is_empty())
    {
        return None;
    }
    let source = named_values_source(table)?;
    if columns.is_empty() {
        return None;
    }
    let selected = columns
        .iter()
        .map(|column| {
            source
                .columns()
                .iter()
                .position(|item| item == &column.value)
        })
        .collect::<Option<Vec<_>>>()?;
    if selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
        return None;
    }
    if selected
        .iter()
        .any(|index| source.column_types()[*index] != FiniteColumnType::Integer)
    {
        return None;
    }
    let preserved = (0..source.columns().len())
        .filter(|index| !selected.contains(index))
        .collect::<Vec<_>>();
    let mut output_columns = preserved
        .iter()
        .map(|index| source.columns()[*index].clone())
        .collect::<Vec<_>>();
    output_columns.push(name.value.clone());
    output_columns.push(value.value.clone());
    let mut output_types = preserved
        .iter()
        .map(|index| source.column_types()[*index])
        .collect::<Vec<_>>();
    output_types.extend([FiniteColumnType::Text, FiniteColumnType::Integer]);
    let include_null = matches!(
        null_inclusion,
        Some(sqlparser::ast::NullInclusion::IncludeNulls)
    );
    let mut rows = Vec::new();
    let mut contributors = Vec::new();
    for (source_index, source_row) in source.rows().iter().enumerate() {
        for (index, column) in selected.iter().zip(columns) {
            let element = &source_row[*index];
            if element.literal_type() == LiteralType::Null && !include_null {
                continue;
            }
            let mut row = preserved
                .iter()
                .map(|index| source_row[*index].clone())
                .collect::<Vec<_>>();
            row.push(LiteralExpression::new(
                LiteralType::String,
                LiteralValue::Text(column.value.clone()),
            ));
            row.push(element.clone());
            rows.push(row);
            contributors.push(vec![source_index]);
        }
    }
    FiniteTable::new(FiniteTableKind::Unpivot, output_columns, output_types, rows)?
        .with_source(source, contributors)
}

// Certify only a single integer SUM over one fixed integer pivot key.
// Static aliases make columns deterministic; dynamic/sampled/session rules fail closed.
fn pivot_select(select: &Select) -> Option<FiniteTable> {
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
    let TableFactor::Pivot {
        table,
        aggregate_functions,
        value_column,
        value_source,
        default_on_null: None,
        alias,
        ..
    } = &from.relation
    else {
        return None;
    };
    if alias
        .as_ref()
        .is_some_and(|alias| !alias.columns.is_empty())
    {
        return None;
    }
    let source = named_values_source(table)?;
    let [key] = value_column.as_slice() else {
        return None;
    };
    let key_index = source
        .columns()
        .iter()
        .position(|name| name == &key.value)?;
    if source.column_types()[key_index] != FiniteColumnType::Integer {
        return None;
    }
    let [aggregate] = aggregate_functions.as_slice() else {
        return None;
    };
    if aggregate.alias.is_some() {
        return None;
    }
    let Expr::Function(function) = &aggregate.expr else {
        return None;
    };
    if !function.name.to_string().eq_ignore_ascii_case("sum") {
        return None;
    }
    let arguments = function_arguments(function)?;
    let [Expr::Identifier(value_name)] = arguments.as_slice() else {
        return None;
    };
    let value_index = source
        .columns()
        .iter()
        .position(|name| name == &value_name.value)?;
    if value_index == key_index || source.column_types()[value_index] != FiniteColumnType::Integer {
        return None;
    }
    let sqlparser::ast::PivotValueSource::List(values) = value_source else {
        return None;
    };
    if values.is_empty() {
        return None;
    }
    let mut keys = Vec::new();
    let mut named = BTreeSet::new();
    for value in values {
        let alias = value.alias.as_ref()?.value.clone();
        let key = integer_literal(&value.expr)?;
        if key.literal_type() != LiteralType::Integer
            || !named.insert(alias.clone())
            || keys
                .iter()
                .any(|(_, previous): &(String, LiteralExpression)| previous == &key)
        {
            return None;
        }
        keys.push((alias, key));
    }
    let group_indices = (0..source.columns().len())
        .filter(|index| *index != key_index && *index != value_index)
        .collect::<Vec<_>>();
    let mut output_columns = group_indices
        .iter()
        .map(|index| source.columns()[*index].clone())
        .collect::<Vec<_>>();
    output_columns.extend(keys.iter().map(|(name, _)| name.clone()));
    let mut output_types = group_indices
        .iter()
        .map(|index| source.column_types()[*index])
        .collect::<Vec<_>>();
    output_types.extend(vec![FiniteColumnType::Integer; keys.len()]);
    type GroupBag<'a> = (
        Vec<LiteralExpression>,
        Vec<(usize, &'a Vec<LiteralExpression>)>,
    );
    let mut groups: Vec<GroupBag<'_>> = Vec::new();
    for (source_index, row) in source.rows().iter().enumerate() {
        let key = group_indices
            .iter()
            .map(|index| row[*index].clone())
            .collect::<Vec<_>>();
        if let Some((_, members)) = groups.iter_mut().find(|(name, _)| *name == key) {
            members.push((source_index, row));
        } else {
            groups.push((key, vec![(source_index, row)]));
        }
    }
    let mut rows = Vec::new();
    let mut contributors = Vec::new();
    for (group, members) in groups {
        let mut out = group;
        for (_, key) in &keys {
            let mut sum = 0_i64;
            let mut present = false;
            for (_, row) in &members {
                if row[key_index] != *key || row[value_index].literal_type() == LiteralType::Null {
                    continue;
                }
                let LiteralValue::Number(number) = row[value_index].value() else {
                    return None;
                };
                let integer = number.parse::<i64>().ok()?;
                sum = sum.checked_add(integer)?;
                present = true;
            }
            out.push(if present {
                LiteralExpression::new(LiteralType::Integer, LiteralValue::Number(sum.to_string()))
            } else {
                LiteralExpression::new(LiteralType::Null, LiteralValue::Null)
            });
        }
        rows.push(out);
        contributors.push(
            members
                .iter()
                .map(|(source_index, _)| *source_index)
                .collect(),
        );
    }
    FiniteTable::new(FiniteTableKind::Pivot, output_columns, output_types, rows)?
        .with_source(source, contributors)
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
        SetExpr::Select(select) => star_select(select)
            .or_else(|| pivot_select(select))
            .or_else(|| unpivot_select(select))
            .or_else(|| unnest_select(select, dialect_name))
            .or_else(|| explode_select(select, dialect_name)),
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
