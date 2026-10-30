//! Canonical, parser-independent, bounded ordinary CTE producer graph.
//!
//! Stage identities are lexical, never a table name alone. Reusing a CTE
//! references one producer bag through independent SQL source instances.

/// A complete ordinary CTE program in producer-first order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteProgram {
    stages: Vec<CteStage>,
    terminal: String,
}

impl CteProgram {
    pub(crate) fn new(stages: Vec<CteStage>, terminal: String) -> Self {
        Self { stages, terminal }
    }

    /// Producer-first stages, including the terminal SELECT.
    pub fn stages(&self) -> &[CteStage] {
        &self.stages
    }

    /// Lexically unique terminal stage identity.
    pub fn terminal(&self) -> &str {
        &self.terminal
    }
}

/// One normalized row-preserving projection or integer equality join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteStage {
    id: String,
    inputs: Vec<CteInput>,
    projections: Vec<CteProjection>,
    equality: Option<(CteColumnRef, CteColumnRef)>,
}

impl CteStage {
    pub(crate) fn new(
        id: String,
        inputs: Vec<CteInput>,
        projections: Vec<CteProjection>,
        equality: Option<(CteColumnRef, CteColumnRef)>,
    ) -> Self {
        Self { id, inputs, projections, equality }
    }

    /// Stable lexical stage identity, even when names are shadowed.
    pub fn id(&self) -> &str { &self.id }

    /// Source instances. Multiple instances may reference the same producer.
    pub fn inputs(&self) -> &[CteInput] { &self.inputs }

    /// Ordered direct-column copies with their output labels.
    pub fn projections(&self) -> &[CteProjection] { &self.projections }

    /// Optional typed input-column equality, evaluated with SQL three-valued truth.
    pub fn equality(&self) -> Option<(&CteColumnRef, &CteColumnRef)> {
        self.equality.as_ref().map(|(a, b)| (a, b))
    }
}

/// One SQL FROM instance, bound to a physical leaf or a lexical CTE stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteInput {
    relation: String,
    alias: String,
}

impl CteInput {
    pub(crate) fn new(relation: String, alias: String) -> Self {
        Self { relation, alias }
    }

    /// Canonical source or lexical producer identity.
    pub fn relation(&self) -> &str { &self.relation }

    /// Local SQL instance identity.
    pub fn alias(&self) -> &str { &self.alias }
}

/// Unambiguous single-column reference inside one stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteColumnRef {
    relation: Option<String>,
    column: String,
}

impl CteColumnRef {
    pub(crate) fn new(relation: Option<String>, column: String) -> Self {
        Self { relation, column }
    }

    /// Explicit SQL instance qualifier, if supplied.
    pub fn relation(&self) -> Option<&str> { self.relation.as_deref() }

    /// Input column label.
    pub fn column(&self) -> &str { &self.column }
}

/// Direct projection into the downstream producer bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteProjection {
    name: String,
    source: CteColumnRef,
}

impl CteProjection {
    pub(crate) fn new(name: String, source: CteColumnRef) -> Self {
        Self { name, source }
    }

    /// Output column label.
    pub fn name(&self) -> &str { &self.name }

    /// Exact originating input-column expression.
    pub fn source(&self) -> &CteColumnRef { &self.source }
}
