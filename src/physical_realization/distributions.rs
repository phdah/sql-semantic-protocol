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

/// Exact, complete physical-source frequencies for one terminal goal.
struct MappedDistributions {
    source: String,
    rows: u64,
    columns: Vec<(String, Vec<OutputValueCount>)>,
}

/// Construct complete physical histograms through type-certified copy producers,
/// optionally inverting a guarded reversible terminal integer expression.
/// Intermediate projections are never independently writable.
pub(crate) fn physical_distribution_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &OutcomeGoal,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    let assignment = mapped_distributions(bundle, target_layer_id, goal)?;
    crate::outcome_proofs::construct_mapped_source(
        bundle,
        &assignment.source,
        assignment.rows,
        assignment.columns,
    )
}

/// Two value distributions are mathematically incompatible when their fully
/// certified inverses target the very same physical column with different
/// complete frequencies. An unproved inverse is residual, not impossible.
pub(crate) fn physical_distribution_conflict(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &OutcomeGoal,
) -> bool {
    let Some(assignment) = mapped_distributions(bundle, target_layer_id, goal) else {
        return false;
    };
    let mut seen = BTreeMap::<String, Vec<OutputValueCount>>::new();
    for (column, mut values) in assignment.columns {
        values.sort_by(|left, right| left.value().cmp(right.value()));
        if let Some(other) = seen.get(&column) {
            if !crate::outcome_goals::equivalent_frequencies(other, &values) {
                return true;
            }
        } else {
            seen.insert(column, values);
        }
    }
    false
}

/// Map and validate every intermediate expression before considering either
/// feasibility or a necessary contradiction. This avoids inferring
/// impossibility from underconstrained or partially produced relations.
fn mapped_distributions(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &OutcomeGoal,
) -> Option<MappedDistributions> {
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
    if !unconstrained_produced_relations(bundle, &walker) {
        return None;
    }
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
    Some(MappedDistributions {
        source: source.clone(),
        rows,
        columns: mappings,
    })
}

/// Invert an exact terminal histogram back to a proven materialized set
/// producer. The existing canonical set-bag witness still owns branch tuple
/// laws and all physical-source requirements; this function only certifies
/// the downstream, cardinality-preserving scalar transformation.
pub(crate) fn physical_inverted_set_distribution_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &OutcomeGoal,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    let rows = goal.rows()?;
    let [distribution] = goal.distributions() else {
        return None;
    };
    if rows == 0 || goal.groups().is_some() {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if !unconstrained_produced_relations(bundle, &walker) {
        return None;
    }
    let sets = walker
        .nodes
        .iter()
        .filter_map(|node| {
            let PhysicalPlanRef::Layer(id) = node.id() else {
                return None;
            };
            let layer = walker.layers.get(id.as_str()).copied()?;
            query_for(bundle, layer)?
                .set_operation()
                .is_some()
                .then_some(id.as_str())
        })
        .collect::<Vec<_>>();
    let [set_id] = sets.as_slice() else {
        return None;
    };
    if *set_id == target_layer_id {
        return None;
    }

    let mut current = target_layer_id;
    let mut output_name = distribution.column().to_string();
    let mut values = distribution.values().to_vec();
    let mut covered = BTreeSet::<(String, String)>::new();
    let mut expected_type = None::<DataType>;
    for _ in 0..walker.layers.len() {
        if current == *set_id {
            break;
        }
        let layer = walker.layers.get(current).copied()?;
        if !matches!(layer.write_kind(), None | Some(WriteKind::Definition)) {
            return None;
        }
        let query = query_for(bundle, layer)?;
        covered.insert((current.to_string(), output_name.clone()));
        if !safe_projection(query, current, &covered) {
            return None;
        }
        let mut output = query
            .output()
            .columns()
            .iter()
            .filter(|item| item.name() == output_name);
        let projected = output.next()?;
        if output.next().is_some() {
            return None;
        }
        let [lineage] = projected.lineage() else {
            return None;
        };
        if values.iter().any(|bucket| {
            bucket.rows() > 0
                && crate::domain::domain_admits_constraint_value(projected.domain(), bucket.value())
                    == Some(false)
        }) {
            return None;
        }
        let canonical = layer.canonical_relation(lineage.relation());
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == canonical)?;
        let input = schema
            .columns()
            .iter()
            .find(|column| column.name() == lineage.column())?;
        let data_type = input.data_type().clone();
        if expected_type
            .as_ref()
            .is_some_and(|expected| expected != &data_type)
        {
            return None;
        }
        values = invert_values(
            projected.expression(),
            projected.plain_copy_source().is_some(),
            lineage.column(),
            &data_type,
            values,
        )?;
        let mut edges = bundle
            .graph()
            .edges()
            .iter()
            .filter(|edge| edge.consumer_layer_id() == current && edge.relation() == canonical);
        let edge = edges.next()?;
        if edges.next().is_some() || edge.resolution() != RelationResolution::Resolved {
            return None;
        }
        let [producer] = edge.producer_layer_ids() else {
            return None;
        };
        let node = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(current.to_string()))?;
        if node.inputs() != [PhysicalPlanRef::Layer(producer.clone())] {
            return None;
        }
        current = producer;
        output_name = lineage.column().to_string();
        expected_type = Some(data_type);
    }
    if current != *set_id {
        return None;
    }

    let set_layer = walker.layers.get(*set_id).copied()?;
    let set_query = query_for(bundle, set_layer)?;
    let [column] = set_query.output().columns() else {
        return None;
    };
    if column.name() != output_name {
        return None;
    }
    let output_type = expected_type?;
    let histogram = crate::outcome_goals::OutputDistribution::new(output_name, values).ok()?;
    let at_set = OutcomeGoal::new(set_layer.id(), Some(rows), None, vec![histogram]).ok()?;
    let witness = physical_materialized_set_witness(bundle, set_layer.id(), &at_set)?;
    // The set's own materialization may implicitly coerce a branch even
    // when downstream inverse arithmetic and branch-local laws are exact.
    // Every physical tuple column must have the attested set output type.
    let crate::outcome_proofs::OutcomeWitness::SetTuples { case, .. } = &witness else {
        return None;
    };
    for obligation in case.obligations() {
        let boundary = obligation.boundary();
        let [column] = boundary.tuple_columns() else {
            return None;
        };
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == boundary.relation())?;
        let source_column = schema.columns().iter().find(|item| item.name() == column)?;
        if source_column.data_type() != &output_type {
            return None;
        }
    }
    Some(witness)
}

