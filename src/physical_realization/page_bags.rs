//! Exact direct SELECT pagination over a single attested closed-world source bag.
//!
//! A parser-certified row limit does not certify which physical rows survive.
//! Exactness requires a total integer ordering over the complete physical source.

use super::write_states::{physical_closed_world_schema_plan, PhysicalWriteStatePlan};
use super::*;
use crate::protocol::{Expression, LiteralValue, ValueDomain};
use std::cmp::Ordering;

/// One complete physical source row and its proven page membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalPageRow {
    /// Stable physical identity. SQL duplicate values do not merge source rows.
    pub source_row_id: String,
    /// Zero-based rank in the fully ordered source, independent of insertion order.
    pub order_position: usize,
    /// Whether this row survives the complete OFFSET/LIMIT selection.
    pub survives: bool,
}

/// Closed source identities, rejected rows, complete terminal bag and exact domains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFinitePageExact {
    /// Canonical source and producer graph.
    pub nodes: Vec<PhysicalPlanNode>,
    /// The one attested physical source relation.
    pub source: String,
    /// All physical source identities, including rows rejected by pagination.
    pub rows: Vec<PhysicalPageRow>,
    /// Entire page output with SQL duplicate multiplicities preserved.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact per-column terminal value domains, including empty and NULL.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// Complete evidence classification for direct single-source pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFinitePageProof {
    /// One total ordering and whole source/terminal bags are certified.
    Exact(Box<PhysicalFinitePageExact>),
    /// Enforced physical constraints contradict the proposed source bag.
    Impossible { reason: String },
    /// Missing or engine-dependent laws prevent an exact physical proof.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalFinitePageProof {
    PhysicalFinitePageProof::Residual {
        reason: reason.into(),
    }
}

