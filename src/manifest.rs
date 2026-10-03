//! Versioned analysis-manifest contract.
//!
//! This module parses and validates manifest JSON without performing file I/O or depending on
//! sqlparser AST types. Callers resolve file paths and dialect implementations at their boundaries.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Map, Value};

/// Active analysis-manifest contract version.
pub const ANALYSIS_MANIFEST_VERSION: &str = "1";

/// Post-analysis output projection requested by a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestOutputScope {
    /// Emit the complete analyzed bundle.
    All,
    /// Emit only explicitly requested targets and their required in-bundle ancestors.
    Targets,
}

/// Optional default catalog/schema context declared by a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRelationContext {
    default_catalog: Option<String>,
    default_schema: Option<String>,
}

impl ManifestRelationContext {
    /// Return the optional default catalog identifier.
    pub fn default_catalog(&self) -> Option<&str> {
        self.default_catalog.as_deref()
    }

    /// Return the optional default schema identifier.
    pub fn default_schema(&self) -> Option<&str> {
        self.default_schema.as_deref()
    }
}

/// Source declared for one manifest input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestInputSource {
    /// SQL embedded directly in the manifest.
    Inline {
        /// SQL text to analyze.
        sql: String,
    },
    /// SQL loaded from a path relative to the manifest location when the path is relative.
    File {
        /// Caller-visible path from the manifest.
        path: String,
    },
}

/// One validated manifest input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestInput {
    id: String,
    dialect: Option<String>,
    relation_context: Option<ManifestRelationContext>,
    source: ManifestInputSource,
}

impl ManifestInput {
    /// Return the stable input identifier declared by the manifest.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the optional per-input dialect override.
    pub fn dialect(&self) -> Option<&str> {
        self.dialect.as_deref()
    }

    /// Return optional input-specific relation context.
    pub fn relation_context(&self) -> Option<&ManifestRelationContext> {
        self.relation_context.as_ref()
    }

    /// Return the declared inline or file source.
    pub fn source(&self) -> &ManifestInputSource {
        &self.source
    }
}

/// Validated declarative analysis request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisManifest {
    default_dialect: String,
    catalog_relations: Vec<String>,
    relation_context: Option<ManifestRelationContext>,
    output_scope: ManifestOutputScope,
    targets: Vec<String>,
    inputs: Vec<ManifestInput>,
}

impl AnalysisManifest {
    /// Return the bundle-level dialect used when an input does not override it.
    pub fn default_dialect(&self) -> &str {
        &self.default_dialect
    }

    /// Return canonical catalog relations used for optional relation resolution.
    pub fn catalog_relations(&self) -> &[String] {
        &self.catalog_relations
    }

    /// Return optional bundle-level default catalog/schema context.
    pub fn relation_context(&self) -> Option<&ManifestRelationContext> {
        self.relation_context.as_ref()
    }

    /// Return the effective relation context for one input.
    ///
    /// An input-specific context replaces the bundle-level context for that input.
    pub fn relation_context_for<'a>(
        &'a self,
        input: &'a ManifestInput,
    ) -> Option<&'a ManifestRelationContext> {
        input.relation_context().or_else(|| self.relation_context())
    }

    /// Return the post-analysis output projection.
    pub fn output_scope(&self) -> ManifestOutputScope {
        self.output_scope
    }

    /// Return explicit target relation identifiers in manifest order.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }

    /// Return inputs in deterministic manifest order.
    pub fn inputs(&self) -> &[ManifestInput] {
        &self.inputs
    }

    /// Resolve the effective dialect name for one input.
    pub fn dialect_for<'a>(&'a self, input: &'a ManifestInput) -> &'a str {
        input.dialect().unwrap_or(self.default_dialect())
    }
}

