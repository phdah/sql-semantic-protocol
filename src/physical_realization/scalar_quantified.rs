//! Finite scalar-subquery cardinality and quantified-comparison bag laws.
//!
//! A caller supplies *complete*, schema-attested source populations. These
//! proofs never infer cardinality from a sample or turn runtime failure into an
//! impossible source population. Quantification here is an operator-local law;
//! it is not promoted to whole-query exactness without canonical SQL binding.

use std::collections::BTreeMap;

use super::in_bags::{checked_ids, row_id, scalar, signed_integer_source};
use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::{query_for, Walker};
use crate::bundle::AnalysisBundle;
use crate::protocol::{ComparisonOperator, Expression, LiteralValue};

/// Complete scalar value, including SQL NULL on an empty or NULL-only singleton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalScalarSubqueryExact {
    /// Physical source whose entire bag was examined.
    pub source: String,
    /// Source row identities in bag order, including identical SQL values.
    pub source_row_ids: Vec<String>,
    /// Exact result of the SQL scalar expression.
    pub value: LiteralValue,
    /// Inclusive signed-integer bounds, absent for SQL NULL.
    pub bounds: Option<(i128, i128)>,
}

/// A scalar subquery is not a Boolean exclusion when it returns two rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalScalarSubqueryProof {
    /// Exactly zero or one physical rows and a known scalar result.
    Exact(PhysicalScalarSubqueryExact),
    /// SQL cardinality violation: the source population is valid but the
    /// expression raises an error instead of returning an output value.
    CardinalityViolation {
        /// Every source row identity; duplicates remain distinct.
        source_row_ids: Vec<String>,
    },
    /// Complete source evidence contradicts its authoritative schema or IDs.
    Impossible { reason: String },
    /// Nested SQL shape or datatype cannot be safely proved.
    Residual { reason: String },
}

/// Prove a scalar subquery projected as the sole column of a SELECT statement.
///
/// Only uncorrelated, candidate-preserving single-relation signed-integer
/// scalar SELECTs have exact physical cardinality. An empty source produces
/// SQL NULL even when the nested SELECT includes an unproved WHERE condition.
/// Nonempty nested filters, aggregation, joins, correlated scopes, and multiple
/// projections conservatively return Residual.
pub fn physical_finite_scalar_subquery_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalScalarSubqueryProof {
    let residual = |reason: &str| PhysicalScalarSubqueryProof::Residual {
        reason: reason.to_string(),
    };
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalScalarSubqueryProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(target) = walker.layers.get(target_layer_id).copied() else {
        return residual("target is not a canonical layer");
    };
    let Some(query) = query_for(bundle, target) else {
        return residual("target is not a canonical SELECT query");
    };
    let [output] = query.output().columns() else {
        return residual("scalar proof requires one output column");
    };
    let Expression::ScalarSubquery(scalar_expression) = output.expression() else {
        return residual("output is not a normalized scalar subquery");
    };
    let nested = scalar_expression.subquery();
    let [source] = nested.sources() else {
        return residual("scalar proof requires exactly one nested direct relation");
    };
    let [projected] = nested.output().columns() else {
        return residual("scalar nested SELECT must have exactly one output column");
    };
    let Some(physical_column) = projected.plain_copy_source() else {
        return residual("scalar nested SELECT must copy one physical column");
    };
    if !query.sources().is_empty()
        || query.predicates().where_predicate().is_some()
        || query.aggregation().is_some()
        || !query.joins().is_empty()
        || !query.diagnostics().is_empty()
        || !nested.row_shape_preserves_candidates()
        || !nested.joins().is_empty()
        || !nested.correlations().is_empty()
        || !nested.diagnostics().is_empty()
        || physical_column.relation() != source.name()
        || snapshots.len() != 1
    {
        return residual("scalar source identity, row shape, or correlation is not certified");
    }
    let [snapshot] = snapshots else {
        return residual("scalar proof requires one complete nested source");
    };
    if snapshot.relation() != source.name()
        || !signed_integer_source(bundle, source.name(), physical_column.column())
    {
        return residual("scalar physical column lacks matched signed integer schema evidence");
    }
    if let Err(reason) = checked_ids(snapshot) {
        return PhysicalScalarSubqueryProof::Impossible {
            reason: reason.to_string(),
        };
    }
    let ids = snapshot
        .rows()
        .iter()
        .filter_map(|row| row_id(row).map(str::to_string))
        .collect::<Vec<_>>();
    if !ids.is_empty() && nested.predicates().where_predicate().is_some() {
        return residual("nonempty nested WHERE requires a complete filter proof");
    }
    if ids.len() > 1 {
        return PhysicalScalarSubqueryProof::CardinalityViolation {
            source_row_ids: ids,
        };
    }
    // An empty nested source cannot produce any row through a
    // candidate-preserving SELECT, regardless of an additional filter.
    if ids.is_empty() {
        return PhysicalScalarSubqueryProof::Exact(PhysicalScalarSubqueryExact {
            source: source.name().to_string(),
            source_row_ids: ids,
            value: LiteralValue::Null,
            bounds: None,
        });
    }
    let Some(row) = snapshot.rows().first() else {
        return residual("singleton source row disappeared");
    };
    let value = match scalar(row, physical_column.column()) {
        Ok(Some(number)) => LiteralValue::Number(number.to_string()),
        Ok(None) => LiteralValue::Null,
        Err(reason) => return residual(reason),
    };
    let bounds = match &value {
        LiteralValue::Number(number) => number.parse::<i128>().ok().map(|n| (n, n)),
        _ => None,
    };
    PhysicalScalarSubqueryProof::Exact(PhysicalScalarSubqueryExact {
        source: source.name().to_string(),
        source_row_ids: ids,
        value,
        bounds,
    })
}

