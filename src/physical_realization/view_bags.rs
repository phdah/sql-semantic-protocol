//! Exact bounded logical VIEW evaluation from canonical source-local expressions.
//! A view has no independently insertable physical population. Its rows are
//! re-evaluated from complete caller-attested leaf rows for each terminal read.

use std::collections::BTreeMap;

use crate::bundle::AnalysisBundle;
use crate::protocol::{LiteralValue, ProtocolStatement, QueryStatement, WriteDefinitionMode};

use super::exists_producers::{complete_domain, PhysicalProducerDomain, PhysicalProducerRow};
use super::write_states::{
    evaluate_simple_select_with_source_ids, physical_closed_world_schema_plan, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan,
};

/// An exact view producer and terminal read, with original physical occurrence IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalViewExact {
    /// Name of the SQL logical view, never an independent physical source.
    pub view_name: String,
    /// Complete original physical source rows.
    pub source_bag: Vec<PhysicalProducerRow>,
    /// Re-evaluated view rows, retaining physical input occurrence identities.
    pub view_bag: Vec<PhysicalProducerRow>,
    /// Complete final SELECT output with inherited source identities.
    pub terminal: Vec<PhysicalProducerRow>,
    /// Exact typed terminal NULL and inclusive integer bounds.
    pub terminal_domains: BTreeMap<String, PhysicalProducerDomain>,
}

/// Bounded view proof strength.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalViewProof {
    /// Original physical input and view/read output fully proved.
    Exact(Box<PhysicalViewExact>),
    /// Requested tuple is absent from a complete terminal bag or schema contradiction.
    Impossible { reason: String },
    /// Unsupported query/view law or missing evidence cannot be proved.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalViewProof {
    PhysicalViewProof::Residual {
        reason: reason.into(),
    }
}

fn query_for<'a>(
    bundle: &'a AnalysisBundle,
    layer: &crate::bundle::TransformationLayer,
) -> Option<&'a QueryStatement> {
    bundle
        .inputs()
        .iter()
        .find(|input| input.id() == layer.input_id())
        .and_then(|input| input.statements().get(layer.statement_index()))
        .and_then(|statement| match statement {
            ProtocolStatement::Query(query) => Some(query),
            ProtocolStatement::Unsupported(_) => None,
        })
}

fn domain_membership(
    terminal: &[PhysicalProducerRow],
    domains: &BTreeMap<String, PhysicalProducerDomain>,
    wanted: &BTreeMap<String, LiteralValue>,
) -> Result<bool, &'static str> {
    if wanted.len() != domains.len() || wanted.keys().any(|column| !domains.contains_key(column)) {
        return Err("requested view tuple differs from canonical output columns");
    }
    let mut normalized = BTreeMap::new();
    for (name, value) in wanted {
        let scalar = match value {
            LiteralValue::Null => None,
            LiteralValue::Number(raw) => Some(
                raw.parse::<i128>()
                    .map_err(|_| "requested view numeric value is not a signed integer")?,
            ),
            _ => return Err("requested view tuple is not a typed finite integer"),
        };
        normalized.insert(name, scalar);
    }
    Ok(terminal.iter().any(|row| {
        normalized
            .iter()
            .all(|(column, wanted)| match (row.values.get(*column), wanted) {
                (Some(LiteralValue::Null), None) => true,
                (Some(LiteralValue::Number(actual)), Some(wanted)) => {
                    actual.parse::<i128>().ok() == Some(*wanted)
                }
                _ => false,
            })
    }))
}