/// Error returned when manifest JSON does not satisfy the versioned contract.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestError {
    /// The document is not valid JSON.
    InvalidJson {
        /// JSON parser message.
        message: String,
    },
    /// The document uses an unsupported manifest version.
    UnsupportedVersion {
        /// Version found in the document.
        version: String,
    },
    /// The document violates the manifest contract.
    InvalidConfiguration {
        /// Human-readable validation failure.
        message: String,
    },
    /// More than one input uses the same stable identifier.
    DuplicateInputId {
        /// Duplicated identifier.
        id: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => write!(formatter, "invalid manifest JSON: {message}"),
            Self::UnsupportedVersion { version } => write!(
                formatter,
                "unsupported analysis manifest version '{version}'; expected {ANALYSIS_MANIFEST_VERSION}"
            ),
            Self::InvalidConfiguration { message } => write!(formatter, "{message}"),
            Self::DuplicateInputId { id } => {
                write!(formatter, "manifest input id '{id}' is duplicated")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// Parse and validate one analysis-manifest JSON document.
///
/// Parsing is pure: file contents and sqlparser dialect implementations are deliberately resolved
/// by the caller after validation.
pub fn parse_analysis_manifest(json: &str) -> Result<AnalysisManifest, ManifestError> {
    let value =
        serde_json::from_str::<Value>(json).map_err(|error| ManifestError::InvalidJson {
            message: error.to_string(),
        })?;
    let object = require_object(&value, "manifest")?;
    reject_unknown_fields(
        object,
        &[
            "manifest_version",
            "dialect",
            "catalog_relations",
            "relation_context",
            "output_scope",
            "targets",
            "inputs",
        ],
        "manifest",
    )?;

    let version = require_string(object, "manifest_version", "manifest")?;
    if version != ANALYSIS_MANIFEST_VERSION {
        return Err(ManifestError::UnsupportedVersion {
            version: version.to_string(),
        });
    }

    let default_dialect = optional_non_empty_string(object, "dialect", "manifest")?
        .unwrap_or_else(|| "generic".to_string());
    let catalog_relations = parse_unique_string_array(
        object.get("catalog_relations"),
        "manifest.catalog_relations",
        "manifest catalog relation",
    )?;
    let relation_context =
        parse_relation_context(object.get("relation_context"), "manifest.relation_context")?;
    let output_scope = match optional_non_empty_string(object, "output_scope", "manifest")?
        .as_deref()
        .unwrap_or("all")
    {
        "all" => ManifestOutputScope::All,
        "targets" => ManifestOutputScope::Targets,
        value => {
            return invalid(format!(
                "manifest.output_scope must be 'all' or 'targets', got '{value}'"
            ))
        }
    };

    let targets = parse_targets(object.get("targets"))?;
    match output_scope {
        ManifestOutputScope::All if !targets.is_empty() => {
            return invalid("manifest.targets requires output_scope 'targets'".to_string())
        }
        ManifestOutputScope::Targets if targets.is_empty() => {
            return invalid(
                "manifest.output_scope 'targets' requires at least one target".to_string(),
            )
        }
        ManifestOutputScope::All | ManifestOutputScope::Targets => {}
    }

    let input_values = object
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or_else(|| ManifestError::InvalidConfiguration {
            message: "manifest.inputs must be an array".to_string(),
        })?;
    if input_values.is_empty() {
        return invalid("manifest.inputs must contain at least one input".to_string());
    }

    let mut inputs = Vec::with_capacity(input_values.len());
    let mut ids = BTreeSet::new();
    for (index, value) in input_values.iter().enumerate() {
        let input = parse_input(value, index + 1)?;
        if !ids.insert(input.id.clone()) {
            return Err(ManifestError::DuplicateInputId {
                id: input.id.clone(),
            });
        }
        inputs.push(input);
    }

    Ok(AnalysisManifest {
        default_dialect,
        catalog_relations,
        relation_context,
        output_scope,
        targets,
        inputs,
    })
}

fn parse_targets(value: Option<&Value>) -> Result<Vec<String>, ManifestError> {
    parse_unique_string_array(value, "manifest.targets", "manifest target")
}

fn parse_unique_string_array(
    value: Option<&Value>,
    context: &str,
    duplicate_label: &str,
) -> Result<Vec<String>, ManifestError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| ManifestError::InvalidConfiguration {
            message: format!("{context} must be an array"),
        })?;

    let mut parsed = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let item = value
            .as_str()
            .filter(|item| !item.trim().is_empty())
            .ok_or_else(|| ManifestError::InvalidConfiguration {
                message: format!("{context}[{index}] must be a non-empty string"),
            })?;
        if !seen.insert(item.to_string()) {
            return invalid(format!("{duplicate_label} '{item}' is duplicated"));
        }
        parsed.push(item.to_string());
    }
    Ok(parsed)
}

fn parse_relation_context(
    value: Option<&Value>,
    context: &str,
) -> Result<Option<ManifestRelationContext>, ManifestError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let object = require_object(value, context)?;
    reject_unknown_fields(object, &["default_catalog", "default_schema"], context)?;

    let default_catalog = optional_non_empty_string(object, "default_catalog", context)?;
    let default_schema = optional_non_empty_string(object, "default_schema", context)?;
    if default_catalog.is_none() && default_schema.is_none() {
        return invalid(format!(
            "{context} must specify default_catalog, default_schema, or both"
        ));
    }

    Ok(Some(ManifestRelationContext {
        default_catalog,
        default_schema,
    }))
}

fn parse_input(value: &Value, position: usize) -> Result<ManifestInput, ManifestError> {
    let context = format!("manifest.inputs[{}]", position - 1);
    let object = require_object(value, &context)?;
    reject_unknown_fields(
        object,
        &["id", "dialect", "relation_context", "sql", "file"],
        &context,
    )?;

    let id = require_non_empty_string(object, "id", &context)?.to_string();
    let dialect = optional_non_empty_string(object, "dialect", &context)?;
    let relation_context = parse_relation_context(
        object.get("relation_context"),
        &format!("{context}.relation_context"),
    )?;
    let sql = object.get("sql");
    let file = object.get("file");

    let source = match (sql, file) {
        (Some(sql), None) => {
            let sql = sql
                .as_str()
                .filter(|sql| !sql.trim().is_empty())
                .ok_or_else(|| ManifestError::InvalidConfiguration {
                    message: format!("{context}.sql must be a non-empty string"),
                })?;
            ManifestInputSource::Inline {
                sql: sql.to_string(),
            }
        }
        (None, Some(file)) => {
            let path = file
                .as_str()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| ManifestError::InvalidConfiguration {
                    message: format!("{context}.file must be a non-empty string"),
                })?;
            ManifestInputSource::File {
                path: path.to_string(),
            }
        }
        (Some(_), Some(_)) => {
            return invalid(format!(
                "{context} must specify exactly one of 'sql' or 'file'"
            ))
        }
        (None, None) => {
            return invalid(format!(
                "{context} must specify exactly one of 'sql' or 'file'"
            ))
        }
    };

    Ok(ManifestInput {
        id,
        dialect,
        relation_context,
        source,
    })
}