/// SQL ANY (including SOME) versus SQL ALL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalQuantifier {
    /// Existential: FALSE for an empty bag.
    Any,
    /// Universal: TRUE for an empty bag.
    All,
}

/// SQL three-valued comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalQuantifiedTruth {
    /// At least one qualifying comparison (ANY), or all qualifying (ALL).
    True,
    /// No qualifying comparison (ANY), or at least one FALSE (ALL).
    False,
    /// No decisive comparison and at least one SQL NULL comparison.
    Unknown,
}

/// Exact inclusive terminal column value domain, independent of bag identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalQuantifiedColumnDomain {
    /// All distinct non-NULL values, sorted.
    pub values: Vec<i128>,
    /// Whether SQL NULL occurs in a selected row.
    pub null_allowed: bool,
}
impl PhysicalQuantifiedColumnDomain {
    /// Inclusive lower/upper bounds, absent for all-NULL or empty output.
    pub fn bounds(&self) -> Option<(i128, i128)> {
        Some((*self.values.first()?, *self.values.last()?))
    }
}

/// Operator-local proof for each complete outer and inner physical row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteQuantifiedExact {
    /// Source names in outer/inner role order.
    pub sources: [String; 2],
    /// Selected outer row IDs in bag order.
    pub selected: Vec<String>,
    /// NOT TRUE outer IDs in bag order, including UNKNOWN.
    pub rejected: Vec<String>,
    /// All candidate inner IDs for each outer row, preserving duplicates.
    pub candidate_inner: BTreeMap<String, Vec<String>>,
    /// Per-outer-row SQL three-valued truth.
    pub truth: BTreeMap<String, PhysicalQuantifiedTruth>,
    /// Complete output bag with projected outer column values.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// The exact typed terminal domain of the outer projected column.
    pub terminal_domain: PhysicalQuantifiedColumnDomain,
}

/// Proof status for a full finite operator-local quantified comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteQuantifiedProof {
    /// Full source bag and exact three-valued terminal result.
    Exact(PhysicalFiniteQuantifiedExact),
    /// Physical data or row identities violate authoritative evidence.
    Impossible { reason: String },
    /// Missing type, operator or source identity law.
    Residual { reason: String },
}

fn compare(a: Option<i128>, b: Option<i128>, operator: ComparisonOperator) -> Option<bool> {
    let (Some(a), Some(b)) = (a, b) else {
        return None;
    };
    Some(match operator {
        ComparisonOperator::Eq => a == b,
        ComparisonOperator::Neq => a != b,
        ComparisonOperator::Lt => a < b,
        ComparisonOperator::Lte => a <= b,
        ComparisonOperator::Gt => a > b,
        ComparisonOperator::Gte => a >= b,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
            return None;
        }
    })
}

