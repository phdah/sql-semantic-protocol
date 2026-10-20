//! TASK-126: exact navigation and deterministic ROWS frames over complete source bags.
//!
//! All operations consume parser-independent canonical window expressions. Rows
//! are identified by their original physical positions; SQL ORDER BY determines
//! window membership only when it has a provably total ordering.

use super::rank_bags::{compare_key, exact_domain, key_value, rank_keys, RankKeys};
use super::write_states::{physical_closed_world_schema_plan, PhysicalWriteStatePlan};
use super::*;
use crate::protocol::{
    Expression, LiteralType, LiteralValue, WindowFrame, WindowFrameBound, WindowFrameUnits,
    WindowFunctionExpression,
};

/// Complete source-attested physical identity and terminal window bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteWindowExact {
    /// Canonical source dependency graph.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Full physical table whose rows were independently validated.
    pub source: String,
    /// All physical row identities, including SQL duplicates.
    pub source_row_ids: Vec<String>,
    /// Complete terminal row multiset in source identity order.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Finite and inclusive terminal value domains, including SQL NULL.
    pub terminal_domains: BTreeMap<String, crate::protocol::ValueDomain>,
}

/// Physical proof classification for the bounded portable navigation subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteWindowProof {
    /// Entire input and terminal bag, with exact source identities and domains.
    Exact(Box<PhysicalFiniteWindowExact>),
    /// Full source evidence violates an enforced schema constraint.
    Impossible { reason: String },
    /// A parser, type, order, frame or SQL/session law is not proven.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalFiniteWindowProof {
    PhysicalFiniteWindowProof::Residual {
        reason: reason.into(),
    }
}

#[derive(Debug, Clone)]
enum WindowKind {
    Lag { value: String, offset: usize },
    Lead { value: String, offset: usize },
    First { value: String, frame: WindowFrame },
    Last { value: String, frame: WindowFrame },
    CountRows { frame: WindowFrame },
}

#[derive(Debug, Clone)]
struct WindowColumn {
    output: String,
    kind: WindowKind,
}

fn direct_source_column(
    expression: &Expression,
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
) -> Result<String, &'static str> {
    let Expression::Column(column) = expression else {
        return Err("navigation argument must be a direct physical column");
    };
    if column
        .relation()
        .is_some_and(|relation| relation != source.name() && Some(relation) != source.alias())
        || !schema.columns().iter().any(|c| c.name() == column.name())
    {
        return Err("navigation argument is not in authoritative source schema");
    }
    Ok(column.name().to_string())
}

fn positive_offset(expression: &Expression) -> Result<usize, &'static str> {
    let Expression::Literal(value) = expression else {
        return Err("navigation offset is not a constant integer");
    };
    if value.literal_type() != LiteralType::Integer {
        return Err("navigation offset has no attested integer type");
    }
    let LiteralValue::Number(number) = value.value() else {
        return Err("navigation offset is SQL NULL or nonnumeric");
    };
    let offset = number
        .parse::<usize>()
        .map_err(|_| "navigation offset is negative or out of range")?;
    Ok(offset)
}

fn rows_frame(window: &WindowFunctionExpression) -> Result<WindowFrame, &'static str> {
    let frame = window
        .window()
        .frame()
        .ok_or("ordered value window without explicit ROWS has implicit RANGE semantics")?;
    if frame.units() != WindowFrameUnits::Rows {
        return Err("RANGE/GROUPS frame has no certified deterministic ROWS semantics");
    }
    // Reject invalid frame direction rather than treating SQL parse success
    // as evidence for engine-dependent empty-frame normalization.
    if matches!(frame.start_bound(), WindowFrameBound::UnboundedFollowing)
        || matches!(frame.end_bound(), WindowFrameBound::UnboundedPreceding)
    {
        return Err("invalid ROWS frame direction");
    }
    for bound in [frame.start_bound(), frame.end_bound()] {
        if let WindowFrameBound::Preceding(offset) | WindowFrameBound::Following(offset) = bound {
            positive_offset(offset)?;
        }
    }
    Ok(frame.clone())
}

