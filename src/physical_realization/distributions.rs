//! Canonical physical-column inversion for complete terminal histograms.
//!
//! A histogram describes every row. Only independently verified row-preserving
//! producer boundaries can be traversed, and each derived scalar must invert
//! to an actual typed physical column with checked integer arithmetic.

use super::*;
use crate::constraints::ConstraintValue;
use crate::data_type::DataType;
use crate::outcome_goals::{OutcomeGoal, OutputValueCount};
use crate::protocol::{
    BinaryOperator, ColumnRef, LiteralType, LiteralValue, UnaryOperator,
};

/// Construct complete physical histograms through type-certified copy producers,
/// optionally inverting a guarded reversible terminal integer expression.
/// Intermediate projections are never independently writable.
pub(crate) fn physical_distribution_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &OutcomeGoal,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    let rows = goal.rows()?;
    if rows == 0 || goal.groups().is_some() || goal.distributions().is_empty() {
        return None;
    }
    let physical = physical_source_plan(bundle, target_layer_id);
    let [source] = physical.sources() else {
        return None;
    };
    if !matches!(
        physical_row_count_plan(bundle, target_layer_id, rows),
        WitnessDirection::Feasible(_)
    ) {
        return None;
    }

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.window_witness().is_some()
            || !query.subquery_witnesses().is_empty()
            || !query.joins().is_empty()
            || !query.diagnostics().is_empty()
            || (id != target_layer_id && !transparent_projection(query))
            || (id == target_layer_id && !safe_terminal_projection(query, goal))
        {
            return None;
        }
    }

    let target = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, target)?;
    let mut mappings = Vec::new();
    for distribution in goal.distributions() {
        let mut matches = query
            .output()
            .columns()
            .iter()
            .filter(|column| column.name() == distribution.column());
        let projected = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let [lineage] = projected.lineage() else {
            return None;
        };
        let column = ColumnRef::new(
            Some(lineage.relation().to_string()),
            lineage.column().to_string(),
        );
        let physical_column = if lineage.relation() == source {
            column
        } else {
            resolve_filter_column_with_evidence(
                bundle,
                &walker,
                target_layer_id,
                &column,
                0,
                true,
            )?
        };
        if physical_column.relation() != Some(source.as_str()) {
            return None;
        }

        let values = if projected.plain_copy_source().is_some() {
            distribution.values().to_vec()
        } else {
            let bits = integer_width(bundle, source, physical_column.name())?;
            distribution
                .values()
                .iter()
                .map(|bucket| {
                    let input = match bucket.value() {
                        ConstraintValue::Null => ConstraintValue::Null,
                        ConstraintValue::Integer(output) => {
                            let source_value = invert_integer(
                                projected.expression(),
                                *output,
                                lineage.column(),
                            )?;
                            if !fits_integer(source_value, bits)
                                || evaluate_integer(
                                    projected.expression(),
                                    source_value,
                                    lineage.column(),
                                    bits,
                                )? != *output
                            {
                                return None;
                            }
                            ConstraintValue::Integer(source_value)
                        }
                        _ => return None,
                    };
                    Some(OutputValueCount::new(input, bucket.rows()))
                })
                .collect::<Option<Vec<_>>>()?
        };
        mappings.push((physical_column.name().to_string(), values));
    }
    crate::outcome_proofs::construct_mapped_source(bundle, source, rows, mappings)
}

/// Row-preserving shape is required for the whole SELECT. Every expression not
/// pinned by a complete histogram must be trivially total for every source row.
fn safe_terminal_projection(query: &QueryStatement, goal: &OutcomeGoal) -> bool {
    query.row_preserving_projection()
        && query.sources().len() == 1
        && query.condition_exactness().is_exact()
        && query.output().columns().iter().all(|column| {
            goal.distributions().iter().any(|goal| goal.column() == column.name())
                || column.plain_copy_source().is_some()
                || matches!(column.expression(), Expression::Literal(_))
        })
}

fn integer_width(bundle: &AnalysisBundle, source: &str, column: &str) -> Option<u16> {
    let schema = bundle.source_schemas().iter().find(|s| s.relation() == source)?;
    let column = schema.columns().iter().find(|c| c.name() == column)?;
    let mut data_type = column.data_type();
    while let DataType::Nullable(inner) = data_type {
        data_type = inner.as_ref();
    }
    match data_type {
        DataType::SignedInteger { bits: Some(bits @ 8..=64) } => Some(*bits),
        _ => None,
    }
}

fn fits_integer(value: i64, bits: u16) -> bool {
    if bits == 64 {
        true
    } else {
        let half = 1_i128 << (bits - 1);
        let value = i128::from(value);
        -half <= value && value < half
    }
}

fn literal_integer(expression: &Expression) -> Option<i64> {
    let Expression::Literal(literal) = expression else {
        return None;
    };
    if literal.literal_type() != LiteralType::Integer {
        return None;
    }
    match literal.value() {
        LiteralValue::Number(text) => text.parse().ok(),
        _ => None,
    }
}

/// Inversion is allowed only for a single column under bijective arithmetic.
fn invert_integer(expression: &Expression, output: i64, name: &str) -> Option<i64> {
    match expression {
        Expression::Column(column) if column.name() == name => Some(output),
        Expression::Unary(unary) => match unary.operator() {
            UnaryOperator::Plus => invert_integer(unary.operand(), output, name),
            UnaryOperator::Minus => invert_integer(unary.operand(), output.checked_neg()?, name),
            UnaryOperator::BitwiseNot => None,
        },
        Expression::Binary(binary) => {
            if let Some(right) = literal_integer(binary.right()) {
                let input = match binary.operator() {
                    BinaryOperator::Add => output.checked_sub(right)?,
                    BinaryOperator::Subtract => output.checked_add(right)?,
                    _ => return None,
                };
                invert_integer(binary.left(), input, name)
            } else if let Some(left) = literal_integer(binary.left()) {
                let input = match binary.operator() {
                    BinaryOperator::Add => output.checked_sub(left)?,
                    BinaryOperator::Subtract => left.checked_sub(output)?,
                    _ => return None,
                };
                invert_integer(binary.right(), input, name)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Validate the *forward* expression and every intermediate arithmetic result
/// against the physical signed integer width. This rules out source overflow,
/// coercion and spurious inverse witnesses.
fn evaluate_integer(expression: &Expression, input: i64, name: &str, bits: u16) -> Option<i64> {
    let value = match expression {
        Expression::Column(column) if column.name() == name => input,
        Expression::Literal(_) => literal_integer(expression)?,
        Expression::Unary(unary) => {
            let operand = evaluate_integer(unary.operand(), input, name, bits)?;
            match unary.operator() {
                UnaryOperator::Plus => operand,
                UnaryOperator::Minus => operand.checked_neg()?,
                UnaryOperator::BitwiseNot => return None,
            }
        }
        Expression::Binary(binary) => {
            let left = evaluate_integer(binary.left(), input, name, bits)?;
            let right = evaluate_integer(binary.right(), input, name, bits)?;
            match binary.operator() {
                BinaryOperator::Add => left.checked_add(right)?,
                BinaryOperator::Subtract => left.checked_sub(right)?,
                _ => return None,
            }
        }
        _ => return None,
    };
    fits_integer(value, bits).then_some(value)
}
