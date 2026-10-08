//! Open Data Contract Standard v3.2 metadata adapter.
//!
//! ODCS remains an evidence source. Parsed contracts are normalized into the same canonical
//! relation schemas and constraints used by SQL and dbt before they reach protocol emission.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use saphyr::{LoadableYamlNode, Yaml};
use serde_json::{Map, Number, Value};

use crate::constraints::{
    merge_relation_constraint_sets, ConstraintEnforcement, ConstraintEvidence,
    ConstraintMetadataError, ConstraintProvenance, ConstraintSourceKind, ConstraintValue,
    RelationConstraint, RelationConstraintSet,
};
use crate::data_type::{parse_data_type, DataType};
use crate::relation::{
    RelationCatalog, RelationResolutionError, RelationSchema, SchemaColumn, SchemaSourceKind,
};
use crate::AnalysisBundle;

/// ODCS API version accepted by this adapter.
pub const SUPPORTED_ODCS_API_VERSION: &str = "v3.2.0";

/// One explicitly supplied ODCS document.
///
/// Source is the stable identity used to resolve external contract references. It may be a file
/// name, repository-relative path, or URL, but matching is exact and the adapter never fetches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdcsDocument {
    source: String,
    yaml: String,
}

impl OdcsDocument {
    /// Construct an ODCS document with a non-empty source identity.
    pub fn new(source: impl Into<String>, yaml: impl Into<String>) -> Result<Self, OdcsError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(OdcsError::InvalidDocument {
                source,
                path: "$".to_string(),
                message: "document source cannot be empty".to_string(),
            });
        }
        Ok(Self {
            source,
            yaml: yaml.into(),
        })
    }

    /// Return the stable source identity used for cross-contract references.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Return the original YAML text.
    pub fn yaml(&self) -> &str {
        &self.yaml
    }
}

/// Non-fatal ODCS evidence issue preserved by the adapter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OdcsDiagnostic {
    source: String,
    code: String,
    message: String,
}

impl OdcsDiagnostic {
    fn new(source: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            code: code.into(),
            message: message.into(),
        }
    }

    /// Return the document source that supplied the evidence.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Return the stable diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Return the human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Canonical schema and constraint evidence derived from one or more ODCS contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdcsMetadata {
    source_schemas: Vec<RelationSchema>,
    relation_constraints: Vec<RelationConstraintSet>,
    diagnostics: Vec<OdcsDiagnostic>,
}

impl OdcsMetadata {
    /// Return canonical relation schemas supplied by ODCS.
    pub fn source_schemas(&self) -> &[RelationSchema] {
        &self.source_schemas
    }

    /// Return canonical relation constraints supplied by ODCS.
    pub fn relation_constraints(&self) -> &[RelationConstraintSet] {
        &self.relation_constraints
    }

    /// Return non-fatal adapter diagnostics in deterministic order.
    pub fn diagnostics(&self) -> &[OdcsDiagnostic] {
        &self.diagnostics
    }

    /// Merge ODCS evidence into an existing analysis bundle.
    ///
    /// Existing typed schema evidence has higher authority. A datatype disagreement is returned
    /// explicitly rather than silently overridden. Constraint merging uses the protocol-owned
    /// conflict semantics shared with SQL and dbt.
    pub fn enrich_bundle(&self, bundle: &mut AnalysisBundle) -> Result<(), OdcsError> {
        let merged_schemas = merge_source_schemas(bundle.source_schemas(), &self.source_schemas)?;
        bundle.replace_source_schemas(merged_schemas);
        bundle.enrich_relation_constraints(&self.relation_constraints);
        Ok(())
    }
}

/// Error returned when ODCS metadata cannot be translated without guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OdcsError {
    /// YAML parsing failed.
    InvalidYaml {
        /// Source document identity.
        source: String,
        /// YAML parser error text.
        message: String,
    },
    /// The YAML document is structurally invalid for the supported adapter surface.
    InvalidDocument {
        /// Source document identity.
        source: String,
        /// JSONPath-like location.
        path: String,
        /// Explanation of the invalid value.
        message: String,
    },
    /// The contract API version is valid but unsupported.
    UnsupportedApiVersion {
        /// Source document identity.
        source: String,
        /// Version declared by the contract.
        version: String,
    },
    /// A relation could not be resolved safely against the supplied catalog.
    RelationResolution {
        /// Source document identity.
        source: String,
        /// Relation identifier from ODCS.
        relation: String,
        /// Resolution failure.
        error: RelationResolutionError,
    },
    /// Catalog metadata was supplied but no known relation matched an ODCS schema identity.
    UnresolvedRelation {
        /// Source document identity.
        source: String,
        /// Relation identifier from ODCS.
        relation: String,
    },
    /// A cross-contract relationship names a document the caller did not supply.
    MissingReferencedContract {
        /// Source document identity containing the relationship.
        source: String,
        /// Referenced external source identity.
        referenced_source: String,
    },
    /// A relationship reference is syntactically valid text but outside the supported direct
    /// schema/property reference surface.
    UnsupportedReference {
        /// Source document identity containing the relationship.
        source: String,
        /// Original reference.
        reference: String,
        /// Explanation of the unsupported reference shape.
        message: String,
    },
    /// Canonical constraint construction failed.
    ConstraintMetadata {
        /// Source document identity.
        source: String,
        /// Underlying canonical metadata failure.
        message: String,
    },
    /// Higher-authority schema evidence and ODCS disagree on a datatype.
    DatatypeConflict {
        /// Canonical relation identity.
        relation: String,
        /// Column name.
        column: String,
        /// Higher-authority datatype.
        existing: Box<DataType>,
        /// ODCS datatype.
        odcs: Box<DataType>,
    },
}