fn compare_ordered_rows(
    lhs: &[Option<i128>],
    rhs: &[Option<i128>],
    keys: &[crate::protocol::SetSortKey],
) -> Ordering {
    lhs.iter()
        .zip(rhs)
        .zip(keys)
        .map(|((left, right), key)| match (left, right) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => {
                if key.nulls_first == Some(true) {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (Some(_), None) => {
                if key.nulls_first == Some(true) {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (Some(a), Some(b)) => {
                if key.descending {
                    b.cmp(a)
                } else {
                    a.cmp(b)
                }
            }
        })
        .find(|order| *order != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

/// Prove the entire ordered page, every rejected physical identity and all finite
/// terminal domains. The SQL AST is never inspected outside analysis: all
/// ordering and page boundaries are read from the canonical query statement.
///
/// This deliberately refuses WHERE/JOIN/GROUP/QUALIFY, producer composition,
/// nonintegral sort keys, ties and unproved implicit NULL ordering.
pub fn physical_finite_page_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFinitePageProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFinitePageProof::Impossible { reason }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let [snapshot] = snapshots else {
        return residual("pagination requires one complete physical source");
    };
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("unknown page target layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("page target has no canonical query");
    };
    let Some(shaping) = query.row_shaping() else {
        return residual("query has no normalized deterministic row-selection bounds");
    };
    let [source] = query.sources() else {
        return residual("pagination requires a direct single source");
    };
    if snapshot.relation() != source.name()
        || !walker.sources.contains(source.name())
        || !query.joins().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
    {
        return residual("query has unproved source identity or additional row shaping");
    }
    let Some(schema) = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source.name())
    else {
        return residual("pagination needs an authoritative physical source schema");
    };
    let mut output_names = BTreeSet::new();
    let mut projection = Vec::new();
    for column in query.output().columns() {
        if !output_names.insert(column.name()) {
            return residual("duplicate page output column identity");
        }
        let Expression::Column(expr) = column.expression() else {
            return residual("pagination requires source-column-only projection");
        };
        if expr
            .relation()
            .is_some_and(|relation| relation != source.name() && Some(relation) != source.alias())
            || !schema
                .columns()
                .iter()
                .any(|schema_column| schema_column.name() == expr.name())
        {
            return residual("page projection does not resolve to one physical source column");
        }
        projection.push((column.name().to_string(), expr.name().to_string()));
    }
    if projection.is_empty() || shaping.keys.is_empty() {
        return residual("page has no certified output or order keys");
    }
    let mut sort_names = Vec::new();
    for key in &shaping.keys {
        let Some((_, source_name)) = projection.get(key.index) else {
            return residual("ORDER BY refers to an absent projected column");
        };
        let Some(declared) = schema
            .columns()
            .iter()
            .find(|column| column.name() == source_name)
        else {
            return residual("ORDER BY has no authoritative column type");
        };
        if !super::aggregate_bags::signed_integer(declared.data_type()) {
            return residual("ORDER BY lacks portable signed integer comparison semantics");
        }
        sort_names.push(source_name);
    }

    let mut rows = Vec::<(usize, Vec<Option<i128>>)>::new();
    for (index, row) in snapshot.rows().iter().enumerate() {
        let mut keys = Vec::new();
        for (key, source_name) in shaping.keys.iter().zip(&sort_names) {
            let value = match super::rank_bags::key_value(row, source_name) {
                Ok(value) => value,
                Err(reason) => return residual(reason),
            };
            if value.is_none() && key.nulls_first.is_none() {
                return residual("NULL source key requires explicit NULLS FIRST or NULLS LAST");
            }
            keys.push(value);
        }
        rows.push((index, keys));
    }
    rows.sort_by(|left, right| compare_ordered_rows(&left.1, &right.1, &shaping.keys));
    if rows
        .windows(2)
        .any(|pair| compare_ordered_rows(&pair[0].1, &pair[1].1, &shaping.keys) == Ordering::Equal)
    {
        return residual("non-unique ORDER BY keys do not establish exact physical survivors");
    }

    let mut selected = vec![None; snapshot.rows().len()];
    let mut terminal = Vec::new();
    for (position, (source_index, _)) in rows.iter().enumerate() {
        let survives = position >= shaping.offset && position - shaping.offset < shaping.limit;
        selected[*source_index] = Some(PhysicalPageRow {
            source_row_id: format!("{}#{}", source.name(), source_index),
            order_position: position,
            survives,
        });
        if !survives {
            continue;
        }
        let Some(row) = snapshot.rows().get(*source_index) else {
            return residual("physical ordered row is absent from source bag");
        };
        let mut output = BTreeMap::new();
        for (output_name, source_name) in &projection {
            let Some(value) = row.values().get(source_name) else {
                return residual("physical page projection is missing a declared column");
            };
            output.insert(output_name.clone(), value.clone());
        }
        terminal.push(output);
    }
    let Some(rows) = selected.into_iter().collect::<Option<Vec<_>>>() else {
        return residual("incomplete page identity coverage");
    };
    let mut terminal_domains = BTreeMap::new();
    for (name, _) in &projection {
        terminal_domains.insert(
            name.clone(),
            super::rank_bags::exact_domain(
                terminal.iter().filter_map(|row| row.get(name).cloned()),
            ),
        );
    }
    PhysicalFinitePageProof::Exact(Box::new(PhysicalFinitePageExact {
        nodes: walker.nodes,
        source: source.name().to_string(),
        rows,
        terminal,
        terminal_domains,
    }))
}

/// Check membership of one requested terminal row against the entire proved page.
///
/// An absent terminal value tuple is Impossible even when that tuple exists in
/// the physical source before ORDER BY and OFFSET/LIMIT. This requested-row
/// impossibility is distinct from an impossible physical source snapshot.
pub fn physical_finite_page_membership_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
    requested: &BTreeMap<String, LiteralValue>,
) -> PhysicalFinitePageProof {
    match physical_finite_page_bag_plan(bundle, target_layer_id, snapshots) {
        PhysicalFinitePageProof::Exact(exact) => {
            if exact.terminal.contains(requested) {
                PhysicalFinitePageProof::Exact(exact)
            } else {
                PhysicalFinitePageProof::Impossible {
                    reason: "requested row absent from certified complete page bag".to_string(),
                }
            }
        }
        other => other,
    }
}
