//! TASK-137 physical proof for complete canonical source-free tables.
//!
//! No parser access or independently guessed source rows are permitted.

use super::*;
use crate::protocol::{LiteralValue, ValueDomain};

/// Exact output of a finite literal table. Producer identities are stable and
/// duplicates remain separate even though SQL result ordering is unspecified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteTableExact {
    /// Canonical source-free layer graph.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Identity of each positionally constructed row.
    pub row_ids: Vec<String>,
    /// Complete terminal SQL output bag with duplicates.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Exact finite typed outcome domains, including NULL and bounds.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// Sound classification of complete literal rows and optional row membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteTableProof {
    /// Complete source-free output and producer identity bag.
    Exact(Box<PhysicalFiniteTableExact>),
    /// Requested row is absent from a fully constructed output bag.
    Impossible { reason: String },
    /// No complete typed table-law evidence has been certified.
    Residual { reason: String },
}

/// Prove the exact source-free finite output. If `wanted` is supplied, prove
/// or disprove its *membership* in the complete terminal bag.
///
/// This is not a general table-source solver: column-based array expansions,
/// lateral fanout, opaque generators, unproved element types, session-dependent
/// coercions and composed producers remain Residual. An absent row in a fully
/// attested table is Impossible, not merely an unsupported direction.
pub fn physical_finite_table_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalFiniteTableProof {
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return PhysicalFiniteTableProof::Residual {
            reason: gap.as_str().to_string(),
        };
    }
    if !walker.sources.is_empty() {
        return PhysicalFiniteTableProof::Residual {
            reason: "finite literal table cannot depend on a physical source".to_string(),
        };
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return PhysicalFiniteTableProof::Residual {
            reason: "target layer is missing".to_string(),
        };
    };
    let Some(query) = query_for(bundle, layer) else {
        return PhysicalFiniteTableProof::Residual {
            reason: "target has no canonical query".to_string(),
        };
    };
    let Some(table) = query.finite_table() else {
        return PhysicalFiniteTableProof::Residual {
            reason: "target has no certified finite source-free table".to_string(),
        };
    };
    if !query.dependencies().is_empty()
        || !query.sources().is_empty()
        || !query.diagnostics().is_empty()
        || !query.condition_exactness().is_exact()
        || query.write().is_some()
        || query.set_operation().is_some()
    {
        return PhysicalFiniteTableProof::Residual {
            reason: "finite table has unproved dependency or query semantics".to_string(),
        };
    }
    let mut terminal = Vec::new();
    let mut row_ids = Vec::new();
    for (index, values) in table.rows().iter().enumerate() {
        let row = table.columns().iter().cloned().zip(
            values.iter().map(|literal| literal.value().clone())
        ).collect::<BTreeMap<_, _>>();
        terminal.push(row);
        row_ids.push(format!("{}:{index}", table.kind().as_str()));
    }
    if let Some(wanted) = wanted {
        if !terminal.iter().any(|row| row == wanted) {
            return PhysicalFiniteTableProof::Impossible {
                reason: "requested output tuple is absent from the complete finite table".to_string(),
            };
        }
    }
    let terminal_domains = query.output().columns().iter().map(|column| {
        (column.name().to_string(), column.domain().clone())
    }).collect();
    PhysicalFiniteTableProof::Exact(Box::new(PhysicalFiniteTableExact {
        nodes: walker.nodes,
        row_ids,
        terminal,
        terminal_domains,
    }))
}