impl fmt::Display for OdcsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidYaml { source, message } => {
                write!(formatter, "invalid ODCS YAML from '{source}': {message}")
            }
            Self::InvalidDocument {
                source,
                path,
                message,
            } => write!(
                formatter,
                "invalid ODCS document '{source}' at '{path}': {message}"
            ),
            Self::UnsupportedApiVersion { source, version } => write!(
                formatter,
                "unsupported ODCS API version '{version}' in '{source}'; supported version is {SUPPORTED_ODCS_API_VERSION}"
            ),
            Self::RelationResolution {
                source,
                relation,
                error,
            } => write!(
                formatter,
                "cannot resolve ODCS relation '{relation}' from '{source}': {error}"
            ),
            Self::UnresolvedRelation { source, relation } => write!(
                formatter,
                "ODCS relation '{relation}' from '{source}' does not match any supplied catalog relation"
            ),
            Self::MissingReferencedContract {
                source,
                referenced_source,
            } => write!(
                formatter,
                "ODCS relationship in '{source}' references unsupplied contract '{referenced_source}'"
            ),
            Self::UnsupportedReference {
                source,
                reference,
                message,
            } => write!(
                formatter,
                "unsupported ODCS reference '{reference}' in '{source}': {message}"
            ),
            Self::ConstraintMetadata { source, message } => write!(
                formatter,
                "invalid canonical constraint metadata from ODCS document '{source}': {message}"
            ),
            Self::DatatypeConflict {
                relation,
                column,
                existing,
                odcs,
            } => write!(
                formatter,
                "datatype conflict for '{relation}.{column}': existing evidence is {existing:?}, ODCS evidence is {odcs:?}"
            ),
        }
    }
}

impl std::error::Error for OdcsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RelationResolution { error, .. } => Some(error),
            Self::InvalidYaml { .. }
            | Self::InvalidDocument { .. }
            | Self::UnsupportedApiVersion { .. }
            | Self::UnresolvedRelation { .. }
            | Self::MissingReferencedContract { .. }
            | Self::UnsupportedReference { .. }
            | Self::ConstraintMetadata { .. }
            | Self::DatatypeConflict { .. } => None,
        }
    }
}

/// Parse one inline ODCS v3.2 YAML contract.
///
/// External cross-contract relationship references fail unless the caller uses
/// parse_odcs_documents and supplies every referenced contract explicitly.
pub fn parse_odcs_yaml(
    yaml: &str,
    dialect_name: &str,
    catalog: &RelationCatalog,
) -> Result<OdcsMetadata, OdcsError> {
    let document = OdcsDocument::new("<inline>", yaml)?;
    parse_odcs_documents(&[document], dialect_name, catalog)
}

/// Parse one or more explicitly supplied ODCS v3.2 YAML contracts.
///
/// The first document is the primary contract whose metadata is returned. Additional documents
/// participate in deterministic relationship resolution and also contribute their own canonical
/// schema and constraint evidence.
pub fn parse_odcs_documents(
    documents: &[OdcsDocument],
    dialect_name: &str,
    catalog: &RelationCatalog,
) -> Result<OdcsMetadata, OdcsError> {
    if documents.is_empty() {
        return Err(OdcsError::InvalidDocument {
            source: "<documents>".to_string(),
            path: "$".to_string(),
            message: "at least one ODCS document is required".to_string(),
        });
    }

    let mut diagnostics = Vec::new();
    let mut parsed = BTreeMap::new();
    for document in documents {
        if parsed.contains_key(document.source()) {
            return Err(OdcsError::InvalidDocument {
                source: document.source().to_string(),
                path: "$".to_string(),
                message: "document source identity is duplicated".to_string(),
            });
        }
        let contract = parse_contract(document, dialect_name, catalog, &mut diagnostics)?;
        parsed.insert(document.source().to_string(), contract);
    }

    let mut source_schemas = Vec::new();
    let mut relation_constraints = Vec::new();

    for contract in parsed.values() {
        for schema in &contract.schemas {
            if let Some(relation_schema) = &schema.relation_schema {
                source_schemas.push(relation_schema.clone());
            }

            let mut constraints = property_constraints(contract, schema)?;
            constraints.extend(relationship_constraints(contract, schema, &parsed)?);
            if !constraints.is_empty() {
                let set = RelationConstraintSet::new(schema.relation.clone(), constraints)
                    .map_err(|error| constraint_error(&contract.source, error))?;
                merge_relation_constraint_sets(&mut relation_constraints, &[set]);
            }
        }
    }

    source_schemas.sort_by(|left, right| left.relation().cmp(right.relation()));
    for pair in source_schemas.windows(2) {
        if pair[0].relation() == pair[1].relation() && pair[0] != pair[1] {
            return Err(OdcsError::InvalidDocument {
                source: "<documents>".to_string(),
                path: "$.schema".to_string(),
                message: format!(
                    "multiple ODCS schemas resolve to relation '{}' with different typed schemas",
                    pair[0].relation()
                ),
            });
        }
    }
    source_schemas.dedup();

    diagnostics.sort();
    diagnostics.dedup();

    Ok(OdcsMetadata {
        source_schemas,
        relation_constraints,
        diagnostics,
    })
}

