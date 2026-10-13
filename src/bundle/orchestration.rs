//! Input analysis entry points and error boundaries.

use super::*;

/// Error produced while parsing or analyzing one input in a multi-input invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputAnalysisError {
    input_id: String,
    source: SqlInputSource,
    error: Error,
}

impl InputAnalysisError {
    /// Return the deterministic identifier of the input that failed.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Return the source identity of the input that failed.
    pub fn input_source(&self) -> &SqlInputSource {
        &self.source
    }

    /// Return the underlying parse or analysis error.
    pub fn error(&self) -> &Error {
        &self.error
    }
}

impl fmt::Display for InputAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            SqlInputSource::Inline => {
                write!(
                    formatter,
                    "input {} (inline): {}",
                    self.input_id, self.error
                )
            }
            SqlInputSource::File { path } => write!(
                formatter,
                "input {} (file '{}'): {}",
                self.input_id, path, self.error
            ),
        }
    }
}

impl std::error::Error for InputAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Error produced before or during configured multi-input analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfiguredInputAnalysisError {
    /// A configured input identifier is empty or whitespace-only.
    InvalidInputId {
        /// One-based input position in caller order.
        position: usize,
    },
    /// More than one configured input uses the same stable identity.
    DuplicateInputId {
        /// Duplicated stable input identifier.
        id: String,
    },
    /// SQL parsing or semantic analysis failed for one configured input.
    Input(InputAnalysisError),
    /// Caller-supplied relation metadata could not resolve one relation safely.
    RelationResolution {
        /// Stable input identifier whose relation could not be resolved.
        input_id: String,
        /// Relation-resolution failure.
        error: RelationResolutionError,
    },
}

impl ConfiguredInputAnalysisError {
    /// Return the underlying per-input analysis error when SQL processing failed.
    pub fn input_error(&self) -> Option<&InputAnalysisError> {
        match self {
            Self::Input(error) => Some(error),
            Self::InvalidInputId { .. }
            | Self::DuplicateInputId { .. }
            | Self::RelationResolution { .. } => None,
        }
    }
}

impl fmt::Display for ConfiguredInputAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInputId { position } => {
                write!(formatter, "configured input {position} has an empty id")
            }
            Self::DuplicateInputId { id } => {
                write!(formatter, "configured input id '{id}' is duplicated")
            }
            Self::Input(error) => write!(formatter, "{error}"),
            Self::RelationResolution { input_id, error } => {
                write!(formatter, "configured input '{input_id}': {error}")
            }
        }
    }
}

impl std::error::Error for ConfiguredInputAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::RelationResolution { error, .. } => Some(error),
            Self::InvalidInputId { .. } | Self::DuplicateInputId { .. } => None,
        }
    }
}

pub(super) fn analyze_input(
    input_id: String,
    input: &SqlInput,
    dialect_name: &str,
    dialect: &dyn Dialect,
    relation_context: Option<RelationContext>,
    catalog: Option<&RelationCatalog>,
) -> Result<AnalyzedInput, InputAnalysisError> {
    let protocol = match catalog {
        Some(catalog) => analyze_sql_with_catalog(
            input.sql(),
            dialect_name,
            dialect,
            catalog,
            relation_context.as_ref(),
        ),
        None => analyze_sql(input.sql(), dialect_name, dialect),
    }
    .map_err(|error| InputAnalysisError {
        input_id: input_id.clone(),
        source: input.source().clone(),
        error,
    })?;

    Ok(AnalyzedInput {
        id: input_id,
        source: input.source().clone(),
        dialect: dialect_name.to_string(),
        relation_context,
        statements: protocol.statements().to_vec(),
        relation_constraints: protocol.relation_constraints().to_vec(),
        source_schemas: protocol.source_schemas().to_vec(),
    })
}

