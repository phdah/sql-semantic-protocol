//! Physical DAG traversal and schema-backed reference resolution.

use super::*;

pub(super) fn query_for<'a>(
    bundle: &'a AnalysisBundle,
    layer: &TransformationLayer,
) -> Option<&'a QueryStatement> {
    let input = bundle
        .inputs()
        .iter()
        .find(|input| input.id() == layer.input_id())?;
    match input.statements().get(layer.statement_index())? {
        ProtocolStatement::Query(query) => Some(query),
        ProtocolStatement::Unsupported(_) => None,
    }
}

pub(super) fn transparent_projection(query: &QueryStatement) -> bool {
    query.row_preserving_projection()
        && query.sources().len() == 1
        && query.diagnostics().is_empty()
        && query.condition_exactness().is_exact()
        && query
            .output()
            .columns()
            .iter()
            .all(|column| column.plain_copy_source().is_some())
}

/// A regular, non-empty grouping key list cannot form a group from no input
/// rows. ROLLUP, CUBE and GROUPING SETS may contain the empty grouping set,
/// which emits a global group even when every input table is empty.
/// Conservative expressions that evaluate once per input row without
/// arithmetic overflow, casts, opaque functions or row-generating behavior.
/// A CASE of simple source comparisons and literal/copy results is safe for
/// cardinality, even though its produced value is not invertible lineage.
pub(super) fn total_scalar_projection(expression: &Expression) -> bool {
    match expression {
        Expression::Column(_) | Expression::Literal(_) => true,
        Expression::Case(case) => {
            case.operand().is_none_or(total_scalar_projection)
                && case.branches().iter().all(|branch| {
                    let condition_is_total = match branch.condition() {
                        Expression::BooleanPredicate(predicate) => {
                            total_scalar_predicate(predicate)
                        }
                        condition => total_scalar_projection(condition),
                    };
                    condition_is_total && total_scalar_projection(branch.result())
                })
                && case.else_result().is_none_or(total_scalar_projection)
        }
        Expression::BooleanPredicate(predicate) => total_scalar_predicate(predicate),
        Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::Function(_)
        | Expression::ScalarSubquery(_)
        | Expression::SignedIntegerCast(_)
        | Expression::Unary(_)
        | Expression::Binary(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => false,
    }
}

pub(super) fn total_scalar_predicate(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Comparison(comparison) => {
            matches!(
                comparison.left(),
                Expression::Column(_) | Expression::Literal(_)
            ) && matches!(
                comparison.right(),
                Expression::Column(_) | Expression::Literal(_)
            )
        }
        Predicate::IsNull(test) => total_scalar_projection(test.expression()),
        Predicate::And(logical) | Predicate::Or(logical) => {
            logical.operands().iter().all(total_scalar_predicate)
        }
        Predicate::Not(negated) => total_scalar_predicate(negated.operand()),
        Predicate::LikePrefix(like) => total_scalar_projection(like.expression()),
        Predicate::Between(_)
        | Predicate::In(_)
        | Predicate::BooleanExpression(_)
        | Predicate::Exists(_)
        | Predicate::InSubquery(_)
        | Predicate::Unknown(_)
        | Predicate::Unsupported(_) => false,
    }
}

pub(super) fn empty_input_eliminates_groups(query: &QueryStatement) -> bool {
    matches!(
        query.aggregation().and_then(|aggregation| aggregation.group_by()),
        Some(GroupBy::Expressions(groups))
            if !groups.is_empty()
                && groups.iter().all(|group| matches!(group, GroupingExpression::Expression(_)))
    )
}

pub(super) fn row_local_predicate(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Comparison(comparison) => {
            row_local_expression(comparison.left()) && row_local_expression(comparison.right())
        }
        Predicate::LikePrefix(like) => row_local_expression(like.expression()),
        Predicate::And(logical) | Predicate::Or(logical) => {
            logical.operands().iter().all(row_local_predicate)
        }
        Predicate::Not(not) => row_local_predicate(not.operand()),
        Predicate::IsNull(null) => row_local_expression(null.expression()),
        Predicate::In(values) => {
            row_local_expression(values.expression())
                && values.values().iter().all(row_local_expression)
        }
        Predicate::Between(between) => {
            row_local_expression(between.expression())
                && row_local_expression(between.lower())
                && row_local_expression(between.upper())
        }
        Predicate::BooleanExpression(expression) => row_local_expression(expression),
        Predicate::Exists(_)
        | Predicate::InSubquery(_)
        | Predicate::Unknown(_)
        | Predicate::Unsupported(_) => false,
    }
}

