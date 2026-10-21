//! TASK-126/189: exact navigation and deterministic aggregate ROWS windows over complete source bags.
//!
//! All operations consume parser-independent canonical window expressions. Rows
//! are identified by their original physical positions; SQL ORDER BY determines
//! window membership only when it has a provably total ordering.

use super::rank_bags::{
    compare_key, exact_domain, key_value, qualifies, rank_function, rank_keys, RankFunction,
    RankKeys,
};
use super::write_states::{physical_closed_world_schema_plan, PhysicalWriteStatePlan};
use super::*;
use crate::data_type::DataType;
use crate::protocol::{
    ComparisonOperator, Expression, LiteralType, LiteralValue, WindowFrame, WindowFrameBound,
    WindowFrameUnits, WindowFunctionExpression,
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
    /// Source identities that survive the SQL QUALIFY filter, in terminal order.
    pub terminal_row_ids: Vec<String>,
    /// Source identities for which the SQL QUALIFY predicate is NOT TRUE.
    pub rejected_row_ids: Vec<String>,
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
    Lag {
        value: String,
        offset: usize,
    },
    Lead {
        value: String,
        offset: usize,
    },
    First {
        value: String,
        frame: WindowFrame,
    },
    Last {
        value: String,
        frame: WindowFrame,
    },
    Aggregate {
        operation: AggregateKind,
        value: Option<String>,
        frame: Option<WindowFrame>,
        numeric: Option<NumericLaw>,
    },
    Ranking(RankFunction),
}

#[derive(Debug, Clone, Copy)]
enum AggregateKind {
    CountAll,
    CountExpr,
    Sum,
    Avg,
    Min,
    Max,
}

// Only exact, declared finite numeric storage is admitted. DECIMAL remains
// fixed point throughout the physical evaluator; no f64 arithmetic or
// implicit cast occurs. Decimal proof also requires binary-exact values,
// so a floating implementation cannot silently round the claimed outcome.
#[derive(Debug, Clone, Copy)]
enum NumericLaw {
    Integer { bits: Option<u16> },
    Decimal { precision: u64, scale: u32 },
}

fn numeric_law(kind: &DataType) -> Result<NumericLaw, &'static str> {
    match kind {
        DataType::Nullable(inner) => numeric_law(inner),
        DataType::SignedInteger { bits } if super::aggregate_bags::signed_integer(kind) => {
            Ok(NumericLaw::Integer { bits: *bits })
        }
        DataType::Decimal {
            precision: Some(precision),
            scale: Some(scale),
        } if *precision <= 38 && scale <= precision && *scale <= 18 => Ok(NumericLaw::Decimal {
            precision: *precision,
            scale: u32::try_from(*scale)
                .map_err(|_| "DECIMAL scale exceeds supported fixed-point width")?,
        }),
        _ => Err("window arithmetic requires attested exact numeric precision and scale"),
    }
}

fn decimal_units(text: &str, scale: u32) -> Result<i128, &'static str> {
    let (negative, magnitude) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text)
    };
    let (whole, fraction) = magnitude.split_once('.').unwrap_or((magnitude, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("decimal source value has no exact fixed-point representation");
    }
    let significant = fraction.trim_end_matches('0');
    let scale = usize::try_from(scale).map_err(|_| "invalid decimal scale")?;
    if significant.len() > scale {
        return Err("decimal source requires unproved rounding");
    }
    let units = format!(
        "{whole}{significant}{}",
        "0".repeat(scale - significant.len())
    )
    .parse::<i128>()
    .map_err(|_| "decimal value exceeds exact signed width")?;
    if negative {
        units
            .checked_neg()
            .ok_or("negative decimal exceeds signed width")
    } else {
        Ok(units)
    }
}

