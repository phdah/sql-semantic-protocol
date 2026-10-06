//! Optional parser-independent catalog-aware relation resolution.
//!
//! SQL parsing remains responsible for producing textual relation references. This module resolves
//! those references against caller-supplied catalog metadata and per-input default context without
//! depending on sqlparser AST types.

use std::collections::BTreeSet;
use std::fmt;

use crate::data_type::{parse_data_type, DataType};

/// Default catalog and schema context applied to one configured SQL input.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationContext {
    default_catalog: Option<ParsedIdentifier>,
    default_schema: Option<ParsedIdentifier>,
}

impl RelationContext {
    /// Construct validated default catalog and schema context.
    ///
    /// Each default is one SQL identifier. Quote it when case must be preserved exactly.
    pub fn new(
        default_catalog: Option<&str>,
        default_schema: Option<&str>,
    ) -> Result<Self, RelationMetadataError> {
        Ok(Self {
            default_catalog: parse_optional_identifier("default_catalog", default_catalog)?,
            default_schema: parse_optional_identifier("default_schema", default_schema)?,
        })
    }

    /// Return the configured default catalog using deterministic SQL identifier rendering.
    pub fn default_catalog(&self) -> Option<String> {
        self.default_catalog.as_ref().map(ParsedIdentifier::render)
    }

    /// Return the configured default schema using deterministic SQL identifier rendering.
    pub fn default_schema(&self) -> Option<String> {
        self.default_schema.as_ref().map(ParsedIdentifier::render)
    }
}

/// Parser-independent contract for resolving textual SQL relation references.
///
/// Implementations may use caller-owned metadata, but must return deterministic canonical relation
/// identities and fail explicitly when a reference cannot be resolved safely.
pub trait RelationResolver {
    /// Resolve one relation reference in the context of its SQL dialect and optional input defaults.
    fn resolve_relation(
        &self,
        reference: &str,
        dialect_name: &str,
        context: Option<&RelationContext>,
    ) -> Result<String, RelationResolutionError>;
}

/// Provenance category for a typed relation schema.
///
/// Provenance is metadata evidence only. It does not change the canonical datatype semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaSourceKind {
    /// Warehouse-introspected schema from a dbt `catalog.json` artifact.
    DbtCatalog,
    /// Declared schema from dbt manifest metadata, typically originating in project YAML.
    DbtManifest,
}

impl SchemaSourceKind {
    /// Return the stable protocol representation of this provenance category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DbtCatalog => "dbt_catalog",
            Self::DbtManifest => "dbt_manifest",
        }
    }
}

/// One typed column in caller-supplied relation schema metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaColumn {
    name: String,
    data_type: DataType,
}

impl SchemaColumn {
    /// Construct a validated schema column from an already normalized datatype.
    pub fn new(
        name: impl Into<String>,
        data_type: DataType,
    ) -> Result<Self, RelationMetadataError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(RelationMetadataError::InvalidSchema {
                relation: String::new(),
                message: "column name cannot be empty".to_string(),
            });
        }

        Ok(Self { name, data_type })
    }

    /// Construct a schema column from dialect-specific SQL datatype syntax.
    ///
    /// The syntax is normalized immediately into the parser-independent protocol datatype model.
    pub fn from_sql_type(
        name: impl Into<String>,
        sql_type: &str,
        dialect_name: &str,
    ) -> Result<Self, RelationMetadataError> {
        let data_type = parse_data_type(sql_type, dialect_name).map_err(|error| {
            RelationMetadataError::InvalidSchema {
                relation: String::new(),
                message: error.to_string(),
            }
        })?;
        Self::new(name, data_type)
    }

    /// Return the column name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the canonical declared datatype.
    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

/// Declared typed schema for one canonical source relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSchema {
    relation: String,
    columns: Vec<SchemaColumn>,
    source_kind: Option<SchemaSourceKind>,
}

impl RelationSchema {
    /// Construct a validated relation schema.
    pub fn new(
        relation: impl Into<String>,
        columns: Vec<SchemaColumn>,
    ) -> Result<Self, RelationMetadataError> {
        let relation = relation.into();
        let canonical = relation.trim();
        if canonical.is_empty() {
            return Err(RelationMetadataError::InvalidSchema {
                relation,
                message: "relation cannot be empty".to_string(),
            });
        }
        parse_relation(canonical).map_err(|message| RelationMetadataError::InvalidSchema {
            relation: canonical.to_string(),
            message,
        })?;

        let mut names = BTreeSet::new();
        for column in &columns {
            if !names.insert(column.name().to_string()) {
                return Err(RelationMetadataError::InvalidSchema {
                    relation: canonical.to_string(),
                    message: format!("duplicate column '{}'", column.name()),
                });
            }
        }

        Ok(Self {
            relation: canonical.to_string(),
            columns,
            source_kind: None,
        })
    }