/// In the absence of grouping, expressions must be scalar on each input row.
/// An aggregate nested inside COALESCE, arithmetic or a cast can synthesize a
/// single result on an empty input, even if the query's projection/WHERE shape
/// otherwise looks row-preserving.
pub(super) fn row_local_expression(expression: &Expression) -> bool {
    match expression {
        Expression::Column(_) | Expression::Literal(_) => true,
        Expression::Function(function) => function.arguments().iter().all(row_local_expression),
        Expression::SignedIntegerCast(cast) => row_local_expression(cast.expression()),
        Expression::Unary(unary) => row_local_expression(unary.operand()),
        Expression::Binary(binary) => {
            row_local_expression(binary.left()) && row_local_expression(binary.right())
        }
        Expression::Case(case) => {
            case.operand().is_none_or(row_local_expression)
                && case.branches().iter().all(|branch| {
                    row_local_expression(branch.condition())
                        && row_local_expression(branch.result())
                })
                && case.else_result().is_none_or(row_local_expression)
        }
        Expression::BooleanPredicate(predicate) => row_local_predicate(predicate),
        Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => false,
    }
}

/// Every query-shaped leaf must be a single-row-boundary read, without
/// GROUPING SETS, global aggregates or opaque producers. Set operators,
/// including DISTINCT and nested bag operations, cannot create a tuple from
/// entirely empty leaf inputs.
pub(super) fn empty_input_eliminates_set(
    operation: &SetOperation,
    layer: &TransformationLayer,
) -> bool {
    fn leaf_count(operation: &SetOperation) -> usize {
        let count = |operand: &SetOperand| match operand {
            SetOperand::Query => 1,
            SetOperand::Operation(nested) => leaf_count(nested),
        };
        count(operation.left()) + count(operation.right())
    }
    if operation.branches().len() != leaf_count(operation) {
        return false;
    }
    operation.branches().iter().all(|branch| {
        branch.sources().len() == 1
            && branch.empty_input_preserving()
            && branch.predicates().having_predicate().is_none()
            && branch.sources().iter().all(|source| {
                layer
                    .consumes()
                    .contains(&layer.canonical_relation(source.name()))
            })
    })
}

