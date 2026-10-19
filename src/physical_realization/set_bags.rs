//! Closed-world, two-column SQL set bag execution from canonical semantics.
//!
//! Each input is a complete, schema-validated physical relation. Counts are
//! keyed by the *entire* nullable tuple, never by independent column domains.

use super::*;
use crate::protocol::{LiteralValue, SetMultiplicityRule, SetWitnessDirection};

/// A complete nullable two-integer tuple and its closed-world source counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSetTuple {
    /// Exact positional values, with None representing SQL NULL.
    pub values: [Option<i128>; 2],
    /// Stable identities of every matching row in the left physical source.
    pub left_row_ids: Vec<String>,
    /// Stable identities of every matching row in the right physical source.
    pub right_row_ids: Vec<String>,
    /// Exact resulting multiplicity, including zero for rejected candidates.
    pub output_count: u64,
}

/// Exact finite column value domain, retaining NULL and inclusive bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSetColumnDomain {
    /// All distinct present finite integer values in sorted order.
    pub values: Vec<i128>,
    /// Whether a surviving terminal row has SQL NULL in this position.
    pub contains_null: bool,
}

impl PhysicalSetColumnDomain {
    /// Inclusive minimum and maximum, or None for no finite values.
    pub fn bounds(&self) -> Option<(i128, i128)> {
        Some((*self.values.first()?, *self.values.last()?))
    }
}

/// Whole-source, whole-output proof for an independent two-column set operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteSetExact {
    /// Canonical dependency graph including the producer/operator layer.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Source relation identities in positional left/right order.
    pub sources: [String; 2],
    /// Every candidate tuple, including those with zero output multiplicity.
    pub tuples: Vec<PhysicalSetTuple>,
    /// Exact terminal SQL bag with output names and all duplicates preserved.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact two-column terminal domains, including NULL and inclusive bounds.
    pub terminal_domains: [PhysicalSetColumnDomain; 2],
}

/// Closed-world proof status. Missing evidence is never a negative proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteSetProof {
    /// Complete certified bag, including all absent/rejected source tuples.
    Exact(Box<PhysicalFiniteSetExact>),
    /// A supplied physical population contradicts an enforced invariant.
    Impossible { reason: String },
    /// Unsupported parser, alignment, projection or source evidence.
    Residual { reason: String },
}

fn residual(reason: &str) -> PhysicalFiniteSetProof {
    PhysicalFiniteSetProof::Residual {
        reason: reason.to_string(),
    }
}

fn integer(value: &LiteralValue) -> Result<Option<i128>, &'static str> {
    match value {
        LiteralValue::Null => Ok(None),
        LiteralValue::Number(text) => text
            .parse::<i128>()
            .map(Some)
            .map_err(|_| "set column is not an exact signed integer"),
        _ => Err("set column lacks an attested exact integer/collation law"),
    }
}

fn tuple_from_row(
    row: &PhysicalWriteRow,
    columns: &[String],
) -> Result<[Option<i128>; 2], &'static str> {
    let [first, second] = columns else {
        return Err("set branch does not project exactly two columns");
    };
    Ok([
        integer(row.values().get(first).ok_or("missing first set column")?)?,
        integer(
            row.values()
                .get(second)
                .ok_or("missing second set column")?,
        )?,
    ])
}

fn physical_identity(row: &PhysicalWriteRow) -> Option<&str> {
    match row.values().get("id")? {
        LiteralValue::Text(id) if !id.is_empty() => Some(id.as_str()),
        _ => None,
    }
}

