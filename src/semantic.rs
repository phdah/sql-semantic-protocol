use sqlparser::ast::{
    BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr,
    ObjectNamePart, Select, SelectItem, TableFactor, WindowType,
};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Column-centric view of a SQL SELECT query.
#[derive(Debug)]
pub struct QuerySchema {
    pub table: String,
    pub columns: Vec<ColumnSemantics>,
}

/// Everything known about a single column across all clauses.
#[derive(Debug)]
pub struct ColumnSemantics {
    /// Raw column name (or alias when no source column can be determined).
    pub name: String,
    /// How the column appears in the SELECT list.
    pub projection: Option<Projection>,
    /// All filter conditions referencing this column (WHERE / HAVING / QUALIFY).
    pub conditions: Vec<Condition>,
    /// Whether the column is part of GROUP BY.
    pub grouped: bool,
}

/// How a column is expressed in the SELECT list.
#[derive(Debug)]
pub struct Projection {
    pub alias: Option<String>,
    /// The full expression before `AS`, e.g. `DATE_TRUNC('day', created_at)`.
    pub expression: String,
}

/// A single filter condition applied to a column, e.g. `>= 0` or `< 100`.
#[derive(Debug)]
pub struct Condition {
    pub clause: ClauseKind,
    pub operator: String,
    /// The right-hand side of the comparison rendered as SQL.
    pub operand: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ClauseKind {
    Where,
    Having,
    Qualify,
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl std::fmt::Display for ClauseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClauseKind::Where => write!(f, "WHERE"),
            ClauseKind::Having => write!(f, "HAVING"),
            ClauseKind::Qualify => write!(f, "QUALIFY"),
        }
    }
}