fn fixed_point(units: i128, scale: u32) -> LiteralValue {
    if scale == 0 {
        return LiteralValue::Number(units.to_string());
    }
    let magnitude = units.unsigned_abs().to_string();
    let scale = scale as usize;
    let padded = format!("{magnitude:0>width$}", width = scale + 1);
    let cut = padded.len() - scale;
    let sign = if units < 0 { "-" } else { "" };
    LiteralValue::Number(format!("{sign}{}.{}", &padded[..cut], &padded[cut..]))
}

fn binary_exact(units: i128, scale: u32) -> bool {
    let mut numerator = units;
    for _ in 0..scale {
        if numerator % 5 != 0 {
            return false;
        }
        numerator /= 5;
    }
    numerator.unsigned_abs() <= (1_u128 << 53)
}

fn within_precision(units: i128, precision: u64) -> bool {
    u64::try_from(units.unsigned_abs().to_string().len()).is_ok_and(|digits| digits <= precision)
}

fn numeric_units(value: &LiteralValue, numeric: NumericLaw) -> Result<i128, &'static str> {
    match numeric {
        NumericLaw::Integer { .. } => {
            super::aggregate_bags::numeric(value)?.ok_or("integer input is unexpectedly NULL")
        }
        NumericLaw::Decimal { precision, scale } => {
            let LiteralValue::Number(text) = value else {
                return Err("decimal aggregate argument has no numeric value");
            };
            let units = decimal_units(text, scale)?;
            if !within_precision(units, precision) || !binary_exact(units, scale) {
                return Err("decimal source value lacks portable exact arithmetic law");
            }
            Ok(units)
        }
    }
}

fn numeric_output(units: i128, numeric: NumericLaw) -> Result<LiteralValue, &'static str> {
    match numeric {
        NumericLaw::Integer { .. } => Ok(LiteralValue::Number(units.to_string())),
        NumericLaw::Decimal { precision, scale } => {
            if !within_precision(units, precision) || !binary_exact(units, scale) {
                return Err(
                    "decimal result exceeds declared precision or exact portable magnitude",
                );
            }
            Ok(fixed_point(units, scale))
        }
    }
}

fn decimal_average(
    sum: i128,
    count: usize,
    precision: u64,
    scale: u32,
) -> Result<LiteralValue, &'static str> {
    let count = i128::try_from(count).map_err(|_| "AVG count exceeds numeric width")?;
    if count == 0 {
        return Ok(LiteralValue::Null);
    }
    if !within_precision(sum, precision) {
        return Err("DECIMAL AVG accumulator needs unproved engine promotion");
    }
    // Extend a declared source scale only when division terminates without
    // rounding and the exact result is representable on all admitted paths.
    let mut units = sum;
    for places in scale..=18 {
        if units % count == 0 {
            let mean = units / count;
            if !within_precision(mean, precision.max(u64::from(places)))
                || !binary_exact(mean, places)
            {
                return Err("DECIMAL AVG promotion or precision is unproven");
            }
            return Ok(fixed_point(mean, places));
        }
        units = units
            .checked_mul(10)
            .ok_or("DECIMAL AVG overflows exact width")?;
    }
    Err("DECIMAL AVG requires engine rounding or repeating-fraction attestation")
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

fn aggregate_frame(window: &WindowFunctionExpression) -> Result<Option<WindowFrame>, &'static str> {
    if window.window().order_by().is_empty() {
        if window.window().frame().is_some() {
            return Err("unordered ROWS frame has no deterministic physical membership");
        }
        return Ok(None); // An unordered window spans the entire partition.
    }
    rows_frame(window).map(Some)
}

