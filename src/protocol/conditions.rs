//! Condition exactness, assumptions and residual proofs.

use super::*;

/// Query predicates grouped by their SQL clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicates {
    where_predicate: Option<Predicate>,
    having_predicate: Option<Predicate>,
    qualify_predicate: Option<Predicate>,
}

impl Predicates {
    pub(crate) fn new(
        where_predicate: Option<Predicate>,
        having_predicate: Option<Predicate>,
        qualify_predicate: Option<Predicate>,
    ) -> Self {
        Self {
            where_predicate,
            having_predicate,
            qualify_predicate,
        }
    }

    /// Return the WHERE predicate when the query contains one.
    pub fn where_predicate(&self) -> Option<&Predicate> {
        self.where_predicate.as_ref()
    }

    /// Return the HAVING predicate when the query contains one.
    pub fn having_predicate(&self) -> Option<&Predicate> {
        self.having_predicate.as_ref()
    }

    /// Return the QUALIFY predicate when the query contains one.
    pub fn qualify_predicate(&self) -> Option<&Predicate> {
        self.qualify_predicate.as_ref()
    }
}

/// Whether row-membership conditions are completely represented by protocol domains and joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionExactnessStatus {
    /// Every row-membership condition is represented by the allow-listed exact contract.
    Exact,
    /// All domains are represented but comparison settings must be confirmed by the caller.
    Conditional,
    /// One or more row-membership conditions remain outside the exact representation.
    Residual,
}

impl ConditionExactnessStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Conditional => "conditional",
            Self::Residual => "residual",
        }
    }
}

/// A comparison setting a caller can attest to for an analyzed warehouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComparisonAssumption {
    /// String equality and ordering use binary, case-sensitive comparison.
    BinaryCollation,
    /// Fixed-width character comparisons do not silently pad or trim strings.
    NoCharPadding,
    /// Floating columns contain no NaN values.
    NoNan,
    /// Positive and negative floating zero compare as equal.
    SignedZeroEquivalent,
    /// Session timezone interpretation is deterministic and matches supplied timestamp literals.
    SessionTimeZone,
}

impl ComparisonAssumption {
    /// Return the stable protocol and CLI name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BinaryCollation => "binary_collation",
            Self::NoCharPadding => "no_char_padding",
            Self::NoNan => "no_nan",
            Self::SignedZeroEquivalent => "signed_zero_equivalent",
            Self::SessionTimeZone => "session_time_zone",
        }
    }

    /// Parse a stable comparison-setting name.
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "binary_collation" => Some(Self::BinaryCollation),
            "no_char_padding" => Some(Self::NoCharPadding),
            "no_nan" => Some(Self::NoNan),
            "signed_zero_equivalent" => Some(Self::SignedZeroEquivalent),
            "session_time_zone" => Some(Self::SessionTimeZone),
            _ => None,
        }
    }
}

/// A row-condition dependency on a comparison setting.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConditionalCondition {
    assumption: ComparisonAssumption,
    clause: ConditionClause,
    identity: String,
    origin_layer_id: Option<String>,
    origin_scope: Option<String>,
}

impl ConditionalCondition {
    pub(crate) fn new(
        assumption: ComparisonAssumption,
        clause: ConditionClause,
        identity: impl Into<String>,
    ) -> Self {
        Self {
            assumption,
            clause,
            identity: identity.into(),
            origin_layer_id: None,
            origin_scope: None,
        }
    }

    /// Required comparison setting.
    pub fn assumption(&self) -> ComparisonAssumption {
        self.assumption
    }
    /// SQL clause containing the condition.
    pub fn clause(&self) -> ConditionClause {
        self.clause
    }
    /// Stable condition identity.
    pub fn identity(&self) -> &str {
        &self.identity
    }
    /// Composed layer that introduced the condition.
    pub fn origin_layer_id(&self) -> Option<&str> {
        self.origin_layer_id.as_deref()
    }
    /// Original query or local-relation scope.
    pub fn origin_scope(&self) -> Option<&str> {
        self.origin_scope.as_deref()
    }

