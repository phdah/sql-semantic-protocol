//! Generator-facing witnesses for source-level subquery membership.
//!
//! These are operator-local contracts: they never silently promote the enclosing
//! query's independent column domains to an exact correlated row predicate.

use crate::protocol::{
    ColumnDomain, ColumnExpression, ColumnRef, ComparisonOperator, Expression, Predicate,
    QueryStatement, ResidualConditionReason, SourceRelation, SubquerySemantics,
};

/// SQL subquery-membership operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubqueryMembershipKind {
    /// SQL EXISTS.
    Exists,
    /// SQL NOT EXISTS.
    NotExists,
    /// SQL IN (subquery).
    In,
    /// SQL NOT IN (subquery).
    NotIn,
}

impl SubqueryMembershipKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Exists => "exists",
            Self::NotExists => "not_exists",
            Self::In => "in",
            Self::NotIn => "not_in",
        }
    }
}

/// One source-column equality, using distinct aliases for source instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubqueryCorrelation {
    outer: ColumnRef,
    inner: ColumnRef,
}

impl SubqueryCorrelation {
    /// Outer source column, including its physical relation identity.
    pub fn outer(&self) -> &ColumnRef {
        &self.outer
    }

    /// Inner source column, including its physical relation identity.
    pub fn inner(&self) -> &ColumnRef {
        &self.inner
    }
}

/// An operator-local truth witness. A row is selected only when the condition is TRUE.
///
/// `NoCandidates` means zero rows survive the nested WHERE and correlation filters.
/// `NoMatchNoNull` means the outer key is non-NULL, no equal candidate exists and
/// every remaining candidate key is non-NULL. Null cases intentionally distinguish
/// SQL UNKNOWN from FALSE, which are both rejected by WHERE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubqueryMembershipCase {
    /// At least one nested row satisfies the correlation and local filters.
    MatchingRow,
    /// A non-NULL inner value equals a non-NULL outer value.
    MatchingNonNullKey,
    /// The nested relation has no qualifying candidates.
    NoCandidates,
    /// Nonempty nested candidates have neither an equal nor a NULL key.
    NoMatchNoNull,
    /// No key matches but at least one candidate has a NULL membership key.
    NoMatchNullCandidate,
    /// The outer key is NULL and the nested result is nonempty.
    OuterNullNonempty,
}

impl SubqueryMembershipCase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MatchingRow => "matching_row",
            Self::MatchingNonNullKey => "matching_non_null_key",
            Self::NoCandidates => "no_candidates",
            Self::NoMatchNoNull => "no_match_no_null",
            Self::NoMatchNullCandidate => "no_match_null_candidate",
            Self::OuterNullNonempty => "outer_null_nonempty",
        }
    }
}

/// A proved classification or a default-deny residual diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubqueryMembershipDirection {
    /// Exhaustive constructible cases for this operator and source boundary.
    Exact(Vec<SubqueryMembershipCase>),
    /// The direction cannot be proven from current evidence.
    Residual { reason: String },
}

/// Typed local witness with source identities and independent truth directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubqueryMembershipWitness {
    kind: SubqueryMembershipKind,
    outer_relation: String,
    inner_relation: Option<String>,
    correlations: Vec<SubqueryCorrelation>,
    membership_key: Option<SubqueryCorrelation>,
    inner_column_domains: Vec<ColumnDomain>,
    qualifying: SubqueryMembershipDirection,
    rejected: SubqueryMembershipDirection,
}

impl SubqueryMembershipWitness {
    /// Operator responsible for the evidence.
    pub fn kind(&self) -> SubqueryMembershipKind {
        self.kind
    }

    /// Outer relation instance, not implicitly interchangeable with another alias.
    pub fn outer_relation(&self) -> &str {
        &self.outer_relation
    }

    /// Physical inner relation when its identity is unambiguous.
    pub fn inner_relation(&self) -> Option<&str> {
        self.inner_relation.as_deref()
    }

