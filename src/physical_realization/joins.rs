//! Join population constructive proofs.

use super::*;

/// Derive an exact typed TRUE row restriction for one physical source.
///
/// Tries the certified scalar witness first, then falls back to the legacy
/// typed Boolean/integer literal equality used by filtered producers. Returns
/// `None` for join-key or ambiguous predicates so callers fail closed.
fn exact_source_filter(
    bundle: &AnalysisBundle,
    source: &crate::protocol::SourceRelation,
    predicate: &Predicate,
) -> Option<(String, WitnessFormula)> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source.name())?;
    if blocks_unconstrained_physical_plans(bundle, source.name()) {
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

/// Legacy typed `column = literal` equality for filtered producers.
///
/// Supports only Boolean and range-checked integer literals against the
/// declared column type. Anything else stays residual by returning `None`.
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

/// Attach a scalar predicate to exactly one physical input.
///
/// Succeeds only when the predicate resolves to a single source; ambiguous
/// or multi-source predicates return `None` so join construction fails closed.
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

/// Whether a join filter formula mentions a join-key column.
///
/// Used to reject key restrictions that would need reconciliation with each
/// candidate key population, which this bounded construction does not attempt.
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

/// Whether a witness term references a join-key column.
fn term_references_column(term: &WitnessTerm, column: &crate::protocol::ColumnRef) -> bool {
    matches!(term, WitnessTerm::Column { column: actual, .. } if actual == column)
}

/// Combine several per-input restrictions into one conjunctive formula.
fn combined_filter(filters: &[WitnessFormula]) -> WitnessFormula {
    match filters {
        [one] => one.clone(),
        many => WitnessFormula::All(many.to_vec()),
    }
}

/// Flatten an ON conjunction without changing SQL operator semantics.
///
/// Parser normalization may produce nested AND trees. Treating a nested AND
/// that includes the join key as a scalar source restriction would reject an
/// otherwise independently provable non-key predicate.
fn join_conjuncts<'a>(predicate: &'a Predicate, output: &mut Vec<&'a Predicate>) {
    match predicate {
        Predicate::And(and) => {
            for operand in and.operands() {
                join_conjuncts(operand, output);
            }
        }
        _ => output.push(predicate),
    }
}

/// Extract the finite one- or two-column equijoin key and scalar filters.
///
/// Every key equality must link the two distinct input instances. Composite
/// keys are a conjunction over one complete physical bag, never separate pair
/// witnesses. Additional equality keys or ambiguous conditions fail closed.
type JoinKeyExpressions<'a> = Vec<(
    &'a crate::protocol::ColumnExpression,
    &'a crate::protocol::ColumnExpression,
)>;

/// The full ON key tuple and any independent scalar filters.
type JoinKeysAndFilters<'a> = (JoinKeyExpressions<'a>, Vec<&'a Predicate>);

fn join_key_and_scalar_filters(query: &QueryStatement) -> Option<JoinKeysAndFilters<'_>> {
    use crate::protocol::ComparisonOperator;

    let condition = query.joins().first()?.condition()?;
    let mut operands = Vec::new();
    join_conjuncts(condition, &mut operands);
    let mut keys = Vec::new();
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
            keys.push(match (comparison.left(), comparison.right()) {
                (Expression::Column(left), Expression::Column(right)) => (left, right),
                _ => return None,
            });
        } else {
            filters.push(operand);
        }
    }
    if !(1..=2).contains(&keys.len()) {
        return None;
    }
    if let Some(predicate) = query.predicates().where_predicate() {
        filters.push(predicate);
    }
    Some((keys, filters))
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