/// A completely empty controllable source ensures zero output only through
/// transformations whose row-shape cannot invent rows.
///
/// An empty source is not a constructive proof of any positive cardinality,
/// and must not be applied to global aggregates or source-free projections.
pub(super) fn prove_zero_rows(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    target: &str,
) -> WitnessDirection {
    if walker.sources.is_empty() {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    }
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let Some(layer) = walker.layers.get(id.as_str()).copied() else {
            return residual(PhysicalProofGap::MissingProducer);
        };
        let Some(query) = query_for(bundle, layer) else {
            return residual(PhysicalProofGap::UnresolvedSemantics);
        };
        let regular_grouping = empty_input_eliminates_groups(query);
        let duplicate_elimination = query
            .aggregation()
            .is_some_and(|aggregation| aggregation.distinct() && aggregation.group_by().is_none());
        let ranked_window = query.ranked_goal_output_shape() && query.window_witness().is_some();
        let set_empty = query
            .set_operation()
            .is_some_and(|operation| empty_input_eliminates_set(operation, layer));
        if (query.sources().is_empty() && !set_empty)
            || (query.aggregation().is_some() && !regular_grouping && !duplicate_elimination)
            || (query.set_operation().is_some() && !set_empty)
            || query.proven_single_row_output()
            || (query.predicates().having_predicate().is_some() && !regular_grouping)
            // Unsupported ORDER BY value/order expressions may affect which
            // nonempty rows survive LIMIT, but cannot synthesize rows from
            // an empty input. All other diagnostics remain blocking.
            || (!regular_grouping
                && !set_empty
                && query
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code() != "unsupported_order_by"))
            || (!regular_grouping
                && !ranked_window
                && !set_empty
                && !query
                    .output()
                    .columns()
                    .iter()
                    .all(|column| row_local_expression(column.expression())))
        {
            return residual(PhysicalProofGap::NonInvertibleTransformation);
        }
        if set_empty {
            // Individual leaves already establish a row-preserving, named
            // boundary. Any bag or set operator on zero leaves returns zero.
            continue;
        }
        if query.joins().is_empty() {
            // Single-source transparent copies and filter-only queries are
            // zero-preserving. No assumption about predicate satisfiability is
            // needed when the entire physical source is controlled as empty.
            if query.sources().len() != 1
                || !(query.row_preserving_projection()
                    || query.filter_only_row_shape()
                    || regular_grouping
                    || duplicate_elimination
                    || ranked_window)
            {
                return residual(PhysicalProofGap::NonInvertibleTransformation);
            }
        } else {
            // A known binary join of *named, controlled* row sources is empty
            // when all inputs are empty, regardless of INNER/OUTER/SEMI/ANTI
            // multiplicity. An opaque joined relation or row producer may
            // emit rows independently of the named physical sources.
            let names = query
                .sources()
                .iter()
                .map(|source| source.name())
                .collect::<BTreeSet<_>>();
            if query.joins().iter().any(|join| {
                join.kind() == crate::protocol::JoinKind::Unknown
                    || !names.contains(join.left().relation())
                    || !names.contains(join.right().relation())
            }) || names
                .iter()
                .any(|name| !layer.consumes().contains(&layer.canonical_relation(name)))
            {
                return residual(PhysicalProofGap::UnsupportedOperator);
            }
        }
    }
    let Some(bounds) = CountBounds::new(0, Some(0)) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    let mut obligations = Vec::new();
    for source in &walker.sources {
        let Some(boundary) = WitnessBoundary::new(source, GroupBoundaryKind::Physical, target)
        else {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        };
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds,
            predicate: WitnessFormula::IsNull {
                term: WitnessTerm::Integer(1),
                negated: true,
            },
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
    }
    let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::UnresolvedSemantics))
}

/// Catalog-attested datatype for a column reference, when its relation
/// declares the column. Missing schemas or columns stay unproved.
fn bundle_column_datatype(
    bundle: &AnalysisBundle,
    column: &crate::protocol::ColumnRef,
) -> Option<crate::data_type::DataType> {
    let relation = column.relation()?;
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == relation)?;
    super::joint_counts::physical_column_datatype(schema, column)
}

/// Reject untransported relation laws at a materialization boundary.
///
/// A copied value or widening CAST does not discharge constraints declared
/// independently on the produced table. Defaults, generated expressions,
/// CHECKs and even unresolved diagnostics must not silently disappear when
/// its rows are mapped back to a physical source.
pub(super) fn untransported_materialization_laws(bundle: &AnalysisBundle, relation: &str) -> bool {
    bundle.relation_constraints().iter().any(|set| {
        set.relation() == relation
            && (!set.constraints().is_empty()
                || !set.column_value_sources().is_empty()
                || !set.diagnostics().is_empty())
    })
}

/// Follow an identity-only column through an intermediate producer. The
/// producer is permitted to filter its own rows because the *joint* terminal
/// proof later requires all filters, but the intermediate's local witness is
/// never independently relabeled as a physical-source sufficient proof.
pub(super) fn resolve_filter_column(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    consumer_id: &str,
    column: &crate::protocol::ColumnRef,
    depth: usize,
) -> Option<crate::protocol::ColumnRef> {
    resolve_filter_column_with_evidence(bundle, walker, consumer_id, column, depth, false)
}

