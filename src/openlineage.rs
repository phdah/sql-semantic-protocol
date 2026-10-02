//! OpenLineage export adapter for lineage that the SQL Semantic Protocol can represent safely.
//!
//! The SQL Semantic Protocol remains the authoritative model. This adapter emits OpenLineage
//! DatasetEvents only for named datasets whose composed semantics resolved successfully. Richer
//! protocol semantics such as predicates and value domains intentionally remain in the protocol.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{json, Map, Value};

use crate::{AnalysisBundle, ComposedSemantics};

const PRODUCER: &str = "https://github.com/phdah/sql-semantic-protocol";
const DATASET_EVENT_SCHEMA: &str =
    "https://openlineage.io/spec/2-0-2/OpenLineage.json#/$defs/DatasetEvent";
const LINEAGE_FACET_SCHEMA: &str =
    "https://openlineage.io/spec/facets/1-0-0/LineageFacet.json#/$defs/LineageDatasetFacet";

/// Error returned when a protocol bundle cannot be exported to deterministic OpenLineage JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenLineageExportError {
    /// Dataset namespace is required because SQL alone cannot infer an OpenLineage namespace.
    EmptyNamespace,
    /// DatasetEvent requires a caller-supplied event timestamp.
    EmptyEventTime,
    /// OpenLineage field lineage is keyed by output field name, so duplicates are ambiguous.
    DuplicateOutputField {
        /// Named output dataset containing the duplicate field.
        dataset: String,
        /// Duplicate output field name.
        field: String,
    },
}

impl fmt::Display for OpenLineageExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyNamespace => write!(formatter, "OpenLineage export requires a namespace"),
            Self::EmptyEventTime => write!(formatter, "OpenLineage export requires an event time"),
            Self::DuplicateOutputField { dataset, field } => write!(
                formatter,
                "OpenLineage field lineage for dataset '{dataset}' is ambiguous because field '{field}' is duplicated"
            ),
        }
    }
}

impl std::error::Error for OpenLineageExportError {}

/// Export resolved named datasets as OpenLineage DatasetEvents.
///
/// The namespace is applied to every relation because plain SQL does not carry OpenLineage
/// namespace identity. The event time must be supplied by the caller so this deterministic analyzer
/// does not read the clock. The function does not validate timestamp syntax.
///
/// Anonymous outputs and unresolved named layers are intentionally omitted because they cannot be
/// mapped to trustworthy OpenLineage dataset lineage. Callers should inspect the original
/// AnalysisBundle for those richer or unresolved semantics.
pub fn to_openlineage_json(
    bundle: &AnalysisBundle,
    namespace: &str,
    event_time: &str,
) -> Result<String, OpenLineageExportError> {
    if namespace.trim().is_empty() {
        return Err(OpenLineageExportError::EmptyNamespace);
    }
    if event_time.trim().is_empty() {
        return Err(OpenLineageExportError::EmptyEventTime);
    }

    let mut events = Vec::new();

    for layer in bundle.layers() {
        let Some(dataset_name) = layer.produces().iter().find_map(|dataset| dataset.relation_name())
        else {
            continue;
        };
        let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
            continue;
        };

        let mut fields = Map::new();
        let mut field_names = BTreeSet::new();

        for column in semantics.output().columns() {
            if !field_names.insert(column.name()) {
                return Err(OpenLineageExportError::DuplicateOutputField {
                    dataset: dataset_name.to_string(),
                    field: column.name().to_string(),
                });
            }

            if column.lineage().is_empty() {
                continue;
            }

            let inputs = column
                .lineage()
                .iter()
                .map(|source| {
                    json!({
                        "namespace": namespace,
                        "name": source.relation(),
                        "type": "DATASET",
                        "field": source.column()
                    })
                })
                .collect::<Vec<_>>();

            fields.insert(column.name().to_string(), json!({ "inputs": inputs }));
        }

        let inputs = semantics
            .dependencies()
            .iter()
            .map(|relation| {
                json!({
                    "namespace": namespace,
                    "name": relation,
                    "type": "DATASET"
                })
            })
            .collect::<Vec<_>>();

        let mut facets = Map::new();
        if !inputs.is_empty() || !fields.is_empty() {
            facets.insert(
                "lineage".to_string(),
                json!({
                    "_producer": PRODUCER,
                    "_schemaURL": LINEAGE_FACET_SCHEMA,
                    "inputs": inputs,
                    "fields": fields
                }),
            );
        }

        events.push(json!({
            "eventTime": event_time,
            "producer": PRODUCER,
            "schemaURL": DATASET_EVENT_SCHEMA,
            "dataset": {
                "namespace": namespace,
                "name": dataset_name,
                "facets": facets
            }
        }));
    }

    Ok(Value::Array(events).to_string())
}