fn aggregate_kind(
    window: &WindowFunctionExpression,
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
) -> Result<WindowKind, &'static str> {
    let function = window.function();
    let [argument] = function.arguments() else {
        return Err("window aggregate requires one normalized argument");
    };
    let operation = match function.name().to_ascii_lowercase().as_str() {
        "count" => {
            if matches!(argument, Expression::Literal(lit)
                if lit.literal_type() == LiteralType::Integer
                && lit.value() == &LiteralValue::Number("1".to_string()))
            {
                AggregateKind::CountAll
            } else {
                AggregateKind::CountExpr
            }
        }
        "sum" => AggregateKind::Sum,
        "avg" => AggregateKind::Avg,
        "min" => AggregateKind::Min,
        "max" => AggregateKind::Max,
        _ => return Err("aggregate window has no certified physical law"),
    };
    let value = match operation {
        AggregateKind::CountAll => None,
        _ => Some(direct_source_column(argument, source, schema)?),
    };
    let numeric = if matches!(
        operation,
        AggregateKind::CountAll | AggregateKind::CountExpr
    ) {
        None
    } else {
        let Some(column) = value
            .as_deref()
            .and_then(|name| schema.columns().iter().find(|c| c.name() == name))
        else {
            return Err("window aggregate argument has no authoritative datatype");
        };
        Some(numeric_law(column.data_type())?)
    };
    Ok(WindowKind::Aggregate {
        operation,
        value,
        frame: aggregate_frame(window)?,
        numeric,
    })
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
        "row_number" | "rank" | "dense_rank" => {
            if window.window().frame().is_some() || window.window().order_by().is_empty() {
                return Err("composed ranking requires ORDER BY without an explicit frame");
            }
            rank_function(window)
                .map(WindowKind::Ranking)
                .ok_or("composed ranking has unsupported arguments")
        }
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
        "count" => aggregate_kind(window, source, schema),
        "sum" | "avg" | "min" | "max" => aggregate_kind(window, source, schema),
        _ => Err("navigation or aggregate function has no certified physical window law"),
    }
}

// A conjunction of canonical window comparisons is evaluated against a single
// complete physical bag. Every comparison is classified under SQL three-valued
// truth: NULL is NOT TRUE. Unsupported Boolean shapes fail closed.
fn qualify_comparisons<'a>(
    query: &'a QueryStatement,
) -> Result<Vec<(&'a WindowFunctionExpression, ComparisonOperator, i128)>, &'static str> {
    fn collect<'a>(
        predicate: &'a Predicate,
        comparisons: &mut Vec<(&'a WindowFunctionExpression, ComparisonOperator, i128)>,
    ) -> Result<(), &'static str> {
        match predicate {
            Predicate::And(group) => {
                for operand in group.operands() {
                    collect(operand, comparisons)?;
                }
            }
            Predicate::Comparison(comparison) => {
                let (window, operator, threshold) =
                    match (comparison.left(), comparison.right()) {
                        (Expression::WindowFunction(window), Expression::Literal(value)) => {
                            (window, comparison.operator(), value)
                        }
                        (Expression::Literal(value), Expression::WindowFunction(window)) => {
                            (window, comparison.operator().reversed(), value)
                        }
                        _ => return Err("QUALIFY needs a canonical window and integer literal"),
                    };
                if matches!(
                    operator,
                    ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
                ) {
                    return Err("NULL-safe QUALIFY needs explicit three-valued comparison evidence");
                }
                if threshold.literal_type() != LiteralType::Integer {
                    return Err("QUALIFY threshold requires an exact integer");
                }
                let LiteralValue::Number(number) = threshold.value() else {
                    return Err("QUALIFY threshold is nonnumeric or NULL");
                };
                let threshold = number
                    .parse::<i128>()
                    .map_err(|_| "QUALIFY threshold exceeds signed width")?;
                comparisons.push((window, operator, threshold));
            }
            _ => return Err("QUALIFY Boolean expression lacks a complete execution law"),
        }
        Ok(())
    }
    let mut comparisons = Vec::new();
    if let Some(predicate) = query.predicates().qualify_predicate() {
        collect(predicate, &mut comparisons)?;
    }
    Ok(comparisons)
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
    let start = frame_position(frame.start_bound(), index, len)?
        .max(0)
        .min(len);
    let end = frame_position(frame.end_bound(), index, len)?
        .max(-1)
        .min(len - 1);
    let start = usize::try_from(start).map_err(|_| "invalid frame start")?;
    let end = usize::try_from(end + 1).map_err(|_| "invalid frame end")?;
    Ok(start..end.max(start))
}

