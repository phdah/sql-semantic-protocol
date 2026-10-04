//! dbt manifest adapter.
//!
//! dbt-specific artifact metadata is translated into the existing configured-input, relation
//! catalog, and dependency-graph APIs before semantic analysis. The core protocol model remains
//! independent of dbt types.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Map, Value};
use sqlparser::dialect::Dialect;

use crate::{
    analyze_configured_inputs_with_catalog, AnalysisBundle, ComposedSemantics,
    ConfiguredInputAnalysisError, ConfiguredSqlInput, RelationCatalog, RelationContext,
    RelationSchema, SchemaColumn, SqlInput,
};

/// dbt manifest schema versions accepted by the adapter.
///
/// These schemas cover dbt manifest v10, v11, and v12. The adapter intentionally reads only the
/// stable fields it needs at the integration boundary.
pub const SUPPORTED_DBT_MANIFEST_VERSIONS: &[u32] = &[10, 11, 12];


/// dbt catalog schema versions accepted by the adapter.
///
/// Catalog v0 and v1 share the table/column shape needed by the protocol. v1 adds artifact
/// metadata while v0 exposes its generation timestamp at the root.
pub const SUPPORTED_DBT_CATALOG_VERSIONS: &[u32] = &[0, 1];

/// Parsed dbt catalog metadata containing warehouse-introspected relation schemas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbtCatalog {
    schema_version: u32,
    dbt_version: Option<String>,
    resources: BTreeMap<String, DbtCatalogResource>,
}

impl DbtCatalog {
    /// Return the dbt catalog schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Return the dbt version recorded by catalog v1 when present.
    pub fn dbt_version(&self) -> Option<&str> {
        self.dbt_version.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DbtCatalogResource {
    columns: Vec<DbtCatalogColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DbtCatalogColumn {
    name: String,
    data_type: String,
    index: i64,
}

/// Error returned when dbt catalog metadata cannot be translated safely.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbtCatalogError {
    /// The catalog artifact is not valid JSON.
    InvalidJson {
        /// JSON parser error text.
        message: String,
    },
    /// A required catalog field is absent or malformed.
    InvalidField {
        /// JSON path identifying the affected field.
        path: String,
        /// Explanation of the invalid value.
        message: String,
    },
    /// The catalog schema version is valid but unsupported.
    UnsupportedSchemaVersion {
        /// Parsed catalog schema version.
        version: u32,
    },
    /// dbt reported warehouse metadata-query failures while producing the catalog.
    CatalogErrors {
        /// Errors reported by dbt.
        errors: Vec<String>,
    },
}

impl fmt::Display for DbtCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => write!(formatter, "invalid dbt catalog JSON: {message}"),
            Self::InvalidField { path, message } => {
                write!(formatter, "invalid dbt catalog field '{path}': {message}")
            }
            Self::UnsupportedSchemaVersion { version } => write!(
                formatter,
                "unsupported dbt catalog schema v{version}; supported versions are {}",
                SUPPORTED_DBT_CATALOG_VERSIONS
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::CatalogErrors { errors } => {
                write!(formatter, "dbt catalog contains metadata errors: {}", errors.join("; "))
            }
        }
    }
}

impl std::error::Error for DbtCatalogError {}

/// Error returned when a manifest/catalog pair cannot produce the complete dbt protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbtArtifactsError {
    /// Manifest parsing or SQL analysis failed.
    Manifest(DbtManifestError),
    /// Catalog parsing failed.
    Catalog(DbtCatalogError),
    /// The catalog contains a resource absent from the paired manifest.
    CatalogResourceNotInManifest {
        /// Catalog resource unique ID.
        unique_id: String,
    },
    /// A catalog resource cannot be attached to a canonical relation identity.
    MissingRelationIdentity {
        /// Resource unique ID.
        unique_id: String,
    },
    /// A warehouse datatype could not be normalized safely.
    ColumnType {
        /// Resource unique ID.
        unique_id: String,
        /// Canonical relation identity.
        relation: String,
        /// Column name.
        column: String,
        /// Warehouse datatype string.
        data_type: String,
        /// Normalization failure.
        message: String,
    },
    /// A physical dependency discovered from dbt SQL has no warehouse schema in catalog.json.
    MissingCatalogSchema {
        /// Canonical physical relation identity.
        relation: String,
    },
}