/// Trace the same identity-only reference, optionally proving equal declared
/// datatypes across *every* materialization along the producer path. A
/// matching endpoint alone cannot justify unseen intermediate coercions.
pub(super) fn resolve_filter_column_with_evidence(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    consumer_id: &str,
    column: &crate::protocol::ColumnRef,
    depth: usize,
    certified_types: bool,
) -> Option<crate::protocol::ColumnRef> {
    if depth > walker.layers.len() {
        return None;
    }
    let layer = walker.layers.get(consumer_id).copied()?;
    let canonical = layer.canonical_relation(column.relation()?);
    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.consumer_layer_id() == consumer_id && edge.relation() == canonical)?;
    match edge.resolution() {
        RelationResolution::External => Some(crate::protocol::ColumnRef::new(
            Some(edge.relation().to_string()),
            column.name().to_string(),
        )),
        RelationResolution::Resolved => {
            if untransported_materialization_laws(bundle, &canonical) {
                return None;
            }
            let [producer_id] = edge.producer_layer_ids() else {
                return None;
            };
            let producer = walker.layers.get(producer_id.as_str()).copied()?;
            let query = query_for(bundle, producer)?;
            if query.sources().len() != 1
                || !(query.row_preserving_projection() || query.filter_only_row_shape())
                || !query.joins().is_empty()
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || !query.diagnostics().is_empty()
            {
                return None;
            }
            let mut output = query
                .output()
                .columns()
                .iter()
                .filter(|item| item.name() == column.name());
            let actual = output.next()?.plain_copy_source()?;
            if output.next().is_some() {
                return None;
            }
            let upstream = crate::protocol::ColumnRef::new(
                Some(actual.relation().to_string()),
                actual.column().to_string(),
            );
            if certified_types {
                let consumed_type = bundle
                    .source_schemas()
                    .iter()
                    .find(|schema| schema.relation() == canonical)?
                    .columns()
                    .iter()
                    .find(|item| item.name() == column.name())?
                    .data_type();
                let upstream_relation = upstream.relation()?;
                let upstream_type = bundle
                    .source_schemas()
                    .iter()
                    .find(|schema| schema.relation() == upstream_relation)?
                    .columns()
                    .iter()
                    .find(|item| item.name() == upstream.name())?
                    .data_type();
                if consumed_type != upstream_type {
                    return None;
                }
            }
            resolve_filter_column_with_evidence(
                bundle,
                walker,
                producer.id(),
                &upstream,
                depth + 1,
                certified_types,
            )
        }
        _ => None,
    }
}

/// The one certified computed projection this physical mapper can invert:
/// an explicit, total signed-integer widening CAST of a single physical
/// column. Its declared materialized width must match the CAST result.
/// Plain copies are permitted alongside it but must retain equal datatypes.
fn widened_or_copied_source(
    bundle: &AnalysisBundle,
    relation: &str,
    output: &crate::protocol::OutputColumn,
    query: &QueryStatement,
) -> Option<crate::protocol::ColumnRef> {
    use crate::data_type::DataType;

    fn signed_width(value: &DataType) -> Option<u16> {
        match value {
            DataType::SignedInteger { bits: Some(bits) } if *bits <= 64 => Some(*bits),
            DataType::Nullable(inner) => signed_width(inner),
            _ => None,
        }
    }

    let [input] = query.sources() else {
        return None;
    };
    let produced =
        crate::protocol::ColumnRef::new(Some(relation.to_string()), output.name().to_string());
    let to = bundle_column_datatype(bundle, &produced)?;
    match output.expression() {
        crate::protocol::Expression::Column(_) => {
            let lineage = output.plain_copy_source()?;
            if lineage.relation() != input.name() {
                return None;
            }
            let physical = crate::protocol::ColumnRef::new(
                Some(lineage.relation().to_string()),
                lineage.column().to_string(),
            );
            (bundle_column_datatype(bundle, &physical)? == to).then_some(physical)
        }
        crate::protocol::Expression::SignedIntegerCast(cast) => {
            // A computed CAST does not necessarily carry plain-copy lineage.
            // Resolve its canonical operand instead; the single named source
            // removes ambiguity without reconstructing SQL or trusting hints.
            let crate::protocol::Expression::Column(column) = cast.expression() else {
                return None;
            };
            let physical = crate::domain::resolve_column(column, query.sources());
            if physical.relation() != Some(input.name()) {
                return None;
            }
            let from = bundle_column_datatype(bundle, &physical)?;
            // A declared NOT NULL boundary cannot silently discard source
            // NULLs. Preserve the complete SQL NULL domain across the cast.
            if matches!(&from, DataType::Nullable(_)) != matches!(&to, DataType::Nullable(_)) {
                return None;
            }
            let source_bits = signed_width(&from)?;
            let target_bits = signed_width(&to)?;
            (source_bits <= target_bits && target_bits == cast.target_bits()).then_some(physical)
        }
        _ => None,
    }
}

