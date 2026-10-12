//! Canonical physical-column inversion for complete terminal histograms.
//!
//! A histogram describes every row. Only independently verified row-preserving
//! producer boundaries can be traversed, and each derived scalar must invert
//! to an actual typed physical column with checked integer arithmetic.

use super::*;
use crate::constraints::ConstraintValue;
use crate::data_type::DataType;
use crate::outcome_goals::{OutcomeGoal, OutputValueCount};
use crate::protocol::{BinaryOperator, ColumnRef, LiteralType, LiteralValue, UnaryOperator};

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
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let mut covered = BTreeSet::<(String, String)>::new();
    let mut mappings = Vec::new();
    for distribution in goal.distributions() {
        let assignment = invert_distribution(
            bundle,
            &walker,
            target_layer_id,
            distribution.column(),
            distribution.values().to_vec(),
            &mut covered,
            0,
        )?;
        if assignment.column.relation() != Some(source.as_str()) {
            return None;
        }
        mappings.push((assignment.column.name().to_string(), assignment.values));
    }

    // Every expression in a materialized producer must be total for the
    // complete, explicitly assigned input bag. Uncontrolled computations
    // could abort a statement and must remain residual.
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if !safe_projection(query, id, &covered) {
            return None;
        }
    }
    crate::outcome_proofs::construct_mapped_source(bundle, source, rows, mappings)
}

/// One fully traced physical column and its complete frequency bag.
struct InvertedDistribution {
    column: ColumnRef,
    values: Vec<OutputValueCount>,
    data_type: DataType,
}

/// Walk one output alias backwards through every producer, applying each
/// locally normalized inverse and checking the declared intermediate type.
fn invert_distribution(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    layer_id: &str,
    output_name: &str,
    values: Vec<OutputValueCount>,
    covered: &mut BTreeSet<(String, String)>,
    depth: usize,
) -> Option<InvertedDistribution> {
    if depth > walker.layers.len() {
        return None;
    }
    let layer = walker.layers.get(layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let mut outputs = query
        .output()
        .columns()
        .iter()
        .filter(|output| output.name() == output_name);
    let output = outputs.next()?;
    if outputs.next().is_some() {
        return None;
    }
    let [lineage] = output.lineage() else {
        return None;
    };
    let canonical = layer.canonical_relation(lineage.relation());
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == canonical)?;
    let column = schema.columns().iter().find(|col| col.name() == lineage.column())?;
    let data_type = column.data_type().clone();
    let values = if output.plain_copy_source().is_some() {
        values
    } else {
        if !reversible_integer_expression(output.expression(), lineage.column()) {
            return None;
        }
        let bits = signed_integer_width(&data_type)?;
        values
            .iter()
            .map(|bucket| {
                let input = match bucket.value() {
                    ConstraintValue::Null => ConstraintValue::Null,
                    ConstraintValue::Integer(wanted) => {
                        let actual = invert_integer(output.expression(), *wanted, lineage.column())?;
                        if !fits_integer(actual, bits)
                            || evaluate_integer(
                                output.expression(), actual, lineage.column(), bits,
                            )? != *wanted
                        {
                            return None;
                        }
                        ConstraintValue::Integer(actual)
                    }
                    _ => return None,
                };
                Some(OutputValueCount::new(input, bucket.rows()))
            })
            .collect::<Option<Vec<_>>>()?
    };
    covered.insert((layer_id.to_string(), output_name.to_string()));
    let mut edges = bundle.graph().edges().iter().filter(|edge| {
        edge.consumer_layer_id() == layer_id && edge.relation() == canonical
    });
    let edge = edges.next()?;
    if edges.next().is_some() {
        return None;
    }
    match edge.resolution() {
        RelationResolution::External => Some(InvertedDistribution {
            column: ColumnRef::new(
                Some(edge.relation().to_string()),
                lineage.column().to_string(),
            ),
            values,
            data_type,
        }),
        RelationResolution::Resolved => {
            let [producer_id] = edge.producer_layer_ids() else {
                return None;
            };
            let assignment = invert_distribution(
                bundle,
                walker,
                producer_id,
                lineage.column(),
                values,
                covered,
                depth + 1,
            )?;
            // No intermediate write, narrowing or implicit cast is inferred.
            if assignment.data_type != data_type {
                return None;
            }
            Some(assignment)
        }
        _ => None,
    }
}

/// A complete histogram pins the values of each checked computation for
/// *every* row. Non-covered computations must be trivially total.
fn safe_projection(
    query: &QueryStatement,
    layer_id: &str,
    covered: &BTreeSet<(String, String)>,
) -> bool {
    query.row_preserving_projection()
        && query.sources().len() == 1
        && query.condition_exactness().is_exact()
        && query.aggregation().is_none()
        && query.set_operation().is_none()
        && query.window_witness().is_none()
        && query.subquery_witnesses().is_empty()
        && query.joins().is_empty()
        && query.diagnostics().is_empty()
        && query.output().columns().iter().all(|column| {
            covered.contains(&(layer_id.to_string(), column.name().to_string()))
                || column.plain_copy_source().is_some()
                || matches!(column.expression(), Expression::Literal(_))
        })
}

fn signed_integer_width(data_type: &DataType) -> Option<u16> {
    let mut data_type = data_type;
    while let DataType::Nullable(inner) = data_type {
        data_type = inner.as_ref();
    }
    match data_type {
        DataType::SignedInteger {
            bits: Some(bits @ 8..=64),
        } => Some(*bits),
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

/// NULL is inverted only after proving the complete expression is
/// NULL-propagating and reversible for every non-NULL source value.
fn reversible_integer_expression(expression: &Expression, name: &str) -> bool {
    match expression {
        Expression::Column(column) => column.name() == name,
        Expression::Unary(unary) => {
            matches!(unary.operator(), UnaryOperator::Plus | UnaryOperator::Minus)
                && reversible_integer_expression(unary.operand(), name)
        }
        Expression::Binary(binary) => {
            matches!(
                binary.operator(),
                BinaryOperator::Add | BinaryOperator::Subtract
            ) && ((literal_integer(binary.right()).is_some()
                && reversible_integer_expression(binary.left(), name))
                || (literal_integer(binary.left()).is_some()
                    && reversible_integer_expression(binary.right(), name)))
        }
        _ => false,
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