    /// Attach adapter provenance to this schema.
    pub fn with_source_kind(mut self, source_kind: SchemaSourceKind) -> Self {
        self.source_kind = Some(source_kind);
        self
    }

    /// Return the provenance category for this schema when an adapter supplied one.
    pub fn source_kind(&self) -> Option<SchemaSourceKind> {
        self.source_kind
    }

    /// Return the canonical relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return columns in caller-declared order.
    pub fn columns(&self) -> &[SchemaColumn] {
        &self.columns
    }
}

/// Optional set of known canonical relations used to disambiguate textual references.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationCatalog {
    relations: Vec<CatalogRelation>,
    schemas: Vec<RelationSchema>,
}

impl RelationCatalog {
    /// Construct a validated catalog from canonical SQL relation identifiers.
    ///
    /// Relation names may be partially or fully qualified. Quoted identifier parts preserve exact
    /// case while unquoted parts follow the selected input dialect's identifier normalization.
    pub fn new(relations: &[&str]) -> Result<Self, RelationMetadataError> {
        let mut unique = BTreeSet::new();
        let mut parsed = Vec::new();

        for relation in relations {
            let canonical = relation.trim();
            if canonical.is_empty() {
                return Err(RelationMetadataError::InvalidRelation {
                    relation: relation.to_string(),
                    message: "relation cannot be empty".to_string(),
                });
            }
            if !unique.insert(canonical.to_string()) {
                continue;
            }

            let relation_parts = parse_relation(canonical).map_err(|message| {
                RelationMetadataError::InvalidRelation {
                    relation: canonical.to_string(),
                    message,
                }
            })?;
            parsed.push(CatalogRelation {
                canonical: canonical.to_string(),
                relation: relation_parts,
            });
        }

        parsed.sort_by(|left, right| left.canonical.cmp(&right.canonical));
        Ok(Self {
            relations: parsed,
            schemas: Vec::new(),
        })
    }

    /// Construct a catalog from typed relation schemas.
    ///
    /// Schema relation identities participate in the same canonical relation resolution as names
    /// supplied to `RelationCatalog::new`.
    pub fn from_schemas(schemas: &[RelationSchema]) -> Result<Self, RelationMetadataError> {
        let relation_names = schemas
            .iter()
            .map(RelationSchema::relation)
            .collect::<Vec<_>>();
        Self::from_relations_and_schemas(&relation_names, schemas)
    }

    /// Construct a catalog from relation identities plus optional typed schemas.
    ///
    /// This is useful for metadata sources such as dbt where every relation identity is known but
    /// typed column evidence may only be available for a subset of resources.
    pub fn from_relations_and_schemas(
        relations: &[&str],
        schemas: &[RelationSchema],
    ) -> Result<Self, RelationMetadataError> {
        let mut catalog = Self::new(relations)?;
        let relation_names = catalog
            .relations
            .iter()
            .map(|relation| relation.canonical.as_str())
            .collect::<BTreeSet<_>>();
        let mut unique = BTreeSet::new();

        for schema in schemas {
            if !unique.insert(schema.relation().to_string()) {
                return Err(RelationMetadataError::InvalidSchema {
                    relation: schema.relation().to_string(),
                    message: "relation schema is duplicated".to_string(),
                });
            }
            if !relation_names.contains(schema.relation()) {
                return Err(RelationMetadataError::InvalidSchema {
                    relation: schema.relation().to_string(),
                    message: "schema relation is not present in catalog relations".to_string(),
                });
            }
        }

        catalog.schemas = schemas.to_vec();
        catalog
            .schemas
            .sort_by(|left, right| left.relation().cmp(right.relation()));
        Ok(catalog)
    }

    /// Return typed relation schemas in deterministic relation order.
    pub fn schemas(&self) -> &[RelationSchema] {
        &self.schemas
    }

    /// Return canonical catalog relation names in deterministic order.
    pub fn relation_names(&self) -> Vec<&str> {
        self.relations
            .iter()
            .map(|relation| relation.canonical.as_str())
            .collect()
    }