impl DbtArtifactsError {
    /// Return the underlying configured-input analysis error when SQL analysis failed.
    pub fn analysis_error(&self) -> Option<&ConfiguredInputAnalysisError> {
        match self {
            Self::Manifest(error) => error.analysis_error(),
            _ => None,
        }
    }
}

impl fmt::Display for DbtArtifactsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(error) => write!(formatter, "{error}"),
            Self::Catalog(error) => write!(formatter, "{error}"),
            Self::CatalogResourceNotInManifest { unique_id } => write!(
                formatter,
                "dbt catalog resource '{unique_id}' is absent from the paired manifest"
            ),
            Self::MissingRelationIdentity { unique_id } => write!(
                formatter,
                "dbt catalog resource '{unique_id}' has no relation identity in the paired manifest"
            ),
            Self::ColumnType {
                unique_id,
                relation,
                column,
                data_type,
                message,
            } => write!(
                formatter,
                "dbt catalog column '{unique_id}' ({relation}.{column}) has unsupported datatype '{data_type}': {message}"
            ),
            Self::MissingCatalogSchema { relation } => write!(
                formatter,
                "dbt catalog has no warehouse schema for physical dependency '{relation}'"
            ),
        }
    }
}

impl std::error::Error for DbtArtifactsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Catalog(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DbtManifestError> for DbtArtifactsError {
    fn from(error: DbtManifestError) -> Self {
        Self::Manifest(error)
    }
}

impl From<DbtCatalogError> for DbtArtifactsError {
    fn from(error: DbtCatalogError) -> Self {
        Self::Catalog(error)
    }
}

/// Parsed dbt manifest metadata and SQL-model inputs ready for protocol analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbtManifest {
    schema_version: u32,
    dbt_version: Option<String>,
    adapter_type: String,
    models: Vec<DbtModel>,
    resources: BTreeMap<String, DbtResource>,
    catalog_relations: Vec<String>,
}

impl DbtManifest {
    /// Return the dbt manifest schema version encoded by `metadata.dbt_schema_version`.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Return the dbt version recorded in artifact metadata, when present.
    pub fn dbt_version(&self) -> Option<&str> {
        self.dbt_version.as_deref()
    }

