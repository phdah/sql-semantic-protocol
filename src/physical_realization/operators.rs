//! Lifting local operator witnesses through producers.

use super::*;

/// Reuse an independently proved source-level operator construction when
/// all later materialized producers are exact row-preserving projections.
/// The first transformation must consume only physical leaves: a join,
/// grouped aggregate, window rank or set across already-transformed inputs
/// needs a separate multi-operator proof and remains residual.
pub(crate) fn physical_operator_count_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    if rows == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let layer_ids = walker.nodes.iter().filter_map(|node| match node.id() {
        PhysicalPlanRef::Layer(id) => Some(id.as_str()),
        PhysicalPlanRef::Source(_) => None,
    });
    let mut layer_ids = layer_ids.peekable();
    let first_id = layer_ids.next()?;
    if first_id == target_layer_id {
        return None;
    }
    let first = walker.layers.get(first_id).copied()?;
    let first_query = query_for(bundle, first)?;
    let goal =
        crate::outcome_goals::OutcomeGoal::new(first_id, Some(rows), None, Vec::new()).ok()?;
    let witness = crate::outcome_proofs::construct(bundle, first, first_query, &goal)?;
    if !matches!(
        witness,
        crate::outcome_proofs::OutcomeWitness::JoinPairs { .. }
            | crate::outcome_proofs::OutcomeWitness::Groups { .. }
            | crate::outcome_proofs::OutcomeWitness::Ranked { .. }
            | crate::outcome_proofs::OutcomeWitness::SetTuples { .. }
    ) {
        return None;
    }
    let mut previous = first_id;
    for id in layer_ids {
        let layer = walker.layers.get(id).copied()?;
        let query = query_for(bundle, layer)?;
        if !transparent_projection(query)
            || query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.window_witness().is_some()
            || !query.subquery_witnesses().is_empty()
        {
            return None;
        }
        // Every downstream layer must consume exactly its immediate
        // predecessor, not silently fork or duplicate the operator output.
        let inputs = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(id.to_string()))?
            .inputs();
        let [PhysicalPlanRef::Layer(input_id)] = inputs else {
            return None;
        };
        if input_id != previous {
            return None;
        }
        previous = id;
    }
    (previous == target_layer_id).then_some(witness)
}

/// Lift a locally exact GROUP BY/HAVING or ROW_NUMBER/QUALIFY population
/// through transparent, type-certified producer chains. Other operators,
/// computed projections, filtered parents, and additional physical sources
/// retain residual classification rather than pretending producer outputs
/// are directly writable.
pub(crate) fn physical_materialized_group_or_rank_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
    groups: Option<u64>,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    if rows == 0 || groups.is_some_and(|count| count != rows) {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if walker.sources.len() != 1 {
        return None;
    }
    let mut operator_id = None;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if !matches!(node.write_kind(), None | Some(WriteKind::Definition)) {
            return None;
        }
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if transparent_projection(query) {
            continue;
        }
        if operator_id.is_some()
            || query.sources().len() != 1
            || node.inputs().len() != 1
            || !query.joins().is_empty()
            || query.set_operation().is_some()
            || !query.subquery_witnesses().is_empty()
        {
            return None;
        }
        let group = query.group_rows_match_surviving_groups() && query.aggregation().is_some();
        let rank = query.window_witness().is_some() && query.aggregation().is_none();
        if !group && !rank {
            return None;
        }
        operator_id = Some(id.as_str());
    }
    let operator_id = operator_id?;
    let operator_layer = walker.layers.get(operator_id).copied()?;
    let operator = query_for(bundle, operator_layer)?;

    // The operator's sole parent must be an actual producer. Direct physical
    // operators use their own existing direct-construction path.
    let node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(operator_id.to_string()))?;
    if !matches!(node.inputs(), [PhysicalPlanRef::Layer(_)]) {
        return None;
    }
    // Its output must be followed by one linear, lossless producer path.
    let mut current = target_layer_id;
    for _ in 0..walker.layers.len() {
        if current == operator_id {
            break;
        }
        let descendant = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(current.to_string()))?;
        let [PhysicalPlanRef::Layer(parent)] = descendant.inputs() else {
            return None;
        };
        current = parent;
    }
    if current != operator_id {
        return None;
    }
    let map = |column: &crate::protocol::ColumnRef| {
        let mapped =
            resolve_filter_column_with_evidence(bundle, &walker, operator_id, column, 0, true)?;
        walker
            .sources
            .contains(mapped.relation()?)
            .then_some(mapped)
    };
    let goal =
        crate::outcome_goals::OutcomeGoal::new(operator_id, Some(rows), groups, Vec::new()).ok()?;
    if operator.aggregation().is_some() {
        crate::outcome_proofs::construct_mapped_group(bundle, operator, &goal, rows, &map)
    } else {
        if groups.is_some() {
            return None;
        }
        crate::outcome_proofs::construct_mapped_rank(bundle, operator, &goal, rows, &map)
    }
}