/// A direct aliased self-join reads the *same* physical bag twice.
/// Assigning N identical, non-NULL typed keys yields exactly N*N ordered
/// matched pairs, including each row matched with itself. No second source
/// population or independent alias cardinality may be created.
fn physical_aliased_self_join_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
    exact_sources: &BTreeMap<String, u64>,
) -> Option<WitnessDirection> {
    use crate::protocol::{ComparisonOperator, JoinKind};

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if walker.nodes.iter().filter(|node| matches!(node.id(), PhysicalPlanRef::Layer(_))).count()
        != 1
    {
        return None;
    }
    let layer = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let [left, right] = query.sources() else {
        return None;
    };
    let [join] = query.joins() else {
        return None;
    };
    if left.name() != right.name()
        || left.alias().is_none()
        || right.alias().is_none()
        || left.alias() == right.alias()
        || join.kind() != JoinKind::Inner
        || query.predicates().where_predicate().is_some()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.diagnostics().is_empty()
        || query.output().columns().iter().any(|column| {
            !matches!(
                column.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
        || blocks_unconstrained_physical_plans(bundle, left.name())
    {
        return None;
    }

    let Predicate::Comparison(condition) = join.condition()? else {
        return None;
    };
    let (Expression::Column(a), Expression::Column(b)) =
        (condition.left(), condition.right())
    else {
        return None;
    };
    if condition.operator() != ComparisonOperator::Eq {
        return None;
    }
    // ON may reverse operands, but its instances must be the two SQL aliases.
    let aliases = [left.alias()?, right.alias()?];
    if !((a.relation() == Some(aliases[0]) && b.relation() == Some(aliases[1]))
        || (a.relation() == Some(aliases[1]) && b.relation() == Some(aliases[0])))
        && !(a.relation() == Some(left.name()) && b.relation() == Some(left.name()))
    {
        return None;
    }
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == left.name())?;
    let left_key = crate::protocol::ColumnRef::new(
        Some(left.name().to_string()),
        a.name().to_string(),
    );
    let right_key = crate::protocol::ColumnRef::new(
        Some(right.name().to_string()),
        b.name().to_string(),
    );
    for key in [&left_key, &right_key] {
        let domain = physical_integer_evidence(schema, key)?;
        if domain.minimum > 0 || domain.maximum < 0 {
            return None;
        }
    }
    let rows = wanted.isqrt();
    if rows.checked_mul(rows)? != wanted
        || exact_sources.get(left.name()).is_some_and(|count| *count != rows)
    {
        return None;
    }
    let left_row = RowVariable::new(left.name(), aliases[0], "self_join_left")?;
    let right_row = RowVariable::new(right.name(), aliases[1], "self_join_right")?;
    let boundary = WitnessBoundary::new(
        left.name(),
        GroupBoundaryKind::Physical,
        target_layer_id,
    )?;
    let mut obligations = vec![
        WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: CountBounds::new(rows, Some(rows))?,
            predicate: count_tautology(),
            closed_world: true,
        },
        WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        },
        WitnessObligation::JoinPopulation {
            left_row: left_row.clone(),
            right_row: right_row.clone(),
            left_key: left_key.clone(),
            right_key: right_key.clone(),
            additional_keys: vec![],
            join_kind: JoinKind::Inner,
            pattern: JoinPopulationPattern::CommonMatched,
            left_rows: rows,
            right_rows: rows,
            output_rows: wanted,
            closed_world: true,
        },
    ];
    if rows > 0 {
        obligations.push(WitnessObligation::JoinPair {
            left_row,
            right_row,
            left: left_key,
            right: right_key,
            comparison: ComparisonOperator::Eq,
            null_extended: None,
        });
    }
    obligations.push(WitnessObligation::OutputRows {
        layer_id: target_layer_id.to_string(),
        bounds: CountBounds::new(wanted, Some(wanted))?,
    });
    WitnessDirection::feasible(vec![WitnessCase::new(
        obligations,
        ProofStrength::Sufficient,
    )?])
}