fn require_object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Map<String, Value>, ManifestError> {
    value
        .as_object()
        .ok_or_else(|| ManifestError::InvalidConfiguration {
            message: format!("{context} must be an object"),
        })
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), ManifestError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return invalid(format!("{context} contains unknown field '{key}'"));
        }
    }
    Ok(())
}

fn require_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, ManifestError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ManifestError::InvalidConfiguration {
            message: format!("{context}.{field} must be a string"),
        })
}

fn require_non_empty_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, ManifestError> {
    require_string(object, field, context).and_then(|value| {
        if value.trim().is_empty() {
            invalid(format!("{context}.{field} must be a non-empty string"))
        } else {
            Ok(value)
        }
    })
}

fn optional_non_empty_string(
    object: &Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<Option<String>, ManifestError> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ManifestError::InvalidConfiguration {
            message: format!("{context}.{field} must be a non-empty string"),
        })?;
    Ok(Some(value.to_string()))
}

fn invalid<T>(message: String) -> Result<T, ManifestError> {
    Err(ManifestError::InvalidConfiguration { message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_bundle_dialect_and_complete_output_scope() {
        let manifest = parse_analysis_manifest(
            r#"{"manifest_version":"1","inputs":[{"id":"orders","sql":"SELECT 1"}]}"#,
        )
        .expect("manifest should parse");

        assert_eq!(manifest.default_dialect(), "generic");
        assert!(manifest.catalog_relations().is_empty());
        assert!(manifest.relation_context().is_none());
        assert_eq!(manifest.output_scope(), ManifestOutputScope::All);
        assert!(manifest.targets().is_empty());
        assert_eq!(manifest.inputs()[0].id(), "orders");
    }

    #[test]
    fn parses_catalog_metadata_and_input_context_override() {
        let manifest = parse_analysis_manifest(
            r#"{
                "manifest_version":"1",
                "catalog_relations":["warehouse.stage.orders","warehouse.raw.orders"],
                "relation_context":{"default_catalog":"warehouse","default_schema":"stage"},
                "inputs":[
                    {"id":"stage","sql":"SELECT 1"},
                    {
                        "id":"finance",
                        "sql":"SELECT 2",
                        "relation_context":{"default_catalog":"warehouse","default_schema":"finance"}
                    }
                ]
            }"#,
        )
        .expect("catalog-aware manifest should parse");

        assert_eq!(
            manifest.catalog_relations(),
            &[
                "warehouse.stage.orders".to_string(),
                "warehouse.raw.orders".to_string()
            ]
        );
        let stage = manifest
            .relation_context_for(&manifest.inputs()[0])
            .expect("root relation context");
        assert_eq!(stage.default_catalog(), Some("warehouse"));
        assert_eq!(stage.default_schema(), Some("stage"));

        let finance = manifest
            .relation_context_for(&manifest.inputs()[1])
            .expect("input relation context");
        assert_eq!(finance.default_catalog(), Some("warehouse"));
        assert_eq!(finance.default_schema(), Some("finance"));
    }

    #[test]
    fn rejects_empty_relation_context() {
        let error = parse_analysis_manifest(
            r#"{
                "manifest_version":"1",
                "relation_context":{},
                "inputs":[{"id":"orders","sql":"SELECT 1"}]
            }"#,
        )
        .expect_err("empty relation context should fail");

        assert_eq!(
            error.to_string(),
            "manifest.relation_context must specify default_catalog, default_schema, or both"
        );
    }

    #[test]
    fn rejects_duplicate_input_ids() {
        let error = parse_analysis_manifest(
            r#"{"manifest_version":"1","inputs":[{"id":"same","sql":"SELECT 1"},{"id":"same","sql":"SELECT 2"}]}"#,
        )
        .expect_err("duplicate ids should fail");

        assert_eq!(
            error,
            ManifestError::DuplicateInputId {
                id: "same".to_string()
            }
        );
    }

    #[test]
    fn target_scope_requires_targets() {
        let error = parse_analysis_manifest(
            r#"{"manifest_version":"1","output_scope":"targets","inputs":[{"id":"one","sql":"SELECT 1"}]}"#,
        )
        .expect_err("target scope without targets should fail");

        assert_eq!(
            error.to_string(),
            "manifest.output_scope 'targets' requires at least one target"
        );
    }
}
