//! Complete physical row proofs for nested portable SQL set trees.
//!
//! All branches read the same caller-supplied closed source assignments.
//! Multiplicities are evaluated bottom-up from the canonical operation tree;
//! no branch can fabricate an independently writable intermediate bag.

use super::*;
use crate::protocol::{ComparisonOperator, LiteralValue, SetBranch, SetMultiplicityRule};

/// Per-leaf proof of SQL TRUE and NOT TRUE source-row membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSetTreeBranch {
    /// Stable canonical leaf identity.
    pub identity: String,
    /// Shared physical relation whose one population feeds this branch.
    pub source: String,
    /// Complete row identities satisfying this branch's WHERE condition.
    pub selected_row_ids: Vec<String>,
    /// Complete row identities for which the branch condition is FALSE.
    pub false_row_ids: Vec<String>,
    /// Complete row identities for which the branch condition is SQL UNKNOWN.
    pub unknown_row_ids: Vec<String>,
}

/// Full positional nullable tuple, source identities, and final multiplicity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSetTreeTuple {
    /// NULL-safe positional key shared by every set operation in the tree.
    pub values: Vec<Option<i128>>,
    /// Selected physical identities, one list per canonical leaf in tree order.
    pub branch_row_ids: Vec<Vec<String>>,
    /// Closed-world terminal multiplicity, including zero for absence.
    pub output_count: u64,
}

/// Exact complete physical-source and terminal result for a nested set tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteSetTreeExact {
    /// Canonical graph with shared sources and materialized producers once.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Distinct source relations, not one writable source per set branch.
    pub sources: Vec<String>,
    /// Independent TRUE/FALSE/UNKNOWN partition of each physical branch read.
    pub branches: Vec<PhysicalSetTreeBranch>,
    /// Complete candidate universe with even cancelled or rejected tuples.
    pub tuples: Vec<PhysicalSetTreeTuple>,
    /// Full named output bag preserving every SQL duplicate.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact positional column domains, including NULL and inclusive bounds.
    pub terminal_domains: Vec<PhysicalSetColumnDomain>,
}

/// Closed-world proof result for a nested set operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteSetTreeProof {
    /// Every physical and intermediate row is accounted for.
    Exact(Box<PhysicalFiniteSetTreeExact>),
    /// Enforced physical schema evidence rules out a supplied assignment.
    Impossible { reason: String },
    /// A missing or unsupported law prevents either exact or impossible proof.
    Residual { reason: String },
}

fn residual(reason: &str) -> PhysicalFiniteSetTreeProof {
    PhysicalFiniteSetTreeProof::Residual {
        reason: reason.to_string(),
    }
}

fn integer(value: &LiteralValue) -> Result<Option<i128>, &'static str> {
    match value {
        LiteralValue::Null => Ok(None),
        LiteralValue::Number(value) => value
            .parse::<i128>()
            .map(Some)
            .map_err(|_| "set source has no certified integer representation"),
        _ => Err("set tree requires attested nullable signed-integer source values"),
    }
}

fn physical_id(row: &PhysicalWriteRow) -> Option<&str> {
    match row.values().get("id")? {
        LiteralValue::Text(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn mapped_column(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    layer_id: &str,
    relation: &str,
    column: &str,
) -> Option<(String, String)> {
    let original = crate::protocol::ColumnRef::new(Some(relation.to_string()), column.to_string());
    let mapped = resolve_filter_column_with_evidence(bundle, walker, layer_id, &original, 0, true)?;
    let source = mapped.relation()?;
    walker
        .sources
        .contains(source)
        .then(|| (source.to_string(), mapped.name().to_string()))
}

fn scalar(
    expression: &Expression,
    row: &PhysicalWriteRow,
    boundary_relation: &str,
    physical_columns: &BTreeMap<String, String>,
) -> Result<Option<i128>, &'static str> {
    match expression {
        Expression::Literal(literal) => integer(literal.value()),
        Expression::Column(column) => {
            if column
                .relation()
                .is_some_and(|source| source != boundary_relation)
            {
                return Err("WHERE refers to a different source instance");
            }
            let physical_column = physical_columns
                .get(column.name())
                .ok_or("WHERE column cannot be traced through a producer")?;
            integer(
                row.values()
                    .get(physical_column)
                    .ok_or("physical source is missing a WHERE column")?,
            )
        }
        _ => Err("WHERE expression has no certified total integer evaluator"),
    }
}

fn sql_comparison(
    operator: ComparisonOperator,
    lhs: Option<i128>,
    rhs: Option<i128>,
) -> Result<Option<bool>, &'static str> {
    if matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    ) {
        return Err("NULL-safe comparisons require a separately certified law");
    }
    let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
        return Ok(None);
    };
    Ok(Some(match operator {
        ComparisonOperator::Eq => lhs == rhs,
        ComparisonOperator::Neq => lhs != rhs,
        ComparisonOperator::Lt => lhs < rhs,
        ComparisonOperator::Lte => lhs <= rhs,
        ComparisonOperator::Gt => lhs > rhs,
        ComparisonOperator::Gte => lhs >= rhs,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
            return Err("NULL-safe comparison needs a separately certified law");
        }
    }))
}

