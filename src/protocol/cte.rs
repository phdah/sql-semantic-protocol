//! Canonical, parser-independent, bounded ordinary CTE producer graph.
//!
//! Stage identities are lexical, never a table name alone. Reusing a CTE
//! references one producer bag through independent SQL source instances.

/// UNION semantics for a certified finite recursive CTE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CteRecursionSetMode {
    /// Retain all generated occurrences.
    All,
    /// Deduplicate generated occurrences. A positive step already guarantees uniqueness.
    Distinct,
}

/// A source-free, single-column, monotonically increasing recursive CTE.
///
/// The guarded step advances by a positive integer, so its reachable values
/// are finite under a checked integer bound. No SQL AST escapes this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteRecursion {
    column: String,
    seed: i128,
    increment: i128,
    bound: i128,
    inclusive: bool,
    set_mode: CteRecursionSetMode,
}

impl CteRecursion {
    pub(crate) fn new(
        column: String,
        seed: i128,
        increment: i128,
        bound: i128,
        inclusive: bool,
        set_mode: CteRecursionSetMode,
    ) -> Option<Self> {
        // Recursive SQL integer literals are not a license to assume an
        // arbitrary-precision engine. Restrict the portable certificate to
        // the common signed 32-bit range, including the next step value.
        (increment > 0
            && i32::try_from(seed).is_ok()
            && i32::try_from(increment).is_ok()
            && i32::try_from(bound).is_ok())
        .then_some(Self {
            column,
            seed,
            increment,
            bound,
            inclusive,
            set_mode,
        })
    }

    /// Complete finite integer sequence when all checked SQL step values fit
    /// the portable width and materialization limit. An unsupported or
    /// unfinished induction must not be represented as a truncated result.
    pub(crate) fn finite_values(&self) -> Option<Vec<i128>> {
        const MAX_ROWS: usize = 10_000;
        let mut current = self.seed;
        let mut values = Vec::new();
        for _ in 0..MAX_ROWS {
            values.push(i128::from(i32::try_from(current).ok()?));
            if current > self.bound || (!self.inclusive && current == self.bound) {
                return Some(values);
            }
            current = current.checked_add(self.increment)?;
        }
        None
    }

    /// Output column identity shared by the base and recursive steps.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Integer emitted by the nonrecursive seed.
    pub fn seed(&self) -> i128 {
        self.seed
    }

    /// Strictly positive step increment.
    pub fn increment(&self) -> i128 {
        self.increment
    }

    /// Comparison bound applied to each previous iteration's row.
    pub fn bound(&self) -> i128 {
        self.bound
    }

    /// Whether the step predicate is <= instead of <.
    pub fn inclusive(&self) -> bool {
        self.inclusive
    }

    /// Recursive UNION mode.
    pub fn set_mode(&self) -> CteRecursionSetMode {
        self.set_mode
    }
}

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

/// Parser-independent comparison for a single integer CTE filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CteComparisonOperator {
    /// SQL equality, with NULL yielding UNKNOWN.
    Equal,
    /// Strict greater-than.
    GreaterThan,
    /// Inclusive greater-than.
    GreaterOrEqual,
    /// Strict less-than.
    LessThan,
    /// Inclusive less-than.
    LessOrEqual,
}

/// One typed predicate over a source/producer column and signed integer bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteFilter {
    column: CteColumnRef,
    operator: CteComparisonOperator,
    bound: i128,
}

impl CteFilter {
    pub(crate) fn new(column: CteColumnRef, operator: CteComparisonOperator, bound: i128) -> Self {
        Self { column, operator, bound }
    }

    /// Column identity within the SELECT's source scope.
    pub fn column(&self) -> &CteColumnRef {
        &self.column
    }

    /// Typed comparison used to partition TRUE from FALSE and UNKNOWN.
    pub fn operator(&self) -> CteComparisonOperator {
        self.operator
    }

    /// Literal integer comparison bound.
    pub fn bound(&self) -> i128 {
        self.bound
    }
}

/// One normalized row-preserving projection or integer equality join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteStage {
    id: String,
    inputs: Vec<CteInput>,
    projections: Vec<CteProjection>,
    equality: Option<(CteColumnRef, CteColumnRef)>,
    filter: Option<CteFilter>,
    recursion: Option<CteRecursion>,
}

impl CteStage {
    pub(crate) fn new(
        id: String,
        inputs: Vec<CteInput>,
        projections: Vec<CteProjection>,
        equality: Option<(CteColumnRef, CteColumnRef)>,
        filter: Option<CteFilter>,
    ) -> Self {
        Self {
            id,
            inputs,
            projections,
            equality,
            filter,
            recursion: None,
        }
    }

    pub(crate) fn recursive(id: String, alias: String, recursion: CteRecursion) -> Self {
        Self {
            inputs: vec![CteInput::new(id.clone(), alias)],
            id,
            projections: Vec::new(),
            equality: None,
            filter: None,
            recursion: Some(recursion),
        }
    }

    /// Certified monotone recursive operation, if this stage is recursive.
    pub fn recursion(&self) -> Option<&CteRecursion> {
        self.recursion.as_ref()
    }

    /// Stable lexical stage identity, even when names are shadowed.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Source instances. Multiple instances may reference the same producer.
    pub fn inputs(&self) -> &[CteInput] {
        &self.inputs
    }

    /// Ordered direct-column copies with their output labels.
    pub fn projections(&self) -> &[CteProjection] {
        &self.projections
    }

    /// Optional typed input-column equality, evaluated with SQL three-valued truth.
    pub fn equality(&self) -> Option<(&CteColumnRef, &CteColumnRef)> {
        self.equality.as_ref().map(|(a, b)| (a, b))
    }

    /// Optional row filter, applied after JOIN and before projection.
    pub fn filter(&self) -> Option<&CteFilter> {
        self.filter.as_ref()
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
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Local SQL instance identity.
    pub fn alias(&self) -> &str {
        &self.alias
    }
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
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    /// Input column label.
    pub fn column(&self) -> &str {
        &self.column
    }
}

/// Direct projection into the downstream producer bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CteProjection {
    name: String,
    source: CteColumnRef,
    offset: i128,
}

impl CteProjection {
    pub(crate) fn new(name: String, source: CteColumnRef) -> Self {
        Self { name, source, offset: 0 }
    }

    pub(crate) fn with_offset(name: String, source: CteColumnRef, offset: i128) -> Self {
        Self { name, source, offset }
    }

    /// Output column label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact originating input-column expression.
    pub fn source(&self) -> &CteColumnRef {
        &self.source
    }

    /// Checked signed integer offset; zero denotes direct column projection.
    pub fn offset(&self) -> i128 {
        self.offset
    }
}