    /// Correlated equality keys; each equality is jointly required.
    pub fn correlations(&self) -> &[SubqueryCorrelation] {
        &self.correlations
    }

    /// Key used by IN or NOT IN, separate from correlation keys.
    pub fn membership_key(&self) -> Option<&SubqueryCorrelation> {
        self.membership_key.as_ref()
    }

    /// Constraints on the inner candidate set, before membership evaluation.
    pub fn inner_column_domains(&self) -> &[ColumnDomain] {
        &self.inner_column_domains
    }

    /// Source obligations that make the subquery predicate TRUE.
    pub fn qualifying(&self) -> &SubqueryMembershipDirection {
        &self.qualifying
    }

    /// Source obligations that make the predicate FALSE or UNKNOWN.
    pub fn rejected(&self) -> &SubqueryMembershipDirection {
        &self.rejected
    }

    fn residual(
        kind: SubqueryMembershipKind,
        outer_relation: String,
        inner_relation: Option<String>,
        reason: &str,
    ) -> Self {
        let direction = SubqueryMembershipDirection::Residual {
            reason: reason.to_string(),
        };
        Self {
            kind,
            outer_relation,
            inner_relation,
            correlations: Vec::new(),
            membership_key: None,
            inner_column_domains: Vec::new(),
            qualifying: direction.clone(),
            rejected: direction,
        }
    }
}

/// Analyze only top-level conjunctive WHERE subquery predicates.
/// Other logical combinations retain their existing residual exactness.
pub(crate) fn analyze(query: &QueryStatement) -> Vec<SubqueryMembershipWitness> {
    let mut witnesses = Vec::new();
    if let Some(predicate) = query.predicates().where_predicate() {
        collect(predicate, query.sources(), &mut witnesses);
    }
    witnesses
}

fn collect(
    predicate: &Predicate,
    outer_sources: &[SourceRelation],
    witnesses: &mut Vec<SubqueryMembershipWitness>,
) {
    match predicate {
        Predicate::And(and) => {
            for operand in and.operands() {
                collect(operand, outer_sources, witnesses);
            }
        }
        Predicate::Exists(exists) => witnesses.push(derive(
            if exists.negated() {
                SubqueryMembershipKind::NotExists
            } else {
                SubqueryMembershipKind::Exists
            },
            exists.subquery(),
            None,
            outer_sources,
        )),
        Predicate::InSubquery(membership) => witnesses.push(derive(
            if membership.negated() {
                SubqueryMembershipKind::NotIn
            } else {
                SubqueryMembershipKind::In
            },
            membership.subquery(),
            Some(membership.expression()),
            outer_sources,
        )),
        _ => {}
    }
}