fn window_kind(
    window: &WindowFunctionExpression,
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
) -> Result<WindowKind, &'static str> {
    let function = window.function();
    if function.distinct() {
        return Err("DISTINCT window arguments need separate membership evidence");
    }
    let args = function.arguments();
    match function.name().to_ascii_lowercase().as_str() {
        "lag" | "lead" => {
            if window.window().frame().is_some() {
                return Err("navigation with explicit frame has unproven vendor behavior");
            }
            if !(1..=3).contains(&args.len()) {
                return Err("navigation expects one to three arguments");
            }
            let value = direct_source_column(&args[0], source, schema)?;
            let offset = if args.len() >= 2 {
                positive_offset(&args[1])?
            } else {
                1
            };
            if args.len() == 3
                && !matches!(&args[2], Expression::Literal(lit)
                    if lit.value() == &LiteralValue::Null)
            {
                return Err("navigation non-NULL default needs typed casting evidence");
            }
            if function.name().eq_ignore_ascii_case("lag") {
                Ok(WindowKind::Lag { value, offset })
            } else {
                Ok(WindowKind::Lead { value, offset })
            }
        }
        "first_value" | "last_value" => {
            let [argument] = args else {
                return Err("FIRST_VALUE/LAST_VALUE require one argument");
            };
            let value = direct_source_column(argument, source, schema)?;
            let frame = rows_frame(window)?;
            if function.name().eq_ignore_ascii_case("first_value") {
                Ok(WindowKind::First { value, frame })
            } else {
                Ok(WindowKind::Last { value, frame })
            }
        }
        "count" => {
            // The canonical analyzer rewrites COUNT(*) to COUNT(1). The
            // constant non-NULL argument counts every physical frame member.
            let [Expression::Literal(one)] = args else {
                return Err("this bounded frame proof only certifies COUNT(*)");
            };
            if one.literal_type() != LiteralType::Integer
                || one.value() != &LiteralValue::Number("1".to_string())
            {
                return Err("COUNT frame argument is not an exact non-NULL row constant");
            }
            Ok(WindowKind::CountRows {
                frame: rows_frame(window)?,
            })
        }
        _ => Err("navigation function has no certified physical window law"),
    }
}

fn frame_position(bound: &WindowFrameBound, index: i128, len: i128) -> Result<i128, &'static str> {
    match bound {
        WindowFrameBound::CurrentRow => Ok(index),
        WindowFrameBound::UnboundedPreceding => Ok(0),
        WindowFrameBound::UnboundedFollowing => Ok(len - 1),
        WindowFrameBound::Preceding(value) => {
            let offset = positive_offset(value)?;
            Ok(index - i128::try_from(offset).map_err(|_| "frame offset exceeds signed width")?)
        }
        WindowFrameBound::Following(value) => {
            let offset = positive_offset(value)?;
            Ok(index + i128::try_from(offset).map_err(|_| "frame offset exceeds signed width")?)
        }
    }
}

fn frame_members(
    frame: &WindowFrame,
    index: usize,
    len: usize,
) -> Result<std::ops::Range<usize>, &'static str> {
    let index = i128::try_from(index).map_err(|_| "frame row index exceeds signed width")?;
    let len = i128::try_from(len).map_err(|_| "frame count exceeds signed width")?;
    let start = frame_position(frame.start_bound(), index, len)?.max(0).min(len);
    let end = frame_position(frame.end_bound(), index, len)?
        .max(-1)
        .min(len - 1);
    let start = usize::try_from(start).map_err(|_| "invalid frame start")?;
    let end = usize::try_from(end + 1).map_err(|_| "invalid frame end")?;
    Ok(start..end.max(start))
}

fn frame_value(
    kind: &WindowKind,
    rows: &[(usize, Vec<Option<i128>>)],
    index: usize,
    snapshot: &PhysicalWriteState,
) -> Result<LiteralValue, &'static str> {
    let position = match kind {
        WindowKind::Lag { offset, .. } => index.checked_sub(*offset),
        WindowKind::Lead { offset, .. } => index.checked_add(*offset),
        WindowKind::First { frame, .. } => {
            let range = frame_members(frame, index, rows.len())?;
            (!range.is_empty()).then_some(range.start)
        }
        WindowKind::Last { frame, .. } => {
            let range = frame_members(frame, index, rows.len())?;
            (!range.is_empty()).then_some(range.end - 1)
        }
        WindowKind::CountRows { frame } => {
            let range = frame_members(frame, index, rows.len())?;
            return Ok(LiteralValue::Number(range.len().to_string()));
        }
    };
    let Some((source_index, _)) = position.and_then(|position| rows.get(position)) else {
        return Ok(LiteralValue::Null);
    };
    let column = match kind {
        WindowKind::Lag { value, .. }
        | WindowKind::Lead { value, .. }
        | WindowKind::First { value, .. }
        | WindowKind::Last { value, .. } => value,
        WindowKind::CountRows { .. } => return Err("unreachable COUNT frame result"),
    };
    snapshot.rows()[*source_index]
        .values()
        .get(column)
        .cloned()
        .ok_or("window value column missing from complete source bag")
}