/// Certify a finite two-edge INNER chain as one complete physical assignment.
///
/// This is not a product of independent pairwise witnesses: both equalities
/// are satisfied on the *same* physical bags by using the same non-NULL typed
/// integer key value for every referenced key. Each source instance is named
/// separately, while repeated producer aliases point to a single source bag.
/// Non-equality conditions, additional joins, filters and derived transforms
/// have no such proof and remain residual.
fn physical_three_way_inner_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
    exact_sources: &BTreeMap<String, u64>,
) -> Option<WitnessDirection> {
    use crate::protocol::{ComparisonOperator, JoinKind};

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let layer = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let [first_join, second_join] = query.joins() else {
        return None;
    };
    let [left, middle, right] = query.sources() else {
        return None;
    };
    if [first_join, second_join]
        .iter()
        .any(|join| join.kind() != JoinKind::Inner)
        || query.predicates().where_predicate().is_some()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.diagnostics().is_empty()
        || query.output().columns().iter().any(|column| {
            !matches!(
                column.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
        || walker.nodes.iter().any(|node| {
            node.produced_relations()
                .iter()
                .any(|name| untransported_materialization_laws(bundle, name))
        })
    {
        return None;
    }

    // Only plain-copy producers may sit upstream of the two SQL joins.
    // In particular no second local join or filtered producer may make one
    // alias a different (and hence incompatible) physical row selection.
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if id == target_layer_id {
            continue;
        }
        let producer = walker.layers.get(id.as_str()).copied()?;
        if !matches!(node.write_kind(), None | Some(WriteKind::Definition))
            || !transparent_projection(query_for(bundle, producer)?)
        {
            return None;
        }
    }

    let sources = [left, middle, right];
    let names = sources.map(|source| source.alias().unwrap_or(source.name()));
    if names[0] == names[1] || names[1] == names[2] || names[0] == names[2] {
        return None;
    }
    let relations = sources.map(|source| source.name());

    // Resolve every normalized equality through the same canonical column
    // mapper used by the binary physical join. The physical relation may be
    // identical for different SQL aliases, but each ON side must identify the
    // expected source role in its join edge.
    let resolve = |position: usize,
                   expression: &crate::protocol::ColumnExpression|
     -> Option<crate::protocol::ColumnRef> {
        let instance = names[position];
        let reference = crate::protocol::ColumnRef::new(
            Some(instance.to_string()),
            expression.name().to_string(),
        );
        let mapped = resolve_filter_column_with_evidence(
            bundle,
            &walker,
            target_layer_id,
            &reference,
            0,
            true,
        )?;
        let visible = expression.relation()?;
        if mapped.relation()? != relations[position] && !walker.sources.contains(mapped.relation()?)
        {
            return None;
        }
        if visible != instance && visible != relations[position] && visible != mapped.relation()? {
            return None;
        }
        Some(mapped)
    };
    let equality = |join: &crate::protocol::Join,
                    a: usize,
                    b: usize|
     -> Option<(crate::protocol::ColumnRef, crate::protocol::ColumnRef)> {
        let Predicate::Comparison(comparison) = join.condition()? else {
            return None;
        };
        if comparison.operator() != ComparisonOperator::Eq {
            return None;
        }
        let (Expression::Column(left), Expression::Column(right)) =
            (comparison.left(), comparison.right())
        else {
            return None;
        };
        if let (Some(x), Some(y)) = (resolve(a, left), resolve(b, right)) {
            return Some((x, y));
        }
        Some((resolve(a, right)?, resolve(b, left)?))
    };
    let (left_key, middle_left_key) = equality(first_join, 0, 1)?;
    let (middle_right_key, right_key) = equality(second_join, 1, 2)?;
    let keys = [
        (&left_key, 0usize),
        (&middle_left_key, 1),
        (&middle_right_key, 1),
        (&right_key, 2),
    ];
    // The sufficient construction chooses literal zero, which must be
    // representable in every attested physical signed-integer column type.
    for (key, _) in keys {
        let relation = key.relation()?;
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == relation)?;
        let bounds = physical_integer_evidence(schema, key)?;
        if bounds.minimum > 0
            || bounds.maximum < 0
            || blocks_unconstrained_physical_plans(bundle, relation)
        {
            return None;
        }
    }

    let instances = [
        RowVariable::new(left_key.relation()?, names[0], "chain_left")?,
        RowVariable::new(middle_left_key.relation()?, names[1], "chain_middle")?,
        RowVariable::new(right_key.relation()?, names[2], "chain_right")?,
    ];

    let mut candidates = Vec::new();
    if wanted == 0 {
        candidates.extend([[0, 1, 1], [1, 0, 1], [1, 1, 0], [0, 0, 0]]);
    } else {
        for left_rows in 1..=wanted.min(64) {
            if !wanted.is_multiple_of(left_rows) {
                continue;
            }
            let remaining = wanted / left_rows;
            for middle_rows in 1..=remaining.min(64) {
                if remaining.is_multiple_of(middle_rows) {
                    candidates.push([left_rows, middle_rows, remaining / middle_rows]);
                }
            }
        }
    }
    let mut cases = Vec::new();
    for counts in candidates {
        let mut physical_counts = BTreeMap::new();
        let mut compatible = true;
        for (index, row) in instances.iter().enumerate() {
            let count = counts[index];
            if physical_counts
                .insert(row.relation().to_string(), count)
                .is_some_and(|previous| previous != count)
            {
                compatible = false;
            }
        }
        if !compatible
            || exact_sources.iter().any(|(source, rows)| {
                physical_counts
                    .get(source)
                    .is_some_and(|count| count != rows)
            })
        {
            continue;
        }
        let mut obligations = Vec::new();
        for (relation, count) in &physical_counts {
            let boundary =
                WitnessBoundary::new(relation, GroupBoundaryKind::Physical, target_layer_id)?;
            obligations.push(WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: CountBounds::new(*count, Some(*count))?,
                predicate: count_tautology(),
                closed_world: true,
            });
            obligations.push(WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            });
        }
        obligations.push(WitnessObligation::JoinChainPopulation {
            left_row: instances[0].clone(),
            middle_row: instances[1].clone(),
            right_row: instances[2].clone(),
            left_key: left_key.clone(),
            middle_left_key: middle_left_key.clone(),
            middle_right_key: middle_right_key.clone(),
            right_key: right_key.clone(),
            matched_value: 0,
            left_rows: counts[0],
            middle_rows: counts[1],
            right_rows: counts[2],
            output_rows: wanted,
            closed_world: true,
        });
        obligations.push(WitnessObligation::OutputRows {
            layer_id: target_layer_id.to_string(),
            bounds: CountBounds::new(wanted, Some(wanted))?,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            cases.push(case);
        }
    }
    WitnessDirection::feasible(cases)
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

    if let Some(plan) =
        physical_three_way_inner_count_plan(bundle, target_layer_id, wanted, exact_sources)
    {
        return Some(plan);
    }
    if let Some(plan) =
        physical_aliased_self_join_count_plan(bundle, target_layer_id, wanted, exact_sources)
    {
        return Some(plan);
    }
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
    // A materialized relation may impose additional warehouse laws not
    // entailed by its SELECT. These complete source-row examples cannot
    // establish that such an intermediate state can actually be written.
    if walker.nodes.iter().any(|node| {
        node.produced_relations()
            .iter()
            .any(|relation| untransported_materialization_laws(bundle, relation))
    }) {
        return None;
    }
    if !(join_query.plain_goal_output_shape() || join_query.filtered_join_goal_output_shape())
        || join_query.joins().len() != 1
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
    let (join_keys, scalar_filters) = join_key_and_scalar_filters(join_query)?;
    let &(left_key_expression, right_key_expression) = join_keys.first()?;

    let ComposedSemantics::Resolved(semantics) = join_layer.composed_semantics() else {
        return None;
    };
    let join = match semantics.join_witnesses() {
        [join] if join.origin_layer_id() == join_layer.id() => Some(join),
        _ => None,
    };
    let sql_join = join_query.joins().first()?;
    let join_kind = sql_join.kind();
    if matches!(join_kind, JoinKind::Cross | JoinKind::Unknown) {
        return None;
    }
    // Source-row restrictions do not represent SQL WHERE evaluated after
    // NULL extension. In outer joins even an empty nonpreserved input can
    // produce rows that WHERE then discards.
    if join_kind != JoinKind::Inner && join_query.predicates().where_predicate().is_some() {
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

    // Individually exact scalar filters need not share a satisfying row.
    // In particular ON x > 5 AND x < 2 cannot be certified by combining
    // independent sufficient examples. A mixed or unproved conjunction is
    // left residual rather than emitting an inconsistent complete population.
    if source_filters.iter().any(|(source, filters)| {
        filters.len() > 1
            && !matches!(
                joint_source_truths_satisfiable(bundle, source, filters),
                Some(true)
            )
    }) {
        return None;
    }

    // A residual local join witness cannot certify source rows through a
    // filtered producer. The physical construction below independently
    // proves an ordinary typed equijoin over complete filtered source bags:
    // each filter is an exact typed TRUE restriction on its own closed
    // physical bag, and keys are recovered from the same canonical query.
    // This bypasses the local gate only when such filters exist; without
    // them, retain the stronger original local gate.
    if source_filters.is_empty()
        && join_keys.len() == 1
        && !matches!(join?.qualifying(), JoinWitnessDirection::Exact(_))
    {
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

    let (left_key, left_instance, right_key, right_instance) =
        if source_filters.is_empty() && join_keys.len() == 1 {
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
            // INNER matches are symmetric. A composed local witness above already
            // owns its physical source orientation, which can differ from the
            // immediate SQL producer names; do not reinterpret that proof.
            if join_kind == JoinKind::Inner {
                (left, left_instance, right, right_instance)
            } else {
                // Outer/semi/anti preservation is positional, not the ON equality's
                // operand order. Orient only endpoints freshly recovered from ON.
                let sql_left = sql_join
                    .left()
                    .alias()
                    .unwrap_or(sql_join.left().relation());
                let sql_right = sql_join
                    .right()
                    .alias()
                    .unwrap_or(sql_join.right().relation());
                if left_instance == sql_left && right_instance == sql_right {
                    (left, left_instance, right, right_instance)
                } else if left_instance == sql_right && right_instance == sql_left {
                    (right, right_instance, left, left_instance)
                } else {
                    return None;
                }
            }
        };
    if left_instance == right_instance {
        return None;
    }
    let left_source = left_key.relation()?;
    let right_source = right_key.relation()?;
    // The composite condition is a conjunction, not two independent joins.
    // Map every additional key to the same physical source instances, including
    // reversed ON operands. The full population assigns the same key-group
    // sequence to all components, so all matched row pairs satisfy *every* '='.
    let additional_keys = join_keys
        .iter()
        .skip(1)
        .map(|&(sql_left, sql_right)| {
            let map = |column: &crate::protocol::ColumnExpression| {
                let instance = column.relation()?;
                let reference = crate::protocol::ColumnRef::new(
                    Some(instance.to_string()),
                    column.name().to_string(),
                );
                let physical =
                    resolve_filter_column(bundle, &walker, join_layer.id(), &reference, 0)?;
                Some((physical, instance.to_string()))
            };
            let (a, a_instance) = map(sql_left)?;
            let (b, b_instance) = map(sql_right)?;
            let (left, right) = if a_instance == left_instance && b_instance == right_instance {
                (a, b)
            } else if a_instance == right_instance && b_instance == left_instance {
                (b, a)
            } else {
                return None;
            };
            if left.relation()? != left_source
                || right.relation()? != right_source
                || left == left_key
                || right == right_key
            {
                return None;
            }
            Some(crate::constructive::JoinKeyEquality::new(left, right))
        })
        .collect::<Option<Vec<_>>>()?;

    // The construction controls join keys independently of source filters.
    // If a filter mentions either join key, the literal must instead be
    // reconciled with each candidate population, which is not done here.
    if source_filters.iter().any(|(source, filters)| {
        filters.iter().any(|filter| {
            (source == left_source && formula_references_column(filter, &left_key))
                || (source == right_source && formula_references_column(filter, &right_key))
                || additional_keys.iter().any(|pair| {
                    (source == left_source && formula_references_column(filter, pair.left()))
                        || (source == right_source
                            && formula_references_column(filter, pair.right()))
                })
        })
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
    let additional_bounds = additional_keys
        .iter()
        .map(|pair| {
            Some((
                physical_integer_evidence(left_schema, pair.left())?,
                physical_integer_evidence(right_schema, pair.right())?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    if additional_bounds.iter().any(|(left, right)| {
        left.minimum > 0 || right.minimum > 0 || left.maximum < 0 || right.maximum < 0
    }) || left_bounds.minimum > 0
        || right_bounds.minimum > 0
        || left_bounds.maximum < 0
        || right_bounds.maximum < 0
        || [left_source, right_source]
            .iter()
            .any(|relation| blocks_unconstrained_physical_plans(bundle, relation))
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
            || additional_bounds.iter().any(|(left, right)| {
                (left_rows > 0 && max_value > left.maximum)
                    || (right_rows > 0 && max_value > right.maximum)
            })
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
            additional_keys: additional_keys.clone(),
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
                for pair in &additional_keys {
                    obligations.push(WitnessObligation::JoinPair {
                        left_row: left_row.clone(),
                        right_row: right_row.clone(),
                        left: pair.left().clone(),
                        right: pair.right().clone(),
                        comparison: crate::protocol::ComparisonOperator::Eq,
                        null_extended: None,
                    });
                }
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

/// Whether the entire counted physical bag is certified to satisfy one
/// or more TRUE row constraints. This deliberately excludes negative filters,
/// general symbolic obligations, and unproved disjunctions.
fn exact_positive_source_filter(formula: &WitnessFormula, relation: &str) -> bool {
    match formula {
        WitnessFormula::RowTruth {
            row,
            predicate,
            truth: crate::boolean_witness::BooleanTruthCase::True,
        } => row.relation() == relation && predicate.is_exact(),
        WitnessFormula::All(parts) => {
            !parts.is_empty()
                && parts
                    .iter()
                    .all(|part| exact_positive_source_filter(part, relation))
        }
        _ => false,
    }
}

/// Restrict overlap reconciliation to the same physical row's certified
/// non-key TRUE predicates. A residual, a different row variable or a
/// disjunction over separate row bags cannot establish joint feasibility.
fn source_true_conjuncts(formula: &WitnessFormula, source: &str) -> Option<Vec<WitnessFormula>> {
    if *formula == count_tautology() {
        return Some(Vec::new());
    }
    match formula {
        WitnessFormula::RowTruth {
            row,
            predicate,
            truth: crate::boolean_witness::BooleanTruthCase::True,
        } if row.relation() == source && predicate.is_exact() => Some(vec![formula.clone()]),
        WitnessFormula::All(children) if !children.is_empty() => {
            let mut conjuncts = Vec::new();
            for child in children {
                conjuncts.extend(source_true_conjuncts(child, source)?);
            }
            Some(conjuncts)
        }
        _ => None,
    }
}

/// Reconcile multiple complete source restrictions without independently
/// sampling each join's source rows. The positive source population must be
/// nonempty for a satisfiability proof to constrain the assignment.
fn joined_source_true_filters(
    bundle: &AnalysisBundle,
    source: &str,
    rows: u64,
    left: &WitnessFormula,
    right: &WitnessFormula,
) -> Option<WitnessFormula> {
    let mut truths = source_true_conjuncts(left, source)?;
    truths.extend(source_true_conjuncts(right, source)?);
    truths.dedup();
    if truths.is_empty() {
        return Some(count_tautology());
    }
    if rows > 0 && joint_source_truths_satisfiable(bundle, source, &truths) != Some(true) {
        return None;
    }
    Some(match truths.as_slice() {
        [one] => one.clone(),
        many => WitnessFormula::All(many.to_vec()),
    })
}

/// Correlate a DISTINCT-MATCHED join population with grouping of the
/// same physical equality key. The join already fixes every input key to
/// a distinct 0..N-1 value; grouping that key emits exactly N singleton
/// groups on the *same* closed bag. Other aggregates or grouping keys must
/// be proved by a different operator correlation and stay residual.
pub(super) fn joint_join_and_distinct_group_goals(
    bundle: &AnalysisBundle,
    goals: &[(&str, u64)],
) -> Option<WitnessDirection> {
    use crate::outcome_proofs::OutcomeWitness;

    if goals.len() < 2 {
        return None;
    }
    let join_goals = goals
        .iter()
        .filter_map(|&(id, rows)| {
            physical_join_population_count_plan(bundle, id, rows)
                .is_some()
                .then_some((id, rows))
        })
        .collect::<Vec<_>>();
    let [(join_id, join_rows)] = join_goals.as_slice() else {
        return None;
    };
    let WitnessDirection::Feasible(join_cases) =
        physical_join_population_count_plan(bundle, join_id, *join_rows)?
    else {
        return None;
    };
    let mut groups = Vec::new();
    for &(id, rows) in goals {
        if id == *join_id {
            continue;
        }
        let (witness, counts, _) = physical_operator_population_case(bundle, id, rows)?;
        let OutcomeWitness::Groups {
            relation,
            key,
            groups: count,
            rows_per_group: 1,
        } = &witness
        else {
            return None;
        };
        if *count != rows || counts.get(relation) != Some(&rows) || counts.len() != 1 {
            return None;
        }
        groups.push((id, rows, relation.clone(), key.clone(), witness));
    }
    if groups.is_empty() {
        return None;
    }

    let mut cases = Vec::new();
    for join_case in join_cases {
        let Some((left_key, right_key, left_rows, right_rows)) =
            join_case.obligations().iter().find_map(|obligation| {
                if let WitnessObligation::JoinPopulation {
                    left_key,
                    right_key,
                    left_rows,
                    right_rows,
                    pattern: JoinPopulationPattern::DistinctMatched,
                    closed_world: true,
                    ..
                } = obligation
                {
                    Some((left_key, right_key, *left_rows, *right_rows))
                } else {
                    None
                }
            })
        else {
            continue;
        };
        let mut obligations = join_case.obligations().to_vec();
        let mut valid = true;
        for (id, rows, source, key, witness) in &groups {
            let owned = if left_key == key {
                left_rows == *rows
            } else if right_key == key {
                right_rows == *rows
            } else {
                false
            };
            if !owned
                || key.relation() != Some(source.as_str())
                || !join_case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::ClosedWorld {
                            boundary,
                            coverage: ClosedWorldCoverage::EntireRelation
                        } if boundary.kind() == GroupBoundaryKind::Physical
                            && boundary.relation() == source
                    )
                })
            {
                valid = false;
                break;
            }
            let population = WitnessObligation::OperatorPopulation {
                witness: witness.clone(),
            };
            if !obligations.contains(&population) {
                obligations.push(population);
            }
            obligations.push(WitnessObligation::OutputRows {
                layer_id: id.to_string(),
                bounds: CountBounds::new(*rows, Some(*rows))?,
            });
        }
        if valid {
            if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
                cases.push(case);
            }
        }
    }
    Some(
        WitnessDirection::feasible(cases)
            .unwrap_or_else(|| residual(PhysicalProofGap::UnprovedCrossRowCorrelation)),
    )
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
        let mut counts = BTreeMap::<String, (u64, WitnessFormula)>::new();
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
                    && (*predicate == count_tautology()
                        || exact_positive_source_filter(predicate, boundary.relation())) =>
                {
                    let count = bounds.minimum();
                    let entry = counts.entry(boundary.relation().to_string());
                    match entry {
                        std::collections::btree_map::Entry::Vacant(slot) => {
                            slot.insert((count, predicate.clone()));
                        }
                        std::collections::btree_map::Entry::Occupied(mut slot) => {
                            let (prior_count, prior_filter) = slot.get_mut();
                            if *prior_count != count {
                                valid = false;
                                break;
                            }
                            let Some(joint) = joined_source_true_filters(
                                bundle,
                                boundary.relation(),
                                count,
                                prior_filter,
                                predicate,
                            ) else {
                                // Conflicting or unverified predicates cannot be
                                // conjoined into one physical source assignment.
                                // Alternative source bags might still exist, so
                                // this is residual rather than impossible.
                                valid = false;
                                break;
                            };
                            *prior_filter = joint;
                        }
                    }
                }
                WitnessObligation::JoinPopulation {
                    left_row,
                    right_row,
                    left_key,
                    right_key,
                    additional_keys,
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
                        ]
                        .into_iter()
                        .chain(additional_keys.iter().flat_map(|pair| {
                            [
                                (left_row, pair.left(), left_rows),
                                (right_row, pair.right(), right_rows),
                            ]
                        })) {
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
        // Filters on any controlled key require a different key-aware
        // population construction. Independent non-key TRUE filters may
        // coexist with complete join populations, including disjoint joins.
        if counts.iter().any(|(source, (_, filter))| {
            populations.iter().any(|population| {
                if let WitnessObligation::JoinPopulation {
                    left_key,
                    right_key,
                    additional_keys,
                    ..
                } = population
                {
                    (left_key.relation() == Some(source.as_str())
                        && formula_references_column(filter, left_key))
                        || (right_key.relation() == Some(source.as_str())
                            && formula_references_column(filter, right_key))
                        || additional_keys.iter().any(|pair| {
                            (pair.left().relation() == Some(source.as_str())
                                && formula_references_column(filter, pair.left()))
                                || (pair.right().relation() == Some(source.as_str())
                                    && formula_references_column(filter, pair.right()))
                        })
                } else {
                    false
                }
            })
        }) {
            continue;
        }
        let mut result = Vec::new();
        let origin = goals[0].0;
        for (source, (rows, predicate)) in &counts {
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
                predicate: predicate.clone(),
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