    fn with_scope(mut self, scope: &str) -> Self {
        self.origin_scope = Some(scope.to_string());
        self
    }
    fn with_layer_origin(mut self, layer_id: &str) -> Self {
        self.origin_layer_id = Some(layer_id.to_string());
        if self.origin_scope.is_none() {
            self.origin_scope = Some("query".to_string());
        }
        self
    }
}

/// SQL clause that owns a residual row-membership condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConditionClause {
    /// SELECT output projection.
    Select,
    /// WHERE clause.
    Where,
    /// JOIN ON or USING condition.
    JoinOn,
    /// HAVING clause.
    Having,
    /// QUALIFY clause.
    Qualify,
    /// UNION, INTERSECT, or EXCEPT set operation.
    SetOperation,
    /// LIMIT, OFFSET, FETCH, TOP, TABLESAMPLE, or another row-set operator.
    RowSetOperator,
}

impl ConditionClause {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Where => "where",
            Self::JoinOn => "on",
            Self::Having => "having",
            Self::Qualify => "qualify",
            Self::SetOperation => "set_operation",
            Self::RowSetOperator => "row_set_operator",
        }
    }
}

/// Stable reason why a row-membership condition is not represented exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualConditionReason {
    /// OR spans multiple source columns and therefore carries correlation not representable by independent domains.
    CrossColumnDisjunction,
    /// Logical NOT is not on the exactness allow-list.
    LogicalNot,
    /// A comparison relates source columns outside an equi-join.
    ColumnComparison,
    /// A condition depends on a computed expression rather than a plain source column.
    ComputedExpression,
    /// Comparison semantics are not established for the source datatype.
    ComparisonSemantics,
    /// A scalar literal has an incompatible canonical type.
    LiteralTypeMismatch,
    /// Numeric coercion would lose precision.
    LossyCoercion,
    /// A literal exceeds the source datatype's allowed range or precision.
    OutOfRangeLiteral,
    /// A referenced physical column is absent from available schema evidence.
    UnknownSchemaColumn,
    /// A condition uses a subquery predicate.
    SubqueryPredicate,
    /// A normalized predicate is unknown or unsupported.
    UnsupportedPredicate,
    /// A constant FALSE or NULL condition cannot be represented as independent column domains.
    ConstantFalseOrNull,
    /// HAVING drops groups based on aggregate or grouped results.
    Having,
    /// QUALIFY drops rows based on window results.
    Qualify,
    /// An outer-join condition cannot constrain both inputs as an inner-row equality contract.
    OuterJoin,
    /// A non-inner join kind decides membership through semantics not represented by domains/equalities.
    UnsupportedJoinKind,
    /// The same physical relation is read through multiple instances whose identities collapse in column domains.
    RepeatedSourceInstance,
    /// LIMIT affects which otherwise qualifying rows survive.
    Limit,
    /// OFFSET affects which otherwise qualifying rows survive.
    Offset,
    /// FETCH affects which otherwise qualifying rows survive.
    Fetch,
    /// DISTINCT ON selects rows based on ordering within duplicate groups.
    DistinctOn,
    /// TABLESAMPLE drops otherwise qualifying source rows.
    TableSample,
    /// Set-operation row membership is not represented exactly.
    SetOperation,
    /// TOP limits the qualifying row set.
    Top,
    /// PREWHERE is parsed but not represented as an exact source predicate.
    Prewhere,
    /// CONNECT BY changes row membership through recursive traversal.
    ConnectBy,
    /// A condition-affecting analysis diagnostic prevents an exact guarantee.
    AnalysisDiagnostic,
    /// A correlated subquery depends on an outer row outside the local domain contract.
    CorrelatedSubquery,
}

