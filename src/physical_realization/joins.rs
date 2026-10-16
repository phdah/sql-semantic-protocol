//! Join population constructive proofs.

use super::*;

fn exact_source_filter(
    bundle: &AnalysisBundle,
    source: &crate::protocol::SourceRelation,
    predicate: &Predicate,
) -> Option<(String, WitnessFormula)> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source.name())?;
    if bundle
        .relation_constraints()
        .iter()
        .any(|set| set.relation() == source.name() && set.blocks_unconstrained_physical_plans())
    {
        return None;
    }
    let witness = crate::boolean_witness::analyze_physical_scalar(
        Some(predicate),
        std::slice::from_ref(source),
        |column| {
            (column.relation() == Some(source.name()))
                .then(|| physical_integer_evidence(schema, column))
                .flatten()
        },
        |_column| None,
        |column| {
            (column.relation() == Some(source.name()))
                .then(|| physical_column_datatype(schema, column))
                .flatten()
        },
    );
    if let Some(witness) = witness.filter(|witness| {
        matches!(
            witness.qualifying(),
            crate::boolean_witness::BooleanWitnessDirection::Exact(
                crate::boolean_witness::BooleanTruthCase::True
            )
        )
    }) {
        let row = RowVariable::new(source.name(), source.name(), "candidate")?;
        return Some((
            source.name().to_string(),
            WitnessFormula::RowTruth {
                row,
                predicate: witness.condition().clone(),
                truth: crate::boolean_witness::BooleanTruthCase::True,
            },
        ));
    }
    typed_literal_equality_filter(source, schema, predicate)
}

fn typed_literal_equality_filter(
    source: &crate::protocol::SourceRelation,
    schema: &crate::relation::RelationSchema,
    predicate: &Predicate,
) -> Option<(String, WitnessFormula)> {
    use crate::protocol::{ComparisonOperator, LiteralType, LiteralValue};

    let Predicate::Comparison(comparison) = predicate else {
        return None;
    };
    if comparison.operator() != ComparisonOperator::Eq {
        return None;
    }
    let (Expression::Column(column), Expression::Literal(literal)) =
        (comparison.left(), comparison.right())
    else {
        return None;
    };
    if column
        .relation()
        .is_some_and(|relation| relation != source.name())
    {
        return None;
    }
    let resolved =
        crate::protocol::ColumnRef::new(Some(source.name().to_string()), column.name().to_string());
    let typed = schema
        .columns()
        .iter()
        .find(|item| item.name() == resolved.name())?;
    let term = match (literal.literal_type(), literal.value()) {
        (LiteralType::Boolean, LiteralValue::Boolean(value))
            if typed.data_type().kind() == "boolean" =>
        {
            WitnessTerm::Boolean(*value)
        }
        (LiteralType::Integer, LiteralValue::Number(value)) => {
            let integer = value.parse::<i64>().ok()?;
            let bounds = physical_integer_evidence(schema, &resolved)?;
            if i128::from(integer) < bounds.minimum || i128::from(integer) > bounds.maximum {
                return None;
            }
            WitnessTerm::Integer(integer)
        }
        _ => return None,
    };
    Some((
        source.name().to_string(),
        WitnessFormula::Comparison {
            left: WitnessTerm::Column {
                row: RowVariable::new(source.name(), source.name(), "filtered_source")?,
                column: resolved,
            },
            operator: ComparisonOperator::Eq,
            right: term,
        },
    ))
}

fn source_filter_for_predicate(
    bundle: &AnalysisBundle,
    sources: &[crate::protocol::SourceRelation],
    predicate: &Predicate,
) -> Option<(String, WitnessFormula)> {
    let mut filters = sources
        .iter()
        .filter_map(|source| exact_source_filter(bundle, source, predicate));
    let filter = filters.next()?;
    filters.next().is_none().then_some(filter)
}