    /// Return the dbt adapter type recorded in artifact metadata.
    ///
    /// CLI callers can pass this value through `sqlparser::dialect::dialect_from_str`; library
    /// callers remain free to provide a compatible dialect implementation explicitly.
    pub fn adapter_type(&self) -> &str {
        &self.adapter_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DbtModel {
    unique_id: String,
    relation_name: String,
    original_file_path: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    sql: String,
    dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DbtResource {
    relation_name: Option<String>,
}

/// Error returned when a dbt manifest cannot be translated without guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbtManifestError {
    /// The artifact is not valid JSON.
    InvalidJson {
        /// JSON parser error text.
        message: String,
    },
    /// A required artifact field is absent or has an incompatible type or value.
    InvalidField {
        /// JSON path identifying the affected field.
        path: String,
        /// Explanation of the invalid value.
        message: String,
    },
    /// The manifest schema version is valid but not supported by this adapter.
    UnsupportedSchemaVersion {
        /// Parsed manifest schema version.
        version: u32,
    },
    /// A dbt model cannot be represented safely as a named SQL transformation.
    UnsupportedModel {
        /// dbt resource unique ID.
        unique_id: String,
        /// Reason the model cannot be analyzed safely.
        reason: String,
    },
    /// A model declares a dependency that is not present in manifest resources.
    UnknownDependency {
        /// dbt model unique ID.
        model_id: String,
        /// Missing dependency unique ID.
        dependency_id: String,
    },
    /// A declared dependency lacks the relation identity required by the protocol graph.
    UnsupportedDependency {
        /// dbt model unique ID.
        model_id: String,
        /// Dependency unique ID.
        dependency_id: String,
        /// Reason the dependency cannot be represented safely.
        reason: String,
    },
    /// dbt model dependency metadata contains a cycle.
    DependencyCycle {
        /// Model IDs participating in or blocked by the cycle.
        model_ids: Vec<String>,
    },
    /// A declared dbt dependency is not represented by the analyzed SQL relation graph.
    DependencyNotRepresented {
        /// dbt model unique ID.
        model_id: String,
        /// Declared dependency unique ID.
        dependency_id: String,
        /// Canonical dependency relation name.
        relation: String,
    },
    /// dbt relation metadata cannot be represented by the generic relation resolver.
    RelationMetadata {
        /// dbt resource or model unique ID associated with the metadata.
        resource_id: String,
        /// Relation metadata error text.
        message: String,
    },
    /// Core SQL parsing, analysis, or relation resolution failed for an adapted input.
    Analysis(ConfiguredInputAnalysisError),
}

impl DbtManifestError {
    /// Return the underlying configured-input analysis error, when SQL analysis failed.
    pub fn analysis_error(&self) -> Option<&ConfiguredInputAnalysisError> {
        match self {
            Self::Analysis(error) => Some(error),
            Self::InvalidJson { .. }
            | Self::InvalidField { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::UnsupportedModel { .. }
            | Self::UnknownDependency { .. }
            | Self::UnsupportedDependency { .. }
            | Self::DependencyCycle { .. }
            | Self::DependencyNotRepresented { .. }
            | Self::RelationMetadata { .. } => None,
        }
    }
}

impl fmt::Display for DbtManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => {
                write!(formatter, "invalid dbt manifest JSON: {message}")
            }
            Self::InvalidField { path, message } => {
                write!(formatter, "invalid dbt manifest field '{path}': {message}")
            }
            Self::UnsupportedSchemaVersion { version } => write!(
                formatter,
                "unsupported dbt manifest schema v{version}; supported versions are {}",
                SUPPORTED_DBT_MANIFEST_VERSIONS
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::UnsupportedModel { unique_id, reason } => {
                write!(formatter, "dbt model '{unique_id}' is unsupported: {reason}")
            }
            Self::UnknownDependency {
                model_id,
                dependency_id,
            } => write!(
                formatter,
                "dbt model '{model_id}' depends on unknown resource '{dependency_id}'"
            ),
            Self::UnsupportedDependency {
                model_id,
                dependency_id,
                reason,
            } => write!(
                formatter,
                "dbt model '{model_id}' dependency '{dependency_id}' is unsupported: {reason}"
            ),
            Self::DependencyCycle { model_ids } => write!(
                formatter,
                "dbt model dependency metadata contains a cycle involving {}",
                model_ids.join(", ")
            ),
            Self::DependencyNotRepresented {
                model_id,
                dependency_id,
                relation,
            } => write!(
                formatter,
                "dbt model '{model_id}' declares dependency '{dependency_id}' ({relation}) but its analyzed SQL does not consume that relation"
            ),
            Self::RelationMetadata {
                resource_id,
                message,
            } => write!(
                formatter,
                "dbt resource '{resource_id}' has invalid relation metadata: {message}"
            ),
            Self::Analysis(error) => write!(formatter, "dbt-adapted SQL analysis failed: {error}"),
        }
    }
}

impl std::error::Error for DbtManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Analysis(error) => Some(error),
            _ => None,
        }
    }
}