impl ResidualConditionReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CrossColumnDisjunction => "cross_column_disjunction",
            Self::LogicalNot => "logical_not",
            Self::ColumnComparison => "column_comparison",
            Self::ComputedExpression => "computed_expression",
            Self::ComparisonSemantics => "comparison_semantics",
            Self::LiteralTypeMismatch => "literal_type_mismatch",
            Self::LossyCoercion => "lossy_coercion",
            Self::OutOfRangeLiteral => "out_of_range_literal",
            Self::UnknownSchemaColumn => "unknown_schema_column",
            Self::SubqueryPredicate => "subquery_predicate",
            Self::UnsupportedPredicate => "unsupported_predicate",
            Self::ConstantFalseOrNull => "constant_false_or_null",
            Self::Having => "having",
            Self::Qualify => "qualify",
            Self::OuterJoin => "outer_join",
            Self::UnsupportedJoinKind => "unsupported_join_kind",
            Self::RepeatedSourceInstance => "repeated_source_instance",
            Self::Limit => "limit",
            Self::Offset => "offset",
            Self::Fetch => "fetch",
            Self::DistinctOn => "distinct_on",
            Self::TableSample => "table_sample",
            Self::SetOperation => "set_operation",
            Self::Top => "top",
            Self::Prewhere => "prewhere",
            Self::ConnectBy => "connect_by",
            Self::AnalysisDiagnostic => "analysis_diagnostic",
            Self::CorrelatedSubquery => "correlated_subquery",
        }
    }
}

/// One row-membership condition that remains outside the exact representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualCondition {
    reason: ResidualConditionReason,
    clause: ConditionClause,
    identity: String,
    origin_layer_id: Option<String>,
    origin_scope: Option<String>,
}

impl ResidualCondition {
    pub(crate) fn new(
        reason: ResidualConditionReason,
        clause: ConditionClause,
        identity: impl Into<String>,
    ) -> Self {
        Self {
            reason,
            clause,
            identity: identity.into(),
            origin_layer_id: None,
            origin_scope: None,
        }
    }

    pub(crate) fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.origin_scope = Some(scope.into());
        self
    }

    pub(crate) fn with_layer_origin(mut self, layer_id: impl Into<String>) -> Self {
        self.origin_layer_id = Some(layer_id.into());
        if self.origin_scope.is_none() {
            self.origin_scope = Some("query".to_string());
        }
        self
    }

    /// Return the stable residual reason.
    pub fn reason(&self) -> ResidualConditionReason {
        self.reason
    }

    /// Return the SQL clause that owns the residual.
    pub fn clause(&self) -> ConditionClause {
        self.clause
    }

    /// Return deterministic identity locating the residual inside its query scope.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Return the transformation layer where this residual originated after composition.
    pub fn origin_layer_id(&self) -> Option<&str> {
        self.origin_layer_id.as_deref()
    }

    /// Return the query, CTE, or derived-table scope where this residual originated.
    pub fn origin_scope(&self) -> Option<&str> {
        self.origin_scope.as_deref()
    }
}

/// Exactness contract for the row-membership conditions of one query scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionExactness {
    residual_conditions: Vec<ResidualCondition>,
    required_assumptions: Vec<ConditionalCondition>,
    declared_assumptions: BTreeSet<ComparisonAssumption>,
}

impl ConditionExactness {
    pub(crate) fn from_residuals(mut residual_conditions: Vec<ResidualCondition>) -> Self {
        residual_conditions.sort_by(|left, right| {
            (
                left.origin_layer_id.as_deref(),
                left.origin_scope.as_deref(),
                left.clause.as_str(),
                left.identity.as_str(),
                left.reason.as_str(),
            )
                .cmp(&(
                    right.origin_layer_id.as_deref(),
                    right.origin_scope.as_deref(),
                    right.clause.as_str(),
                    right.identity.as_str(),
                    right.reason.as_str(),
                ))
        });
        residual_conditions.dedup();
        Self {
            residual_conditions,
            required_assumptions: Vec::new(),
            declared_assumptions: BTreeSet::new(),
        }
    }