fn formula_references_column(
    formula: &WitnessFormula,
    column: &crate::protocol::ColumnRef,
) -> bool {
    match formula {
        WitnessFormula::All(items) | WitnessFormula::Any(items) => items
            .iter()
            .any(|item| formula_references_column(item, column)),
        WitnessFormula::Not(item) => formula_references_column(item, column),
        WitnessFormula::RowTruth { predicate, .. } => predicate.references_column(column),
        WitnessFormula::Comparison { left, right, .. } => {
            term_references_column(left, column) || term_references_column(right, column)
        }
        WitnessFormula::IsNull { term, .. } | WitnessFormula::StringPrefix { term, .. } => {
            term_references_column(term, column)
        }
        WitnessFormula::TupleComparison { left, right, .. } => left
            .iter()
            .chain(right)
            .any(|term| term_references_column(term, column)),
    }
}

fn term_references_column(term: &WitnessTerm, column: &crate::protocol::ColumnRef) -> bool {
    matches!(term, WitnessTerm::Column { column: actual, .. } if actual == column)
}

fn combined_filter(filters: &[WitnessFormula]) -> WitnessFormula {
    match filters {
        [one] => one.clone(),
        many => WitnessFormula::All(many.to_vec()),
    }
}

fn join_key_and_scalar_filters(
    query: &QueryStatement,
) -> Option<(
    &crate::protocol::ColumnExpression,
    &crate::protocol::ColumnExpression,
    Vec<&Predicate>,
)> {
    use crate::protocol::ComparisonOperator;

    let condition = query.joins().first()?.condition()?;
    let operands = match condition {
        Predicate::And(and) => and.operands().iter().collect::<Vec<_>>(),
        predicate => vec![predicate],
    };
    let mut key = None;
    let mut filters = Vec::new();
    for operand in operands {
        let Predicate::Comparison(comparison) = operand else {
            filters.push(operand);
            continue;
        };
        let (Expression::Column(left), Expression::Column(right)) =
            (comparison.left(), comparison.right())
        else {
            filters.push(operand);
            continue;
        };
        let left = crate::domain::resolve_column(left, query.sources());
        let right = crate::domain::resolve_column(right, query.sources());
        if comparison.operator() == ComparisonOperator::Eq
            && left.relation().is_some()
            && right.relation().is_some()
            && left.relation() != right.relation()
        {
            if key.is_some() {
                return None;
            }
            key = Some(match (comparison.left(), comparison.right()) {
                (Expression::Column(left), Expression::Column(right)) => (left, right),
                _ => return None,
            });
        } else {
            filters.push(operand);
        }
    }
    let (left, right) = key?;
    if let Some(predicate) = query.predicates().where_predicate() {
        filters.push(predicate);
    }
    Some((left, right, filters))
}

/// Exact positive row selector for a direct filtered producer. Repeating a
/// typed row is safe for an unconstrained physical bag; a join-key restriction
/// must be reconciled separately and is intentionally unsupported here.
fn direct_filtered_source(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    node: &PhysicalPlanNode,
) -> Option<(String, WitnessFormula)> {
    let [PhysicalPlanRef::Source(source)] = node.inputs() else {
        return None;
    };
    let PhysicalPlanRef::Layer(id) = node.id() else {
        return None;
    };
    let layer = walker.layers.get(id.as_str()).copied()?;
    let query = query_for(bundle, layer)?;
    let [input] = query.sources() else {
        return None;
    };
    if input.name() != source
        || !matches!(node.write_kind(), Some(WriteKind::Definition))
        || !query.filter_only_row_shape()
        || !query.joins().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.diagnostics().is_empty()
        || !query
            .output()
            .columns()
            .iter()
            .all(|column| total_scalar_projection(column.expression()))
    {
        return None;
    }
    exact_source_filter(bundle, input, query.predicates().where_predicate()?)
}