// Exact finite decimal expansions preserve AVG's non-integral result without
// asserting floating-point or repeating-fraction session behavior.
// Integer AVG has engine-specific result datatypes. In particular SQL Server
// returns INT for AVG(INT), so fractional integer AVG cannot be certified as
// portable, even when a rational value terminates in base ten.
fn exact_average(sum: i128, count: usize) -> Result<LiteralValue, &'static str> {
    let count = i128::try_from(count).map_err(|_| "AVG count exceeds numeric width")?;
    if count == 0 {
        return Ok(LiteralValue::Null);
    }
    if sum % count != 0 {
        return Err("fractional AVG of integer input needs attested engine result type");
    }
    let quotient = sum / count;
    if quotient.unsigned_abs() > (1_u128 << 53) {
        return Err("integer AVG exceeds portable floating precision");
    }
    Ok(LiteralValue::Number(quotient.to_string()))
}

fn portable_integer_sum(sum: i128, bits: Option<u16>) -> Result<(), &'static str> {
    let bits = bits.ok_or("integer accumulator requires attested signed width")?;
    if bits == 0 || bits > 128 {
        return Err("integer accumulator has an invalid signed width");
    }
    if bits < 128 {
        let half = 1_i128
            .checked_shl(u32::from(bits - 1))
            .ok_or("integer accumulator width exceeds exact signed width")?;
        if sum < -half || sum >= half {
            return Err("integer SUM or AVG needs unproved engine widening");
        }
    }
    if sum.unsigned_abs() > (1_u128 << 53) {
        return Err("integer SUM or AVG exceeds portable numeric magnitude");
    }
    Ok(())
}

fn aggregate_value(
    operation: AggregateKind,
    value: Option<&str>,
    frame: Option<&WindowFrame>,
    numeric: Option<NumericLaw>,
    rows: &[(usize, Vec<Option<i128>>)],
    index: usize,
    snapshot: &PhysicalWriteState,
) -> Result<LiteralValue, &'static str> {
    let indices = match frame {
        Some(frame) => frame_members(frame, index, rows.len())?,
        None => 0..rows.len(),
    };
    if matches!(operation, AggregateKind::CountAll) {
        return Ok(LiteralValue::Number(indices.len().to_string()));
    }
    let column = value.ok_or("missing window aggregate argument")?;
    let mut count = 0usize;
    let mut sum = 0i128;
    let mut min: Option<i128> = None;
    let mut max: Option<i128> = None;
    for position in indices {
        let (source_index, _) = rows.get(position).ok_or("missing aggregate frame row")?;
        let scalar = snapshot
            .rows()
            .get(*source_index)
            .and_then(|row| row.values().get(column))
            .ok_or("missing aggregate source column")?;
        if matches!(scalar, LiteralValue::Null) {
            continue;
        }
        count = count.checked_add(1).ok_or("COUNT overflow")?;
        if matches!(operation, AggregateKind::CountExpr) {
            continue;
        }
        let law = numeric.ok_or("window aggregate numeric law missing")?;
        let number = numeric_units(scalar, law)?;
        if matches!(operation, AggregateKind::Sum | AggregateKind::Avg) {
            sum = sum
                .checked_add(number)
                .ok_or("SUM or AVG exact numeric overflow")?;
        }
        min = Some(min.map_or(number, |current| current.min(number)));
        max = Some(max.map_or(number, |current| current.max(number)));
    }
    let law = numeric.unwrap_or(NumericLaw::Integer { bits: None });
    if count > 0 && matches!(operation, AggregateKind::Sum | AggregateKind::Avg) {
        if let NumericLaw::Integer { bits } = law {
            portable_integer_sum(sum, bits)?;
        }
    }
    match operation {
        AggregateKind::CountAll | AggregateKind::CountExpr => {
            Ok(LiteralValue::Number(count.to_string()))
        }
        AggregateKind::Sum if count > 0 => numeric_output(sum, law),
        AggregateKind::Avg if count > 0 => match law {
            NumericLaw::Integer { .. } => exact_average(sum, count),
            NumericLaw::Decimal { precision, scale } => {
                decimal_average(sum, count, precision, scale)
            }
        },
        AggregateKind::Avg => Ok(LiteralValue::Null),
        AggregateKind::Min => match min {
            Some(value) => numeric_output(value, law),
            None => Ok(LiteralValue::Null),
        },
        AggregateKind::Max => match max {
            Some(value) => numeric_output(value, law),
            None => Ok(LiteralValue::Null),
        },
        AggregateKind::Sum => Ok(LiteralValue::Null),
    }
}

