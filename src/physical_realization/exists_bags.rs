//! Complete finite EXISTS/NOT EXISTS source-bag proofs.
//!
//! Caller-supplied physical rows are closed-world evidence, not examples to
//! extrapolate from. Only one direct outer relation, one direct inner relation,
//! and a single attested integer equality correlation (or no correlation) are
//! executable. Unknown nested filtering and row shaping remain residual.

use std::collections::{BTreeMap, BTreeSet};

use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::protocol::{ComparisonOperator, LiteralValue};
use crate::subquery_witness::{SubqueryMembershipDirection, SubqueryMembershipKind};

/// Exact finite values appearing in one output column, with inclusive bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalExistsColumnDomain {
    values: Vec<i128>,
    null_allowed: bool,
}

impl PhysicalExistsColumnDomain {
    /// All distinct non-NULL integer values in ascending order.
    pub fn values(&self) -> &[i128] {
        &self.values
    }

    /// Whether SQL NULL occurs in the complete terminal bag.
    pub fn null_allowed(&self) -> bool {
        self.null_allowed
    }

    /// Inclusive lower and upper bounds, absent when all values are NULL or empty.
    pub fn bounds(&self) -> Option<(i128, i128)> {
        Some((*self.values.first()?, *self.values.last()?))
    }
}

/// Complete proof of both selected and SQL NOT TRUE outer row identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteExistsExact {
    /// Source-complete canonical dependency graph.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Physical outer and inner source names, in semantic role order.
    pub sources: [String; 2],
    /// EXISTS or NOT EXISTS as normalized by the SQL analysis.
    pub kind: SubqueryMembershipKind,
    /// Selected outer row IDs, each occurring exactly once.
    pub selected: Vec<String>,
    /// Outer IDs whose predicate is NOT TRUE, including NULL comparisons.
    pub rejected: Vec<String>,
    /// Every qualifying inner row ID for each outer row (including duplicates).
    /// Empty vectors prove absence against the *entire* supplied inner bag.
    pub matching_inner: BTreeMap<String, Vec<String>>,
    /// Complete SQL output bag, retaining outer duplicate values.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact enumerated terminal integer domain and NULL status.
    pub terminal_domains: BTreeMap<String, PhysicalExistsColumnDomain>,
}

/// Complete physical proof, contradicted source evidence, or explicit gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteExistsProof {
    /// Complete schema-validated physical bags and exact SELECT membership.
    Exact(Box<PhysicalFiniteExistsExact>),
    /// Physical rows contradict a required schema or identity condition.
    Impossible { reason: String },
    /// Evidence or SQL semantics do not establish an exact conclusion.
    Residual { reason: String },
}

fn residual(reason: &str) -> PhysicalFiniteExistsProof {
    PhysicalFiniteExistsProof::Residual {
        reason: reason.to_string(),
    }
}