/// Fail closed when any materialized producer carries unproved source
/// constraints. A transparent relational projection is not an exemption from
/// accepted values, uniqueness or NOT NULL obligations.
fn unconstrained_produced_relations(bundle: &AnalysisBundle, walker: &Walker<'_>) -> bool {
    walker.nodes.iter().all(|node| {
        node.produced_relations().iter().all(|relation| {
            !bundle
                .relation_constraints()
                .iter()
                .any(|set| set.relation() == relation && set.blocks_unconstrained_physical_plans())
        })
    })
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
    if values.iter().any(|bucket| {
        bucket.rows() > 0
            && crate::domain::domain_admits_constraint_value(output.domain(), bucket.value())
                == Some(false)
    }) {
        return None;
    }
    let canonical = layer.canonical_relation(lineage.relation());
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == canonical)?;
    let column = schema
        .columns()
        .iter()
        .find(|col| col.name() == lineage.column())?;
    let data_type = column.data_type().clone();
    let values = invert_values(
        output.expression(),
        output.plain_copy_source().is_some(),
        lineage.column(),
        &data_type,
        values,
    )?;
    covered.insert((layer_id.to_string(), output_name.to_string()));
    let mut edges = bundle
        .graph()
        .edges()
        .iter()
        .filter(|edge| edge.consumer_layer_id() == layer_id && edge.relation() == canonical);
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

/// Invert one normalized, NULL-propagating scalar projection, retaining
/// exact frequencies and verifying *both* inverse and forward arithmetic.
fn invert_values(
    expression: &Expression,
    plain_copy: bool,
    column_name: &str,
    input_type: &DataType,
    values: Vec<OutputValueCount>,
) -> Option<Vec<OutputValueCount>> {
    if plain_copy {
        return Some(values);
    }
    if !reversible_integer_expression(expression, column_name) {
        return None;
    }
    let bits = signed_integer_width(input_type)?;
    values
        .into_iter()
        .map(|bucket| {
            let input = match bucket.value() {
                ConstraintValue::Null => ConstraintValue::Null,
                ConstraintValue::Integer(wanted) => {
                    let actual = invert_integer(expression, *wanted, column_name)?;
                    if !fits_integer(actual, bits)
                        || evaluate_integer(expression, actual, column_name, bits)? != *wanted
                    {
                        return None;
                    }
                    ConstraintValue::Integer(actual)
                }
                _ => return None,
            };
            Some(OutputValueCount::new(input, bucket.rows()))
        })
        .collect()
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