/// Only complete, unfiltered producer definitions with total projections
/// are admissible. CAST inversion cannot make a partial DML producer physical.
fn total_widening_producer(
    bundle: &AnalysisBundle,
    node: &PhysicalPlanNode,
    query: &QueryStatement,
) -> bool {
    let [relation] = node.produced_relations() else {
        return false;
    };
    matches!(node.write_kind(), Some(WriteKind::Definition))
        && query.sources().len() == 1
        && query.joins().is_empty()
        && query.aggregation().is_none()
        && query.set_operation().is_none()
        && query.window_witness().is_none()
        && query.subquery_witnesses().is_empty()
        && query.predicates().where_predicate().is_none()
        && query.predicates().having_predicate().is_none()
        && query.predicates().qualify_predicate().is_none()
        && query.diagnostics().is_empty()
        && !query.output().columns().is_empty()
        && query
            .output()
            .columns()
            .iter()
            .all(|column| widened_or_copied_source(bundle, relation, column, query).is_some())
}

/// Resolve only those computed materializations whose entire producer chain
/// consists of identity columns or certified lossless signed widening casts.
fn resolve_widened_filter_column(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    consumer_id: &str,
    column: &crate::protocol::ColumnRef,
    depth: usize,
) -> Option<crate::protocol::ColumnRef> {
    if depth > walker.layers.len() {
        return None;
    }
    let layer = walker.layers.get(consumer_id).copied()?;
    let canonical = layer.canonical_relation(column.relation()?);
    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.consumer_layer_id() == consumer_id && edge.relation() == canonical)?;
    match edge.resolution() {
        RelationResolution::External => Some(crate::protocol::ColumnRef::new(
            Some(edge.relation().to_string()),
            column.name().to_string(),
        )),
        RelationResolution::Resolved => {
            // A materialized relation may carry its own CHECK, key, default
            // or generated-column law not entailed by a copied source value.
            // Until those laws are transported, such edges are not invertible.
            if untransported_materialization_laws(bundle, &canonical) {
                return None;
            }
            let [producer_id] = edge.producer_layer_ids() else {
                return None;
            };
            let node = walker
                .nodes
                .iter()
                .find(|node| node.id() == &PhysicalPlanRef::Layer(producer_id.clone()))?;
            let producer = walker.layers.get(producer_id.as_str()).copied()?;
            let query = query_for(bundle, producer)?;
            if !total_widening_producer(bundle, node, query) {
                return None;
            }
            let mut outputs = query
                .output()
                .columns()
                .iter()
                .filter(|output| output.name() == column.name());
            let output = outputs.next()?;
            if outputs.next().is_some() {
                return None;
            }
            let physical = widened_or_copied_source(bundle, &canonical, output, query)?;
            resolve_widened_filter_column(bundle, walker, producer.id(), &physical, depth + 1)
        }
        _ => None,
    }
}