fn row_id(row: &PhysicalWriteRow) -> Option<&str> {
    match row.values().get("id")? {
        LiteralValue::Text(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn checked_ids(snapshot: &PhysicalWriteState) -> Result<(), &'static str> {
    let mut ids = BTreeSet::new();
    for row in snapshot.rows() {
        let Some(id) = row_id(row) else {
            return Err("source row requires nonempty text id");
        };
        if !ids.insert(id) {
            return Err("duplicate physical source row identity");
        }
    }
    Ok(())
}

fn scalar(row: &PhysicalWriteRow, column: &str) -> Result<Option<i128>, &'static str> {
    match row.values().get(column) {
        Some(LiteralValue::Null) => Ok(None),
        Some(LiteralValue::Number(number)) => number
            .parse::<i128>()
            .map(Some)
            .map_err(|_| "source integer exceeds supported exact signed domain"),
        _ => Err("source column is absent or not an attested integer"),
    }
}

fn output_domain(
    rows: &[BTreeMap<String, LiteralValue>],
    column: &str,
) -> Option<PhysicalExistsColumnDomain> {
    let mut values = BTreeSet::new();
    let mut null_allowed = false;
    for row in rows {
        match row.get(column)? {
            LiteralValue::Number(number) => {
                values.insert(number.parse::<i128>().ok()?);
            }
            LiteralValue::Null => null_allowed = true,
            _ => return None,
        }
    }
    Some(PhysicalExistsColumnDomain {
        values: values.into_iter().collect(),
        null_allowed,
    })
}

/// Prove exact EXISTS/NOT EXISTS results from independently supplied complete
/// physical bags and a parser-independent normalized SQL query.
///
/// The only executable correlation is one integer source-column equality.
/// SQL NULL equality is UNKNOWN (never a match). EXISTS is insensitive to
/// inner duplicates, but their *individual* row identities are preserved in
/// `matching_inner`. For NOT EXISTS, an empty matching vector proves exclusion
/// of every potential TRUE partner in the full supplied inner population.
///
/// Without an attested schema, supported normalized predicate, and complete
/// direct source populations, the result is Residual rather than guessed Exact.
pub fn physical_finite_exists_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteExistsProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteExistsProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }

    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(target) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing canonical target");
    };
    let Some(query) = query_for(bundle, target) else {
        return residual("target is not a canonical query");
    };
    let [outer] = query.sources() else {
        return residual("EXISTS requires exactly one direct outer source");
    };
    let [witness] = query.subquery_witnesses() else {
        return residual("EXISTS requires exactly one normalized subquery witness");
    };
    let [output] = query.output().columns() else {
        return residual("finite EXISTS output must project one copied source column");
    };
    let Some(projected) = output.plain_copy_source() else {
        return residual("EXISTS output projection must be a copied physical column");
    };
    let (Some(outer_name), Some(inner_name)) =
        (witness.outer_source_relation(), witness.inner_relation())
    else {
        return residual("unresolved physical source identities");
    };
    if !query.filter_only_row_shape()
        || query.aggregation().is_some()
        || !query.joins().is_empty()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.diagnostics().is_empty()
        || outer.name() != outer_name
        || projected.relation() != outer_name
        || outer_name == inner_name
        || snapshots.len() != 2
        || !matches!(witness.qualifying(), SubqueryMembershipDirection::Exact(_))
        || !matches!(witness.rejected(), SubqueryMembershipDirection::Exact(_))
    {
        return residual("EXISTS outer query or local witness is not exactly supported");
    }
    let Some(Predicate::Exists(exists)) = query.predicates().where_predicate() else {
        return residual("EXISTS must be the only outer filter");
    };
    let kind = if exists.negated() {
        SubqueryMembershipKind::NotExists
    } else {
        SubqueryMembershipKind::Exists
    };
    if witness.kind() != kind {
        return residual("EXISTS predicate and witness disagree");
    }
    let correlation = match witness.correlations() {
        [] => {
            if exists.subquery().predicates().where_predicate().is_some() {
                return residual("uncorrelated nested filtering requires a complete predicate law");
            }
            None
        }
        [pair] => {
            // A single normalized equality is the complete inner WHERE.
            // Additional filters, disjunctions, or unknown comparisons are
            // not silently treated as TRUE or FALSE.
            let Some(Predicate::Comparison(cmp)) = exists.subquery().predicates().where_predicate()
            else {
                return residual("inner filter is not one equality correlation");
            };
            if cmp.operator() != ComparisonOperator::Eq
                || !matches!(
                    (cmp.left(), cmp.right()),
                    (Expression::Column(_), Expression::Column(_))
                )
                || pair.outer().relation() != Some(outer_name)
                || pair.inner().relation() != Some(inner_name)
            {
                return residual("correlation is not an identified source equality");
            }
            Some((pair.outer().name(), pair.inner().name()))
        }
        _ => return residual("only one correlated equality is certified"),
    };

    let [Some(outer_bag), Some(inner_bag)] = [
        snapshots
            .iter()
            .find(|state| state.relation() == outer_name),
        snapshots
            .iter()
            .find(|state| state.relation() == inner_name),
    ] else {
        return residual("both complete source snapshots are required");
    };
    if walker.sources.len() != 2
        || !walker.sources.contains(outer_name)
        || !walker.sources.contains(inner_name)
    {
        return residual("a source is not a direct independent physical leaf");
    }
    for bag in [outer_bag, inner_bag] {
        if let Err(reason) = checked_ids(bag) {
            return PhysicalFiniteExistsProof::Impossible {
                reason: reason.to_string(),
            };
        }
    }

    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut matching_inner = BTreeMap::new();
    let mut terminal = Vec::new();
    for outer_row in outer_bag.rows() {
        let Some(outer_id) = row_id(outer_row) else {
            return residual("outer row has no physical identity");
        };
        let outer_value = match correlation {
            Some((outer_column, _)) => match scalar(outer_row, outer_column) {
                Ok(value) => value,
                Err(reason) => return residual(reason),
            },
            None => None,
        };
        let mut partners = Vec::new();
        for inner_row in inner_bag.rows() {
            let matches = match correlation {
                None => true,
                Some((_, inner_column)) => {
                    let inner_value = match scalar(inner_row, inner_column) {
                        Ok(value) => value,
                        Err(reason) => return residual(reason),
                    };
                    matches!((outer_value, inner_value), (Some(a), Some(b)) if a == b)
                }
            };
            if matches {
                let Some(inner_id) = row_id(inner_row) else {
                    return residual("inner row has no physical identity");
                };
                partners.push(inner_id.to_string());
            }
        }
        let survives = (kind == SubqueryMembershipKind::Exists) == !partners.is_empty();
        if survives {
            let Some(value) = outer_row.values().get(projected.column()) else {
                return residual("projected outer source column is absent");
            };
            if !matches!(value, LiteralValue::Number(_) | LiteralValue::Null) {
                return residual("finite EXISTS projection is not an integer value");
            }
            terminal.push(BTreeMap::from([(output.name().to_string(), value.clone())]));
            selected.push(outer_id.to_string());
        } else {
            rejected.push(outer_id.to_string());
        }
        matching_inner.insert(outer_id.to_string(), partners);
    }
    let Some(domain) = output_domain(&terminal, output.name()) else {
        return residual("terminal does not have an exact typed integer domain");
    };
    PhysicalFiniteExistsProof::Exact(Box::new(PhysicalFiniteExistsExact {
        nodes: walker.nodes,
        sources: [outer_name.to_string(), inner_name.to_string()],
        kind,
        selected,
        rejected,
        matching_inner,
        terminal,
        terminal_domains: BTreeMap::from([(output.name().to_string(), domain)]),
    }))
}