#[derive(Debug, Clone)]
struct ParsedContract {
    source: String,
    contract_id: String,
    schemas: Vec<ParsedSchema>,
}

#[derive(Debug, Clone)]
struct ParsedSchema {
    id: Option<String>,
    name: String,
    relation: String,
    properties: Vec<ParsedProperty>,
    relationships: Vec<RawRelationship>,
    relation_schema: Option<RelationSchema>,
}

#[derive(Debug, Clone)]
struct ParsedProperty {
    id: Option<String>,
    name: String,
    column: String,
    required: bool,
    primary_key: bool,
    primary_key_position: Option<u64>,
    unique: bool,
    enum_values: Option<Vec<ConstraintValue>>,
    relationships: Vec<RawRelationship>,
}

#[derive(Debug, Clone)]
struct RawRelationship {
    id: Option<String>,
    from: Option<Vec<String>>,
    to: Vec<String>,
    path: String,
}

fn parse_contract(
    document: &OdcsDocument,
    dialect_name: &str,
    catalog: &RelationCatalog,
    diagnostics: &mut Vec<OdcsDiagnostic>,
) -> Result<ParsedContract, OdcsError> {
    let value = parse_yaml(document)?;
    let root = object(&value, document.source(), "$")?;

    let api_version = required_string(root, document.source(), "$.apiVersion", "apiVersion")?;
    if api_version != SUPPORTED_ODCS_API_VERSION {
        return Err(OdcsError::UnsupportedApiVersion {
            source: document.source().to_string(),
            version: api_version.to_string(),
        });
    }

    let kind = required_string(root, document.source(), "$.kind", "kind")?;
    if kind != "DataContract" {
        return Err(invalid(
            document.source(),
            "$.kind",
            format!("expected 'DataContract', found '{kind}'"),
        ));
    }

    let contract_id = required_string(root, document.source(), "$.id", "id")?.to_string();
    let _contract_version = required_string(root, document.source(), "$.version", "version")?;
    let schemas = root
        .get("schema")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(document.source(), "$.schema", "expected a schema array"))?;

    let mut parsed_schemas = Vec::with_capacity(schemas.len());
    for (index, value) in schemas.iter().enumerate() {
        parsed_schemas.push(parse_schema(
            document.source(),
            value,
            index,
            dialect_name,
            catalog,
            diagnostics,
        )?);
    }

    validate_schema_identities(document.source(), &parsed_schemas)?;

    Ok(ParsedContract {
        source: document.source().to_string(),
        contract_id,
        schemas: parsed_schemas,
    })
}

fn parse_schema(
    source: &str,
    value: &Value,
    index: usize,
    dialect_name: &str,
    catalog: &RelationCatalog,
    diagnostics: &mut Vec<OdcsDiagnostic>,
) -> Result<ParsedSchema, OdcsError> {
    let path = format!("$.schema[{index}]");
    let schema = object(value, source, &path)?;
    let id = optional_string(schema, source, &format!("{path}.id"), "id")?.map(str::to_string);
    let name = required_string(schema, source, &format!("{path}.name"), "name")?.to_string();
    let physical_name = optional_string(
        schema,
        source,
        &format!("{path}.physicalName"),
        "physicalName",
    )?;
    let relation_candidate = physical_name.unwrap_or(&name);
    let relation = resolve_odcs_relation(source, relation_candidate, dialect_name, catalog)?;

    let properties = schema
        .get("properties")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(source, format!("{path}.properties"), "expected an array"))?;

    let mut parsed_properties = Vec::with_capacity(properties.len());
    let mut columns = Vec::with_capacity(properties.len());
    let mut complete_types = true;

    for (property_index, property_value) in properties.iter().enumerate() {
        let parsed = parse_property(
            source,
            &path,
            &name,
            property_value,
            property_index,
            dialect_name,
            diagnostics,
        )?;
        if let Some(data_type) = parsed.1 {
            let mut column = SchemaColumn::new(parsed.0.column.clone(), data_type)
                .map_err(|error| invalid(source, format!("{path}.properties[{property_index}]"), error.to_string()))?;
            let zone = property_value.as_object()
                .and_then(|property| property.get("physicalType"))
                .and_then(Value::as_str)
                .and_then(crate::relation::TimestampZone::from_sql_type);
            if let Some(zone) = zone {
                if matches!(column.data_type(), DataType::Timestamp { .. }) {
                    column = column.with_timestamp_zone(zone)
                        .map_err(|error| invalid(source, format!("{path}.properties[{property_index}]"), error.to_string()))?;
                }
            }
            columns.push(column);
        } else {
            complete_types = false;
        }
        parsed_properties.push(parsed.0);
    }

    validate_property_identities(source, &path, &parsed_properties)?;

    let relation_schema = if complete_types {
        Some(
            RelationSchema::new(relation.clone(), columns)
                .map_err(|error| invalid(source, &path, error.to_string()))?
                .with_source_kind(SchemaSourceKind::ExternalMetadata),
        )
    } else {
        diagnostics.push(OdcsDiagnostic::new(
            source,
            "odcs_incomplete_source_schema",
            format!(
                "ODCS schema '{name}' has at least one property without a safely normalizable datatype; typed source schema emission is omitted for relation '{relation}'"
            ),
        ));
        None
    };

    let relationships = parse_relationship_array(
        schema.get("relationships"),
        source,
        &format!("{path}.relationships"),
        None,
    )?;

    Ok(ParsedSchema {
        id,
        name,
        relation,
        properties: parsed_properties,
        relationships,
        relation_schema,
    })
}