fn where_truth(
    predicate: &Predicate,
    row: &PhysicalWriteRow,
    relation: &str,
    columns: &BTreeMap<String, String>,
) -> Result<Option<bool>, &'static str> {
    match predicate {
        Predicate::Comparison(comparison) => sql_comparison(
            comparison.operator(),
            scalar(comparison.left(), row, relation, columns)?,
            scalar(comparison.right(), row, relation, columns)?,
        ),
        Predicate::And(and) => {
            let mut unknown = false;
            for operand in and.operands() {
                match where_truth(operand, row, relation, columns)? {
                    Some(false) => return Ok(Some(false)),
                    Some(true) => {}
                    None => unknown = true,
                }
            }
            Ok(if unknown { None } else { Some(true) })
        }
        _ => Err("set branch WHERE lacks a certified SQL three-valued law"),
    }
}

/// Type-certified source projection and row-filter columns for one leaf.
struct ResolvedSetBranch<'a> {
    source: String,
    relation: &'a str,
    projected: Vec<String>,
    predicate_columns: BTreeMap<String, String>,
}

fn branch_source<'a>(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    layer_id: &str,
    branch: &'a SetBranch,
) -> Result<ResolvedSetBranch<'a>, &'static str> {
    if !branch.condition_exactness().is_exact()
        || !branch.empty_input_preserving()
        || branch.predicates().having_predicate().is_some()
        || branch.predicates().qualify_predicate().is_some()
        || branch.sources().len() != 1
        || branch.output().columns().is_empty()
    {
        return Err("set leaf has unsupported row-shaping or ambiguous source evidence");
    }
    let relation = branch
        .witness_boundary()
        .map(|boundary| boundary.relation())
        .unwrap_or_else(|| branch.sources()[0].name());
    let certified_cte = relation != branch.sources()[0].name()
        && walker.sources.contains(relation)
        && branch
            .witness_boundary()
            .is_some_and(|witness| !witness.is_intermediate());
    let source = physical_transparent_producer_leaf(bundle, walker, relation)
        .ok_or("set branch producer is not a certified lossless physical projection")?;
    if !walker.sources.contains(&source) {
        return Err("set branch resolves to no complete physical source");
    }
    let mut mapped = Vec::new();
    let mut columns = BTreeMap::new();
    for (index, output) in branch.output().columns().iter().enumerate() {
        let copied = output
            .plain_copy_source()
            .ok_or("set projection is not a positional column copy")?;
        let name = if certified_cte {
            let name = branch
                .witness_boundary()
                .and_then(|witness| witness.tuple_columns().get(index))
                .ok_or("CTE lacks a complete certified positional source map")?;
            if copied.column() != name {
                return Err("CTE output differs from its certified physical column");
            }
            name.clone()
        } else {
            let (physical_relation, name) =
                mapped_column(bundle, walker, layer_id, copied.relation(), copied.column())
                    .ok_or("set output column lacks type-certified physical provenance")?;
            if physical_relation != source {
                return Err("set output columns do not belong to one complete physical source");
            }
            name
        };
        columns.insert(copied.column().to_string(), name.clone());
        mapped.push(name);
    }
    // Also map a branch-local predicate column not projected in the result.
    // The physical source is the only binding allowed at this boundary.
    if let Some(predicate) = branch.predicates().where_predicate() {
        fn collect_columns<'a>(
            predicate: &'a Predicate,
            out: &mut Vec<&'a str>,
        ) -> Result<(), &'static str> {
            match predicate {
                Predicate::Comparison(compare) => {
                    for expression in [compare.left(), compare.right()] {
                        match expression {
                            Expression::Column(column) => out.push(column.name()),
                            Expression::Literal(_) => {}
                            _ => return Err("WHERE contains an unsupported expression"),
                        }
                    }
                    Ok(())
                }
                Predicate::And(and) => {
                    for operand in and.operands() {
                        collect_columns(operand, out)?;
                    }
                    Ok(())
                }
                _ => Err("WHERE contains an unsupported predicate"),
            }
        }
        let mut names = Vec::new();
        collect_columns(predicate, &mut names)?;
        for name in names {
            if columns.contains_key(name) {
                continue;
            }
            if certified_cte {
                return Err("CTE predicate refers to an unproven unprojected source column");
            }
            let (physical_relation, physical_name) =
                mapped_column(bundle, walker, layer_id, relation, name)
                    .ok_or("WHERE column lacks physical provenance")?;
            if physical_relation != source {
                return Err("WHERE column belongs to another physical source");
            }
            columns.insert(name.to_string(), physical_name);
        }
    }
    Ok(ResolvedSetBranch {
        source,
        relation: branch.sources()[0].name(),
        projected: mapped,
        predicate_columns: columns,
    })
}