/// Parse and analyze an arbitrary number of SQL inputs using one caller-selected dialect.
///
/// Inputs are analyzed in caller order. Generated IDs start at `input-0001`; the numeric width
/// expands when necessary rather than imposing a maximum input count.
pub fn analyze_inputs(
    inputs: &[SqlInput],
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<AnalysisBundle, InputAnalysisError> {
    let width = inputs.len().max(1).to_string().len().max(4);
    let mut analyzed_inputs = Vec::with_capacity(inputs.len());

    for (index, input) in inputs.iter().enumerate() {
        let input_id = format!("input-{:0width$}", index + 1, width = width);
        analyzed_inputs.push(analyze_input(
            input_id,
            input,
            dialect_name,
            dialect,
            None,
            None,
        )?);
    }

    Ok(AnalysisBundle::from_inputs(analyzed_inputs))
}

/// Analyze inputs that each carry an explicit stable identity and dialect.
///
/// Input order remains significant for deterministic layer ordering. Identifiers must be non-empty
/// and unique. Dialect implementations are supplied by the caller so sqlparser remains confined to
/// the parsing boundary.
pub fn analyze_configured_inputs(
    inputs: &[ConfiguredSqlInput<'_>],
) -> Result<AnalysisBundle, ConfiguredInputAnalysisError> {
    let catalog = RelationCatalog::default();
    analyze_configured_inputs_with_catalog(inputs, &catalog)
}

/// Analyze configured inputs with optional canonical catalog metadata.
///
/// Per-input default catalog/schema context is taken from each `ConfiguredSqlInput`. The catalog
/// can additionally resolve unique partially qualified names and detect ambiguous identities.
pub fn analyze_configured_inputs_with_catalog(
    inputs: &[ConfiguredSqlInput<'_>],
    catalog: &RelationCatalog,
) -> Result<AnalysisBundle, ConfiguredInputAnalysisError> {
    let mut seen_ids = BTreeSet::new();
    let mut analyzed_inputs = Vec::with_capacity(inputs.len());

    for (index, configured) in inputs.iter().enumerate() {
        let id = configured.id().trim();
        if id.is_empty() {
            return Err(ConfiguredInputAnalysisError::InvalidInputId {
                position: index + 1,
            });
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ConfiguredInputAnalysisError::DuplicateInputId { id: id.to_string() });
        }

        let analyzed = analyze_input(
            id.to_string(),
            configured.input(),
            configured.dialect_name(),
            configured.dialect(),
            configured.relation_context().cloned(),
            Some(catalog),
        )
        .map_err(ConfiguredInputAnalysisError::Input)?;
        analyzed_inputs.push(analyzed);
    }

    let mut bundle =
        AnalysisBundle::from_inputs_with_resolver(analyzed_inputs, catalog).map_err(|failure| {
            ConfiguredInputAnalysisError::RelationResolution {
                input_id: failure.input_id,
                error: failure.error,
            }
        })?;
    let mut schemas = bundle.source_schemas().to_vec();
    for declared in catalog.schemas() {
        if let Some(position) = schemas.iter().position(|schema| schema.relation() == declared.relation()) {
            if schemas[position].columns() != declared.columns() {
                let mut diagnostic = RelationConstraintSet::new(declared.relation(), vec![])
                    .expect("catalog relation is validated");
                diagnostic.add_diagnostic(ConstraintDiagnostic::new(
                    "ddl_catalog_schema_conflict",
                    format!("SQL DDL and caller catalog disagree on typed schema for '{}'", declared.relation()),
                ));
                bundle.enrich_relation_constraints(&[diagnostic]);
            }
            // Explicit catalog metadata is preferred over a parser-derived declaration;
            // contradictory evidence stays visible and blocks physical proofs.
            schemas[position] = declared.clone();
        } else {
            schemas.push(declared.clone());
        }
    }
    bundle.replace_source_schemas(schemas);
    Ok(bundle)
}

/// Analyze configured inputs with a caller-supplied relation resolver implementation.
///
/// This is the integration boundary for database catalogs or other metadata providers. Resolver
/// implementations remain independent of sqlparser AST types and may canonicalize relation
/// identity only from the textual SQL reference plus explicitly supplied metadata.
pub fn analyze_configured_inputs_with_resolver(
    inputs: &[ConfiguredSqlInput<'_>],
    resolver: &dyn RelationResolver,
) -> Result<AnalysisBundle, ConfiguredInputAnalysisError> {
    let mut seen_ids = BTreeSet::new();
    let mut analyzed_inputs = Vec::with_capacity(inputs.len());

    for (index, configured) in inputs.iter().enumerate() {
        let id = configured.id().trim();
        if id.is_empty() {
            return Err(ConfiguredInputAnalysisError::InvalidInputId {
                position: index + 1,
            });
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ConfiguredInputAnalysisError::DuplicateInputId { id: id.to_string() });
        }

        let analyzed = analyze_input(
            id.to_string(),
            configured.input(),
            configured.dialect_name(),
            configured.dialect(),
            configured.relation_context().cloned(),
            None,
        )
        .map_err(ConfiguredInputAnalysisError::Input)?;
        analyzed_inputs.push(analyzed);
    }

    AnalysisBundle::from_inputs_with_resolver(analyzed_inputs, resolver).map_err(|failure| {
        ConfiguredInputAnalysisError::RelationResolution {
            input_id: failure.input_id,
            error: failure.error,
        }
    })
}