    /// Resolve one textual relation reference to a canonical identity.
    ///
    /// The resolver first applies explicit input defaults where they supply missing qualification.
    /// When no defaults are available, a unique catalog suffix match may resolve a partial name.
    /// Missing catalog metadata falls back to the original textual identity. Ambiguous matches are
    /// returned as errors instead of selecting an arbitrary relation.
    pub fn resolve(
        &self,
        reference: &str,
        dialect_name: &str,
        context: Option<&RelationContext>,
    ) -> Result<String, RelationResolutionError> {
        if self.relations.is_empty() && context.is_none() {
            return Ok(reference.to_string());
        }

        let parsed = parse_relation(reference).map_err(|message| {
            RelationResolutionError::InvalidReference {
                reference: reference.to_string(),
                message,
            }
        })?;

        let qualified = qualify_with_context(&parsed, context);
        let context_changed = qualified != parsed;

        if context_changed {
            let matches = self.exact_matches(&qualified, dialect_name);
            return match matches.as_slice() {
                [] => Ok(qualified.render()),
                [relation] => Ok(relation.canonical.clone()),
                _ => Err(ambiguous(reference, matches)),
            };
        }

        let matches = self.suffix_matches(&parsed, dialect_name);
        match matches.as_slice() {
            [] => Ok(reference.to_string()),
            [relation] => Ok(relation.canonical.clone()),
            _ => Err(ambiguous(reference, matches)),
        }
    }

    fn exact_matches<'a>(
        &'a self,
        relation: &ParsedRelation,
        dialect_name: &str,
    ) -> Vec<&'a CatalogRelation> {
        let key = relation.normalized_parts(dialect_name);
        self.relations
            .iter()
            .filter(|candidate| candidate.relation.normalized_parts(dialect_name) == key)
            .collect()
    }

    fn suffix_matches<'a>(
        &'a self,
        relation: &ParsedRelation,
        dialect_name: &str,
    ) -> Vec<&'a CatalogRelation> {
        let key = relation.normalized_parts(dialect_name);
        self.relations
            .iter()
            .filter(|candidate| {
                let candidate_key = candidate.relation.normalized_parts(dialect_name);
                candidate_key.len() >= key.len()
                    && candidate_key[candidate_key.len() - key.len()..] == key
            })
            .collect()
    }
}

impl RelationResolver for RelationCatalog {
    fn resolve_relation(
        &self,
        reference: &str,
        dialect_name: &str,
        context: Option<&RelationContext>,
    ) -> Result<String, RelationResolutionError> {
        self.resolve(reference, dialect_name, context)
    }
}

/// Invalid caller-supplied relation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelationMetadataError {
    /// A canonical catalog relation could not be parsed safely.
    InvalidRelation {
        /// Original relation value.
        relation: String,
        /// Explanation of the invalid syntax.
        message: String,
    },
    /// Caller-supplied typed schema metadata is invalid.
    InvalidSchema {
        /// Relation associated with the invalid schema when available.
        relation: String,
        /// Explanation of the invalid schema.
        message: String,
    },
    /// A default catalog or schema value was not exactly one valid identifier.
    InvalidDefault {
        /// Configuration field name.
        field: String,
        /// Original configured value.
        value: String,
        /// Explanation of the invalid syntax.
        message: String,
    },
}

impl fmt::Display for RelationMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelation { relation, message } => {
                write!(
                    formatter,
                    "invalid catalog relation '{relation}': {message}"
                )
            }
            Self::InvalidSchema { relation, message } => {
                if relation.is_empty() {
                    write!(formatter, "invalid relation schema: {message}")
                } else {
                    write!(formatter, "invalid relation schema '{relation}': {message}")
                }
            }
            Self::InvalidDefault {
                field,
                value,
                message,
            } => write!(
                formatter,
                "invalid relation context {field} '{value}': {message}"
            ),
        }
    }
}

impl std::error::Error for RelationMetadataError {}

/// Failure to resolve one SQL relation against supplied metadata safely.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelationResolutionError {
    /// The analyzer produced a textual reference the resolver could not parse safely.
    InvalidReference {
        /// Relation reference being resolved.
        reference: String,
        /// Explanation of the invalid syntax.
        message: String,
    },
    /// More than one canonical catalog relation matches the reference.
    Ambiguous {
        /// Relation reference being resolved.
        reference: String,
        /// Matching canonical relation names in deterministic order.
        candidates: Vec<String>,
    },
}

impl RelationResolutionError {
    /// Return the relation reference that failed resolution.
    pub fn reference(&self) -> &str {
        match self {
            Self::InvalidReference { reference, .. } | Self::Ambiguous { reference, .. } => {
                reference
            }
        }
    }