fn parse_property(
    source: &str,
    schema_path: &str,
    schema_name: &str,
    value: &Value,
    index: usize,
    dialect_name: &str,
    diagnostics: &mut Vec<OdcsDiagnostic>,
) -> Result<(ParsedProperty, Option<DataType>), OdcsError> {
    let path = format!("{schema_path}.properties[{index}]");
    let property = object(value, source, &path)?;
    let id = optional_string(property, source, &format!("{path}.id"), "id")?.map(str::to_string);
    let name = required_string(property, source, &format!("{path}.name"), "name")?.to_string();
    let physical_name = optional_string(
        property,
        source,
        &format!("{path}.physicalName"),
        "physicalName",
    )?;
    let column = physical_name.unwrap_or(&name).to_string();

    let data_type = property_data_type(source, &path, property, dialect_name, diagnostics)?;
    let required =
        optional_bool(property, source, &format!("{path}.required"), "required")?.unwrap_or(false);
    let primary_key = optional_bool(
        property,
        source,
        &format!("{path}.primaryKey"),
        "primaryKey",
    )?
    .unwrap_or(false);
    let primary_key_position = optional_u64(
        property,
        source,
        &format!("{path}.primaryKeyPosition"),
        "primaryKeyPosition",
    )?;
    let unique =
        optional_bool(property, source, &format!("{path}.unique"), "unique")?.unwrap_or(false);
    let enum_values = parse_enum(property.get("enum"), source, &format!("{path}.enum"))?;
    let implicit_from = format!("{schema_name}.{name}");
    let relationships = parse_relationship_array(
        property.get("relationships"),
        source,
        &format!("{path}.relationships"),
        Some(&implicit_from),
    )?;

    Ok((
        ParsedProperty {
            id,
            name,
            column,
            required,
            primary_key,
            primary_key_position,
            unique,
            enum_values,
            relationships,
        },
        data_type,
    ))
}

fn property_data_type(
    source: &str,
    path: &str,
    property: &Map<String, Value>,
    dialect_name: &str,
    diagnostics: &mut Vec<OdcsDiagnostic>,
) -> Result<Option<DataType>, OdcsError> {
    let physical = optional_string(
        property,
        source,
        &format!("{path}.physicalType"),
        "physicalType",
    )?;
    let logical = optional_string(
        property,
        source,
        &format!("{path}.logicalType"),
        "logicalType",
    )?;

    let logical_type = logical
        .map(|logical| logical_data_type(source, &format!("{path}.logicalType"), logical))
        .transpose()?;

    if let Some(physical) = physical {
        match parse_data_type(physical, dialect_name) {
            Ok(physical_type) => {
                if let Some(logical_type) = &logical_type {
                    if logical_type != &physical_type {
                        diagnostics.push(OdcsDiagnostic::new(
                            source,
                            "odcs_data_type_disagreement",
                            format!(
                                "{path} declares physicalType '{physical}' and logicalType '{}' with different canonical meanings; physicalType is retained",
                                logical.unwrap_or_default()
                            ),
                        ));
                    }
                }
                return Ok(Some(physical_type));
            }
            Err(error) => {
                if let Some(logical_type) = logical_type {
                    diagnostics.push(OdcsDiagnostic::new(
                        source,
                        "odcs_physical_type_fallback",
                        format!(
                            "{path}.physicalType '{physical}' could not be normalized ({error}); logicalType is used as lower-authority evidence"
                        ),
                    ));
                    return Ok(Some(logical_type));
                }
                diagnostics.push(OdcsDiagnostic::new(
                    source,
                    "odcs_unresolved_data_type",
                    format!(
                        "{path}.physicalType '{physical}' could not be normalized and no logicalType fallback is available: {error}"
                    ),
                ));
                return Ok(None);
            }
        }
    }

    if logical_type.is_none() {
        diagnostics.push(OdcsDiagnostic::new(
            source,
            "odcs_unresolved_data_type",
            format!("{path} declares neither physicalType nor logicalType"),
        ));
    }
    Ok(logical_type)
}

fn logical_data_type(source: &str, path: &str, logical: &str) -> Result<DataType, OdcsError> {
    match logical.to_ascii_lowercase().as_str() {
        "string" => Ok(DataType::String {
            length: None,
            fixed: false,
        }),
        "date" => Ok(DataType::Date),
        "timestamp" => Ok(DataType::Timestamp { precision: None }),
        "time" => Ok(DataType::Time { precision: None }),
        "number" => Ok(DataType::Decimal {
            precision: None,
            scale: None,
        }),
        "integer" => Ok(DataType::SignedInteger { bits: None }),
        "boolean" => Ok(DataType::Boolean),
        "object" => Ok(DataType::Json),
        "array" => Ok(DataType::Array {
            element: None,
            length: None,
        }),
        "map" => Ok(DataType::Map {
            key: Box::new(DataType::Any),
            value: Box::new(DataType::Any),
        }),
        "vector" => Ok(DataType::Array {
            element: Some(Box::new(DataType::FloatingPoint { bits: None })),
            length: None,
        }),
        other => Err(invalid(
            source,
            path,
            format!("unsupported ODCS logicalType '{other}'"),
        )),
    }
}

