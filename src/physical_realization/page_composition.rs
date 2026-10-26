//! Complete physical bags for two independently ordered CTE pagination stages.
//! Offset windows are evaluated at their producer boundary, never flattened.

use super::write_states::{physical_closed_world_schema_plan, PhysicalWriteStatePlan};
use super::*;
use crate::protocol::{LiteralValue, NestedPage, SetRowShaping, ValueDomain};
use std::collections::{BTreeMap, BTreeSet};

/// Physical identities and exact final bag from a two-stage page program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalComposedPageExact {
    /// Canonical dependency graph from the original SQL analysis.
    pub nodes: Vec<PhysicalPlanNode>,
    /// The authoritative physical input relation.
    pub source: String,
    /// Every physical row, including rows eliminated by either page.
    pub source_row_ids: Vec<String>,
    /// Final page contributors in terminal row order.
    pub selected_source_ids: Vec<String>,
    /// All other physical identities, including producer-page rejections.
    pub rejected_source_ids: Vec<String>,
    /// Complete terminal multiset after both ordered page boundaries.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact terminal NULL membership and finite value domains.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// No unsupported page or source behavior may silently become exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalComposedPageProof {
    /// Independently evaluable ordered source and complete terminal bag.
    Exact(Box<PhysicalComposedPageExact>),
    /// Source constraints or a requested terminal tuple are impossible.
    Impossible { reason: String },
    /// An unproved producer, ordering, type or source boundary.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalComposedPageProof {
    PhysicalComposedPageProof::Residual {
        reason: reason.into(),
    }
}

fn stage(
    shape: &SetRowShaping,
    page: &NestedPage,
    snapshot: &PhysicalWriteState,
    schema: &crate::relation::RelationSchema,
    indexes: &[usize],
) -> Result<Vec<usize>, String> {
    if shape.keys.is_empty() {
        return Err("page stage has no attested ordering keys".to_string());
    }
    let mut keyed = Vec::with_capacity(indexes.len());
    for &index in indexes {
        let row = snapshot
            .rows()
            .get(index)
            .ok_or("missing original source row")?;
        let mut values = Vec::with_capacity(shape.keys.len());
        for key in &shape.keys {
            let (_, source_column) = page
                .projected_columns()
                .get(key.index)
                .ok_or("ORDER BY references an unprojected CTE column")?;
            let declared = schema
                .columns()
                .iter()
                .find(|c| c.name() == source_column)
                .ok_or("page ORDER BY lacks a physical column declaration")?;
            if !super::aggregate_bags::signed_integer(declared.data_type()) {
                return Err("page ORDER BY lacks attested integer comparison".to_string());
            }
            let value = super::rank_bags::key_value(row, source_column)
                .map_err(|reason| reason.to_string())?;
            if value.is_none() && key.nulls_first.is_none() {
                return Err("NULL page key requires explicit NULLS FIRST/LAST".to_string());
            }
            values.push(value);
        }
        keyed.push((index, values));
    }
    keyed.sort_by(|a, b| super::page_bags::compare_ordered_rows(&a.1, &b.1, &shape.keys));
    if !shape.with_ties
        && keyed.windows(2).any(|pair| {
            super::page_bags::compare_ordered_rows(&pair[0].1, &pair[1].1, &shape.keys)
                == std::cmp::Ordering::Equal
        })
    {
        return Err("ambiguous page peer identities cannot be certified".to_string());
    }
    if shape.with_ties && shape.offset > 0 {
        return Err("OFFSET WITH TIES has no attested producer law".to_string());
    }
    let cutoff = if shape.with_ties && shape.limit > 0 {
        keyed.get(shape.limit - 1).map(|(_, keys)| keys.clone())
    } else {
        None
    };
    Ok(keyed
        .into_iter()
        .enumerate()
        .filter_map(|(position, (index, keys))| {
            let window = position >= shape.offset && position - shape.offset < shape.limit;
            let peer = cutoff.as_deref().is_some_and(|last| {
                super::page_bags::compare_ordered_rows(&keys, last, &shape.keys)
                    == std::cmp::Ordering::Equal
            });
            (window || peer).then_some(index)
        })
        .collect())
}