/// Evaluate a simple two-source, two-column canonical set operation against
/// complete caller-attested physical bags.
///
/// Each branch must be an independent, direct, row-preserving projection of
/// one physical source. Nested operators, casts, filtering, vendor collation,
/// alignment by name and materialized producers remain Residual. This function
/// evaluates canonical set laws, not SQL source text or fabricated source rows.
pub fn physical_finite_set_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteSetProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteSetProof::Impossible { reason };
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
        return residual("target does not contain a set operation");
    };
    let Some(rule) = operation.multiplicity_rule() else {
        return residual("set alignment has no certified positional tuple law");
    };
    if !matches!(operation.left(), SetOperand::Query)
        || !matches!(operation.right(), SetOperand::Query)
        || !query.diagnostics().is_empty()
        || query.output().columns().len() != 2
        || walker.sources.len() != 2
        || snapshots.len() != 2
    {
        return residual("set requires two independent physical branches and two output columns");
    }
    if !matches!(
        operation.witness_directions(),
        (SetWitnessDirection::Exact(_), SetWitnessDirection::Exact(_))
    ) {
        return residual("branch membership or set-level shaping is not independently exact");
    }
    let [left, right] = operation.branches() else {
        return residual("set does not have exactly two leaf branches");
    };
    let mut observed = BTreeMap::<[Option<i128>; 2], [Vec<String>; 2]>::new();
    let mut source_names = Vec::new();
    for (index, branch) in [left, right].into_iter().enumerate() {
        let Some(boundary) = branch.witness_boundary() else {
            return residual("branch lacks an exact source-column boundary");
        };
        if boundary.is_intermediate()
            || boundary.tuple_columns().len() != 2
            || !branch.condition_exactness().is_exact()
            || !branch.empty_input_preserving()
            || branch.dependencies().len() != 1
            || branch.predicates().where_predicate().is_some()
            || branch.predicates().having_predicate().is_some()
            || branch.predicates().qualify_predicate().is_some()
        {
            return residual("branch needs a direct complete row-preserving physical projection");
        }
        let relation = boundary.relation();
        if !walker.sources.contains(relation) || source_names.contains(&relation.to_string()) {
            return residual("set source instances are shared or not directly physical");
        }
        let Some(snapshot) = snapshots.iter().find(|state| state.relation() == relation) else {
            return residual("missing complete physical set source");
        };
        source_names.push(relation.to_string());
        let mut identities = BTreeSet::new();
        for row in snapshot.rows() {
            let Some(id) = physical_identity(row) else {
                return residual("source row requires a stable nonempty text id");
            };
            if !identities.insert(id) {
                // A duplicate caller-supplied ID is an unproven identity map,
                // not a contradiction of SQL bag semantics.
                return residual("physical rows do not have distinct stable identities");
            }
            let Ok(key) = tuple_from_row(row, boundary.tuple_columns()) else {
                return residual("set source projection lacks two exact nullable integer values");
            };
            observed
                .entry(key)
                .or_insert_with(|| [Vec::new(), Vec::new()])[index]
                .push(id.to_string());
        }
    }
    let [source_left, source_right] = source_names.as_slice() else {
        return residual("missing source instances");
    };
    // The bag equality key is the entire tuple: (1, NULL) equals (1, NULL)
    // for set membership, while (1, NULL) and (1, 4) stay distinct.
    let mut tuples = Vec::new();
    let mut terminal = Vec::new();
    let mut distinct_values = [BTreeSet::new(), BTreeSet::new()];
    let mut contains_null = [false; 2];
    let names = query
        .output()
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    for (values, [left_row_ids, right_row_ids]) in observed {
        let Ok(left_count) = u64::try_from(left_row_ids.len()) else {
            return residual("set input multiplicity overflow");
        };
        let Ok(right_count) = u64::try_from(right_row_ids.len()) else {
            return residual("set input multiplicity overflow");
        };
        if rule == SetMultiplicityRule::Sum && left_count.checked_add(right_count).is_none() {
            return residual("UNION ALL output multiplicity overflow");
        }
        let output_count = rule.evaluate(left_count, right_count);
        if output_count > 0 {
            for (index, value) in values.iter().enumerate() {
                match value {
                    Some(value) => {
                        distinct_values[index].insert(*value);
                    }
                    None => contains_null[index] = true,
                }
            }
            let output_row = names
                .iter()
                .zip(values)
                .map(|(name, value)| {
                    (
                        name.clone(),
                        value.map_or(LiteralValue::Null, |number| {
                            LiteralValue::Number(number.to_string())
                        }),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for _ in 0..output_count {
                terminal.push(output_row.clone());
            }
        }
        tuples.push(PhysicalSetTuple {
            values,
            left_row_ids,
            right_row_ids,
            output_count,
        });
    }
    let terminal_domains = std::array::from_fn(|index| PhysicalSetColumnDomain {
        values: distinct_values[index].iter().copied().collect(),
        contains_null: contains_null[index],
    });
    PhysicalFiniteSetProof::Exact(Box::new(PhysicalFiniteSetExact {
        nodes: walker.nodes,
        sources: [source_left.clone(), source_right.clone()],
        tuples,
        terminal,
        terminal_domains,
    }))
}