fn tree_count(
    operation: &SetOperation,
    counts: &[u64],
    cursor: &mut usize,
) -> Result<u64, &'static str> {
    if !operation.set_level_safe() {
        return Err("set or nested branch has unproved ORDER LIMIT FETCH semantics");
    }
    let rule = operation
        .multiplicity_rule()
        .ok_or("set tree uses an unsupported alignment or coercion law")?;
    fn operand_count(
        operand: &SetOperand,
        counts: &[u64],
        cursor: &mut usize,
    ) -> Result<u64, &'static str> {
        match operand {
            SetOperand::Query => {
                let value = counts
                    .get(*cursor)
                    .copied()
                    .ok_or("canonical leaf count does not match set tree")?;
                *cursor += 1;
                Ok(value)
            }
            SetOperand::Operation(inner) => tree_count(inner, counts, cursor),
        }
    }
    let left = operand_count(operation.left(), counts, cursor)?;
    let right = operand_count(operation.right(), counts, cursor)?;
    if rule == SetMultiplicityRule::Sum && left.checked_add(right).is_none() {
        return Err("nested UNION ALL overflows exact physical multiplicity");
    }
    Ok(rule.evaluate(left, right))
}

/// Evaluate nested set bag laws against one complete physical assignment.
///
/// Certifies direct and losslessly materialized signed-integer columns, exact
/// branch-local integer comparisons, NULL tuple equality, shared source rows
/// and arbitrary supported nested DISTINCT/ALL operators. Nontransparent
/// producers, casts, ordering and unattested comparisons fail closed.
pub fn physical_finite_set_tree_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteSetTreeProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteSetTreeProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing canonical set layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("missing canonical set query");
    };
    let Some(operation) = query.set_operation() else {
        return residual("target is not a SQL set operation");
    };
    if !query.diagnostics().is_empty() || !operation.set_level_safe() {
        return residual("set-level semantics are unknown or have unproved row shaping");
    }
    let names = query
        .output()
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    if names.is_empty()
        || names.len() > 2
        || names.iter().collect::<BTreeSet<_>>().len() != names.len()
        || operation.branches().len() < 2
    {
        return residual("set tree needs one or two distinct positional output columns");
    }
    let sources = walker.sources.iter().cloned().collect::<Vec<_>>();
    if snapshots.len() != sources.len()
        || snapshots
            .iter()
            .map(|state| state.relation())
            .collect::<BTreeSet<_>>()
            .len()
            != snapshots.len()
        || snapshots
            .iter()
            .any(|state| !walker.sources.contains(state.relation()))
    {
        return residual(
            "exactly one complete population per distinct physical source is required",
        );
    }
    let mut identities = BTreeMap::<String, BTreeSet<String>>::new();
    for state in snapshots {
        for row in state.rows() {
            let Some(id) = physical_id(row) else {
                return residual("every source row needs a nonempty stable text id");
            };
            if !identities
                .entry(state.relation().to_string())
                .or_default()
                .insert(id.to_string())
            {
                return residual("physical source contains duplicate row identities");
            }
        }
    }

    let mut observed = BTreeMap::<Vec<Option<i128>>, Vec<Vec<String>>>::new();
    let mut branches = Vec::new();
    for (index, branch) in operation.branches().iter().enumerate() {
        let resolved = match branch_source(bundle, &walker, target_layer_id, branch) {
            Ok(resolved) => resolved,
            Err(reason) => return residual(reason),
        };
        let ResolvedSetBranch {
            source,
            relation,
            projected: mapped,
            predicate_columns: columns,
        } = resolved;
        if mapped.len() != names.len() {
            return residual("set leaves disagree on positional output arity");
        }
        let Some(state) = snapshots.iter().find(|state| state.relation() == source) else {
            return residual("missing source snapshot for a set branch");
        };
        let mut selected_row_ids = Vec::new();
        let mut false_row_ids = Vec::new();
        let mut unknown_row_ids = Vec::new();
        for row in state.rows() {
            let Some(id) = physical_id(row) else {
                return residual("source identity vanished while evaluating branch");
            };
            let tuple = mapped
                .iter()
                .map(|name| {
                    row.values()
                        .get(name)
                        .ok_or("missing projected physical value")
                        .and_then(integer)
                })
                .collect::<Result<Vec<_>, _>>();
            let Ok(tuple) = tuple else {
                return residual("set projection has unsupported physical scalar evidence");
            };
            let truth = match branch.predicates().where_predicate() {
                Some(predicate) => where_truth(predicate, row, relation, &columns),
                None => Ok(Some(true)),
            };
            let Ok(truth) = truth else {
                return residual("branch filter lacks exact physical SQL truth semantics");
            };
            let rows = observed
                .entry(tuple)
                .or_insert_with(|| vec![Vec::new(); operation.branches().len()]);
            match truth {
                Some(true) => {
                    rows[index].push(id.to_string());
                    selected_row_ids.push(id.to_string());
                }
                Some(false) => false_row_ids.push(id.to_string()),
                None => unknown_row_ids.push(id.to_string()),
            }
        }
        branches.push(PhysicalSetTreeBranch {
            identity: branch.identity().to_string(),
            source,
            selected_row_ids,
            false_row_ids,
            unknown_row_ids,
        });
    }
    let mut tuples = Vec::new();
    let mut terminal = Vec::new();
    let mut distinct = vec![BTreeSet::new(); names.len()];
    let mut contains_null = vec![false; names.len()];
    for (values, branch_row_ids) in observed {
        let counts = branch_row_ids
            .iter()
            .map(|ids| u64::try_from(ids.len()).map_err(|_| "set source multiplicity overflow"))
            .collect::<Result<Vec<_>, _>>();
        let Ok(counts) = counts else {
            return residual("set source multiplicity overflow");
        };
        let mut cursor = 0;
        let Ok(output_count) = tree_count(operation, &counts, &mut cursor) else {
            return residual("nested set operator has no exact multiplicity law");
        };
        if cursor != branches.len() {
            return residual("canonical set leaves do not match the operator tree");
        }
        if output_count > 0 {
            let named = names
                .iter()
                .zip(values.iter())
                .map(|(name, value)| {
                    (
                        name.clone(),
                        value.map_or(LiteralValue::Null, |number| {
                            LiteralValue::Number(number.to_string())
                        }),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for (index, value) in values.iter().enumerate() {
                match value {
                    Some(value) => {
                        distinct[index].insert(*value);
                    }
                    None => contains_null[index] = true,
                }
            }
            for _ in 0..output_count {
                terminal.push(named.clone());
            }
        }
        tuples.push(PhysicalSetTreeTuple {
            values,
            branch_row_ids,
            output_count,
        });
    }
    let terminal_domains = distinct
        .into_iter()
        .zip(contains_null)
        .map(|(values, contains_null)| PhysicalSetColumnDomain {
            values: values.into_iter().collect(),
            contains_null,
        })
        .collect();
    PhysicalFiniteSetTreeProof::Exact(Box::new(PhysicalFiniteSetTreeExact {
        nodes: walker.nodes,
        sources,
        branches,
        tuples,
        terminal,
        terminal_domains,
    }))
}