fn parse_enum(
    value: Option<&Value>,
    source: &str,
    path: &str,
) -> Result<Option<Vec<ConstraintValue>>, OdcsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| invalid(source, path, "expected an array"))?;

    let mut parsed = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let value_path = format!("{path}[{index}]");
        let scalar = match value.as_object() {
            Some(object) => object
                .get("value")
                .ok_or_else(|| invalid(source, &value_path, "enum object requires value"))?,
            None => value,
        };
        parsed.push(constraint_value(scalar, source, &value_path)?);
    }
    Ok(Some(parsed))
}

fn constraint_value(value: &Value, source: &str, path: &str) -> Result<ConstraintValue, OdcsError> {
    match value {
        Value::Null => Ok(ConstraintValue::Null),
        Value::Bool(value) => Ok(ConstraintValue::Boolean(*value)),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(ConstraintValue::Integer(value))
            } else if let Some(value) = value.as_u64() {
                Ok(ConstraintValue::UnsignedInteger(value))
            } else {
                Ok(ConstraintValue::Number(value.to_string()))
            }
        }
        Value::String(value) => Ok(ConstraintValue::String(value.clone())),
        Value::Array(_) | Value::Object(_) => Err(invalid(
            source,
            path,
            "accepted-value enum members must be scalar",
        )),
    }
}

fn parse_relationship_array(
    value: Option<&Value>,
    source: &str,
    path: &str,
    implicit_from: Option<&str>,
) -> Result<Vec<RawRelationship>, OdcsError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let relationships = value
        .as_array()
        .ok_or_else(|| invalid(source, path, "expected an array"))?;
    let mut parsed = Vec::with_capacity(relationships.len());

    for (index, value) in relationships.iter().enumerate() {
        let relationship_path = format!("{path}[{index}]");
        let object = object(value, source, &relationship_path)?;
        let relationship_type =
            optional_string(object, source, &format!("{relationship_path}.type"), "type")?
                .unwrap_or("foreignKey");
        if relationship_type != "foreignKey" {
            return Err(invalid(
                source,
                format!("{relationship_path}.type"),
                format!("unsupported relationship type '{relationship_type}'"),
            ));
        }

        let id = optional_string(object, source, &format!("{relationship_path}.id"), "id")?
            .map(str::to_string);
        let to_value = object.get("to").ok_or_else(|| {
            invalid(
                source,
                format!("{relationship_path}.to"),
                "field is required",
            )
        })?;
        let from = match implicit_from {
            Some(reference) => {
                if object.contains_key("from") {
                    return Err(invalid(
                        source,
                        format!("{relationship_path}.from"),
                        "property-level relationship has an implicit source property and must not declare from",
                    ));
                }
                if to_value.is_array() {
                    return Err(invalid(
                        source,
                        format!("{relationship_path}.to"),
                        "property-level relationship to must be a string because its implicit from is a string",
                    ));
                }
                Some(vec![reference.to_string()])
            }
            None => {
                let from_value = object.get("from").ok_or_else(|| {
                    invalid(
                        source,
                        format!("{relationship_path}.from"),
                        "schema-level relationship requires from",
                    )
                })?;
                if from_value.is_array() != to_value.is_array() {
                    return Err(invalid(
                        source,
                        &relationship_path,
                        "schema-level relationship from and to must both be strings or both be arrays",
                    ));
                }
                Some(reference_list(
                    from_value,
                    source,
                    &format!("{relationship_path}.from"),
                )?)
            }
        };
        let to = reference_list(to_value, source, &format!("{relationship_path}.to"))?;

        parsed.push(RawRelationship {
            id,
            from,
            to,
            path: relationship_path,
        });
    }

    Ok(parsed)
}

fn reference_list(value: &Value, source: &str, path: &str) -> Result<Vec<String>, OdcsError> {
    if let Some(reference) = value.as_str() {
        if reference.trim().is_empty() {
            return Err(invalid(source, path, "reference cannot be empty"));
        }
        return Ok(vec![reference.to_string()]);
    }

    let values = value
        .as_array()
        .ok_or_else(|| invalid(source, path, "expected a string or array of strings"))?;
    if values.is_empty() {
        return Err(invalid(source, path, "reference array cannot be empty"));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    invalid(
                        source,
                        format!("{path}[{index}]"),
                        "expected a non-empty string reference",
                    )
                })
        })
        .collect()
}

fn property_constraints(
    contract: &ParsedContract,
    schema: &ParsedSchema,
) -> Result<Vec<RelationConstraint>, OdcsError> {
    let mut constraints = Vec::new();
    let primary_properties = schema
        .properties
        .iter()
        .filter(|property| property.primary_key)
        .collect::<Vec<_>>();

    if !primary_properties.is_empty() {
        let primary_columns = if primary_properties.len() == 1 {
            vec![primary_properties[0].column.clone()]
        } else {
            ordered_primary_key(contract, schema, &primary_properties)?
        };
        constraints.push(
            RelationConstraint::primary_key(
                primary_columns,
                vec![evidence(
                    contract,
                    schema,
                    "primary_key",
                    schema.id.as_deref().unwrap_or(&schema.name),
                )?],
            )
            .map_err(|error| constraint_error(&contract.source, error))?,
        );
    }

    for property in &schema.properties {
        let identity = property.id.as_deref().unwrap_or(&property.name);
        if property.unique {
            constraints.push(
                RelationConstraint::unique_key(
                    vec![property.column.clone()],
                    vec![evidence(contract, schema, "unique", identity)?],
                )
                .map_err(|error| constraint_error(&contract.source, error))?,
            );
        }
        if property.required {
            constraints.push(
                RelationConstraint::not_null(
                    property.column.clone(),
                    vec![evidence(contract, schema, "required", identity)?],
                )
                .map_err(|error| constraint_error(&contract.source, error))?,
            );
        }
        if let Some(values) = &property.enum_values {
            let strings = values
                .iter()
                .filter(|value| matches!(value, ConstraintValue::String(_)))
                .count();
            if strings != 0 && strings != values.len() {
                return Err(invalid(
                    &contract.source,
                    format!(
                        "schema.{}.properties.{}.enum",
                        schema.name, property.name
                    ),
                    "mixed string and non-string enum values cannot preserve canonical quote semantics",
                ));
            }
            constraints.push(
                RelationConstraint::accepted_values(
                    property.column.clone(),
                    values.clone(),
                    strings == values.len(),
                    vec![evidence(contract, schema, "enum", identity)?],
                )
                .map_err(|error| constraint_error(&contract.source, error))?,
            );
        }
    }

    Ok(constraints)
}

