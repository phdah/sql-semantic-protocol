//! Exact finite EXISTS/NOT EXISTS membership through one nested INNER JOIN.
//!
//! All nested ON and WHERE predicates must be represented as typed source-column
//! equalities; complete caller-attested physical bags prove both presence and
//! absence without SQL reparsing or independence assumptions.

use std::collections::BTreeMap;

use super::exists_bags::{checked_ids, output_domain, row_id, scalar, signed_integer_source, PhysicalExistsColumnDomain};
use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::protocol::{ColumnExpression, ComparisonOperator, JoinKind, LiteralValue};

/// Three physical source instances and the complete nested join witness bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalJoinedExistsExact {
    /// Canonical dependency nodes visited for the target.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Independent source populations: outer, inner, joined partner.
    pub sources: [String; 3],
    /// Physical outer rows that survive EXISTS or NOT EXISTS.
    pub selected: Vec<String>,
    /// Physical outer rows whose predicate is not TRUE.
    pub rejected: Vec<String>,
    /// For each outer identity, complete qualifying inner/joined identity pairs.
    /// An empty list is a closed-world absence proof over both full inner bags.
    pub matching_pairs: BTreeMap<String, Vec<[String; 2]>>,
    /// Complete terminal SQL bag preserving duplicate outer values and NULL.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Typed exhaustive values and inclusive bounds from the terminal bag.
    pub terminal_domains: BTreeMap<String, PhysicalExistsColumnDomain>,
}

/// A closed-world proof, a contradictory physical identity or an uncertified law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalJoinedExistsProof {
    /// All predicates and all three complete bags jointly certified.
    Exact(Box<PhysicalJoinedExistsExact>),
    /// Contradictory source identity/schema evidence.
    Impossible { reason: String },
    /// Missing evidence or a nonportable nested execution law.
    Residual { reason: String },
}

fn residual(reason: &str) -> PhysicalJoinedExistsProof {
    PhysicalJoinedExistsProof::Residual {
        reason: reason.to_string(),
    }
}

type ColumnPair<'a> = (&'a ColumnExpression, &'a ColumnExpression);

// Only a complete conjunction of normalized equality-column predicates is
// executable. OR, NOT, unknown and non-equality are deliberately not weakened.
fn equality_pairs(predicate: &Predicate) -> Option<Vec<ColumnPair<'_>>> {
    match predicate {
        Predicate::Comparison(compare) if compare.operator() == ComparisonOperator::Eq => {
            match (compare.left(), compare.right()) {
                (Expression::Column(left), Expression::Column(right)) => Some(vec![(left, right)]),
                _ => None,
            }
        }
        Predicate::And(and) => {
            let mut pairs = Vec::new();
            for operand in and.operands() {
                pairs.extend(equality_pairs(operand)?);
            }
            Some(pairs)
        }
        _ => None,
    }
}

struct Sources<'a> {
    names: [&'a str; 3],
    aliases: [&'a str; 3],
    rows: [&'a PhysicalWriteRow; 3],
    bundle: &'a AnalysisBundle,
}

impl Sources<'_> {
    fn column_value(&self, column: &ColumnExpression) -> Result<(usize, Option<i128>), &'static str> {
        let qualifier = column.relation().ok_or("unqualified nested column is ambiguous")?;
        let owners = (0..3)
            .filter(|index| self.aliases[*index] == qualifier || self.names[*index] == qualifier)
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            return Err("column has no unique nested physical source instance");
        };
        if !signed_integer_source(self.bundle, self.names[*owner], column.name()) {
            return Err("nested equality column lacks authoritative signed integer type");
        }
        Ok((*owner, scalar(self.rows[*owner], column.name())?))
    }

    fn compare(
        &self,
        pair: &ColumnPair<'_>,
        expected_roles: (usize, usize),
    ) -> Result<bool, &'static str> {
        let (left_role, left) = self.column_value(pair.0)?;
        let (right_role, right) = self.column_value(pair.1)?;
        if !((left_role, right_role) == expected_roles
            || (right_role, left_role) == expected_roles)
        {
            return Err("comparison escapes its certified source/alias scope");
        }
        Ok(matches!((left, right), (Some(a), Some(b)) if a == b))
    }
}