fn derive(
    kind: SubqueryMembershipKind,
    subquery: &SubquerySemantics,
    expression: Option<&Expression>,
    outer_sources: &[SourceRelation],
) -> SubqueryMembershipWitness {
    let outer_name = outer_sources
        .first()
        .map(|s| s.alias().unwrap_or(s.name()).to_string())
        .unwrap_or_default();
    let inner_name = subquery.dependencies().first().cloned();
    let residual = |reason: &str| {
        SubqueryMembershipWitness::residual(kind, outer_name.clone(), inner_name.clone(), reason)
    };

    let [outer_source] = outer_sources else {
        return residual("requires_single_outer_relation_instance");
    };
    let [inner] = subquery.dependencies() else {
        return residual("requires_single_inner_physical_relation");
    };
    if inner == outer_source.name() {
        return residual("repeated_physical_relation_requires_distinct_instance_evidence");
    }
    if !subquery.row_shape_preserves_candidates() {
        return residual("subquery_row_shape_not_proven");
    }
    if !subquery.joins().is_empty() || !subquery.diagnostics().is_empty() {
        return residual("nested_join_or_analysis_diagnostic");
    }
    if !subquery.condition_exactness().required_assumptions().is_empty() {
        return residual("undeclared_inner_comparison_assumptions");
    }
    if subquery.condition_exactness().residual_conditions().iter().any(|item| {
        !matches!(
            item.reason(),
            ResidualConditionReason::CorrelatedSubquery
                | ResidualConditionReason::ColumnComparison
        )
    }) {
        return residual("inner_predicate_not_exact");
    }

    let outer_instance = outer_source.alias().unwrap_or(outer_source.name());
    let mut pairs = Vec::new();
    if let Some(predicate) = subquery.predicates().where_predicate() {
        if !collect_correlations(predicate, outer_source, outer_instance, inner, &mut pairs) {
            return residual("unsupported_inner_predicate_or_correlation");
        }
    }
    pairs.sort_by(|a, b| (&a.outer, &a.inner).cmp(&(&b.outer, &b.inner)));
    pairs.dedup();
    if pairs.len() != subquery.correlations().len() {
        return residual("unresolved_or_non_equality_correlation");
    }
    if pairs.iter().any(|pair| {
        !subquery.correlations().iter().any(|source| {
            source.relation() == pair.outer.relation().unwrap_or_default()
                && source.column() == pair.outer.name()
        })
    }) {
        return residual("correlation_not_physical");
    }

    let membership_key = match expression {
        None => None,
        Some(Expression::Column(outer_column)) => {
            let [projected] = subquery.output().columns() else {
                return residual("membership_requires_one_inner_output_column");
            };
            let Some(inner_column) = projected.plain_copy_source() else {
                return residual("membership_key_must_be_plain_source_column");
            };
            if inner_column.relation() != inner {
                return residual("unresolved_inner_membership_key");
            }
            let Some(outer) = source_column(outer_column, outer_source, outer_instance) else {
                return residual("membership_key_must_be_outer_source_column");
            };
            Some(SubqueryCorrelation {
                outer,
                inner: ColumnRef::new(Some(inner.to_string()), inner_column.column().to_string()),
            })
        }
        Some(_) => return residual("membership_expression_not_plain_column"),
    };

    let (qualifying, rejected) = match kind {
        SubqueryMembershipKind::Exists => (
            vec![SubqueryMembershipCase::MatchingRow],
            vec![SubqueryMembershipCase::NoCandidates],
        ),
        SubqueryMembershipKind::NotExists => (
            vec![SubqueryMembershipCase::NoCandidates],
            vec![SubqueryMembershipCase::MatchingRow],
        ),
        SubqueryMembershipKind::In => (
            vec![SubqueryMembershipCase::MatchingNonNullKey],
            vec![
                SubqueryMembershipCase::NoCandidates,
                SubqueryMembershipCase::NoMatchNoNull,
                SubqueryMembershipCase::NoMatchNullCandidate,
                SubqueryMembershipCase::OuterNullNonempty,
            ],
        ),
        SubqueryMembershipKind::NotIn => (
            vec![
                SubqueryMembershipCase::NoCandidates,
                SubqueryMembershipCase::NoMatchNoNull,
            ],
            vec![
                SubqueryMembershipCase::MatchingNonNullKey,
                SubqueryMembershipCase::NoMatchNullCandidate,
                SubqueryMembershipCase::OuterNullNonempty,
            ],
        ),
    };

    SubqueryMembershipWitness {
        kind,
        outer_relation: outer_instance.to_string(),
        inner_relation: Some(inner.to_string()),
        correlations: pairs,
        membership_key,
        inner_column_domains: subquery.column_domains().to_vec(),
        qualifying: SubqueryMembershipDirection::Exact(qualifying),
        rejected: SubqueryMembershipDirection::Exact(rejected),
    }
}