impl std::fmt::Display for QuerySchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Table: {}", self.table)?;
        for col in &self.columns {
            writeln!(f, "\nColumn: {}", col.name)?;
            if let Some(proj) = &col.projection {
                let alias_part = proj
                    .alias
                    .as_deref()
                    .map(|a| format!(" AS {a}"))
                    .unwrap_or_default();
                writeln!(f, "  select:   {}{alias_part}", proj.expression)?;
            }
            if col.grouped {
                writeln!(f, "  group by: yes")?;
            }
            for cond in &col.conditions {
                writeln!(
                    f,
                    "  {}:    {} {}",
                    cond.clause, cond.operator, cond.operand
                )?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Extraction entry point
// ---------------------------------------------------------------------------

#[inline(never)]
pub fn extract_schema(select: &Select) -> QuerySchema {
    let table = extract_table_name(select);
    let mut columns: Vec<ColumnSemantics> = Vec::new();

    process_projections(select, &mut columns);

    process_optional_filter(select.selection.as_ref(), ClauseKind::Where, &mut columns);
    process_optional_filter(select.having.as_ref(), ClauseKind::Having, &mut columns);
    process_optional_qualify(select.qualify.as_ref(), &mut columns);

    process_group_by(select, &mut columns);

    QuerySchema { table, columns }
}

fn process_optional_filter(
    expr: Option<&Expr>,
    clause: ClauseKind,
    columns: &mut Vec<ColumnSemantics>,
) {
    match expr {
        Some(expr) => process_filter_expr(expr, clause, columns),
        None => {}
    }
}

fn process_optional_qualify(expr: Option<&Expr>, columns: &mut Vec<ColumnSemantics>) {
    match expr {
        Some(expr) => process_qualify_expr(expr, columns),
        None => {}
    }
}

// ---------------------------------------------------------------------------
// Per-clause processors
// ---------------------------------------------------------------------------

#[inline(never)]
fn process_projections(select: &Select, columns: &mut Vec<ColumnSemantics>) {
    for item in &select.projection {
        match item {
            SelectItem::ExprWithAlias { expr, alias } => {
                let key = projection_key_for_alias_expr(expr, alias.value.as_str());
                let col = get_or_insert(columns, &key);
                col.projection = Some(Projection {
                    alias: Some(alias.value.clone()),
                    expression: format!("{expr}"),
                });
            }
            SelectItem::UnnamedExpr(expr) => {
                let refs = collect_column_refs(expr);
                let key = refs.into_iter().next().unwrap_or_else(|| format!("{expr}"));
                let col = get_or_insert(columns, &key);
                col.projection = Some(Projection {
                    alias: None,
                    expression: format!("{expr}"),
                });
            }
            _ => {}
        }
    }
}

#[inline(never)]
fn projection_key_for_alias_expr(expr: &Expr, alias: &str) -> String {
    let refs = collect_column_refs(expr);
    match refs.first() {
        Some(first) => first.clone(),
        None => alias.to_string(),
    }
}

/// Recursively walk a WHERE/HAVING expression and attach each leaf comparison
/// to the column it references.  AND/OR chains are flattened so that
/// `a > 10 AND a < 20` correctly produces two conditions on `a`.
#[inline(never)]
fn process_filter_expr(expr: &Expr, clause: ClauseKind, columns: &mut Vec<ColumnSemantics>) {
    match expr {
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And | BinaryOperator::Or => {
                process_filter_expr(left, clause, columns);
                process_filter_expr(right, clause, columns);
            }
            _ => {
                // col OP value  (most common form)
                if let Some(col_name) = as_column_name(left) {
                    get_or_insert(columns, &col_name)
                        .conditions
                        .push(Condition {
                            clause,
                            operator: format!("{op}"),
                            operand: format!("{right}"),
                        });
                // value OP col  (less common, operator is semantically flipped
                //                but stored as-is for a first draft)
                } else if let Some(col_name) = as_column_name(right) {
                    get_or_insert(columns, &col_name)
                        .conditions
                        .push(Condition {
                            clause,
                            operator: format!("{op}"),
                            operand: format!("{left}"),
                        });
                }
            }
        },
        Expr::Nested(inner) => process_filter_expr(inner, clause, columns),
        _ => {}
    }
}

/// For QUALIFY we can't identify a single "column"; instead we collect all
/// column refs inside the window expression and attach the condition to each.
#[inline(never)]
fn process_qualify_expr(expr: &Expr, columns: &mut Vec<ColumnSemantics>) {
    if let Expr::BinaryOp { left, op, right } = expr {
        for col_name in collect_column_refs(left) {
            get_or_insert(columns, &col_name)
                .conditions
                .push(Condition {
                    clause: ClauseKind::Qualify,
                    operator: format!("{op}"),
                    operand: format!("{right}"),
                });
        }
    }
}

#[inline(never)]
fn process_group_by(select: &Select, columns: &mut Vec<ColumnSemantics>) {
    let GroupByExpr::Expressions(exprs, _) = &select.group_by else {
        return;
    };
    for expr in exprs {
        match expr {
            // Positional: GROUP BY 1  →  first projected column
            Expr::Value(v) => {
                if let sqlparser::ast::Value::Number(n, _) = &v.value {
                    if let Ok(pos) = n.parse::<usize>() {
                        if let Some(col) = columns.get_mut(pos - 1) {
                            col.grouped = true;
                        }
                    }
                }
            }
            // Named: GROUP BY col_name
            Expr::Identifier(ident) => {
                if let Some(col) = columns.iter_mut().find(|c| c.name == ident.value) {
                    col.grouped = true;
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a mutable reference to the column with `name`, inserting a blank
/// entry if it does not yet exist.
fn get_or_insert<'a>(columns: &'a mut Vec<ColumnSemantics>, name: &str) -> &'a mut ColumnSemantics {
    if let Some(pos) = columns.iter().position(|c| c.name == name) {
        &mut columns[pos]
    } else {
        columns.push(ColumnSemantics {
            name: name.to_string(),
            projection: None,
            conditions: vec![],
            grouped: false,
        });
        columns.last_mut().unwrap()
    }
}

#[inline(never)]
fn extract_table_name(select: &Select) -> String {
    let first_from = match select.from.first() {
        Some(from) => from,
        None => return String::new(),
    };

    let name = match &first_from.relation {
        TableFactor::Table { name, .. } => name,
        _ => return String::new(),
    };

    let mut parts = Vec::new();
    for part in &name.0 {
        match part {
            ObjectNamePart::Identifier(id) => parts.push(id.value.clone()),
            ObjectNamePart::Function(f) => parts.push(f.name.value.clone()),
        }
    }

    parts.join(".")
}

/// Return `Some(name)` if `expr` is a bare column identifier, `None` otherwise.
fn as_column_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(id) => Some(id.value.clone()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|i| i.value.clone()),
        _ => None,
    }
}

/// Recursively collect every column identifier referenced inside `expr`.
fn collect_column_refs(expr: &Expr) -> Vec<String> {
    let mut refs = Vec::new();
    collect_refs_inner(expr, &mut refs);
    refs
}

fn collect_refs_inner(expr: &Expr, refs: &mut Vec<String>) {
    match expr {
        Expr::Identifier(id) => refs.push(id.value.clone()),
        Expr::CompoundIdentifier(parts) => {
            if let Some(last) = parts.last() {
                refs.push(last.value.clone());
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_refs_inner(left, refs);
            collect_refs_inner(right, refs);
        }
        Expr::UnaryOp { expr, .. } => collect_refs_inner(expr, refs),
        Expr::Nested(inner) => collect_refs_inner(inner, refs),
        Expr::Function(f) => {
            if let FunctionArguments::List(list) = &f.args {
                for arg in &list.args {
                    if let FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) = arg {
                        collect_refs_inner(e, refs);
                    }
                }
            }
            // Also descend into window spec (ORDER BY / PARTITION BY columns).
            if let Some(WindowType::WindowSpec(spec)) = &f.over {
                for o in &spec.order_by {
                    collect_refs_inner(&o.expr, refs);
                }
                for p in &spec.partition_by {
                    collect_refs_inner(p, refs);
                }
            }
        }
        _ => {}
    }
}