/// Prove one ordinary CREATE VIEW and its subsequent row-local SELECT.
///
/// The only independent input is a complete typed physical leaf. Logical VIEW
/// rows are evaluated from canonical normalized projection/filter expressions
/// at read time; no independent view snapshot is accepted. More than one view,
/// REPLACE, DML, joins, aggregation, session modifiers or unsupported expressions
/// remain Residual rather than treating a logical view as a writable table.
pub fn physical_finite_view_read_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    initial_states: &[PhysicalWriteState],
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalViewProof {
    let [definition, read] = bundle.layers() else {
        return residual("logical view proof requires exactly one definition and one read");
    };
    if read.id() != target_layer_id || read.write_kind().is_some() {
        return residual("logical view read must be the terminal SELECT");
    }
    let Some(definition_query) = query_for(bundle, definition) else {
        return residual("view definition has no canonical query");
    };
    let Some(view) = definition_query.write() else {
        return residual("missing canonical CREATE VIEW definition");
    };
    if view.view_execution_unknown() {
        return residual("view execution requires unproved materialization or session laws");
    }
    if view.definition_mode() != Some(WriteDefinitionMode::CreateView) {
        return residual("view replacement or nonlogical materialization is not certified");
    }
    let Some(read_query) = query_for(bundle, read) else {
        return residual("view consumer has no canonical SELECT");
    };
    if read_query.write().is_some() || initial_states.len() != 1 {
        return residual("view proof requires exactly one controlled physical leaf");
    }
    let [source] = definition_query.sources() else {
        return residual("view definition is not a single-source SELECT");
    };
    let [consumer] = read_query.sources() else {
        return residual("view reader is not a single-source SELECT");
    };
    if source.name() == view.target()
        || consumer.name() != view.target()
        || initial_states[0].relation() != source.name()
    {
        return residual("logical view/source identity does not match canonical binding");
    }
    match physical_closed_world_schema_plan(bundle, initial_states) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalViewProof::Impossible { reason }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }

    let source_name = source.name();
    let source_bag = initial_states[0]
        .rows()
        .iter()
        .enumerate()
        .map(|(ordinal, row)| PhysicalProducerRow {
            values: row.values().clone(),
            source_row_ids: vec![format!("{source_name}#{ordinal}")],
        })
        .collect::<Vec<_>>();
    let source_states = BTreeMap::from([(source_name.to_string(), initial_states[0].clone())]);
    let view_results =
        match evaluate_simple_select_with_source_ids(bundle, definition_query, &source_states) {
            Ok(rows) => rows,
            Err(error) => return residual(format!("unproved view producer: {error:?}")),
        };
    let column_names = definition_query
        .output()
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let mut view_bag = Vec::new();
    let mut view_rows = Vec::new();
    for (ordinal, values) in view_results {
        if values.len() != column_names.len() {
            return residual("view producer projection arity is unproved");
        }
        // Ordinary portable integer computed expressions may not borrow a
        // wider view declaration to make an overflowing source computation exact.
        if values.iter().any(|value| match value {
            LiteralValue::Null => false,
            LiteralValue::Number(raw) => raw.parse::<i32>().is_err(),
            _ => true,
        }) {
            return residual("view output requires signed-32-bit portable arithmetic proof");
        }
        let row: BTreeMap<_, _> = column_names.iter().cloned().zip(values).collect();
        let Some(original) = source_bag.get(ordinal) else {
            return residual("view row has no source occurrence");
        };
        view_bag.push(PhysicalProducerRow {
            values: row.clone(),
            source_row_ids: original.source_row_ids.clone(),
        });
        view_rows.push(PhysicalWriteRow::new(row).expect("unique canonical projection columns"));
    }
    let view_state = match PhysicalWriteState::new(view.target().to_string(), view_rows) {
        Some(state) => state,
        None => return residual("invalid view result population"),
    };
    let view_states = BTreeMap::from([(view.target().to_string(), view_state)]);
    let rows = match evaluate_simple_select_with_source_ids(bundle, read_query, &view_states) {
        Ok(rows) => rows,
        Err(error) => return residual(format!("unproved view consumer: {error:?}")),
    };
    let output_columns = read_query
        .output()
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>();
    let mut terminal = Vec::new();
    for (ordinal, values) in rows {
        if values.len() != output_columns.len() {
            return residual("view terminal projection arity is unproved");
        }
        let Some(producer) = view_bag.get(ordinal) else {
            return residual("view consumer has no producer occurrence");
        };
        terminal.push(PhysicalProducerRow {
            values: output_columns.iter().cloned().zip(values).collect(),
            source_row_ids: producer.source_row_ids.clone(),
        });
    }
    let mut terminal_domains = BTreeMap::new();
    for column in &output_columns {
        let Some(domain) = complete_domain(&terminal, column) else {
            return residual("view terminal lacks complete typed value domains");
        };
        terminal_domains.insert(column.clone(), domain);
    }
    if let Some(wanted) = wanted {
        match domain_membership(&terminal, &terminal_domains, wanted) {
            Ok(true) => {}
            Ok(false) => {
                return PhysicalViewProof::Impossible {
                    reason: "requested row absent from exact logical view output".to_string(),
                }
            }
            Err(reason) => return residual(reason),
        }
    }
    PhysicalViewProof::Exact(Box::new(PhysicalViewExact {
        view_name: view.target().to_string(),
        source_bag,
        view_bag,
        terminal,
        terminal_domains,
    }))
}