    /// Return matching canonical candidates when resolution was ambiguous.
    pub fn candidates(&self) -> &[String] {
        match self {
            Self::InvalidReference { .. } => &[],
            Self::Ambiguous { candidates, .. } => candidates,
        }
    }
}

impl fmt::Display for RelationResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReference { reference, message } => {
                write!(
                    formatter,
                    "cannot resolve relation '{reference}': {message}"
                )
            }
            Self::Ambiguous {
                reference,
                candidates,
            } => write!(
                formatter,
                "relation '{reference}' is ambiguous in catalog metadata; matches {}",
                candidates.join(", ")
            ),
        }
    }
}

impl std::error::Error for RelationResolutionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogRelation {
    canonical: String,
    relation: ParsedRelation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRelation {
    parts: Vec<ParsedIdentifier>,
}

impl ParsedRelation {
    fn render(&self) -> String {
        self.parts
            .iter()
            .map(ParsedIdentifier::render)
            .collect::<Vec<_>>()
            .join(".")
    }

    fn normalized_parts(&self, dialect_name: &str) -> Vec<String> {
        self.parts
            .iter()
            .map(|part| part.normalized(dialect_name))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedIdentifier {
    value: String,
    quote: Option<QuoteStyle>,
}

impl ParsedIdentifier {
    fn render(&self) -> String {
        match self.quote {
            None => self.value.clone(),
            Some(QuoteStyle::Double) => {
                format!("\"{}\"", self.value.replace('"', "\"\""))
            }
            Some(QuoteStyle::Backtick) => {
                format!("`{}`", self.value.replace('`', "``"))
            }
            Some(QuoteStyle::Bracket) => {
                format!("[{}]", self.value.replace(']', "]]"))
            }
        }
    }

    fn normalized(&self, dialect_name: &str) -> String {
        if self.quote.is_some() {
            return self.value.clone();
        }

        match dialect_name.to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" | "redshift" => self.value.to_ascii_lowercase(),
            "ansi" | "snowflake" => self.value.to_ascii_uppercase(),
            _ => self.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteStyle {
    Double,
    Backtick,
    Bracket,
}

fn parse_optional_identifier(
    field: &str,
    value: Option<&str>,
) -> Result<Option<ParsedIdentifier>, RelationMetadataError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed =
        parse_relation(value).map_err(|message| RelationMetadataError::InvalidDefault {
            field: field.to_string(),
            value: value.to_string(),
            message,
        })?;
    let [identifier] = parsed.parts.as_slice() else {
        return Err(RelationMetadataError::InvalidDefault {
            field: field.to_string(),
            value: value.to_string(),
            message: "value must contain exactly one identifier".to_string(),
        });
    };
    Ok(Some(identifier.clone()))
}

fn qualify_with_context(
    relation: &ParsedRelation,
    context: Option<&RelationContext>,
) -> ParsedRelation {
    let Some(context) = context else {
        return relation.clone();
    };

    let mut parts = relation.parts.clone();
    match parts.len() {
        1 => {
            let mut qualified = Vec::new();
            if let Some(catalog) = &context.default_catalog {
                qualified.push(catalog.clone());
            }
            if let Some(schema) = &context.default_schema {
                qualified.push(schema.clone());
            }
            qualified.extend(parts);
            parts = qualified;
        }
        2 => {
            if let Some(catalog) = &context.default_catalog {
                let mut qualified = vec![catalog.clone()];
                qualified.extend(parts);
                parts = qualified;
            }
        }
        _ => {}
    }

    ParsedRelation { parts }
}

fn ambiguous(reference: &str, matches: Vec<&CatalogRelation>) -> RelationResolutionError {
    RelationResolutionError::Ambiguous {
        reference: reference.to_string(),
        candidates: matches
            .into_iter()
            .map(|relation| relation.canonical.clone())
            .collect(),
    }
}

fn parse_relation(input: &str) -> Result<ParsedRelation, String> {
    let chars = input.trim().chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return Err("relation cannot be empty".to_string());
    }

    let mut parts = Vec::new();
    let mut index = 0_usize;

    while index < chars.len() {
        skip_whitespace(&chars, &mut index);
        if index >= chars.len() {
            return Err("relation cannot end with '.'".to_string());
        }

        let identifier = match chars[index] {
            '"' => parse_quoted_identifier(&chars, &mut index, '"', QuoteStyle::Double)?,
            '`' => parse_quoted_identifier(&chars, &mut index, '`', QuoteStyle::Backtick)?,
            '[' => parse_bracket_identifier(&chars, &mut index)?,
            _ => parse_unquoted_identifier(&chars, &mut index)?,
        };
        parts.push(identifier);

        skip_whitespace(&chars, &mut index);
        if index >= chars.len() {
            break;
        }
        if chars[index] != '.' {
            return Err(format!(
                "unexpected character '{}' after identifier",
                chars[index]
            ));
        }
        index += 1;
        if index >= chars.len() {
            return Err("relation cannot end with '.'".to_string());
        }
    }

    Ok(ParsedRelation { parts })
}

fn skip_whitespace(chars: &[char], index: &mut usize) {
    while *index < chars.len() && chars[*index].is_whitespace() {
        *index += 1;
    }
}

fn parse_unquoted_identifier(
    chars: &[char],
    index: &mut usize,
) -> Result<ParsedIdentifier, String> {
    let start = *index;
    while *index < chars.len() && chars[*index] != '.' {
        *index += 1;
    }

    let raw = chars[start..*index].iter().collect::<String>();
    let value = raw.trim();
    if value.is_empty() {
        return Err("identifier cannot be empty".to_string());
    }
    if value.chars().any(char::is_whitespace) {
        return Err(format!("unquoted identifier '{value}' contains whitespace"));
    }

    Ok(ParsedIdentifier {
        value: value.to_string(),
        quote: None,
    })
}

fn parse_quoted_identifier(
    chars: &[char],
    index: &mut usize,
    quote: char,
    style: QuoteStyle,
) -> Result<ParsedIdentifier, String> {
    *index += 1;
    let mut value = String::new();

    while *index < chars.len() {
        let current = chars[*index];
        if current == quote {
            if *index + 1 < chars.len() && chars[*index + 1] == quote {
                value.push(quote);
                *index += 2;
                continue;
            }
            *index += 1;
            if value.is_empty() {
                return Err("quoted identifier cannot be empty".to_string());
            }
            return Ok(ParsedIdentifier {
                value,
                quote: Some(style),
            });
        }
        value.push(current);
        *index += 1;
    }

    Err("unterminated quoted identifier".to_string())
}

fn parse_bracket_identifier(chars: &[char], index: &mut usize) -> Result<ParsedIdentifier, String> {
    *index += 1;
    let mut value = String::new();

    while *index < chars.len() {
        let current = chars[*index];
        if current == ']' {
            if *index + 1 < chars.len() && chars[*index + 1] == ']' {
                value.push(']');
                *index += 2;
                continue;
            }
            *index += 1;
            if value.is_empty() {
                return Err("quoted identifier cannot be empty".to_string());
            }
            return Ok(ParsedIdentifier {
                value,
                quote: Some(QuoteStyle::Bracket),
            });
        }
        value.push(current);
        *index += 1;
    }

    Err("unterminated bracket identifier".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_unquoted_identifiers_fold_lowercase_but_quotes_do_not() {
        let catalog =
            RelationCatalog::new(&["warehouse.public.orders", "warehouse.public.\"Orders\""])
                .expect("catalog should be valid");
        let context = RelationContext::new(Some("warehouse"), Some("public"))
            .expect("context should be valid");

        assert_eq!(
            catalog
                .resolve("ORDERS", "postgresql", Some(&context))
                .expect("unquoted relation should resolve"),
            "warehouse.public.orders"
        );
        assert_eq!(
            catalog
                .resolve("\"Orders\"", "postgresql", Some(&context))
                .expect("quoted relation should resolve"),
            "warehouse.public.\"Orders\""
        );
    }

    #[test]
    fn snowflake_unquoted_identifiers_fold_uppercase() {
        let catalog =
            RelationCatalog::new(&["WAREHOUSE.PUBLIC.ORDERS"]).expect("catalog should be valid");
        let context = RelationContext::new(Some("warehouse"), Some("public"))
            .expect("context should be valid");

        assert_eq!(
            catalog
                .resolve("orders", "snowflake", Some(&context))
                .expect("relation should resolve"),
            "WAREHOUSE.PUBLIC.ORDERS"
        );
    }

    #[test]
    fn partial_name_without_context_requires_unique_catalog_match() {
        let catalog = RelationCatalog::new(&["warehouse.sales.orders", "warehouse.finance.orders"])
            .expect("catalog should be valid");

        let error = catalog
            .resolve("orders", "postgresql", None)
            .expect_err("duplicate suffix should be ambiguous");
        assert_eq!(
            error.candidates(),
            &[
                "warehouse.finance.orders".to_string(),
                "warehouse.sales.orders".to_string()
            ]
        );
    }
}