/// Every nested filter must be reducible to typed scalar source domains or an
/// explicit equality against the enclosing source. No partial AND/OR proof.
fn collect_correlations(
    predicate: &Predicate,
    outer: &SourceRelation,
    outer_instance: &str,
    inner_relation: &str,
    keys: &mut Vec<SubqueryCorrelation>,
) -> bool {
    match predicate {
        Predicate::And(and) => and
            .operands()
            .iter()
            .all(|item| collect_correlations(item, outer, outer_instance, inner_relation, keys)),
        Predicate::Comparison(compare) if compare.operator() == ComparisonOperator::Eq => {
            match (compare.left(), compare.right()) {
                (Expression::Column(a), Expression::Column(b)) => {
                    let pair = source_column(a, outer, outer_instance)
                        .zip(inner_column(b, outer_instance, inner_relation))
                        .or_else(|| {
                            source_column(b, outer, outer_instance).zip(inner_column(
                                a,
                                outer_instance,
                                inner_relation,
                            ))
                        });
                    if let Some((outer, inner)) = pair {
                        keys.push(SubqueryCorrelation { outer, inner });
                        true
                    } else {
                        false
                    }
                }
                (Expression::Column(col), Expression::Literal(_)) => {
                    col.relation() != Some(outer_instance)
                }
                _ => false,
            }
        }
        Predicate::Comparison(compare) => {
            matches!((compare.left(), compare.right()), (Expression::Column(col), Expression::Literal(_)) if col.relation() != Some(outer_instance))
        }
        Predicate::In(_) | Predicate::Between(_) | Predicate::IsNull(_) => true,
        _ => false,
    }
}

fn source_column(
    col: &ColumnExpression,
    outer: &SourceRelation,
    instance: &str,
) -> Option<ColumnRef> {
    if col.relation() == Some(instance) || col.relation() == Some(outer.name()) {
        Some(ColumnRef::new(
            Some(outer.name().to_string()),
            col.name().to_string(),
        ))
    } else {
        None
    }
}

fn inner_column(
    col: &ColumnExpression,
    outer_instance: &str,
    inner_relation: &str,
) -> Option<ColumnRef> {
    let qualifier = col.relation()?;
    if qualifier == outer_instance {
        return None;
    }
    // The distinct qualifier is kept as an instance only in the normalized
    // comparison. The physical relation comes from nested dependency evidence.
    Some(ColumnRef::new(
        Some(inner_relation.to_string()),
        col.name().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analyze_sql, ProtocolStatement};
    use sqlparser::dialect::GenericDialect;

    fn witness(sql: &str) -> SubqueryMembershipWitness {
        let protocol = analyze_sql(sql, "generic", &GenericDialect {}).expect("valid SQL");
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("expected query");
        };
        query.subquery_witnesses().first().expect("witness").clone()
    }

    #[test]
    fn exists_correlated_equality_has_matching_and_absent_obligations() {
        let item = witness("SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM lines l WHERE l.order_id = o.id)");
        assert_eq!(item.kind(), SubqueryMembershipKind::Exists);
        assert_eq!(item.correlations().len(), 1);
        assert!(matches!(
            item.qualifying(),
            SubqueryMembershipDirection::Exact(_)
        ));
        assert!(matches!(
            item.rejected(),
            SubqueryMembershipDirection::Exact(_)
        ));
    }

    #[test]
    fn nullable_not_in_is_not_an_anti_join() {
        let item =
            witness("SELECT o.id FROM orders o WHERE o.id NOT IN (SELECT c.id FROM customers c)");
        assert_eq!(item.kind(), SubqueryMembershipKind::NotIn);
        assert!(
            matches!(item.qualifying(), SubqueryMembershipDirection::Exact(cases) if cases.contains(&SubqueryMembershipCase::NoCandidates))
        );
        assert!(
            matches!(item.rejected(), SubqueryMembershipDirection::Exact(cases) if cases.contains(&SubqueryMembershipCase::NoMatchNullCandidate))
        );
    }

    #[test]
    fn unsupported_correlated_comparison_is_residual() {
        let item = witness("SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM lines l WHERE l.order_id > o.id)");
        assert!(matches!(
            item.qualifying(),
            SubqueryMembershipDirection::Residual { .. }
        ));
    }
}