/// Realize one local equijoin through a complete physical source population
/// shared by all its named producers and aliases. No intermediate relation is
/// treated as writable. Each source receives one cardinality and one assigned
/// integer key sequence, with SQL bag/outer/semi/anti laws checked symbolically.
pub(super) fn physical_join_population_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    physical_join_population_with_source_counts(bundle, target_layer_id, wanted, &BTreeMap::new())
}

/// Enumerate additional complete empty-partner join populations when a
/// second terminal fixes the surviving source's exact positive row count.
/// An empty right/left side cannot create a match, but the same law is not
/// valid for preserved outer or anti-join sides; the existing join-kind
/// cardinality computation decides those cases before admitting a witness.
pub(super) fn physical_join_population_with_source_counts(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
    exact_sources: &BTreeMap<String, u64>,
) -> Option<WitnessDirection> {
    use crate::join_witness::JoinWitnessDirection;
    use crate::protocol::JoinKind;

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let join_layers = walker
        .nodes
        .iter()
        .filter_map(|node| {
            let PhysicalPlanRef::Layer(id) = node.id() else {
                return None;
            };
            let layer = walker.layers.get(id.as_str()).copied()?;
            (query_for(bundle, layer)?.joins().len() == 1).then_some(layer)
        })
        .collect::<Vec<_>>();
    let [join_layer] = join_layers.as_slice() else {
        return None;
    };
    let join_query = query_for(bundle, join_layer)?;
    if join_query.joins().len() != 1
        || join_query.sources().len() != 2
        || join_query.aggregation().is_some()
        || join_query.set_operation().is_some()
        || join_query.window_witness().is_some()
        || !join_query.subquery_witnesses().is_empty()
        || join_query.predicates().having_predicate().is_some()
        || join_query.predicates().qualify_predicate().is_some()
        || !join_query.diagnostics().is_empty()
        || join_query.output().columns().iter().any(|column| {
            !matches!(
                column.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
    {
        return None;
    }
    let (left_key_expression, right_key_expression, scalar_filters) =
        join_key_and_scalar_filters(join_query)?;

    let ComposedSemantics::Resolved(semantics) = join_layer.composed_semantics() else {
        return None;
    };
    let join = match semantics.join_witnesses() {
        [join] if join.origin_layer_id() == join_layer.id() => Some(join),
        _ => None,
    };
    let join_kind = join_query.joins()[0].kind();
    if matches!(join_kind, JoinKind::Cross | JoinKind::Unknown) {
        return None;
    }

    // Identity-only producer chains remain supported. A direct filtered
    // producer may additionally select a typed, repeatable literal equality
    // on an independent column. Its computed projection must be total so
    // that it cannot change row count. No derived input is writable.
    let mut source_filters = BTreeMap::<String, Vec<WitnessFormula>>::new();
    for predicate in scalar_filters {
        let (source, formula) =
            source_filter_for_predicate(bundle, join_query.sources(), predicate)?;
        source_filters.entry(source).or_default().push(formula);
    }
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if id == join_layer.id() {
            continue;
        }
        let layer = walker.layers.get(id.as_str()).copied()?;
        if !matches!(node.write_kind(), None | Some(WriteKind::Definition)) {
            return None;
        }
        if transparent_projection(query_for(bundle, layer)?) {
            continue;
        }
        let (source, predicate) = direct_filtered_source(bundle, &walker, node)?;
        source_filters.entry(source).or_default().push(predicate);
    }

    // A residual local join witness cannot certify source rows through a
    // filtered producer. The physical construction below independently
    // proves an ordinary typed equijoin over complete filtered source bags.
    // Without such a filter, retain the stronger original local gate.
    if source_filters.is_empty() && !matches!(join?.qualifying(), JoinWitnessDirection::Exact(_)) {
        return None;
    }

    // The target may follow the join through a linear sequence of copies,
    // never through unrelated branches or a second join.
    let mut descendant = target_layer_id;
    for _ in 0..walker.layers.len() {
        if descendant == join_layer.id() {
            break;
        }
        let node = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(descendant.to_string()))?;
        let [PhysicalPlanRef::Layer(parent)] = node.inputs() else {
            return None;
        };
        descendant = parent;
    }
    if descendant != join_layer.id() {
        return None;
    }

    let (left_key, left_instance, right_key, right_instance) = if source_filters.is_empty() {
        let join = join?;
        if join.comparison() != Some(crate::protocol::ComparisonOperator::Eq) {
            return None;
        }
        let map = |endpoint: &crate::bundle::ComposedJoinColumn| {
            let column = crate::protocol::ColumnRef::new(
                Some(endpoint.relation().to_string()),
                endpoint.column().to_string(),
            );
            let mapped = if walker.sources.contains(endpoint.relation()) {
                column
            } else {
                resolve_filter_column_with_evidence(
                    bundle,
                    &walker,
                    join_layer.id(),
                    &column,
                    0,
                    true,
                )?
            };
            walker
                .sources
                .contains(mapped.relation()?)
                .then(|| (mapped, endpoint.relation_instance().to_string()))
        };
        let (left, left_instance) = map(join.left()?)?;
        let (right, right_instance) = map(join.right()?)?;
        (left, left_instance, right, right_instance)
    } else {
        // A composed local witness may be residual for filtered parents
        // and therefore have no endpoints. Recover them from the *same*
        // canonical query's typed ON comparison, never by reparsing SQL.
        let map = |column: &crate::protocol::ColumnExpression| {
            let instance = column.relation()?;
            let source = crate::protocol::ColumnRef::new(
                Some(instance.to_string()),
                column.name().to_string(),
            );
            let mapped = resolve_filter_column(bundle, &walker, join_layer.id(), &source, 0)?;
            walker
                .sources
                .contains(mapped.relation()?)
                .then_some((mapped, instance.to_string()))
        };
        let (left, left_instance) = map(left_key_expression)?;
        let (right, right_instance) = map(right_key_expression)?;
        (left, left_instance, right, right_instance)
    };
    if left_instance == right_instance {
        return None;
    }
    let left_source = left_key.relation()?;
    let right_source = right_key.relation()?;
    // The construction controls join keys independently of source filters.
    // If a filter mentions either join key, the literal must instead be
    // reconciled with each candidate population, which is not done here.
    if source_filters.iter().any(|(source, filters)| {
        (source == left_source
            && filters
                .iter()
                .any(|filter| formula_references_column(filter, &left_key)))
            || (source == right_source
                && filters
                    .iter()
                    .any(|filter| formula_references_column(filter, &right_key)))
    }) {
        return None;
    }
    // Require actual attested physical signed integer key types. An opaque
    // type, uniqueness contract or range restriction cannot be guessed.
    let left_schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == left_source)?;
    let right_schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == right_source)?;
    let left_bounds = physical_integer_evidence(left_schema, &left_key)?;
    let right_bounds = physical_integer_evidence(right_schema, &right_key)?;
    if left_bounds.minimum > 0
        || right_bounds.minimum > 0
        || left_bounds.maximum < 0
        || right_bounds.maximum < 0
        || [left_source, right_source].iter().any(|relation| {
            bundle
                .relation_constraints()
                .iter()
                .any(|set| set.relation() == *relation && set.blocks_unconstrained_physical_plans())
        })
    {
        return None;
    }

    let left_row = RowVariable::new(left_source, &left_instance, "join_left")?;
    let right_row = RowVariable::new(right_source, &right_instance, "join_right")?;
    let mut candidates = vec![
        (JoinPopulationPattern::DistinctMatched, wanted, wanted),
        (JoinPopulationPattern::CommonMatched, wanted, 1),
        (JoinPopulationPattern::CommonMatched, 1, wanted),
        (JoinPopulationPattern::EmptyRight, wanted, 0),
        (JoinPopulationPattern::EmptyLeft, 0, wanted),
    ];
    if wanted == 0 {
        candidates.push((JoinPopulationPattern::EmptyLeft, 0, 0));
        if let Some(&count) = exact_sources.get(left_source) {
            candidates.push((JoinPopulationPattern::EmptyRight, count, 0));
        }
        if let Some(&count) = exact_sources.get(right_source) {
            candidates.push((JoinPopulationPattern::EmptyLeft, 0, count));
        }
    }
    // Enumerate bounded factorizations without floating-point roundoff.
    // These are constructive examples only, never an exhaustive proof that
    // other key histograms cannot realize the requested bag count.
    for divisor in 2..=wanted.isqrt().min(64) {
        if wanted.is_multiple_of(divisor) {
            let other = wanted / divisor;
            candidates.push((JoinPopulationPattern::CommonMatched, divisor, other));
            candidates.push((JoinPopulationPattern::CommonMatched, other, divisor));
        }
    }
    // Homogeneous key groups extend the two extreme constructions:
    // one shared key or a bijection. Each matched group has separately
    // controlled source contributors and contributes their product.
    for groups in 2..=wanted.min(64) {
        if !wanted.is_multiple_of(groups) {
            continue;
        }
        let product = wanted / groups;
        candidates.push((
            JoinPopulationPattern::GroupedMatched { groups },
            wanted,
            groups,
        ));
        candidates.push((
            JoinPopulationPattern::GroupedMatched { groups },
            groups,
            wanted,
        ));
        for left_per_group in 2..=product.isqrt().min(64) {
            if product.is_multiple_of(left_per_group) {
                let right_per_group = product / left_per_group;
                candidates.push((
                    JoinPopulationPattern::GroupedMatched { groups },
                    groups.checked_mul(left_per_group)?,
                    groups.checked_mul(right_per_group)?,
                ));
            }
        }
    }
    let mut cases = Vec::new();
    for (pattern, left_rows, right_rows) in candidates {
        if pattern.output_rows(join_kind, left_rows, right_rows) != Some(wanted)
            || (left_source == right_source && left_rows != right_rows)
        {
            continue;
        }
        let max_value = match pattern {
            JoinPopulationPattern::DistinctMatched => i128::from(left_rows.saturating_sub(1)),
            JoinPopulationPattern::GroupedMatched { groups } => i128::from(groups - 1),
            JoinPopulationPattern::CommonMatched
            | JoinPopulationPattern::EmptyLeft
            | JoinPopulationPattern::EmptyRight => 0,
        };
        if (left_rows > 0 && max_value > left_bounds.maximum)
            || (right_rows > 0 && max_value > right_bounds.maximum)
        {
            continue;
        }
        let mut physical_counts = BTreeMap::new();
        physical_counts.insert(left_source.to_string(), left_rows);
        physical_counts.insert(right_source.to_string(), right_rows);
        let mut obligations = Vec::new();
        for (relation, count) in &physical_counts {
            let boundary =
                WitnessBoundary::new(relation, GroupBoundaryKind::Physical, target_layer_id)?;
            let bounds = CountBounds::new(*count, Some(*count))?;
            obligations.push(WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate: source_filters
                    .get(relation)
                    .map(|filters| combined_filter(filters))
                    .unwrap_or_else(count_tautology),
                closed_world: true,
            });
            obligations.push(WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            });
        }
        obligations.push(WitnessObligation::JoinPopulation {
            left_row: left_row.clone(),
            right_row: right_row.clone(),
            left_key: left_key.clone(),
            right_key: right_key.clone(),
            join_kind,
            pattern,
            left_rows,
            right_rows,
            output_rows: wanted,
            closed_world: true,
        });
        match pattern {
            JoinPopulationPattern::DistinctMatched
            | JoinPopulationPattern::CommonMatched
            | JoinPopulationPattern::GroupedMatched { .. } => {
                // The complete key-population law establishes all pairs.
                // Retain an existing typed local matched-pair obligation
                // as one representative without substituting it for that law.
                obligations.push(WitnessObligation::JoinPair {
                    left_row: left_row.clone(),
                    right_row: right_row.clone(),
                    left: left_key.clone(),
                    right: right_key.clone(),
                    comparison: crate::protocol::ComparisonOperator::Eq,
                    null_extended: None,
                });
            }
            JoinPopulationPattern::EmptyRight if left_rows > 0 => {
                obligations.push(WitnessObligation::NoMatchingPartner {
                    candidate: left_row.clone(),
                    partner: right_row.clone(),
                    comparison: crate::protocol::ComparisonOperator::Eq,
                    left: left_key.clone(),
                    right: right_key.clone(),
                    null_extended: matches!(join_kind, JoinKind::Left | JoinKind::Full)
                        .then_some(crate::join_witness::JoinSide::Right),
                    closed_world: true,
                });
            }
            JoinPopulationPattern::EmptyLeft if right_rows > 0 => {
                obligations.push(WitnessObligation::NoMatchingPartner {
                    candidate: right_row.clone(),
                    partner: left_row.clone(),
                    comparison: crate::protocol::ComparisonOperator::Eq,
                    left: right_key.clone(),
                    right: left_key.clone(),
                    null_extended: matches!(join_kind, JoinKind::Right | JoinKind::Full)
                        .then_some(crate::join_witness::JoinSide::Left),
                    closed_world: true,
                });
            }
            JoinPopulationPattern::EmptyLeft | JoinPopulationPattern::EmptyRight => {}
        }
        obligations.push(WitnessObligation::OutputRows {
            layer_id: target_layer_id.to_string(),
            bounds: CountBounds::new(wanted, Some(wanted))?,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            if !cases.contains(&case) {
                cases.push(case);
            }
        }
    }
    WitnessDirection::feasible(cases)
}