fn ordered_primary_key(
    contract: &ParsedContract,
    schema: &ParsedSchema,
    properties: &[&ParsedProperty],
) -> Result<Vec<String>, OdcsError> {
    let mut positioned = Vec::with_capacity(properties.len());
    let mut seen = BTreeSet::new();
    for property in properties {
        let position = property.primary_key_position.ok_or_else(|| {
            invalid(
                &contract.source,
                format!("schema.{}.properties.{}", schema.name, property.name),
                "composite primary-key members require primaryKeyPosition",
            )
        })?;
        if position == 0 || !seen.insert(position) {
            return Err(invalid(
                &contract.source,
                format!("schema.{}.properties.{}", schema.name, property.name),
                "primaryKeyPosition values must be unique positive integers",
            ));
        }
        positioned.push((position, property.column.clone()));
    }
    positioned.sort_by_key(|(position, _)| *position);
    if positioned
        .iter()
        .enumerate()
        .any(|(index, (position, _))| *position != (index + 1) as u64)
    {
        return Err(invalid(
            &contract.source,
            format!("schema.{}.properties", schema.name),
            "composite primaryKeyPosition values must form the sequence 1..N",
        ));
    }
    Ok(positioned.into_iter().map(|(_, column)| column).collect())
}

fn relationship_constraints(
    contract: &ParsedContract,
    schema: &ParsedSchema,
    contracts: &BTreeMap<String, ParsedContract>,
) -> Result<Vec<RelationConstraint>, OdcsError> {
    let mut constraints = Vec::new();

    for relationship in schema.relationships.iter().chain(
        schema
            .properties
            .iter()
            .flat_map(|property| property.relationships.iter()),
    ) {
        let from = relationship.from.as_ref().ok_or_else(|| {
            invalid(
                &contract.source,
                format!("{}.from", relationship.path),
                "schema-level relationship requires from",
            )
        })?;
        if from.len() != relationship.to.len() {
            return Err(invalid(
                &contract.source,
                &relationship.path,
                format!(
                    "foreign-key relationship has {} source references but {} target references",
                    from.len(),
                    relationship.to.len()
                ),
            ));
        }

        let local_refs = from
            .iter()
            .map(|reference| resolve_reference(contract, reference, contracts))
            .collect::<Result<Vec<_>, _>>()?;
        let target_refs = relationship
            .to
            .iter()
            .map(|reference| resolve_reference(contract, reference, contracts))
            .collect::<Result<Vec<_>, _>>()?;

        let local_relation = one_relation(&contract.source, &relationship.path, &local_refs)?;
        if local_relation != schema.relation {
            return Err(invalid(
                &contract.source,
                &relationship.path,
                format!(
                    "relationship is declared on relation '{}' but from resolves to '{local_relation}'",
                    schema.relation
                ),
            ));
        }
        let referenced_relation = one_relation(&contract.source, &relationship.path, &target_refs)?;

        let local_columns = local_refs
            .into_iter()
            .map(|reference| reference.column)
            .collect();
        let referenced_columns = target_refs
            .into_iter()
            .map(|reference| reference.column)
            .collect();

        let identity = relationship
            .id
            .as_deref()
            .unwrap_or(relationship.path.as_str());
        constraints.push(
            RelationConstraint::foreign_key(
                local_columns,
                referenced_relation,
                referenced_columns,
                vec![evidence(contract, schema, "relationship", identity)?],
            )
            .map_err(|error| constraint_error(&contract.source, error))?,
        );
    }

    Ok(constraints)
}

#[derive(Debug)]
struct ResolvedReference {
    relation: String,
    column: String,
}

fn resolve_reference(
    current: &ParsedContract,
    reference: &str,
    contracts: &BTreeMap<String, ParsedContract>,
) -> Result<ResolvedReference, OdcsError> {
    let (contract, reference_body) = if let Some((external, fragment)) = reference.split_once('#') {
        if external.is_empty() {
            (current, fragment)
        } else {
            let referenced =
                contracts
                    .get(external)
                    .ok_or_else(|| OdcsError::MissingReferencedContract {
                        source: current.source.clone(),
                        referenced_source: external.to_string(),
                    })?;
            (referenced, fragment)
        }
    } else {
        (current, reference)
    };

    let normalized = reference_body.trim_start_matches('/');
    if let Some(rest) = normalized.strip_prefix("schema/") {
        let parts = rest.split('/').collect::<Vec<_>>();
        if parts.len() == 3 && parts[1] == "properties" {
            return resolve_schema_property(contract, parts[0], parts[2], reference);
        }
        return Err(OdcsError::UnsupportedReference {
            source: current.source.clone(),
            reference: reference.to_string(),
            message: "only direct schema/<schema>/properties/<property> references are supported"
                .to_string(),
        });
    }

    let shorthand = normalized.split('.').collect::<Vec<_>>();
    if shorthand.len() == 2 {
        return resolve_schema_property(contract, shorthand[0], shorthand[1], reference);
    }

    Err(OdcsError::UnsupportedReference {
        source: current.source.clone(),
        reference: reference.to_string(),
        message: "expected schema.property or schema/<schema>/properties/<property>".to_string(),
    })
}