/// Prove EXISTS/NOT EXISTS after one normalized nested two-source INNER JOIN.
///
/// The complete physical source bags are supplied exactly once, including a
/// stable `id` in every row. Every ON/WHERE equality is evaluated jointly on
/// individual source rows, and NULL comparisons never qualify. A join may
/// multiply matching inner witnesses but may never multiply outer SELECT rows.
/// The only accepted nested shape is a parser-certified ungrouped inner join.
pub fn physical_finite_joined_exists_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalJoinedExistsProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalJoinedExistsProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing canonical target");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("target does not have canonical query semantics");
    };
    let [outer] = query.sources() else {
        return residual("joined EXISTS requires exactly one physical outer source");
    };
    let [projection] = query.output().columns() else {
        return residual("joined EXISTS requires a single copied integer output");
    };
    let Some(projected) = projection.plain_copy_source() else {
        return residual("joined EXISTS requires a copied physical source column");
    };
    let Some(Predicate::Exists(exists)) = query.predicates().where_predicate() else {
        return residual("joined EXISTS must be the sole outer filter");
    };
    let nested = exists.subquery();
    let [inner, joined] = nested.sources() else {
        return residual("nested JOIN requires exactly two physical source instances");
    };
    let [join] = nested.joins() else {
        return residual("nested JOIN requires one canonical join");
    };
    let names = [outer.name(), inner.name(), joined.name()];
    let aliases = [
        outer.alias().unwrap_or(outer.name()),
        inner.alias().unwrap_or(inner.name()),
        joined.alias().unwrap_or(joined.name()),
    ];
    if !query.filter_only_row_shape()
        || query.aggregation().is_some()
        || !query.joins().is_empty()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.diagnostics().is_empty()
        || !nested.joined_exists_equivalent()
        || !nested.diagnostics().is_empty()
        || nested.predicates().having_predicate().is_some()
        || nested.predicates().qualify_predicate().is_some()
        || join.kind() != JoinKind::Inner
        || join.execution().is_some()
        || projected.relation() != names[0]
        || names[0] == names[1]
        || names[0] == names[2]
        || names[1] == names[2]
        || snapshots.len() != 3
        || walker.sources.len() != 3
        || names.iter().any(|name| !walker.sources.contains(*name))
        || nested.dependencies().len() != 2
        || nested.dependencies().iter().any(|name| name != names[1] && name != names[2])
        || join.left().relation() != names[1]
        || join.right().relation() != names[2]
    {
        return residual("nested join shape, source graph or physical provenance is not certified");
    }
    let Some(join_predicate) = join.condition() else {
        return residual("INNER JOIN lacks its complete ON predicate");
    };
    let Some(on_pairs) = equality_pairs(join_predicate) else {
        return residual("INNER JOIN ON is not a conjunction of source equalities");
    };
    let Some(where_predicate) = nested.predicates().where_predicate() else {
        return residual("nested JOIN lacks correlation WHERE");
    };
    let Some(where_pairs) = equality_pairs(where_predicate) else {
        return residual("nested WHERE is not a conjunction of source equalities");
    };
    if on_pairs.is_empty()
        || where_pairs.is_empty()
        || where_pairs.len() != nested.correlations().len()
    {
        return residual("nested JOIN correlation identities are incomplete");
    }
    if !signed_integer_source(bundle, names[0], projected.column()) {
        return residual("outer projection lacks authoritative signed integer schema");
    }

    let Some(states) = names
        .iter()
        .map(|name| snapshots.iter().find(|s| s.relation() == *name))
        .collect::<Option<Vec<_>>>()
    else {
        return residual("three complete source snapshots are required");
    };
    for state in &states {
        if let Err(reason) = checked_ids(state) {
            return PhysicalJoinedExistsProof::Impossible {
                reason: reason.to_string(),
            };
        }
    }
    // A complete inner Cartesian product, not a per-key independent existence
    // witness. Every rejected outer row is tested against all potential pairs.
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut matching_pairs = BTreeMap::new();
    let mut terminal = Vec::new();
    for outer_row in states[0].rows() {
        let Some(outer_id) = row_id(outer_row) else {
            return residual("missing outer physical identity");
        };
        let mut matches = Vec::new();
        for inner_row in states[1].rows() {
            for joined_row in states[2].rows() {
                let scope = Sources {
                    names,
                    aliases,
                    rows: [outer_row, inner_row, joined_row],
                    bundle,
                };
                let mut qualifies = true;
                for pair in &on_pairs {
                    let ok = match scope.compare(pair, (1, 2)) {
                        Ok(value) => value,
                        Err(reason) => return residual(reason),
                    };
                    qualifies &= ok;
                }
                for pair in &where_pairs {
                    let ok = match scope.compare(pair, (0, 1)) {
                        Ok(value) => value,
                        Err(reason) => return residual(reason),
                    };
                    qualifies &= ok;
                }
                if qualifies {
                    let (Some(inner_id), Some(joined_id)) = (row_id(inner_row), row_id(joined_row))
                    else {
                        return residual("nested physical source identity is missing");
                    };
                    matches.push([inner_id.to_string(), joined_id.to_string()]);
                }
            }
        }
        let survives = exists.negated() == matches.is_empty();
        if survives {
            let Some(value) = outer_row.values().get(projected.column()) else {
                return residual("outer projected value is missing");
            };
            terminal.push(BTreeMap::from([(projection.name().to_string(), value.clone())]));
            selected.push(outer_id.to_string());
        } else {
            rejected.push(outer_id.to_string());
        }
        matching_pairs.insert(outer_id.to_string(), matches);
    }
    let Some(domain) = output_domain(&terminal, projection.name()) else {
        return residual("terminal lacks exact typed integer domain");
    };
    PhysicalJoinedExistsProof::Exact(Box::new(PhysicalJoinedExistsExact {
        nodes: walker.nodes,
        sources: names.map(str::to_string),
        selected,
        rejected,
        matching_pairs,
        terminal,
        terminal_domains: BTreeMap::from([(projection.name().to_string(), domain)]),
    }))
}
