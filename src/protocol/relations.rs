//! Output lineage and typed relational references.

use super::*;

/// Final columns produced by a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    columns: Vec<OutputColumn>,
}

impl Output {
    pub(crate) fn new(columns: Vec<OutputColumn>) -> Self {
        Self { columns }
    }

    /// Return final output columns in SELECT-list order.
    pub fn columns(&self) -> &[OutputColumn] {
        &self.columns
    }

    /// Empty every projected column when any witness direction is impossible.
    /// After witness evidence changes without re-running analysis, an
    /// impossible qualifying direction admits no rows. This mirrors the
    /// analysis-time impossible refinement without inventing per-column
    /// domains, and is idempotent for already-empty outputs.
    pub(crate) fn empty_on_impossible<'a>(
        &mut self,
        directions: impl IntoIterator<Item = &'a crate::boolean_witness::BooleanWitnessDirection>,
    ) {
        use crate::boolean_witness::BooleanWitnessDirection;
        if directions
            .into_iter()
            .any(|direction| matches!(direction, BooleanWitnessDirection::Impossible))
        {
            for column in &mut self.columns {
                column.domain = ValueDomain::Empty;
            }
        }
    }
}

/// One final query output column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputColumn {
    name: String,
    expression: Expression,
    domain: ValueDomain,
    lineage: Vec<LineageSource>,
}

impl OutputColumn {
    pub(crate) fn new(
        name: String,
        expression: Expression,
        domain: ValueDomain,
        mut lineage: Vec<LineageSource>,
    ) -> Self {
        lineage.sort();
        lineage.dedup();
        Self {
            name,
            expression,
            domain,
            lineage,
        }
    }

    pub(crate) fn with_domain(mut self, domain: ValueDomain) -> Self {
        self.domain = domain;
        self
    }

    pub(crate) fn plain_copy_source(&self) -> Option<&LineageSource> {
        if !matches!(self.expression, Expression::Column(_)) {
            return None;
        }

        match self.lineage.as_slice() {
            [source] => Some(source),
            _ => None,
        }
    }

    /// Return the final output column name or unresolved projection label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the semantic expression that produces this output column.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the conservative value domain for this produced output value.
    pub fn domain(&self) -> &ValueDomain {
        &self.domain
    }

    /// Return physical source columns contributing to this output value.
    pub fn lineage(&self) -> &[LineageSource] {
        &self.lineage
    }
}

/// One physical source column contributing to an output value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineageSource {
    relation: String,
    column: String,
}

impl LineageSource {
    pub(crate) fn new(relation: String, column: String) -> Self {
        Self { relation, column }
    }

    /// Return the physical relation containing the source column.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the physical source column name.
    pub fn column(&self) -> &str {
        &self.column
    }
}

/// A direct relational input to a query.
///
/// Physical relations also appear in QueryStatement::dependencies. Local relations such as CTEs
/// and derived tables do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRelation {
    name: String,
    alias: Option<String>,
}

impl SourceRelation {
    pub(crate) fn new(name: String, alias: Option<String>) -> Self {
        Self { name, alias }
    }

    /// Return the relation name or stable local relation identity.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the SQL alias when one is present.
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// A relation participating in a join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRef {
    relation: String,
    alias: Option<String>,
}

impl RelationRef {
    pub(crate) fn new(relation: String, alias: Option<String>) -> Self {
        Self { relation, alias }
    }

    /// Return the relation name or stable local relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the SQL alias when one is present.
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// Supported protocol join kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// INNER JOIN or JOIN.
    Inner,
    /// LEFT JOIN.
    Left,
    /// RIGHT JOIN.
    Right,
    /// FULL JOIN.
    Full,
    /// CROSS JOIN.
    Cross,
    /// LEFT SEMI JOIN.
    LeftSemi,
    /// RIGHT SEMI JOIN.
    RightSemi,
    /// LEFT ANTI JOIN.
    LeftAnti,
    /// RIGHT ANTI JOIN.
    RightAnti,
    /// The join exists but its exact kind is unsupported.
    Unknown,
}

impl JoinKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Inner => "inner",
            Self::Left => "left",
            Self::Right => "right",
            Self::Full => "full",
            Self::Cross => "cross",
            Self::LeftSemi => "left_semi",
            Self::RightSemi => "right_semi",
            Self::LeftAnti => "left_anti",
            Self::RightAnti => "right_anti",
            Self::Unknown => "unknown",
        }
    }
}

/// The SQL evaluation law for a non-ordinary join, kept in the canonical
/// model separately from ordinary Boolean pair eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinExecution {
    /// Select at most one right row: the nearest eligible predecessor.
    AsOfNearest,
    /// Evaluate a correlated derived input once for each left row.
    LateralInner,
    /// SQL CROSS APPLY: inner correlated evaluation.
    CrossApply,
    /// SQL OUTER APPLY: left-preserved correlated evaluation.
    OuterApply,
}

impl JoinExecution {
    /// Stable protocol name for the evaluation law.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AsOfNearest => "asof_nearest",
            Self::LateralInner => "lateral_inner",
            Self::CrossApply => "cross_apply",
            Self::OuterApply => "outer_apply",
        }
    }
}

/// A normalized relationship between two query relations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    kind: JoinKind,
    left: RelationRef,
    right: RelationRef,
    condition: Option<Predicate>,
    execution: Option<JoinExecution>,
    right_physical_source: Option<String>,
    right_physical_instance: Option<String>,
}

impl Join {
    pub(crate) fn new(
        kind: JoinKind,
        left: RelationRef,
        right: RelationRef,
        condition: Option<Predicate>,
    ) -> Self {
        Self {
            kind,
            left,
            right,
            condition,
            execution: None,
            right_physical_source: None,
            right_physical_instance: None,
        }
    }

    pub(crate) fn with_execution(
        mut self,
        execution: JoinExecution,
        right_physical_source: Option<String>,
        right_physical_instance: Option<String>,
    ) -> Self {
        self.execution = Some(execution);
        self.right_physical_source = right_physical_source;
        self.right_physical_instance = right_physical_instance;
        self
    }

    /// Return the SQL evaluation law for ASOF and correlated joins.
    pub fn execution(&self) -> Option<JoinExecution> {
        self.execution
    }

    /// The physical source of a single, correlated derived input, if proven.
    pub fn right_physical_source(&self) -> Option<&str> {
        self.right_physical_source.as_deref()
    }

    /// Alias used for the physical right input inside a correlated subquery.
    pub fn right_physical_instance(&self) -> Option<&str> {
        self.right_physical_instance.as_deref()
    }

    /// Return the normalized join kind.
    pub fn kind(&self) -> JoinKind {
        self.kind
    }

    /// Return the left relation participating in the join.
    pub fn left(&self) -> &RelationRef {
        &self.left
    }

    /// Return the right relation participating in the join.
    pub fn right(&self) -> &RelationRef {
        &self.right
    }

    /// Return the normalized join condition when it can be represented safely.
    ///
    /// Equality-column operands are expressed with physical source relation and column identities
    /// when lineage proves a plain-copy path through local relations. An unsafe mapping is retained
    /// as an unknown expression and accompanied by an `unresolved_join_column_lineage` diagnostic.
    pub fn condition(&self) -> Option<&Predicate> {
        self.condition.as_ref()
    }
}