    pub(crate) fn from_requirements(mut requirements: Vec<ConditionalCondition>) -> Self {
        requirements.sort();
        requirements.dedup();
        Self {
            residual_conditions: Vec::new(),
            required_assumptions: requirements,
            declared_assumptions: BTreeSet::new(),
        }
    }

    pub(crate) fn without_projected_rank_where_residual(mut self) -> Self {
        self.residual_conditions.retain(|item| {
            !(item.clause == ConditionClause::Where
                && item.reason == ResidualConditionReason::ComputedExpression
                && item.identity == "where")
        });
        self
    }

    pub(crate) fn without_qualify_residual(mut self) -> Self {
        self.residual_conditions.retain(|item| {
            !(item.clause == ConditionClause::Qualify
                && item.reason == ResidualConditionReason::Qualify)
        });
        self
    }

    pub(crate) fn with_declarations(mut self, declared: &[ComparisonAssumption]) -> Self {
        self.declared_assumptions.extend(declared.iter().copied());
        self
    }

    pub(crate) fn with_scope(&self, scope: impl Into<String>) -> Self {
        let scope = scope.into();
        let mut result = Self::from_residuals(
            self.residual_conditions
                .iter()
                .cloned()
                .map(|residual| residual.with_scope(scope.clone()))
                .collect(),
        );
        result.required_assumptions = self
            .required_assumptions
            .iter()
            .cloned()
            .map(|requirement| requirement.with_scope(&scope))
            .collect();
        result.declared_assumptions = self.declared_assumptions.clone();
        result
    }

    pub(crate) fn with_layer_origin(&self, layer_id: impl Into<String>) -> Self {
        let layer_id = layer_id.into();
        let mut result = Self::from_residuals(
            self.residual_conditions
                .iter()
                .cloned()
                .map(|residual| residual.with_layer_origin(layer_id.clone()))
                .collect(),
        );
        result.required_assumptions = self
            .required_assumptions
            .iter()
            .cloned()
            .map(|requirement| requirement.with_layer_origin(&layer_id))
            .collect();
        result.declared_assumptions = self.declared_assumptions.clone();
        result
    }

    pub(crate) fn merged_with(&self, other: &Self) -> Self {
        let mut merged = Self::from_residuals(
            self.residual_conditions
                .iter()
                .chain(other.residual_conditions.iter())
                .cloned()
                .collect(),
        );
        merged.required_assumptions = self
            .required_assumptions
            .iter()
            .chain(&other.required_assumptions)
            .cloned()
            .collect();
        merged.required_assumptions.sort();
        merged.required_assumptions.dedup();
        merged.declared_assumptions = self
            .declared_assumptions
            .union(&other.declared_assumptions)
            .copied()
            .collect();
        merged
    }

    /// Return whether the scope satisfies the exact row-membership contract.
    pub fn status(&self) -> ConditionExactnessStatus {
        if !self.residual_conditions.is_empty() {
            ConditionExactnessStatus::Residual
        } else if self
            .required_assumptions
            .iter()
            .any(|item| !self.declared_assumptions.contains(&item.assumption))
        {
            ConditionExactnessStatus::Conditional
        } else {
            ConditionExactnessStatus::Exact
        }
    }

    /// Return residual conditions in deterministic clause, identity, and reason order.
    pub fn residual_conditions(&self) -> &[ResidualCondition] {
        &self.residual_conditions
    }

    /// Return the assumptions each condition depends on, including those already declared.
    pub fn required_assumptions(&self) -> &[ConditionalCondition] {
        &self.required_assumptions
    }

    /// Return the settings currently attested by the caller.
    pub fn declared_assumptions(&self) -> &BTreeSet<ComparisonAssumption> {
        &self.declared_assumptions
    }

    /// Return true only when no residual condition or undeclared assumption remains.
    pub fn is_exact(&self) -> bool {
        self.status() == ConditionExactnessStatus::Exact
    }
}