/// Parse and validate a dbt `catalog.json` artifact at the adapter boundary.
///
/// Catalog metadata is warehouse-introspected evidence. The adapter accepts catalog schema v0 and
/// v1, rejects dbt-reported metadata-query errors, and preserves deterministic column order by the
/// catalog ordinal index.
pub fn parse_dbt_catalog(json: &str) -> Result<DbtCatalog, DbtCatalogError> {
    let value =
        serde_json::from_str::<Value>(json).map_err(|error| DbtCatalogError::InvalidJson {
            message: error.to_string(),
        })?;
    let root = catalog_object(&value, "$")?;

    let (schema_version, dbt_version) =
        match catalog_optional_object(root, "metadata", "$.metadata")? {
            Some(metadata) => {
                let schema_url = catalog_required_string(
                    metadata,
                    "dbt_schema_version",
                    "$.metadata.dbt_schema_version",
                )?;
                let version = parse_catalog_schema_version(schema_url)?;
                let dbt_version =
                    catalog_optional_string(metadata, "dbt_version", "$.metadata.dbt_version")?;
                (version, dbt_version)
            }
            None if root.contains_key("generated_at") => (0, None),
            None => {
                return Err(catalog_invalid_field(
                    "$.metadata",
                    "catalog must contain v1 metadata or the v0 generated_at field",
                ));
            }
        };

    if !SUPPORTED_DBT_CATALOG_VERSIONS.contains(&schema_version) {
        return Err(DbtCatalogError::UnsupportedSchemaVersion {
            version: schema_version,
        });
    }

    let errors = catalog_errors(root)?;
    if !errors.is_empty() {
        return Err(DbtCatalogError::CatalogErrors { errors });
    }

    let nodes = catalog_required_object(root, "nodes", "$.nodes")?;
    let sources = catalog_required_object(root, "sources", "$.sources")?;
    let mut resources = BTreeMap::new();
    parse_catalog_resources(nodes, "$.nodes", &mut resources)?;
    parse_catalog_resources(sources, "$.sources", &mut resources)?;

    Ok(DbtCatalog {
        schema_version,
        dbt_version,
        resources,
    })
}

fn parse_catalog_resources(
    resources: &Map<String, Value>,
    path: &str,
    parsed: &mut BTreeMap<String, DbtCatalogResource>,
) -> Result<(), DbtCatalogError> {
    for (resource_id, value) in resources {
        let resource_path = format!("{path}.{resource_id}");
        let object = catalog_object(value, &resource_path)?;
        if let Some(unique_id) = catalog_optional_string(
            object,
            "unique_id",
            &format!("{resource_path}.unique_id"),
        )? {
            if unique_id != *resource_id {
                return Err(catalog_invalid_field(
                    format!("{resource_path}.unique_id"),
                    format!(
                        "value '{unique_id}' does not match dictionary key '{resource_id}'"
                    ),
                ));
            }
        }

        let columns =
            catalog_required_object(object, "columns", &format!("{resource_path}.columns"))?;
        let mut parsed_columns = Vec::with_capacity(columns.len());
        for (column_key, value) in columns {
            let column_path = format!("{resource_path}.columns.{column_key}");
            let column = catalog_object(value, &column_path)?;
            let name =
                catalog_required_string(column, "name", &format!("{column_path}.name"))?.trim();
            if name.is_empty() {
                return Err(catalog_invalid_field(
                    format!("{column_path}.name"),
                    "column name cannot be empty",
                ));
            }
            let data_type =
                catalog_required_string(column, "type", &format!("{column_path}.type"))?.trim();
            if data_type.is_empty() {
                return Err(catalog_invalid_field(
                    format!("{column_path}.type"),
                    "column datatype cannot be empty",
                ));
            }
            let index =
                catalog_required_i64(column, "index", &format!("{column_path}.index"))?;

            parsed_columns.push(DbtCatalogColumn {
                name: name.to_string(),
                data_type: data_type.to_string(),
                index,
            });
        }

        parsed_columns.sort_by(|left, right| {
            left.index
                .cmp(&right.index)
                .then_with(|| left.name.cmp(&right.name))
        });

        if parsed
            .insert(
                resource_id.clone(),
                DbtCatalogResource {
                    columns: parsed_columns,
                },
            )
            .is_some()
        {
            return Err(catalog_invalid_field(
                &resource_path,
                "resource unique ID is duplicated across nodes and sources",
            ));
        }
    }

    Ok(())
}

fn catalog_errors(root: &Map<String, Value>) -> Result<Vec<String>, DbtCatalogError> {
    let Some(value) = root.get("errors") else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let values = value
        .as_array()
        .ok_or_else(|| catalog_invalid_field("$.errors", "expected an array of strings or null"))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                catalog_invalid_field(
                    format!("$.errors[{index}]"),
                    "expected a string catalog error",
                )
            })
        })
        .collect()
}