fn resolve_schema_property(
    contract: &ParsedContract,
    schema_identity: &str,
    property_identity: &str,
    original_reference: &str,
) -> Result<ResolvedReference, OdcsError> {
    let schemas = contract
        .schemas
        .iter()
        .filter(|schema| {
            schema.name == schema_identity || schema.id.as_deref() == Some(schema_identity)
        })
        .collect::<Vec<_>>();
    let [schema] = schemas.as_slice() else {
        return Err(OdcsError::UnsupportedReference {
            source: contract.source.clone(),
            reference: original_reference.to_string(),
            message: if schemas.is_empty() {
                format!("schema '{schema_identity}' was not found")
            } else {
                format!("schema identity '{schema_identity}' is ambiguous")
            },
        });
    };

    let properties = schema
        .properties
        .iter()
        .filter(|property| {
            property.name == property_identity
                || property.id.as_deref() == Some(property_identity)
                || property.column == property_identity
        })
        .collect::<Vec<_>>();
    let [property] = properties.as_slice() else {
        return Err(OdcsError::UnsupportedReference {
            source: contract.source.clone(),
            reference: original_reference.to_string(),
            message: if properties.is_empty() {
                format!(
                    "property '{property_identity}' was not found in schema '{}'",
                    schema.name
                )
            } else {
                format!(
                    "property identity '{property_identity}' is ambiguous in schema '{}'",
                    schema.name
                )
            },
        });
    };

    Ok(ResolvedReference {
        relation: schema.relation.clone(),
        column: property.column.clone(),
    })
}

fn one_relation(
    source: &str,
    path: &str,
    references: &[ResolvedReference],
) -> Result<String, OdcsError> {
    let relations = references
        .iter()
        .map(|reference| reference.relation.as_str())
        .collect::<BTreeSet<_>>();
    if relations.len() != 1 {
        return Err(invalid(
            source,
            path,
            "composite relationship references must all resolve within one relation",
        ));
    }
    relations
        .into_iter()
        .next()
        .map(str::to_string)
        .ok_or_else(|| invalid(source, path, "relationship reference list cannot be empty"))
}

fn evidence(
    contract: &ParsedContract,
    schema: &ParsedSchema,
    kind: &str,
    identity: &str,
) -> Result<ConstraintEvidence, OdcsError> {
    let source_id = format!(
        "odcs:{}:schema:{}:{kind}:{identity}",
        contract.contract_id,
        schema.id.as_deref().unwrap_or(&schema.name)
    );
    let provenance = ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, source_id)
        .map_err(|error| OdcsError::ConstraintMetadata {
        source: contract.source.clone(),
        message: error.to_string(),
    })?;
    Ok(ConstraintEvidence::new(
        provenance,
        ConstraintEnforcement::Unknown,
    ))
}

fn merge_source_schemas(
    existing: &[RelationSchema],
    odcs: &[RelationSchema],
) -> Result<Vec<RelationSchema>, OdcsError> {
    let mut merged = existing.to_vec();

    for incoming in odcs {
        let Some(current) = merged
            .iter()
            .find(|schema| schema.relation() == incoming.relation())
        else {
            merged.push(incoming.clone());
            continue;
        };

        let existing_columns = current
            .columns()
            .iter()
            .map(|column| (column.name(), column.data_type()))
            .collect::<BTreeMap<_, _>>();
        for column in incoming.columns() {
            if let Some(existing_type) = existing_columns.get(column.name()) {
                if *existing_type != column.data_type() {
                    return Err(OdcsError::DatatypeConflict {
                        relation: incoming.relation().to_string(),
                        column: column.name().to_string(),
                        existing: Box::new((*existing_type).clone()),
                        odcs: Box::new(column.data_type().clone()),
                    });
                }
            }
        }
    }

    merged.sort_by(|left, right| left.relation().cmp(right.relation()));
    Ok(merged)
}

fn resolve_odcs_relation(
    source: &str,
    reference: &str,
    dialect_name: &str,
    catalog: &RelationCatalog,
) -> Result<String, OdcsError> {
    let resolved = catalog
        .resolve(reference, dialect_name, None)
        .map_err(|error| OdcsError::RelationResolution {
            source: source.to_string(),
            relation: reference.to_string(),
            error,
        })?;

    if !catalog.relation_names().is_empty()
        && !catalog
            .relation_names()
            .iter()
            .any(|candidate| *candidate == resolved)
    {
        return Err(OdcsError::UnresolvedRelation {
            source: source.to_string(),
            relation: reference.to_string(),
        });
    }

    Ok(resolved)
}