/// Normalize sequential physical WHERE filters into one candidate-row
/// formula, then solve TRUE versus FALSE/UNKNOWN jointly. Distinct independent
/// sufficient examples must never simply be conjoined without this proof.
pub(super) fn joint_physical_filters(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    semantics: &crate::bundle::ResolvedComposedSemantics,
) -> Option<(WitnessDirection, WitnessDirection)> {
    let filters = semantics.boolean_witnesses();
    if filters.is_empty()
        || filters.len() > 12
        || walker.sources.len() != 1
        || (filters.len() == 1 && filters[0].boundary_kind() != GroupBoundaryKind::Intermediate)
    {
        return None;
    }
    let source = walker.sources.iter().next()?;
    // No source-row population may silently ignore enforced or unverified
    // physical constraints when transporting an intermediate witness.
    if blocks_unconstrained_physical_plans(bundle, source) {
        return None;
    }
    if filters
        .iter()
        .any(|filter| filter.boundary_kind() == GroupBoundaryKind::Unresolved)
    {
        return None;
    }
    let origin_ids = filters
        .iter()
        .map(|filter| filter.origin_layer_id())
        .collect::<BTreeSet<_>>();
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        // An append, update or merge reads an existing target state and
        // cannot be modeled as an independently constructed filter chain.
        // Only full definitions and plain read-only queries are safe here.
        if !matches!(node.write_kind(), None | Some(WriteKind::Definition)) {
            return None;
        }
        if origin_ids.contains(layer.id()) {
            if !query.filter_only_row_shape()
                || query.sources().len() != 1
                || !query.joins().is_empty()
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || query.window_witness().is_some()
                || !query.subquery_witnesses().is_empty()
                || query.predicates().where_predicate().is_none()
                || query.predicates().having_predicate().is_some()
                || query.predicates().qualify_predicate().is_some()
                || !query.diagnostics().is_empty()
                || !query
                    .output()
                    .columns()
                    .iter()
                    .all(|c| c.plain_copy_source().is_some())
            {
                return None;
            }
        } else if !transparent_projection(query) && !total_widening_producer(bundle, node, query) {
            return None;
        }
    }
    let physical_witnesses = filters
        .iter()
        .map(|filter| match filter.boundary_kind() {
            GroupBoundaryKind::Physical => {
                (filter.witness().source_relation() == source).then(|| filter.witness().clone())
            }
            GroupBoundaryKind::Intermediate => {
                // Typed comparisons cross a materialization boundary only
                // with certified identical datatypes at both ends of the
                // identity chain. Conditions needing no type evidence keep
                // the historical uncertified path so untyped NULL-test
                // chains still conjoin.
                let certified = filter.witness().mapped_to_physical_certified(
                    |column| {
                        resolve_filter_column_with_evidence(
                            bundle,
                            walker,
                            filter.origin_layer_id(),
                            column,
                            0,
                            true,
                        )
                    },
                    |original, mapped| {
                        let original_type = bundle_column_datatype(bundle, original);
                        let mapped_type = bundle_column_datatype(bundle, mapped);
                        original_type.is_some() && original_type == mapped_type
                    },
                );
                certified
                    .or_else(|| {
                        filter.witness().mapped_to_physical(|column| {
                            resolve_filter_column(
                                bundle,
                                walker,
                                filter.origin_layer_id(),
                                column,
                                0,
                            )
                        })
                    })
                    .or_else(|| {
                        let schema = bundle
                            .source_schemas()
                            .iter()
                            .find(|schema| schema.relation() == source)?;
                        filter.witness().mapped_to_physical_widened(
                            |column| {
                                resolve_widened_filter_column(
                                    bundle,
                                    walker,
                                    filter.origin_layer_id(),
                                    column,
                                    0,
                                )
                            },
                            |column| physical_integer_evidence(schema, column),
                        )
                    })
            }
            GroupBoundaryKind::Unresolved => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if physical_witnesses
        .iter()
        .any(|witness| witness.source_relation() != source)
    {
        return None;
    }
    let physical_references = physical_witnesses.iter().collect::<Vec<_>>();
    let joint =
        crate::boolean_witness::BooleanWitness::conjoin_physical_filters(&physical_references)?;
    let row = crate::constructive::RowVariable::new(source, source, "candidate")?;
    let translate = |direction: &crate::boolean_witness::BooleanWitnessDirection| match direction {
        crate::boolean_witness::BooleanWitnessDirection::Exact(truth) => WitnessCase::new(
            vec![WitnessObligation::Predicate(WitnessFormula::RowTruth {
                row: row.clone(),
                predicate: joint.condition().clone(),
                truth: *truth,
            })],
            ProofStrength::Sufficient,
        )
        .and_then(|case| WitnessDirection::feasible(vec![case]))
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven)),
        crate::boolean_witness::BooleanWitnessDirection::Impossible => WitnessDirection::Impossible,
        crate::boolean_witness::BooleanWitnessDirection::Residual { .. } => {
            residual(PhysicalProofGap::LocalWitnessUnproven)
        }
    };
    Some((translate(joint.qualifying()), translate(joint.rejected())))
}