/// Prove both CTE pagination boundaries from the canonical nested page program.
///
/// Requires one complete source bag, schema-backed integer ordering and a
/// positional direct-column copy across the CTE boundary. Unproved set branches,
/// coercions, implicit NULL ordering and tied winners remain Residual.
pub fn physical_finite_composed_page_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalComposedPageProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalComposedPageProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let [snapshot] = snapshots else {
        return residual("composed page requires exactly one physical source");
    };
    let mut walker = Walker::new(bundle);
    if let Err(reason) = walker.visit(target_layer_id) {
        return residual(reason.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing composed-page consumer layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("composed-page target has no canonical query");
    };
    let Some(page) = query.nested_page() else {
        return residual("target has no certified bounded CTE producer and consumer");
    };
    if snapshot.relation() != page.source() {
        return residual("CTE source identity differs from physical snapshot");
    }
    let Some(schema) = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == page.source())
    else {
        return residual("composed page requires authoritative physical column types");
    };
    let mut names = BTreeSet::new();
    for (output, source) in page.projected_columns() {
        if !names.insert(output) || !schema.columns().iter().any(|c| c.name() == source) {
            return residual("composed page output is ambiguous or lacks a physical source column");
        }
    }
    if names.is_empty() {
        return residual("composed page has no output columns");
    }

    let all = (0..snapshot.rows().len()).collect::<Vec<_>>();
    let first = match stage(page.inner(), page, snapshot, schema, &all) {
        Ok(indices) => indices,
        Err(reason) => return residual(reason),
    };
    let second = match stage(page.outer(), page, snapshot, schema, &first) {
        Ok(indices) => indices,
        Err(reason) => return residual(reason),
    };
    let selected = second.iter().copied().collect::<BTreeSet<_>>();
    let source_row_ids = all
        .iter()
        .map(|i| format!("{}#{i}", page.source()))
        .collect();
    let selected_source_ids = second
        .iter()
        .map(|i| format!("{}#{i}", page.source()))
        .collect();
    let rejected_source_ids = all
        .iter()
        .filter(|i| !selected.contains(i))
        .map(|i| format!("{}#{i}", page.source()))
        .collect();
    let mut terminal = Vec::new();
    for index in second {
        let Some(row) = snapshot.rows().get(index) else {
            return residual("page output refers to a missing physical row");
        };
        let mut projected = BTreeMap::new();
        for (output_name, source_name) in page.projected_columns() {
            let Some(value) = row.values().get(source_name) else {
                return residual("page output column absent from source row");
            };
            projected.insert(output_name.clone(), value.clone());
        }
        terminal.push(projected);
    }
    let mut terminal_domains = BTreeMap::new();
    for (name, _) in page.projected_columns() {
        terminal_domains.insert(
            name.clone(),
            super::rank_bags::exact_domain(
                terminal.iter().filter_map(|row| row.get(name).cloned()),
            ),
        );
    }
    PhysicalComposedPageProof::Exact(Box::new(PhysicalComposedPageExact {
        nodes: walker.nodes,
        source: page.source().to_string(),
        source_row_ids,
        selected_source_ids,
        rejected_source_ids,
        terminal,
        terminal_domains,
    }))
}

/// Classify requested complete output tuples without fabricating extra rows.
pub fn physical_finite_composed_page_membership_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
    requested: &BTreeMap<String, LiteralValue>,
) -> PhysicalComposedPageProof {
    match physical_finite_composed_page_plan(bundle, target_layer_id, snapshots) {
        PhysicalComposedPageProof::Exact(exact)
            if !exact.terminal.iter().any(|row| row == requested) =>
        {
            PhysicalComposedPageProof::Impossible {
                reason: "requested output tuple is absent from complete nested page bag"
                    .to_string(),
            }
        }
        other => other,
    }
}