fn parse_catalog_schema_version(schema_url: &str) -> Result<u32, DbtCatalogError> {
    let marker = "/catalog/v";
    schema_url
        .rsplit_once(marker)
        .and_then(|(_, suffix)| suffix.strip_suffix(".json"))
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| {
            catalog_invalid_field(
                "$.metadata.dbt_schema_version",
                format!(
                    "expected a dbt catalog schema URL ending in /catalog/vN.json, got '{schema_url}'"
                ),
            )
        })
}

fn catalog_object<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Map<String, Value>, DbtCatalogError> {
    value
        .as_object()
        .ok_or_else(|| catalog_invalid_field(path, "expected an object"))
}

fn catalog_required_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, DbtCatalogError> {
    let value = object
        .get(key)
        .ok_or_else(|| catalog_invalid_field(path, "field is required"))?;
    catalog_object(value, path)
}

fn catalog_optional_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<&'a Map<String, Value>>, DbtCatalogError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => catalog_object(value, path).map(Some),
    }
}

fn catalog_required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, DbtCatalogError> {
    object
        .get(key)
        .ok_or_else(|| catalog_invalid_field(path, "field is required"))?
        .as_str()
        .ok_or_else(|| catalog_invalid_field(path, "expected a string"))
}

fn catalog_optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<String>, DbtCatalogError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(catalog_invalid_field(path, "expected a string or null")),
    }
}

fn catalog_required_i64(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<i64, DbtCatalogError> {
    object
        .get(key)
        .ok_or_else(|| catalog_invalid_field(path, "field is required"))?
        .as_i64()
        .ok_or_else(|| catalog_invalid_field(path, "expected an integer"))
}

fn catalog_invalid_field(
    path: impl Into<String>,
    message: impl Into<String>,
) -> DbtCatalogError {
    DbtCatalogError::InvalidField {
        path: path.into(),
        message: message.into(),
    }
}

/// Parse and validate a dbt `manifest.json` artifact at the adapter boundary.
///
/// The adapter accepts manifest schema v10 through v12. SQL models must have a materialized
/// `relation_name` and analyzable SQL. `compiled_code` is preferred; plain `raw_code` is used only
/// when it contains no Jinja delimiters. Python models, relation-less models such as ephemerals,
/// and dependencies without relation identity fail explicitly instead of being omitted.
pub fn parse_dbt_manifest(json: &str) -> Result<DbtManifest, DbtManifestError> {
    let value =
        serde_json::from_str::<Value>(json).map_err(|error| DbtManifestError::InvalidJson {
            message: error.to_string(),
        })?;
    let root = as_object(&value, "$")?;
    let metadata = required_object(root, "metadata", "$.metadata")?;
    let schema_url = required_string(
        metadata,
        "dbt_schema_version",
        "$.metadata.dbt_schema_version",
    )?;
    let schema_version = parse_schema_version(schema_url)?;
    if !SUPPORTED_DBT_MANIFEST_VERSIONS.contains(&schema_version) {
        return Err(DbtManifestError::UnsupportedSchemaVersion {
            version: schema_version,
        });
    }

    let adapter_type = required_string(metadata, "adapter_type", "$.metadata.adapter_type")?
        .trim()
        .to_ascii_lowercase();
    if adapter_type.is_empty() {
        return Err(invalid_field(
            "$.metadata.adapter_type",
            "value cannot be empty",
        ));
    }
    let dbt_version = optional_string(metadata, "dbt_version", "$.metadata.dbt_version")?;

    let nodes = required_object(root, "nodes", "$.nodes")?;
    let sources = optional_object(root, "sources", "$.sources")?;
    let mut resources = BTreeMap::new();

    for (resource_id, value) in nodes {
        let path = format!("$.nodes.{resource_id}");
        let object = as_object(value, &path)?;
        validate_unique_id(resource_id, object, &path)?;
        let relation_name =
            optional_string(object, "relation_name", &format!("{path}.relation_name"))?;
        resources.insert(resource_id.clone(), DbtResource { relation_name });
    }
    if let Some(sources) = sources {
        for (resource_id, value) in sources {
            let path = format!("$.sources.{resource_id}");
            let object = as_object(value, &path)?;
            validate_unique_id(resource_id, object, &path)?;
            let relation_name =
                optional_string(object, "relation_name", &format!("{path}.relation_name"))?;
            if resources
                .insert(resource_id.clone(), DbtResource { relation_name })
                .is_some()
            {
                return Err(invalid_field(
                    &path,
                    "resource unique ID is duplicated across nodes and sources",
                ));
            }
        }
    }

    let mut models = BTreeMap::new();
    for (resource_id, value) in nodes {
        let path = format!("$.nodes.{resource_id}");
        let object = as_object(value, &path)?;
        let resource_type =
            required_string(object, "resource_type", &format!("{path}.resource_type"))?;
        if resource_type != "model" {
            continue;
        }

        let language = optional_string(object, "language", &format!("{path}.language"))?
            .unwrap_or_else(|| "sql".to_string());
        if !language.eq_ignore_ascii_case("sql") {
            return Err(DbtManifestError::UnsupportedModel {
                unique_id: resource_id.clone(),
                reason: format!("language '{language}' is not SQL"),
            });
        }

        let relation_name = resources
            .get(resource_id)
            .and_then(|resource| resource.relation_name.clone())
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| DbtManifestError::UnsupportedModel {
                unique_id: resource_id.clone(),
                reason: "relation_name is unavailable; relation-less models cannot be represented as named protocol outcomes"
                    .to_string(),
            })?;
        let sql = model_sql(resource_id, object, &path)?;
        let dependencies = dependency_ids(object, &path)?;
        for dependency_id in &dependencies {
            let Some(resource) = resources.get(dependency_id) else {
                return Err(DbtManifestError::UnknownDependency {
                    model_id: resource_id.clone(),
                    dependency_id: dependency_id.clone(),
                });
            };
            if resource
                .relation_name
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(DbtManifestError::UnsupportedDependency {
                    model_id: resource_id.clone(),
                    dependency_id: dependency_id.clone(),
                    reason: "resource has no relation_name".to_string(),
                });
            }
        }

        let original_file_path = match optional_string(
            object,
            "original_file_path",
            &format!("{path}.original_file_path"),
        )? {
            Some(path) => Some(path),
            None => optional_string(object, "path", &format!("{path}.path"))?,
        };
        let database = optional_string(object, "database", &format!("{path}.database"))?;
        let schema = optional_string(object, "schema", &format!("{path}.schema"))?;

        models.insert(
            resource_id.clone(),
            DbtModel {
                unique_id: resource_id.clone(),
                relation_name,
                original_file_path,
                database,
                schema,
                sql,
                dependencies,
            },
        );
    }

    let models = topologically_order_models(models)?;
    let mut catalog_relations = resources
        .values()
        .filter_map(|resource| resource.relation_name.as_ref())
        .filter(|relation| !relation.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    catalog_relations.sort();
    catalog_relations.dedup();

    Ok(DbtManifest {
        schema_version,
        dbt_version,
        adapter_type,
        models,
        resources,
        catalog_relations,
    })
}