/// Transport an exact two-branch set law to complete, type-certified
/// physical source populations. Each branch's entire producer path must be
/// row and value preserving; distinct physical sources are required so
/// local branch examples are not mistaken for independent shared rows.
pub(crate) fn physical_materialized_set_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    goal: &crate::outcome_goals::OutcomeGoal,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    let rows = goal.rows()?;
    if rows == 0 || goal.groups().is_some() {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if !(1..=4).contains(&walker.sources.len()) {
        return None;
    }
    let mut operator_id = None::<&str>;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if !matches!(node.write_kind(), None | Some(WriteKind::Definition)) {
            return None;
        }
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if query.set_operation().is_some() {
            if operator_id.is_some() {
                return None;
            }
            operator_id = Some(id);
        } else if !transparent_projection(query) {
            return None;
        }
    }
    let operator_id = operator_id?;
    let operator_layer = walker.layers.get(operator_id).copied()?;
    let query = query_for(bundle, operator_layer)?;
    let branches = query.set_operation()?.branches();
    if !(2..=4).contains(&branches.len()) {
        return None;
    }
    // Require a linear path after the set. Upstream each arm can have its
    // own transparent chain, but unrelated operator DAGs are not admitted.
    let mut current = target_layer_id;
    for _ in 0..walker.layers.len() {
        if current == operator_id {
            break;
        }
        let descendant = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(current.to_string()))?;
        let [PhysicalPlanRef::Layer(parent)] = descendant.inputs() else {
            return None;
        };
        current = parent;
    }
    if current != operator_id {
        return None;
    }

    let map = |boundary: &crate::protocol::SetWitnessBoundary| {
        let [column] = boundary.tuple_columns() else {
            return None;
        };
        let original = crate::protocol::ColumnRef::new(
            Some(boundary.relation().to_string()),
            column.to_string(),
        );
        let mapped =
            resolve_filter_column_with_evidence(bundle, &walker, operator_id, &original, 0, true)?;
        let relation = mapped.relation()?;
        if !walker.sources.contains(relation) {
            return None;
        }
        Some(crate::protocol::SetWitnessBoundary::new(
            relation.to_string(),
            vec![mapped.name().to_string()],
            false,
        ))
    };
    let mapped_relations = branches
        .iter()
        .map(|branch| map(branch.witness_boundary()?))
        .collect::<Option<Vec<_>>>()?;
    let mut physical_columns = std::collections::BTreeMap::new();
    for boundary in &mapped_relations {
        if physical_columns
            .insert(boundary.relation(), boundary.tuple_columns())
            .is_some_and(|previous| previous != boundary.tuple_columns())
        {
            return None;
        }
    }
    crate::outcome_proofs::construct_mapped_set(bundle, query, goal, rows, &map)
}

/// Construct an exact positive equijoin cardinality through two
/// independently materialized, identity-only producer branches. Local join
/// evidence proves matching-pair semantics; tracing both join keys through
/// their producers proves that the *physical* rows can realize those pairs.
/// This deliberately does not admit filtered, grouped or computed branches.
pub(crate) fn physical_materialized_join_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    if rows == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if walker.sources.len() != 2 {
        return None;
    }
    let candidates = walker
        .nodes
        .iter()
        .filter_map(|node| match node.id() {
            PhysicalPlanRef::Layer(id) => {
                let layer = walker.layers.get(id.as_str()).copied()?;
                (query_for(bundle, layer)?.joins().len() == 1).then_some(layer)
            }
            PhysicalPlanRef::Source(_) => None,
        })
        .collect::<Vec<_>>();
    let [join_layer] = candidates.as_slice() else {
        return None;
    };
    let query = query_for(bundle, join_layer)?;
    if !query.plain_goal_output_shape()
        || query.joins().len() != 1
        || query.sources().len() != 2
        || query.dependencies().len() != 2
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || !query.diagnostics().is_empty()
        || query.output().columns().iter().any(|column| {
            !matches!(
                column.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
    {
        return None;
    }
    let ComposedSemantics::Resolved(semantics) = join_layer.composed_semantics() else {
        return None;
    };
    let [join] = semantics.join_witnesses() else {
        return None;
    };
    if join.origin_layer_id() != join_layer.id()
        || !matches!(
            join.kind(),
            crate::protocol::JoinKind::Inner
                | crate::protocol::JoinKind::Left
                | crate::protocol::JoinKind::Right
                | crate::protocol::JoinKind::Full
        )
        || join.comparison() != Some(crate::protocol::ComparisonOperator::Eq)
        || !matches!(
            join.qualifying(),
            crate::join_witness::JoinWitnessDirection::Exact(cases)
                if cases.iter().any(|case|
                    case.shape() == crate::join_witness::JoinWitnessShape::Matched)
        )
    {
        return None;
    }

    // Before the join, only independent value- and row-preserving source
    // copies are permitted. After it, every producer must consume exactly
    // the preceding join result, with no additional row shaping.
    let mut after_join = false;
    let mut previous = None::<String>;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if id == join_layer.id() {
            after_join = true;
            previous = Some(id.clone());
            continue;
        }
        let layer = walker.layers.get(id.as_str()).copied()?;
        if !transparent_projection(query_for(bundle, layer)?) {
            return None;
        }
        if after_join {
            let [PhysicalPlanRef::Layer(input_id)] = node.inputs() else {
                return None;
            };
            if previous.as_deref() != Some(input_id.as_str()) {
                return None;
            }
            previous = Some(id.clone());
        }
    }
    if previous.as_deref() != Some(target_layer_id) {
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
            resolve_filter_column(bundle, &walker, join_layer.id(), &column, 0)?
        };
        let relation = mapped.relation()?;
        walker.sources.contains(relation).then(|| {
            crate::bundle::ComposedJoinColumn::new(
                relation.to_string(),
                mapped.name().to_string(),
                endpoint.relation_instance().to_string(),
            )
        })
    };
    let left = map(join.left()?)?;
    let right = map(join.right()?)?;
    if left.relation() == right.relation() {
        return None;
    }
    crate::outcome_proofs::construct_mapped_join_pairs(bundle, &left, &right, rows)
}