fn window_keys(
    window: &WindowFunctionExpression,
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
) -> Result<RankKeys, &'static str> {
    if !window.window().order_by().is_empty() {
        return rank_keys(window.window(), source, schema, true);
    }
    let mut partition = Vec::new();
    for item in window.window().partition_by() {
        let column = direct_source_column(item, source, schema)?;
        let datatype = schema
            .columns()
            .iter()
            .find(|c| c.name() == column)
            .ok_or("window partition key missing datatype")?;
        if !super::aggregate_bags::signed_integer(datatype.data_type()) {
            return Err("window partition key lacks exact integer equality");
        }
        partition.push(column);
    }
    Ok(RankKeys {
        partition,
        sort: Vec::new(),
    })
}

fn frame_value(
    kind: &WindowKind,
    rows: &[(usize, Vec<Option<i128>>)],
    index: usize,
    snapshot: &PhysicalWriteState,
) -> Result<LiteralValue, &'static str> {
    if let WindowKind::Aggregate {
        operation,
        value,
        frame,
        numeric,
    } = kind
    {
        return aggregate_value(
            *operation,
            value.as_deref(),
            frame.as_ref(),
            *numeric,
            rows,
            index,
            snapshot,
        );
    }
    // A uniquely ordered partition makes ROW_NUMBER, RANK and DENSE_RANK
    // identical. ORDER BY peers are rejected before any frame is evaluated.
    if let WindowKind::Ranking(function) = kind {
        return match function {
            RankFunction::RowNumber | RankFunction::Rank | RankFunction::DenseRank => {
                let ordinal = index.checked_add(1).ok_or("ranking ordinal overflow")?;
                Ok(LiteralValue::Number(ordinal.to_string()))
            }
        };
    }
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
        WindowKind::Aggregate { .. } | WindowKind::Ranking(_) => {
            return Err("unreachable window frame result")
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
        WindowKind::Aggregate { .. } | WindowKind::Ranking(_) => {
            return Err("unreachable window frame result")
        }
    };
    snapshot.rows()[*source_index]
        .values()
        .get(column)
        .cloned()
        .ok_or("window value column missing from complete source bag")
}