/// Analyze a parsed dbt manifest through the same core analyzer used by generic SQL inputs.
///
/// Each dbt model is wrapped as a named query-backed view using its artifact `relation_name` so
/// model identity comes from dbt metadata rather than filenames. dbt relation metadata seeds the
/// generic relation catalog. `depends_on.nodes` establishes deterministic model order and is
/// checked against the relation dependencies discovered from compiled SQL, preventing dbt metadata
/// from inventing semantics the SQL does not support.
pub fn analyze_dbt_manifest(
    manifest: &DbtManifest,
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<AnalysisBundle, DbtManifestError> {
    struct PreparedInput {
        id: String,
        input: SqlInput,
        relation_context: Option<RelationContext>,
    }

    let mut prepared = Vec::with_capacity(manifest.models.len());
    for model in &manifest.models {
        let sql = format!(
            "CREATE VIEW {} AS\n{}",
            model.relation_name,
            model.sql.trim()
        );
        let input = match &model.original_file_path {
            Some(path) => SqlInput::file(path.clone(), sql),
            None => SqlInput::inline(sql),
        };
        let relation_context = if model.database.is_some() || model.schema.is_some() {
            Some(
                RelationContext::new(model.database.as_deref(), model.schema.as_deref()).map_err(
                    |error| DbtManifestError::RelationMetadata {
                        resource_id: model.unique_id.clone(),
                        message: error.to_string(),
                    },
                )?,
            )
        } else {
            None
        };
        prepared.push(PreparedInput {
            id: model.unique_id.clone(),
            input,
            relation_context,
        });
    }

    let configured = prepared
        .iter()
        .map(|input| {
            let configured =
                ConfiguredSqlInput::new(&input.id, &input.input, dialect_name, dialect);
            match input.relation_context.as_ref() {
                Some(context) => configured.with_relation_context(context),
                None => configured,
            }
        })
        .collect::<Vec<_>>();
    let catalog_relations = manifest
        .catalog_relations
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::new(&catalog_relations).map_err(|error| {
        DbtManifestError::RelationMetadata {
            resource_id: "manifest catalog".to_string(),
            message: error.to_string(),
        }
    })?;
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .map_err(DbtManifestError::Analysis)?;

    validate_declared_dependencies(manifest, &bundle)?;
    Ok(bundle)
}

fn validate_declared_dependencies(
    manifest: &DbtManifest,
    bundle: &AnalysisBundle,
) -> Result<(), DbtManifestError> {
    for model in &manifest.models {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| layer.input_id() == model.unique_id)
            .ok_or_else(|| DbtManifestError::UnsupportedModel {
                unique_id: model.unique_id.clone(),
                reason: "adapted SQL produced no query-backed transformation layer".to_string(),
            })?;

        for dependency_id in &model.dependencies {
            let relation = manifest
                .resources
                .get(dependency_id)
                .and_then(|resource| resource.relation_name.as_deref())
                .expect("dependency relation identity is validated during manifest parsing");
            if !layer.consumes().iter().any(|consumed| consumed == relation) {
                return Err(DbtManifestError::DependencyNotRepresented {
                    model_id: model.unique_id.clone(),
                    dependency_id: dependency_id.clone(),
                    relation: relation.to_string(),
                });
            }
        }
    }

    Ok(())
}