/// Evaluate an explicitly normalized ANY/SOME/ALL integer comparison over two
/// independently attested complete physical populations.
///
/// This is a source-independent *operator-local* proof. The caller supplies
/// the canonical operator/columns: this function does not assert that a
/// particular SQL AST or adapter was bound to these operands, and cannot
/// certify correlated nested filters or APPLY semantics.
pub fn physical_finite_quantified_comparison(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalWriteState],
    outer_relation: &str,
    outer_column: &str,
    inner_relation: &str,
    inner_column: &str,
    operator: ComparisonOperator,
    quantifier: PhysicalQuantifier,
) -> PhysicalFiniteQuantifiedProof {
    let residual = |reason: &str| PhysicalFiniteQuantifiedProof::Residual {
        reason: reason.to_string(),
    };
    if matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    ) {
        return residual("null-safe comparison with a quantified subquery is not certified");
    }
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteQuantifiedProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    if outer_relation == inner_relation
        || snapshots.len() != 2
        || !signed_integer_source(bundle, outer_relation, outer_column)
        || !signed_integer_source(bundle, inner_relation, inner_column)
    {
        return residual("quantifier requires two distinct attested signed integer source columns");
    }
    let (Some(outer), Some(inner)) = (
        snapshots
            .iter()
            .find(|bag| bag.relation() == outer_relation),
        snapshots
            .iter()
            .find(|bag| bag.relation() == inner_relation),
    ) else {
        return residual("both complete source populations are required");
    };
    for bag in [outer, inner] {
        if let Err(reason) = checked_ids(bag) {
            return PhysicalFiniteQuantifiedProof::Impossible {
                reason: reason.to_string(),
            };
        }
    }
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut candidate_inner = BTreeMap::new();
    let mut truth = BTreeMap::new();
    let mut terminal = Vec::new();
    let mut values = std::collections::BTreeSet::new();
    let mut null_allowed = false;
    for row in outer.rows() {
        let Some(id) = row_id(row) else {
            return residual("outer row identity missing");
        };
        let outer_value = match scalar(row, outer_column) {
            Ok(v) => v,
            Err(reason) => return residual(reason),
        };
        let mut inner_ids = Vec::new();
        let mut has_true = false;
        let mut has_false = false;
        let mut has_unknown = false;
        for candidate in inner.rows() {
            let Some(inner_id) = row_id(candidate) else {
                return residual("inner row identity missing");
            };
            let candidate_value = match scalar(candidate, inner_column) {
                Ok(v) => v,
                Err(reason) => return residual(reason),
            };
            inner_ids.push(inner_id.to_string());
            match compare(outer_value, candidate_value, operator) {
                Some(true) => has_true = true,
                Some(false) => has_false = true,
                None => has_unknown = true,
            }
        }
        let outcome = match quantifier {
            PhysicalQuantifier::Any if has_true => PhysicalQuantifiedTruth::True,
            PhysicalQuantifier::Any if has_unknown => PhysicalQuantifiedTruth::Unknown,
            PhysicalQuantifier::Any => PhysicalQuantifiedTruth::False,
            PhysicalQuantifier::All if has_false => PhysicalQuantifiedTruth::False,
            PhysicalQuantifier::All if has_unknown => PhysicalQuantifiedTruth::Unknown,
            PhysicalQuantifier::All => PhysicalQuantifiedTruth::True,
        };
        if outcome == PhysicalQuantifiedTruth::True {
            selected.push(id.to_string());
            let value = match outer_value {
                Some(n) => {
                    values.insert(n);
                    LiteralValue::Number(n.to_string())
                }
                None => {
                    null_allowed = true;
                    LiteralValue::Null
                }
            };
            terminal.push(BTreeMap::from([(outer_column.to_string(), value)]));
        } else {
            rejected.push(id.to_string());
        }
        truth.insert(id.to_string(), outcome);
        candidate_inner.insert(id.to_string(), inner_ids);
    }
    PhysicalFiniteQuantifiedProof::Exact(PhysicalFiniteQuantifiedExact {
        sources: [outer_relation.to_string(), inner_relation.to_string()],
        selected,
        rejected,
        candidate_inner,
        truth,
        terminal,
        terminal_domain: PhysicalQuantifiedColumnDomain {
            values: values.into_iter().collect(),
            null_allowed,
        },
    })
}