/// Evaluate portable navigation and COUNT/SUM/AVG/MIN/MAX deterministic
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
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
        || (query.predicates().qualify_predicate().is_some() && !query.ranked_goal_output_shape())
        || (query.predicates().qualify_predicate().is_none() && !query.plain_goal_output_shape())
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
                if let Err(reason) =
                    direct_source_column(&Expression::Column(expression.clone()), source, schema)
                {
                    return residual(reason);
                }
            }
            Expression::WindowFunction(window) => {
                let parsed = match window_keys(window, source, schema) {
                    Ok(parsed) => parsed,
                    Err(reason) => return residual(reason),
                };
                if keys.as_ref().is_some_and(|existing| existing != &parsed) {
                    return residual("window expressions use incompatible partition or order keys");
                }
                let unordered = parsed.sort.is_empty();
                keys = Some(parsed);
                let kind = match window_kind(window, source, schema) {
                    Ok(kind) => kind,
                    Err(reason) => return residual(reason),
                };
                if unordered && !matches!(&kind, WindowKind::Aggregate { .. }) {
                    return residual("navigation requires a deterministic physical ordering");
                }
                operations.push(WindowColumn {
                    output: projected.name().to_string(),
                    kind,
                });
            }
            _ => return residual("computed projection lacks a certified row-local value law"),
        }
    }
    let comparisons = match qualify_comparisons(query) {
        Ok(comparisons) => comparisons,
        Err(reason) => return residual(reason),
    };
    let mut qualify = Vec::new();
    for (index, (window, operator, threshold)) in comparisons.into_iter().enumerate() {
        let projected = query.output().columns().iter().find(|column| {
            matches!(column.expression(), Expression::WindowFunction(candidate) if candidate == window)
        });
        let output = if let Some(column) = projected {
            column.name().to_string()
        } else {
            // Internal evaluation slots never leak into terminal projections.
            let output = format!("__task_127_qualify_{index}");
            if !seen_names.insert(output.clone()) {
                return residual("QUALIFY internal slot collides with output alias");
            }
            let parsed = match window_keys(window, source, schema) {
                Ok(parsed) => parsed,
                Err(reason) => return residual(reason),
            };
            if keys.as_ref().is_some_and(|existing| existing != &parsed) {
                return residual("QUALIFY window uses incompatible partition or order keys");
            }
            if parsed.sort.is_empty()
                && !matches!(
                    window.function().name().to_ascii_lowercase().as_str(),
                    "count" | "sum" | "avg" | "min" | "max"
                )
            {
                return residual("QUALIFY ranking requires deterministic ORDER BY");
            }
            keys = Some(parsed);
            let kind = match window_kind(window, source, schema) {
                Ok(kind) => kind,
                Err(reason) => return residual(reason),
            };
            operations.push(WindowColumn {
                output: output.clone(),
                kind,
            });
            output
        };
        qualify.push((output, operator, threshold));
    }
    let Some(keys) = keys else {
        return residual("window target has no supported window function");
    };
    let mut partitions = BTreeMap::<Vec<Option<i128>>, Vec<(usize, Vec<Option<i128>>)>>::new();
    for (index, row) in snapshot.rows().iter().enumerate() {
        let partition = match keys
            .partition
            .iter()
            .map(|key| key_value(row, key))
            .collect()
        {
            Ok(values) => values,
            Err(reason) => return residual(reason),
        };
        let order = match keys
            .sort
            .iter()
            .map(|key| key_value(row, &key.column))
            .collect()
        {
            Ok(values) => values,
            Err(reason) => return residual(reason),
        };
        partitions
            .entry(partition)
            .or_default()
            .push((index, order));
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
        if !keys.sort.is_empty() && partition.windows(2).any(|rows| rows[0].1 == rows[1].1) {
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
    let mut terminal_row_ids = Vec::new();
    let mut rejected_row_ids = Vec::new();
    for (index, row) in snapshot.rows().iter().enumerate() {
        let row_id = format!("{}#{index}", source.name());
        let mut survives = true;
        for (column, operator, threshold) in &qualify {
            let Some(Some(value)) = evaluated.get(column).and_then(|values| values.get(index))
            else {
                return residual("QUALIFY lacks a complete window evaluation");
            };
            // SQL NULL is UNKNOWN for ordinary comparisons and is NOT TRUE.
            let matches = match value {
                LiteralValue::Null => false,
                LiteralValue::Number(number) => {
                    let Ok(number) = number.parse::<i128>() else {
                        return residual("QUALIFY requires an exact integer comparison");
                    };
                    qualifies(number, *operator, *threshold)
                }
                _ => return residual("QUALIFY has no numeric comparison law"),
            };
            if !matches {
                survives = false;
                break;
            }
        }
        if !survives {
            rejected_row_ids.push(row_id);
            continue;
        }
        terminal_row_ids.push(row_id);
        let mut projected = BTreeMap::new();
        for column in query.output().columns() {
            let value = match column.expression() {
                Expression::Column(source_column) => {
                    row.values().get(source_column.name()).cloned()
                }
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
        terminal_row_ids,
        rejected_row_ids,
        terminal,
        terminal_domains: domains,
    }))
}
