//! TASK-137 physical proof for complete canonical source-free tables.
//!
//! No parser access or independently guessed source rows are permitted.

use super::*;
use crate::protocol::{FiniteColumnType, LiteralValue, ValueDomain};

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
    /// Complete physical input bag to this finite transformation, if any.
    pub source_rows: Vec<BTreeMap<String, LiteralValue>>,
    /// All original source-row identities; duplicates remain distinct.
    pub source_row_ids: Vec<String>,
    /// Explicit source-row witnesses for every terminal row, including fanout.
    pub contributors: Vec<Vec<String>>,
    /// Exact finite typed outcome domains, including NULL and bounds.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
    /// Attested column types, including zero-row results whose domains are Empty.
    pub terminal_column_types: BTreeMap<String, FiniteColumnType>,
    /// Independent complete physical source bags, with left/right scope names.
    pub physical_inputs: BTreeMap<String, Vec<BTreeMap<String, LiteralValue>>>,
    /// Source-row identities for every physical input, retaining duplicates.
    pub physical_input_ids: BTreeMap<String, Vec<String>>,
    /// Exhaustive TRUE pairs for finite joined source inputs.
    pub joined_selected: Vec<[String; 2]>,
    /// Exhaustive NOT TRUE pairs, including NULL-key UNKNOWN.
    pub joined_not_true: Vec<[String; 2]>,
    /// Explicit FALSE comparisons between two non-NULL unequal keys.
    pub joined_false: Vec<[String; 2]>,
    /// Explicit UNKNOWN comparisons with at least one SQL NULL key.
    pub joined_unknown: Vec<[String; 2]>,
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
        || query
            .write()
            .is_some_and(|write| !write.kind().fully_defines_relation())
        || query.set_operation().is_some()
    {
        return PhysicalFiniteTableProof::Residual {
            reason: "finite table has unproved dependency or query semantics".to_string(),
        };
    }
    let mut terminal = Vec::new();
    let mut row_ids = Vec::new();
    for (index, values) in table.rows().iter().enumerate() {
        let row = table
            .columns()
            .iter()
            .cloned()
            .zip(values.iter().map(|literal| literal.value().clone()))
            .collect::<BTreeMap<_, _>>();
        terminal.push(row);
        let position = if matches!(table.kind(), crate::protocol::FiniteTableKind::Values) {
            index
        } else {
            index + 1
        };
        row_ids.push(format!("{}:{position}", table.kind().as_str()));
    }
    if let Some(wanted) = wanted {
        // A malformed typed tuple does not prove impossibility. Text is exact
        // only for the explicitly typed UNPIVOT name column.
        if wanted.len() != table.columns().len()
            || table
                .columns()
                .iter()
                .zip(table.column_types())
                .any(|(name, ty)| match wanted.get(name) {
                    Some(LiteralValue::Null) => false,
                    Some(LiteralValue::Number(number)) if *ty == FiniteColumnType::Integer => {
                        number
                            .parse::<i128>()
                            .map_or(true, |parsed| parsed.to_string() != *number)
                    }
                    Some(LiteralValue::Text(_)) if *ty == FiniteColumnType::Text => false,
                    _ => true,
                })
        {
            return PhysicalFiniteTableProof::Residual {
                reason: "requested output tuple lacks exact typed and complete column identity"
                    .to_string(),
            };
        }
        if !terminal.iter().any(|row| row == wanted) {
            return PhysicalFiniteTableProof::Impossible {
                reason: "requested output tuple is absent from the complete finite table"
                    .to_string(),
            };
        }
    }
    let (source_rows, source_row_ids, contributors) = if let Some(source) = table.source() {
        let source_row_ids = source
            .rows()
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let index = if matches!(source.kind(), crate::protocol::FiniteTableKind::Values) {
                    index
                } else {
                    index + 1
                };
                format!("{}:{index}", source.kind().as_str())
            })
            .collect::<Vec<_>>();
        let source_rows = source
            .rows()
            .iter()
            .map(|row| {
                source
                    .columns()
                    .iter()
                    .cloned()
                    .zip(row.iter().map(|literal| literal.value().clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .collect::<Vec<_>>();
        let contributors = table
            .contributors()
            .iter()
            .map(|indices| {
                indices
                    .iter()
                    .map(|index| {
                        source_row_ids
                            .get(*index)
                            .cloned()
                            .expect("canonical with_source validates all contributor indices")
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        (source_rows, source_row_ids, contributors)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    // The joined producer is a canonical source, not an opportunity to
    // invent independent rows after the group/count has been evaluated.
    // Enumerate all physical input pairs exactly once from its frozen bags.
    let mut physical_inputs = BTreeMap::new();
    let mut physical_input_ids = BTreeMap::new();
    let mut joined_selected = Vec::new();
    let mut joined_not_true = Vec::new();
    let mut joined_false = Vec::new();
    let mut joined_unknown = Vec::new();
    let join = table
        .source()
        .filter(|source| matches!(source.kind(), crate::protocol::FiniteTableKind::Joined))
        .unwrap_or(table);
    if let Some((left, right)) = join.joined_sources() {
        for (label, input) in [("left", left), ("right", right)] {
            let ids = input
                .rows()
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    let position =
                        if matches!(input.kind(), crate::protocol::FiniteTableKind::Values) {
                            index
                        } else {
                            index + 1
                        };
                    format!("{label}/{}:{position}", input.kind().as_str())
                })
                .collect::<Vec<_>>();
            let bag = input
                .rows()
                .iter()
                .map(|row| {
                    input
                        .columns()
                        .iter()
                        .cloned()
                        .zip(row.iter().map(|value| value.value().clone()))
                        .collect::<BTreeMap<_, _>>()
                })
                .collect::<Vec<_>>();
            physical_input_ids.insert(label.to_string(), ids);
            physical_inputs.insert(label.to_string(), bag);
        }
        let Some(left_ids) = physical_input_ids.get("left") else {
            return PhysicalFiniteTableProof::Residual {
                reason: "left joined physical population is not attested".to_string(),
            };
        };
        let Some(right_ids) = physical_input_ids.get("right") else {
            return PhysicalFiniteTableProof::Residual {
                reason: "right joined physical population is not attested".to_string(),
            };
        };
        for (li, left_id) in left_ids.iter().enumerate() {
            for (ri, right_id) in right_ids.iter().enumerate() {
                let pair = [left_id.clone(), right_id.clone()];
                if join.joined_pairs().contains(&[li, ri]) {
                    joined_selected.push(pair);
                } else {
                    let nullable = left
                        .rows()
                        .get(li)
                        .and_then(|row| row.first())
                        .is_some_and(|value| value.value() == &LiteralValue::Null)
                        || right
                            .rows()
                            .get(ri)
                            .and_then(|row| row.first())
                            .is_some_and(|value| value.value() == &LiteralValue::Null);
                    if nullable {
                        joined_unknown.push(pair.clone());
                    } else {
                        joined_false.push(pair.clone());
                    }
                    joined_not_true.push(pair);
                }
            }
        }
    }
    let terminal_domains = query
        .output()
        .columns()
        .iter()
        .map(|column| (column.name().to_string(), column.domain().clone()))
        .collect();
    let terminal_column_types = table
        .columns()
        .iter()
        .cloned()
        .zip(table.column_types().iter().copied())
        .collect();
    PhysicalFiniteTableProof::Exact(Box::new(PhysicalFiniteTableExact {
        nodes: walker.nodes,
        row_ids,
        terminal,
        source_rows,
        source_row_ids,
        contributors,
        terminal_domains,
        terminal_column_types,
        physical_inputs,
        physical_input_ids,
        joined_selected,
        joined_not_true,
        joined_false,
        joined_unknown,
    }))
}