/// Evaluate portable LAG, LEAD, FIRST_VALUE, LAST_VALUE and bounded COUNT(*)
/// ROWS frames against one validated physical source bag. Unattested peer
/// ordering, implicit NULL order, RANGE/GROUPS and complex row shaping remain
/// typed Residual; contradictory source schemas yield Impossible.
pub fn physical_finite_window_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteWindowProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteWindowProof::Impossible { reason }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let [snapshot] = snapshots else {
        return residual("window evaluation requires one complete physical source");
    };
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing window output layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("window target has no canonical query");
    };
    let [source] = query.sources() else {
        return residual("window proof requires one direct physical source");
    };
    if snapshot.relation() != source.name()
        || !walker.sources.contains(source.name())
        || !query.joins().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
        || !query.plain_goal_output_shape()
    {
        return residual("window query has unproved source or terminal row shaping");
    }
    let Some(schema) = bundle
        .source_schemas()
        .iter()
        .find(|item| item.relation() == source.name())
    else {
        return residual("window proof requires authoritative physical datatypes");
    };
    let mut seen_names = BTreeSet::new();
    let mut keys: Option<RankKeys> = None;
    let mut operations = Vec::<WindowColumn>::new();
    for projected in query.output().columns() {
        if !seen_names.insert(projected.name()) {
            return residual("duplicate window output alias");
        }
        match projected.expression() {
            Expression::Column(expression) => {
                if let Err(reason) = direct_source_column(
                    &Expression::Column(expression.clone()),
                    source,
                    schema,
                ) {
                    return residual(reason);
                }
            }
            Expression::WindowFunction(window) => {
                let parsed = match rank_keys(window.window(), source, schema, true) {
                    Ok(parsed) => parsed,
                    Err(reason) => return residual(reason),
                };
                if keys.as_ref().is_some_and(|existing| existing != &parsed) {
                    return residual("window expressions use incompatible partition or order keys");
                }
                keys = Some(parsed);
                let kind = match window_kind(window, source, schema) {
                    Ok(kind) => kind,
                    Err(reason) => return residual(reason),
                };
                operations.push(WindowColumn {
                    output: projected.name().to_string(),
                    kind,
                });
            }
            _ => return residual("computed projection lacks a certified row-local value law"),
        }
    }
    let Some(keys) = keys else {
        return residual("window target has no supported window function");
    };
    let mut partitions = BTreeMap::<Vec<Option<i128>>, Vec<(usize, Vec<Option<i128>>)>>::new();
    for (index, row) in snapshot.rows().iter().enumerate() {
        let partition = match keys.partition.iter().map(|key| key_value(row, key)).collect() {
            Ok(values) => values,
            Err(reason) => return residual(reason),
        };
        let order = match keys.sort.iter().map(|key| key_value(row, &key.column)).collect() {
            Ok(values) => values,
            Err(reason) => return residual(reason),
        };
        partitions.entry(partition).or_default().push((index, order));
    }
    let mut evaluated = BTreeMap::<String, Vec<Option<LiteralValue>>>::new();
    for operation in &operations {
        evaluated.insert(operation.output.clone(), vec![None; snapshot.rows().len()]);
    }
    for partition in partitions.values_mut() {
        partition.sort_by(|(_, left), (_, right)| {
            left.iter()
                .zip(right.iter())
                .zip(&keys.sort)
                .map(|((left, right), sort)| compare_key(*left, *right, sort))
                .find(|order| !order.is_eq())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if partition.windows(2).any(|rows| rows[0].1 == rows[1].1) {
            return residual("window ORDER BY peers lack deterministic physical row order");
        }
        for (position, (source_index, _)) in partition.iter().enumerate() {
            for operation in &operations {
                let value = match frame_value(&operation.kind, partition, position, snapshot) {
                    Ok(value) => value,
                    Err(reason) => return residual(reason),
                };
                let Some(column_values) = evaluated.get_mut(&operation.output) else {
                    return residual("missing window output assignment");
                };
                column_values[*source_index] = Some(value);
            }
        }
    }
    let mut terminal = Vec::new();
    for (index, row) in snapshot.rows().iter().enumerate() {
        let mut projected = BTreeMap::new();
        for column in query.output().columns() {
            let value = match column.expression() {
                Expression::Column(source_column) => row.values().get(source_column.name()).cloned(),
                Expression::WindowFunction(_) => evaluated
                    .get(column.name())
                    .and_then(|values| values.get(index))
                    .cloned()
                    .flatten(),
                _ => None,
            };
            let Some(value) = value else {
                return residual("window source or terminal projection incomplete");
            };
            projected.insert(column.name().to_string(), value);
        }
        terminal.push(projected);
    }
    let mut domains = BTreeMap::new();
    for column in query.output().columns() {
        domains.insert(
            column.name().to_string(),
            exact_domain(
                terminal
                    .iter()
                    .filter_map(|row| row.get(column.name()).cloned()),
            ),
        );
    }
    PhysicalFiniteWindowProof::Exact(Box::new(PhysicalFiniteWindowExact {
        nodes: walker.nodes,
        source: source.name().to_string(),
        source_row_ids: (0..snapshot.rows().len())
            .map(|index| format!("{}#{index}", source.name()))
            .collect(),
        terminal,
        terminal_domains: domains,
    }))
}