fn validate_schema_identities(source: &str, schemas: &[ParsedSchema]) -> Result<(), OdcsError> {
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for schema in schemas {
        if !names.insert(schema.name.as_str()) {
            return Err(invalid(
                source,
                "$.schema",
                format!("schema name '{}' is duplicated", schema.name),
            ));
        }
        if let Some(id) = &schema.id {
            if !ids.insert(id.as_str()) {
                return Err(invalid(
                    source,
                    "$.schema",
                    format!("schema id '{id}' is duplicated"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_property_identities(
    source: &str,
    schema_path: &str,
    properties: &[ParsedProperty],
) -> Result<(), OdcsError> {
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut columns = BTreeSet::new();
    for property in properties {
        if !names.insert(property.name.as_str()) {
            return Err(invalid(
                source,
                format!("{schema_path}.properties"),
                format!("property name '{}' is duplicated", property.name),
            ));
        }
        if !columns.insert(property.column.as_str()) {
            return Err(invalid(
                source,
                format!("{schema_path}.properties"),
                format!("physical column '{}' is duplicated", property.column),
            ));
        }
        if let Some(id) = &property.id {
            if !ids.insert(id.as_str()) {
                return Err(invalid(
                    source,
                    format!("{schema_path}.properties"),
                    format!("property id '{id}' is duplicated"),
                ));
            }
        }
    }
    Ok(())
}

fn parse_yaml(document: &OdcsDocument) -> Result<Value, OdcsError> {
    let docs = Yaml::load_from_str(document.yaml()).map_err(|error| OdcsError::InvalidYaml {
        source: document.source().to_string(),
        message: error.to_string(),
    })?;
    let [document_yaml] = docs.as_slice() else {
        return Err(invalid(
            document.source(),
            "$",
            "expected exactly one YAML document",
        ));
    };
    yaml_to_json(document_yaml, document.source(), "$")
}

fn yaml_to_json(node: &Yaml<'_>, source: &str, path: &str) -> Result<Value, OdcsError> {
    if node.is_null() {
        return Ok(Value::Null);
    }
    if let Some(value) = node.as_bool() {
        return Ok(Value::Bool(value));
    }
    if let Some(value) = node.as_integer() {
        return Ok(Value::Number(Number::from(value)));
    }
    if let Some(value) = node.as_floating_point() {
        let number = Number::from_f64(value)
            .ok_or_else(|| invalid(source, path, "non-finite YAML numbers are unsupported"))?;
        return Ok(Value::Number(number));
    }
    if let Some(value) = node.as_str() {
        return Ok(Value::String(value.to_string()));
    }
    if let Some(sequence) = node.as_sequence() {
        let values = sequence
            .iter()
            .enumerate()
            .map(|(index, value)| yaml_to_json(value, source, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Value::Array(values));
    }
    if let Some(mapping) = node.as_mapping() {
        let mut object = Map::new();
        for (key, value) in mapping {
            let key = key
                .as_str()
                .ok_or_else(|| invalid(source, path, "ODCS mappings require string keys"))?;
            if object.contains_key(key) {
                return Err(invalid(
                    source,
                    path,
                    format!("mapping key '{key}' is duplicated"),
                ));
            }
            object.insert(
                key.to_string(),
                yaml_to_json(value, source, &format!("{path}.{key}"))?,
            );
        }
        return Ok(Value::Object(object));
    }

    Err(invalid(
        source,
        path,
        "YAML aliases, unresolved values, and custom tagged nodes are unsupported",
    ))
}

fn object<'a>(
    value: &'a Value,
    source: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, OdcsError> {
    value
        .as_object()
        .ok_or_else(|| invalid(source, path, "expected an object"))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    source: &str,
    path: &str,
    field: &str,
) -> Result<&'a str, OdcsError> {
    optional_string(object, source, path, field)?
        .ok_or_else(|| invalid(source, path, "field is required"))
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    source: &str,
    path: &str,
    field: &str,
) -> Result<Option<&'a str>, OdcsError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
        Some(Value::String(_)) => Err(invalid(source, path, "string cannot be empty")),
        Some(_) => Err(invalid(source, path, "expected a string")),
    }
}

fn optional_bool(
    object: &Map<String, Value>,
    source: &str,
    path: &str,
    field: &str,
) -> Result<Option<bool>, OdcsError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(invalid(source, path, "expected a boolean")),
    }
}

fn optional_u64(
    object: &Map<String, Value>,
    source: &str,
    path: &str,
    field: &str,
) -> Result<Option<u64>, OdcsError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid(source, path, "expected a non-negative integer")),
        Some(_) => Err(invalid(source, path, "expected an integer")),
    }
}

fn invalid(
    source: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> OdcsError {
    OdcsError::InvalidDocument {
        source: source.into(),
        path: path.into(),
        message: message.into(),
    }
}

fn constraint_error(source: &str, error: ConstraintMetadataError) -> OdcsError {
    OdcsError::ConstraintMetadata {
        source: source.to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_types_map_without_sqlparser_types() {
        assert_eq!(
            logical_data_type("test", "$.logicalType", "integer"),
            Ok(DataType::SignedInteger { bits: None })
        );
        assert_eq!(
            logical_data_type("test", "$.logicalType", "object"),
            Ok(DataType::Json)
        );
    }

    #[test]
    fn exact_document_count_is_required() {
        let document =
            OdcsDocument::new("test.yaml", "---\na: b\n---\nc: d\n").expect("document identity");
        assert!(matches!(
            parse_yaml(&document),
            Err(OdcsError::InvalidDocument { .. })
        ));
    }
}