fn topologically_order_models(
    models: BTreeMap<String, DbtModel>,
) -> Result<Vec<DbtModel>, DbtManifestError> {
    let mut indegree = models
        .keys()
        .map(|id| (id.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut children = BTreeMap::<String, BTreeSet<String>>::new();

    for model in models.values() {
        for dependency_id in &model.dependencies {
            if !models.contains_key(dependency_id) {
                continue;
            }
            if let Some(value) = indegree.get_mut(&model.unique_id) {
                *value += 1;
            }
            children
                .entry(dependency_id.clone())
                .or_default()
                .insert(model.unique_id.clone());
        }
    }

    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered_ids = Vec::with_capacity(models.len());

    while let Some(model_id) = ready.pop_first() {
        ordered_ids.push(model_id.clone());
        let Some(model_children) = children.get(&model_id) else {
            continue;
        };
        for child_id in model_children {
            let Some(degree) = indegree.get_mut(child_id) else {
                continue;
            };
            *degree -= 1;
            if *degree == 0 {
                ready.insert(child_id.clone());
            }
        }
    }

    if ordered_ids.len() != models.len() {
        let model_ids = indegree
            .into_iter()
            .filter(|(_, degree)| *degree > 0)
            .map(|(id, _)| id)
            .collect();
        return Err(DbtManifestError::DependencyCycle { model_ids });
    }

    Ok(ordered_ids
        .into_iter()
        .filter_map(|id| models.get(&id).cloned())
        .collect())
}

fn validate_unique_id(
    key: &str,
    object: &Map<String, Value>,
    path: &str,
) -> Result<(), DbtManifestError> {
    let unique_id = required_string(object, "unique_id", &format!("{path}.unique_id"))?;
    if unique_id != key {
        return Err(invalid_field(
            format!("{path}.unique_id"),
            format!("value '{unique_id}' does not match dictionary key '{key}'"),
        ));
    }
    Ok(())
}

fn model_sql(
    unique_id: &str,
    object: &Map<String, Value>,
    path: &str,
) -> Result<String, DbtManifestError> {
    if let Some(compiled) =
        optional_string(object, "compiled_code", &format!("{path}.compiled_code"))?
            .filter(|sql| !sql.trim().is_empty())
    {
        return Ok(compiled);
    }

    let raw = optional_string(object, "raw_code", &format!("{path}.raw_code"))?
        .filter(|sql| !sql.trim().is_empty())
        .ok_or_else(|| DbtManifestError::UnsupportedModel {
            unique_id: unique_id.to_string(),
            reason: "neither compiled_code nor non-empty raw_code is available".to_string(),
        })?;
    if contains_jinja(&raw) {
        return Err(DbtManifestError::UnsupportedModel {
            unique_id: unique_id.to_string(),
            reason: "compiled_code is unavailable and raw_code still contains Jinja".to_string(),
        });
    }
    Ok(raw)
}

fn dependency_ids(
    object: &Map<String, Value>,
    path: &str,
) -> Result<Vec<String>, DbtManifestError> {
    let Some(depends_on) = object.get("depends_on") else {
        return Ok(Vec::new());
    };
    let depends_on = as_object(depends_on, &format!("{path}.depends_on"))?;
    let Some(nodes) = depends_on.get("nodes") else {
        return Ok(Vec::new());
    };
    let array = nodes.as_array().ok_or_else(|| {
        invalid_field(
            format!("{path}.depends_on.nodes"),
            "expected an array of resource unique IDs",
        )
    })?;
    let mut dependencies = Vec::with_capacity(array.len());
    for (index, value) in array.iter().enumerate() {
        let dependency = value.as_str().ok_or_else(|| {
            invalid_field(
                format!("{path}.depends_on.nodes[{index}]"),
                "expected a string resource unique ID",
            )
        })?;
        dependencies.push(dependency.to_string());
    }
    dependencies.sort();
    dependencies.dedup();
    Ok(dependencies)
}

fn contains_jinja(sql: &str) -> bool {
    sql.contains("{{") || sql.contains("{%") || sql.contains("{#")
}

fn parse_schema_version(schema_url: &str) -> Result<u32, DbtManifestError> {
    let marker = "/manifest/v";
    schema_url
        .rsplit_once(marker)
        .and_then(|(_, suffix)| suffix.strip_suffix(".json"))
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| {
            invalid_field(
                "$.metadata.dbt_schema_version",
                format!(
                    "expected a dbt manifest schema URL ending in /manifest/vN.json, got '{schema_url}'"
                ),
            )
        })
}

fn as_object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, DbtManifestError> {
    value
        .as_object()
        .ok_or_else(|| invalid_field(path, "expected an object"))
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Map<String, Value>, DbtManifestError> {
    let value = object
        .get(key)
        .ok_or_else(|| invalid_field(path, "field is required"))?;
    as_object(value, path)
}

fn optional_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<&'a Map<String, Value>>, DbtManifestError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => as_object(value, path).map(Some),
    }
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, DbtManifestError> {
    object
        .get(key)
        .ok_or_else(|| invalid_field(path, "field is required"))?
        .as_str()
        .ok_or_else(|| invalid_field(path, "expected a string"))
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<String>, DbtManifestError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(invalid_field(path, "expected a string or null")),
    }
}

fn invalid_field(path: impl Into<String>, message: impl Into<String>) -> DbtManifestError {
    DbtManifestError::InvalidField {
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_manifest_schema() {
        let json = r#"{
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v9.json",
                "adapter_type": "postgres"
            },
            "nodes": {},
            "sources": {}
        }"#;

        assert_eq!(
            parse_dbt_manifest(json),
            Err(DbtManifestError::UnsupportedSchemaVersion { version: 9 })
        );
    }

    #[test]
    fn rejects_uncompiled_jinja_model() {
        let json = r#"{
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.orders": {
                    "unique_id": "model.demo.orders",
                    "resource_type": "model",
                    "relation_name": "analytics.orders",
                    "language": "sql",
                    "raw_code": "select * from {{ ref('raw_orders') }}",
                    "depends_on": {"nodes": []}
                }
            },
            "sources": {}
        }"#;

        let error = parse_dbt_manifest(json).expect_err("Jinja without compiled SQL should fail");
        assert!(matches!(
            error,
            DbtManifestError::UnsupportedModel { unique_id, .. }
                if unique_id == "model.demo.orders"
        ));
    }
}