/// A transparent terminal's row count is its one physical source's count,
/// including zero; a filtered terminal count is only sufficient evidence.
pub(super) fn exact_transparent_source(bundle: &AnalysisBundle, target: &str) -> Option<String> {
    let physical = physical_source_plan(bundle, target);
    let [source] = physical.sources() else {
        return None;
    };
    for node in physical.nodes() {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = bundle.layers().iter().find(|layer| layer.id() == id)?;
        if !transparent_projection(query_for(bundle, layer)?)
            || !matches!(node.write_kind(), None | Some(WriteKind::Definition))
        {
            return None;
        }
    }
    Some(source.clone())
}

/// Positive INNER and SEMI joins necessarily read at least one row from
/// each side. A *necessary* transparent zero source count therefore rules
/// out the output; this is not inferred from missing sufficient examples.
pub(super) fn necessarily_missing_positive_join_input(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> bool {
    use crate::protocol::JoinKind;
    let empty = targets
        .iter()
        .filter(|(_, rows)| *rows == 0)
        .filter_map(|(layer, _)| exact_transparent_source(bundle, layer))
        .collect::<BTreeSet<_>>();
    if empty.is_empty() {
        return false;
    }
    targets
        .iter()
        .filter(|(_, rows)| *rows > 0)
        .any(|&(layer, rows)| {
            let Some(WitnessDirection::Feasible(cases)) =
                physical_join_population_count_plan(bundle, layer, rows)
            else {
                return false;
            };
            cases.iter().any(|case| {
                case.obligations().iter().any(|obligation| {
                    let WitnessObligation::JoinPopulation {
                        left_row,
                        right_row,
                        join_kind,
                        ..
                    } = obligation
                    else {
                        return false;
                    };
                    matches!(
                        join_kind,
                        JoinKind::Inner | JoinKind::LeftSemi | JoinKind::RightSemi
                    ) && (empty.contains(left_row.relation())
                        || empty.contains(right_row.relation()))
                })
            })
        })
}

/// A complete, transparent physical source with N > 0 necessarily yields
/// at least N rows on its preserved side of a pure LEFT/RIGHT/FULL join.
/// This is an impossibility law (not merely exhaustion of constructed join
/// populations), and only applies after the same join proof has validated
/// the exact simple-join shape and physical identities.
pub(super) fn necessarily_nonempty_preserved_join_side(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> bool {
    use crate::protocol::JoinKind;

    let fixed = targets
        .iter()
        .filter_map(|&(layer, rows)| {
            exact_transparent_source(bundle, layer).map(|source| (source, rows))
        })
        .collect::<BTreeMap<_, _>>();
    if !fixed.values().any(|count| *count > 0) {
        return false;
    }
    targets
        .iter()
        .filter(|(_, rows)| *rows == 0)
        .any(|&(layer, _)| {
            let Some(WitnessDirection::Feasible(cases)) =
                physical_join_population_count_plan(bundle, layer, 0)
            else {
                return false;
            };
            cases.iter().any(|case| {
                case.obligations().iter().any(|obligation| {
                    let WitnessObligation::JoinPopulation {
                        left_row,
                        right_row,
                        join_kind,
                        ..
                    } = obligation
                    else {
                        return false;
                    };
                    let left = fixed
                        .get(left_row.relation())
                        .is_some_and(|count| *count > 0);
                    let right = fixed
                        .get(right_row.relation())
                        .is_some_and(|count| *count > 0);
                    match join_kind {
                        JoinKind::Left => left,
                        JoinKind::Right => right,
                        JoinKind::Full => left || right,
                        _ => false,
                    }
                })
            })
        })
}

/// Compose closed physical key populations across multiple terminal goals.
/// Never cross-product independently feasible examples unless every selected
/// case can share identical source-row counts and compatible key assignments.
/// The finite candidate list is a sufficient construction only: exhaustion
/// or a conflict among examples is residual, not an impossibility proof.
pub(super) fn jointly_realized_join_goals(
    bundle: &AnalysisBundle,
    goals: &[(&str, u64)],
) -> Option<WitnessDirection> {
    if goals.len() < 2 {
        return None;
    }
    // Row-preserving terminals provide necessary, complete source counts.
    // A zero-count join can use an empty partner while preserving those
    // positive source rows, rather than demanding that both parents vanish.
    let mut exact_sources = BTreeMap::<String, u64>::new();
    for &(layer_id, rows) in goals {
        if let Some(source) = exact_transparent_source(bundle, layer_id) {
            if exact_sources
                .insert(source, rows)
                .is_some_and(|previous| previous != rows)
            {
                return Some(WitnessDirection::Impossible);
            }
        }
    }
    let mut alternatives = Vec::new();
    let mut includes_join = false;
    for &(layer_id, rows) in goals {
        let join_proof =
            physical_join_population_with_source_counts(bundle, layer_id, rows, &exact_sources);
        includes_join |= join_proof.is_some();
        let direction =
            join_proof.unwrap_or_else(|| physical_row_count_plan(bundle, layer_id, rows));
        let WitnessDirection::Feasible(cases) = direction else {
            return None;
        };
        if cases.is_empty() || cases.len() > 16 {
            return Some(residual(PhysicalProofGap::MultipleWitnesses));
        }
        alternatives.push(cases);
    }
    if !includes_join {
        return None;
    }
    let mut conjunctions = vec![Vec::<WitnessObligation>::new()];
    for choices in alternatives {
        if conjunctions.len().saturating_mul(choices.len()) > 256 {
            return Some(residual(PhysicalProofGap::MultipleWitnesses));
        }
        let mut next = Vec::new();
        for prior in conjunctions {
            for choice in &choices {
                let mut obligations = prior.clone();
                obligations.extend_from_slice(choice.obligations());
                next.push(obligations);
            }
        }
        conjunctions = next;
    }
    let mut cases = Vec::new();
    for obligations in conjunctions {
        let mut counts = BTreeMap::<String, u64>::new();
        let mut assignments = BTreeMap::<(String, String), (JoinPopulationPattern, u64)>::new();
        let mut populations = Vec::new();
        let mut valid = true;
        for obligation in &obligations {
            match obligation {
                WitnessObligation::Rows {
                    boundary,
                    bounds,
                    predicate,
                    quantifier: RowQuantifier::ForAll,
                    closed_world: true,
                } if boundary.kind() == GroupBoundaryKind::Physical
                    && bounds.maximum() == Some(bounds.minimum())
                    && *predicate == count_tautology() =>
                {
                    let count = bounds.minimum();
                    if counts
                        .insert(boundary.relation().to_string(), count)
                        .is_some_and(|prior| prior != count)
                    {
                        valid = false;
                        break;
                    }
                }
                WitnessObligation::JoinPopulation {
                    left_row,
                    right_row,
                    left_key,
                    right_key,
                    pattern,
                    left_rows,
                    right_rows,
                    ..
                } => {
                    // Two different joins can constrain the *same physical
                    // key*. Reconcile their complete value assignment, not
                    // just matching scalar types and row counts.
                    if matches!(
                        pattern,
                        JoinPopulationPattern::DistinctMatched
                            | JoinPopulationPattern::CommonMatched
                            | JoinPopulationPattern::GroupedMatched { .. }
                    ) {
                        for (row, key, count) in [
                            (left_row, left_key, left_rows),
                            (right_row, right_key, right_rows),
                        ] {
                            let assignment = (*pattern, *count);
                            let identity = (row.relation().to_string(), key.name().to_string());
                            if assignments
                                .insert(identity, assignment)
                                .is_some_and(|prior| {
                                    prior != assignment &&
                                    // A one-row source has identical keys
                                    // under both zero-based patterns.
                                    !(prior.1 == 1 && assignment.1 == 1)
                                })
                            {
                                valid = false;
                                break;
                            }
                        }
                    }
                    populations.push(obligation.clone());
                }
                WitnessObligation::ClosedWorld {
                    boundary,
                    coverage: ClosedWorldCoverage::EntireRelation,
                } if boundary.kind() == GroupBoundaryKind::Physical => {}
                WitnessObligation::JoinPair { .. }
                | WitnessObligation::NoMatchingPartner {
                    closed_world: true, ..
                }
                | WitnessObligation::OutputRows { .. } => {}
                _ => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid {
            continue;
        }
        let mut result = Vec::new();
        let origin = goals[0].0;
        for (source, rows) in &counts {
            let Some(boundary) = WitnessBoundary::new(source, GroupBoundaryKind::Physical, origin)
            else {
                valid = false;
                break;
            };
            let Some(bounds) = CountBounds::new(*rows, Some(*rows)) else {
                valid = false;
                break;
            };
            result.push(WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate: count_tautology(),
                closed_world: true,
            });
            result.push(WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            });
        }
        if !valid {
            continue;
        }
        result.extend(populations);
        // The original join-local existence and absence facts remain
        // grounded in the certified population, not independent examples.
        for obligation in &obligations {
            if matches!(
                obligation,
                WitnessObligation::JoinPair { .. }
                    | WitnessObligation::NoMatchingPartner {
                        closed_world: true,
                        ..
                    }
            ) && !result.contains(obligation)
            {
                result.push(obligation.clone());
            }
        }
        for &(layer_id, rows) in goals {
            let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
                valid = false;
                break;
            };
            result.push(WitnessObligation::OutputRows {
                layer_id: layer_id.to_string(),
                bounds,
            });
        }
        if !valid {
            continue;
        }
        if let Some(case) = WitnessCase::new(result, ProofStrength::Sufficient) {
            if !cases.contains(&case) {
                cases.push(case);
            }
        }
    }
    if cases.is_empty() {
        Some(residual(PhysicalProofGap::MultipleWitnesses))
    } else {
        WitnessDirection::feasible(cases)
    }
}
