//! Coupled source-row predicates that cannot be represented by independent column domains.
//!
//! Proof is deliberately narrow. An exact direction constrains a *single row* at the
//! originating relation boundary, retaining the logical tree and SQL's three-valued
//! WHERE semantics. No independent Cartesian sampling or SQL reparsing is implied.

use std::collections::{BTreeMap, BTreeSet};

use crate::constraints::{
    ConstraintEnforcement, ConstraintValue, RelationConstraint, RelationConstraintSet,
};

use crate::domain::{comparison_domain, intersect_domains, resolve_column};
use crate::protocol::{
    ColumnDomain, ColumnRef, ComparisonAssumption, ComparisonOperator, Expression,
    LiteralExpression, LiteralType, LiteralValue, Predicate, SourceRelation, ValueDomain,
};

/// A logical operand sequence with at least two children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanOperands {
    operands: Vec<BooleanRowConstraint>,
}

impl BooleanOperands {
    fn new(operands: Vec<BooleanRowConstraint>) -> Option<Self> {
        (operands.len() >= 2).then_some(Self { operands })
    }

    /// Return the operands in SQL evaluation-tree order.
    pub fn as_slice(&self) -> &[BooleanRowConstraint] {
        &self.operands
    }
}

impl std::ops::Deref for BooleanOperands {
    type Target = [BooleanRowConstraint];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Catalog-backed signed integer bounds used to prove feasible truth outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SignedIntegerEvidence {
    pub(crate) minimum: i128,
    pub(crate) maximum: i128,
}

/// Catalog-backed variable-length string evidence for a LIKE-prefix proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StringEvidence {
    pub(crate) max_chars: Option<u64>,
}

/// A generator-facing, typed source-row boolean expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanRowConstraint {
    /// Every child must hold on the same source row.
    All(BooleanOperands),
    /// At least one child must hold on the same source row.
    Any(BooleanOperands),
    /// Test a source value for NULL or NOT NULL.
    NullTest {
        /// Resolved source column.
        column: ColumnRef,
        /// Whether the operand is IS NOT NULL.
        negated: bool,
    },
    /// Compare an integer source value against a signed integer literal.
    IntegerComparison {
        /// Resolved source column.
        column: ColumnRef,
        /// SQL comparison operator.
        operator: ComparisonOperator,
        /// Integer literal, with source datatype verified separately.
        literal: i64,
    },
    /// Test a string's binary prefix, or its SQL NOT LIKE complement.
    StringPrefix {
        /// Resolved string-typed source column.
        column: ColumnRef,
        /// Nonempty ASCII alphanumeric prefix, without its trailing wildcard.
        prefix: String,
        /// Whether this was a SQL NOT LIKE.
        negated: bool,
    },
    /// A subtree that cannot be proved without SQL execution or assumptions.
    Residual {
        /// Why this subtree is not an exact source-row constraint.
        reason: String,
    },
}

impl BooleanRowConstraint {
    /// Whether this complete logical tree is within the proven source-row subset.
    pub fn is_exact(&self) -> bool {
        match self {
            Self::All(children) | Self::Any(children) => {
                !children.is_empty() && children.iter().all(Self::is_exact)
            }
            Self::NullTest { .. } | Self::IntegerComparison { .. } | Self::StringPrefix { .. } => {
                true
            }
            Self::Residual { .. } => false,
        }
    }

    fn mapped_columns(&self, mapped: &BTreeMap<ColumnRef, ColumnRef>) -> Option<Self> {
        match self {
            Self::All(items) => Some(Self::All(BooleanOperands::new(
                items
                    .iter()
                    .map(|item| item.mapped_columns(mapped))
                    .collect::<Option<Vec<_>>>()?,
            )?)),
            Self::Any(items) => Some(Self::Any(BooleanOperands::new(
                items
                    .iter()
                    .map(|item| item.mapped_columns(mapped))
                    .collect::<Option<Vec<_>>>()?,
            )?)),
            Self::NullTest { column, negated } => Some(Self::NullTest {
                column: mapped.get(column)?.clone(),
                negated: *negated,
            }),
            Self::IntegerComparison {
                column,
                operator,
                literal,
            } => Some(Self::IntegerComparison {
                column: mapped.get(column)?.clone(),
                operator: *operator,
                literal: *literal,
            }),
            Self::StringPrefix {
                column,
                prefix,
                negated,
            } => Some(Self::StringPrefix {
                column: mapped.get(column)?.clone(),
                prefix: prefix.clone(),
                negated: *negated,
            }),
            Self::Residual { reason } => Some(Self::Residual {
                reason: reason.clone(),
            }),
        }
    }

    fn requires_source_type_evidence(&self) -> bool {
        match self {
            Self::All(children) | Self::Any(children) => {
                children.iter().any(Self::requires_source_type_evidence)
            }
            Self::IntegerComparison { .. } | Self::StringPrefix { .. } => true,
            Self::NullTest { .. } | Self::Residual { .. } => false,
        }
    }

    fn contains_string_prefix(&self) -> bool {
        match self {
            Self::All(operands) | Self::Any(operands) => {
                operands.iter().any(Self::contains_string_prefix)
            }
            Self::StringPrefix { .. } => true,
            _ => false,
        }
    }

    fn columns(&self, output: &mut Vec<ColumnRef>) {
        match self {
            Self::All(children) | Self::Any(children) => {
                for child in children.iter() {
                    child.columns(output);
                }
            }
            Self::NullTest { column, .. }
            | Self::IntegerComparison { column, .. }
            | Self::StringPrefix { column, .. } => {
                output.push(column.clone());
            }
            Self::Residual { .. } => {}
        }
    }
}

/// A truth obligation, evaluated with SQL's three-valued logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanTruthCase {
    /// Predicate evaluates to TRUE, and WHERE selects the row.
    True,
    /// Predicate evaluates to FALSE or UNKNOWN, and WHERE rejects the row.
    NotTrue,
}

impl BooleanTruthCase {
    /// Stable protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::True => "true",
            Self::NotTrue => "not_true",
        }
    }
}

/// Independent proof statuses for qualifying and rejected source rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanWitnessDirection {
    /// The complete typed boolean expression is a precise row obligation.
    Exact(BooleanTruthCase),
    /// Source-row constructibility cannot be proven.
    Residual {
        /// Explicit conservative diagnostic.
        reason: String,
    },
}

/// One coupled predicate at its original source boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanWitness {
    source_relation: String,
    condition: BooleanRowConstraint,
    qualifying: BooleanWitnessDirection,
    rejected: BooleanWitnessDirection,
    // Non-emitted source type bounds retained to recheck proofs when constraints arrive later.
    integer_bounds: BTreeMap<ColumnRef, SignedIntegerEvidence>,
    string_bounds: BTreeMap<ColumnRef, StringEvidence>,
    comparison_assumptions: BTreeSet<ComparisonAssumption>,
    source_constraints: Vec<RelationConstraintSet>,
}

impl BooleanWitness {
    /// The single source relation whose values must be solved jointly.
    pub fn source_relation(&self) -> &str {
        &self.source_relation
    }

    /// Typed predicate tree. The entire tree applies to one row.
    pub fn condition(&self) -> &BooleanRowConstraint {
        &self.condition
    }

    /// Obligation that makes the filtered row survive.
    pub fn qualifying(&self) -> &BooleanWitnessDirection {
        &self.qualifying
    }

    /// Obligation that makes the filtered row fail WHERE (FALSE or UNKNOWN).
    pub fn rejected(&self) -> &BooleanWitnessDirection {
        &self.rejected
    }

    /// Derive scalar domains only when every conjunct is proven and must be
    /// TRUE for a qualifying row. Disjunctions cannot narrow any one column.
    pub(crate) fn qualifying_conjunctive_domains(&self) -> Vec<ColumnDomain> {
        if !matches!(
            self.qualifying,
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ) {
            return Vec::new();
        }
        fn collect(
            condition: &BooleanRowConstraint,
            domains: &mut BTreeMap<ColumnRef, ValueDomain>,
        ) -> bool {
            match condition {
                BooleanRowConstraint::All(operands) => {
                    operands.iter().all(|item| collect(item, domains))
                }
                BooleanRowConstraint::IntegerComparison {
                    column,
                    operator,
                    literal,
                } => {
                    let derived = comparison_domain(
                        *operator,
                        &LiteralExpression::new(
                            LiteralType::Integer,
                            LiteralValue::Number(literal.to_string()),
                        ),
                    );
                    domains
                        .entry(column.clone())
                        .and_modify(|existing| {
                            *existing = intersect_domains(existing, &derived);
                        })
                        .or_insert(derived);
                    true
                }
                // Other exact conjuncts do not invalidate independently proven
                // integer bounds; they simply contribute no scalar bounds.
                BooleanRowConstraint::NullTest { .. }
                | BooleanRowConstraint::StringPrefix { .. } => true,
                BooleanRowConstraint::Any(_) | BooleanRowConstraint::Residual { .. } => false,
            }
        }
        let mut domains = BTreeMap::new();
        if !collect(&self.condition, &mut domains) {
            return Vec::new();
        }
        domains
            .into_iter()
            .map(|(column, domain)| ColumnDomain::new(column, domain))
            .collect()
    }

    /// Replace intermediate references with proven identity-only physical source columns.
    ///
    /// Every column must reach the same physical relation without a cast,
    /// computation, ambiguous projection or lossy transformation.
    pub(crate) fn mapped_to_physical(
        &self,
        resolve: impl FnMut(&ColumnRef) -> Option<ColumnRef>,
    ) -> Option<Self> {
        // Without authoritative type parity a copied value is only enough
        // to transport SQL NULL tests, not typed comparisons.
        self.mapped_to_physical_certified(resolve, |_, _| false)
    }

    /// Transport typed source conditions only when the caller independently
    /// verifies identical physical and producer-column datatypes through
    /// identity-only materialization. Never reuse this for a cast or opaque
    /// source without complete metadata evidence.
    pub(crate) fn mapped_to_physical_certified(
        &self,
        mut resolve: impl FnMut(&ColumnRef) -> Option<ColumnRef>,
        mut equivalent_type: impl FnMut(&ColumnRef, &ColumnRef) -> bool,
    ) -> Option<Self> {
        let mut columns = Vec::new();
        self.condition.columns(&mut columns);
        let mut mapping = BTreeMap::new();
        for column in columns {
            if !mapping.contains_key(&column) {
                let mapped = resolve(&column)?;
                if self.condition.requires_source_type_evidence()
                    && !equivalent_type(&column, &mapped)
                {
                    return None;
                }
                mapping.insert(column, mapped);
            }
        }
        let relations = mapping
            .values()
            .filter_map(ColumnRef::relation)
            .collect::<BTreeSet<_>>();
        let mut relations = relations.into_iter();
        let relation = relations.next()?;
        if relations.next().is_some() {
            return None;
        }
        if mapping.values().any(|column| column.relation().is_none()) {
            return None;
        }
        let mut integer_bounds = BTreeMap::new();
        for (column, evidence) in &self.integer_bounds {
            let mapped = mapping.get(column)?.clone();
            if integer_bounds
                .insert(mapped, *evidence)
                .is_some_and(|old| old != *evidence)
            {
                return None;
            }
        }
        let mut string_bounds = BTreeMap::new();
        for (column, evidence) in &self.string_bounds {
            let mapped = mapping.get(column)?.clone();
            if string_bounds
                .insert(mapped, *evidence)
                .is_some_and(|old| old != *evidence)
            {
                return None;
            }
        }
        Some(Self {
            source_relation: relation.to_string(),
            condition: self.condition.mapped_columns(&mapping)?,
            qualifying: self.qualifying.clone(),
            rejected: self.rejected.clone(),
            integer_bounds,
            string_bounds,
            comparison_assumptions: self.comparison_assumptions.clone(),
            source_constraints: Vec::new(),
        })
    }

    /// Recheck a sequence of WHERE filters on the *same physical row* jointly.
    ///
    /// Each input must have been independently resolved through transparent
    /// producer projections. Conflicting source evidence is not silently
    /// widened; no upstream materialization boundary may be treated as a leaf.
    pub(crate) fn conjoin_physical_filters(filters: &[&Self]) -> Option<Self> {
        let (first, rest) = filters.split_first()?;
        let mut combined = (*first).clone();
        let mut conditions = vec![combined.condition.clone()];
        for next in rest {
            if combined.source_relation != next.source_relation {
                return None;
            }
            // Only assumptions shared by *every* layer can support a
            // cross-layer comparison; combine all actual source restrictions.
            combined.comparison_assumptions = combined
                .comparison_assumptions
                .intersection(&next.comparison_assumptions)
                .copied()
                .collect();
            for constraints in &next.source_constraints {
                if !combined.source_constraints.contains(constraints) {
                    combined.source_constraints.push(constraints.clone());
                }
            }
            for (column, bounds) in &next.integer_bounds {
                if combined
                    .integer_bounds
                    .insert(column.clone(), *bounds)
                    .is_some_and(|prior| prior != *bounds)
                {
                    return None;
                }
            }
            for (column, bounds) in &next.string_bounds {
                if combined
                    .string_bounds
                    .insert(column.clone(), *bounds)
                    .is_some_and(|prior| prior != *bounds)
                {
                    return None;
                }
            }
            conditions.push(next.condition.clone());
        }
        if conditions.len() > 1 {
            combined.condition = BooleanRowConstraint::All(BooleanOperands::new(conditions)?);
        }
        combined.recheck_truth_directions();
        Some(combined)
    }

    /// Caller attestations permit exact prefix witnesses only under binary,
    /// no-padding string comparisons. Other settings never certify LIKE.
    pub(crate) fn declare_comparison_assumptions(&mut self, assumptions: &[ComparisonAssumption]) {
        self.comparison_assumptions
            .extend(assumptions.iter().copied());
        self.recheck_truth_directions();
    }

    /// Recheck joint feasibility against externally enriched schema constraints.
    pub(crate) fn restrict_with_schema_constraints(&mut self, sets: &[RelationConstraintSet]) {
        self.source_constraints = sets.to_vec();
        self.recheck_truth_directions();
    }

    fn recheck_truth_directions(&mut self) {
        let mut columns = Vec::new();
        self.condition.columns(&mut columns);
        let resolved_source = columns
            .iter()
            .all(|column| column.relation() == Some(&self.source_relation));
        let candidate = self.condition.is_exact() && !columns.is_empty() && resolved_source;
        let prefix_licensed = !self.condition.contains_string_prefix()
            || (self
                .comparison_assumptions
                .contains(&ComparisonAssumption::BinaryCollation)
                && self
                    .comparison_assumptions
                    .contains(&ComparisonAssumption::NoCharPadding));
        let restrictions = self.enforced_restrictions();
        let cases = if candidate && prefix_licensed {
            match restrictions.as_ref() {
                Some(restrictions) => possible_joint_truths(
                    &self.condition,
                    &|column| self.integer_bounds.get(column).copied(),
                    &|column| self.string_bounds.get(column).copied(),
                    Some(restrictions),
                ),
                None => BTreeSet::new(),
            }
        } else {
            BTreeSet::new()
        };
        let reason = if columns.is_empty() {
            "predicate has no resolved source columns"
        } else if !resolved_source {
            "predicate columns do not resolve to the same source identity"
        } else if !self.condition.is_exact() {
            "a boolean branch lacks proven source datatype or supported semantics"
        } else if !prefix_licensed {
            "LIKE prefix requires binary_collation and no_char_padding attestations"
        } else if restrictions.is_none() {
            "source constraint evidence is contradictory, unknown or dependent"
        } else {
            "no provable source-row assignment satisfies the requested truth direction"
        };
        self.qualifying = if cases.contains(&SqlTruth::True) {
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        } else {
            BooleanWitnessDirection::Residual {
                reason: reason.to_string(),
            }
        };
        self.rejected = if cases.contains(&SqlTruth::False) || cases.contains(&SqlTruth::Unknown) {
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        } else {
            BooleanWitnessDirection::Residual {
                reason: reason.to_string(),
            }
        };
    }

    fn enforced_restrictions(&self) -> Option<BTreeMap<String, ColumnRestriction>> {
        let mut restrictions = BTreeMap::<String, ColumnRestriction>::new();
        for set in self
            .source_constraints
            .iter()
            .filter(|set| set.relation() == self.source_relation)
        {
            if !set.diagnostics().is_empty() {
                return None;
            }
            for constraint in set.constraints() {
                if !constraint
                    .evidence()
                    .iter()
                    .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
                {
                    if constraint
                        .evidence()
                        .iter()
                        .any(|e| e.enforcement() == ConstraintEnforcement::Unknown)
                    {
                        return None;
                    }
                    continue;
                }
                match constraint {
                    RelationConstraint::PrimaryKey(key) => {
                        for column in key.columns() {
                            restrictions.entry(column.clone()).or_default().not_null = true;
                        }
                    }
                    RelationConstraint::NotNull(item) => {
                        restrictions
                            .entry(item.column().to_string())
                            .or_default()
                            .not_null = true;
                    }
                    RelationConstraint::AcceptedValues(item) => {
                        let restriction =
                            restrictions.entry(item.column().to_string()).or_default();
                        let mut accepted = BTreeSet::new();
                        for value in item.values() {
                            match value {
                                ConstraintValue::Integer(value) => {
                                    accepted.insert(RowScalar::Integer(i128::from(*value)));
                                }
                                ConstraintValue::UnsignedInteger(value) => {
                                    accepted.insert(RowScalar::Integer(i128::from(*value)));
                                }
                                ConstraintValue::String(value) => {
                                    accepted.insert(RowScalar::String(value.clone()));
                                }
                                ConstraintValue::Null => {}
                                _ => return None,
                            }
                        }
                        restriction.accepted = Some(match restriction.accepted.take() {
                            Some(existing) => existing.intersection(&accepted).cloned().collect(),
                            None => accepted,
                        });
                    }
                    RelationConstraint::UniqueKey(_) => {}
                    RelationConstraint::ForeignKey(_) => return None,
                }
            }
        }
        // A non-null constraint and an empty accepted-value domain are
        // contradictory even when the column is absent from the predicate.
        if restrictions
            .values()
            .any(|item| item.not_null && item.accepted.as_ref().is_some_and(BTreeSet::is_empty))
        {
            return None;
        }
        Some(restrictions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RowScalar {
    Integer(i128),
    String(String),
}

#[derive(Default)]
struct ColumnRestriction {
    not_null: bool,
    accepted: Option<BTreeSet<RowScalar>>,
}

/// Establish an operator-local witness without changing whole-query exactness.
///
/// Signed integer comparisons require authoritative source datatype evidence, and
/// every supported leaf must bind unambiguously to the same relation. Repeated
/// references are evaluated together rather than independently.
pub(crate) fn analyze(
    predicate: Option<&Predicate>,
    sources: &[SourceRelation],
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: impl Fn(&ColumnRef) -> Option<StringEvidence>,
) -> Option<BooleanWitness> {
    analyze_candidate(predicate, sources, integer_evidence, string_evidence, false)
}

/// Build a supplementary source-level scalar row witness without altering
/// the already-serialized operator-local Boolean contract. The physical
/// composer invokes this only for an exactly row-filtering direct source
/// whose datatype comes from authoritative physical schema metadata.
pub(crate) fn analyze_physical_scalar(
    predicate: Option<&Predicate>,
    sources: &[SourceRelation],
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: impl Fn(&ColumnRef) -> Option<StringEvidence>,
) -> Option<BooleanWitness> {
    analyze_candidate(predicate, sources, integer_evidence, string_evidence, true)
}

fn analyze_candidate(
    predicate: Option<&Predicate>,
    sources: &[SourceRelation],
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: impl Fn(&ColumnRef) -> Option<StringEvidence>,
    physical_scalar: bool,
) -> Option<BooleanWitness> {
    let predicate = predicate?;
    if !is_witness_candidate(predicate)
        && !(physical_scalar
            && matches!(predicate, Predicate::Comparison(_) | Predicate::IsNull(_)))
    {
        return None;
    }
    let [source] = sources else {
        return None;
    };
    let condition = normalize(predicate, sources, &integer_evidence, &string_evidence);
    let mut columns = Vec::new();
    condition.columns(&mut columns);
    let integer_bounds = columns
        .iter()
        .filter_map(|column| integer_evidence(column).map(|bounds| (column.clone(), bounds)))
        .collect();
    let string_bounds = columns
        .iter()
        .filter_map(|column| string_evidence(column).map(|bounds| (column.clone(), bounds)))
        .collect();
    let mut witness = BooleanWitness {
        source_relation: source.name().to_string(),
        condition,
        qualifying: BooleanWitnessDirection::Residual {
            reason: "witness proof not yet evaluated".to_string(),
        },
        rejected: BooleanWitnessDirection::Residual {
            reason: "witness proof not yet evaluated".to_string(),
        },
        integer_bounds,
        string_bounds,
        comparison_assumptions: BTreeSet::new(),
        source_constraints: Vec::new(),
    };
    witness.recheck_truth_directions();
    Some(witness)
}

fn is_witness_candidate(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::And(_) | Predicate::Or(_) | Predicate::LikePrefix(_) => true,
        Predicate::Comparison(comparison) => {
            // A lone direct column comparison already has a scalar domain;
            // computed operands need a typed inversion witness.
            [comparison.left(), comparison.right()]
                .iter()
                .any(|expression| {
                    matches!(
                        expression,
                        Expression::SignedIntegerCast(_)
                            | Expression::Binary(_)
                            | Expression::Unary(_)
                    )
                })
        }
        _ => false,
    }
}

fn normalize(
    predicate: &Predicate,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: &impl Fn(&ColumnRef) -> Option<StringEvidence>,
) -> BooleanRowConstraint {
    match predicate {
        Predicate::And(logical) | Predicate::Or(logical) => {
            let operands = logical
                .operands()
                .iter()
                .map(|item| normalize(item, sources, integer_evidence, string_evidence))
                .collect();
            match BooleanOperands::new(operands) {
                Some(operands) if matches!(predicate, Predicate::And(_)) => {
                    BooleanRowConstraint::All(operands)
                }
                Some(operands) => BooleanRowConstraint::Any(operands),
                None => residual("logical predicate has fewer than two operands"),
            }
        }
        Predicate::IsNull(test) => {
            if let Expression::Column(column) = test.expression() {
                BooleanRowConstraint::NullTest {
                    column: resolve_column(column, sources),
                    negated: test.negated(),
                }
            } else {
                residual("NULL test involves a computed expression")
            }
        }
        Predicate::Comparison(comparison) => {
            let (column, operator, literal, offset) = if let Some((column, offset)) =
                affine_integer_operand(comparison.left(), sources, integer_evidence)
            {
                (column, comparison.operator(), comparison.right(), offset)
            } else if let Some((column, offset)) =
                affine_integer_operand(comparison.right(), sources, integer_evidence)
            {
                (
                    column,
                    comparison.operator().reversed(),
                    comparison.left(),
                    offset,
                )
            } else {
                return residual("comparison is noninvertible or correlates two source values");
            };
            if matches!(
                operator,
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ) {
                return residual("null-safe comparison is not part of the integer witness subset");
            }
            let Some(value) = signed_integer_literal(literal) else {
                return residual("comparison literal is not a supported signed integer");
            };
            let Some(value) = i128::from(value)
                .checked_sub(offset)
                .and_then(|value| i64::try_from(value).ok())
            else {
                return residual("comparison literal cannot be inverted to a signed integer");
            };
            let column = resolve_column(column, sources);
            if integer_evidence(&column).is_none() {
                return residual("source datatype does not prove exact signed integer comparison");
            }
            BooleanRowConstraint::IntegerComparison {
                column,
                operator,
                literal: value,
            }
        }
        Predicate::LikePrefix(test) => {
            let Expression::Column(column) = test.expression() else {
                return residual("LIKE prefix requires a direct source string column");
            };
            let column = resolve_column(column, sources);
            if string_evidence(&column).is_none() {
                return residual("LIKE prefix needs a catalog-proven variable-length string type");
            }
            BooleanRowConstraint::StringPrefix {
                column,
                prefix: test.prefix().to_string(),
                negated: test.negated(),
            }
        }
        Predicate::Not(_) => residual("logical NOT needs explicit three-valued inversion"),
        _ => residual("computed, function, collation, cast or other predicate is not invertible"),
    }
}

fn residual(reason: &str) -> BooleanRowConstraint {
    BooleanRowConstraint::Residual {
        reason: reason.to_string(),
    }
}

// Ordinary CASTs to an equal or wider signed integer type preserve every
// source value, order, and NULL. A narrowing cast can truncate/overflow and
// must never be assumed invertible.
fn invertible_integer_column<'a>(
    expression: &'a Expression,
    sources: &[SourceRelation],
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<&'a crate::protocol::ColumnExpression> {
    match expression {
        Expression::SignedIntegerCast(cast) => {
            let column = identity_integer_column(cast.expression())?;
            let source = resolve_column(column, sources);
            let bounds = evidence(&source)?;
            let magnitude = 1_i128 << (u32::from(cast.target_bits()) - 1);
            (bounds.minimum >= -magnitude && bounds.maximum < magnitude).then_some(column)
        }
        other => identity_integer_column(other),
    }
}

// A nonzero shift is invertible only when an explicit ordinary signed cast
// bounds the arithmetic and every possible source value stays in that target
// width. The comparison threshold is shifted back to the source column.
fn affine_integer_operand<'a>(
    expression: &'a Expression,
    sources: &[SourceRelation],
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<(&'a crate::protocol::ColumnExpression, i128)> {
    use crate::protocol::BinaryOperator;

    if let Some(column) = invertible_integer_column(expression, sources, evidence) {
        return Some((column, 0));
    }
    let Expression::Binary(binary) = expression else {
        return None;
    };
    let (cast_expression, delta) = match binary.operator() {
        BinaryOperator::Add => {
            if let Some(value) = signed_integer_literal(binary.right()) {
                (binary.left(), i128::from(value))
            } else {
                (
                    binary.right(),
                    i128::from(signed_integer_literal(binary.left())?),
                )
            }
        }
        BinaryOperator::Subtract => (
            binary.left(),
            -i128::from(signed_integer_literal(binary.right())?),
        ),
        _ => return None,
    };
    let Expression::SignedIntegerCast(cast) = cast_expression else {
        return None;
    };
    let column = identity_integer_column(cast.expression())?;
    let bounds = evidence(&resolve_column(column, sources))?;
    let magnitude = 1_i128 << (u32::from(cast.target_bits()) - 1);
    let minimum = bounds.minimum.checked_add(delta)?;
    let maximum = bounds.maximum.checked_add(delta)?;
    (bounds.minimum >= -magnitude
        && bounds.maximum < magnitude
        && minimum >= -magnitude
        && maximum < magnitude)
        .then_some((column, delta))
}

// Unary plus and arithmetic with zero are identity transformations on
// signed integer columns, including SQL NULL. Other arithmetic can overflow
// or change source values and therefore must remain residual.
fn identity_integer_column(expression: &Expression) -> Option<&crate::protocol::ColumnExpression> {
    use crate::protocol::{BinaryOperator, UnaryOperator};
    match expression {
        Expression::Column(column) => Some(column),
        Expression::Unary(unary) if unary.operator() == UnaryOperator::Plus => {
            identity_integer_column(unary.operand())
        }
        Expression::Binary(binary) => match binary.operator() {
            BinaryOperator::Add if signed_integer_literal(binary.right()) == Some(0) => {
                identity_integer_column(binary.left())
            }
            BinaryOperator::Add if signed_integer_literal(binary.left()) == Some(0) => {
                identity_integer_column(binary.right())
            }
            BinaryOperator::Subtract if signed_integer_literal(binary.right()) == Some(0) => {
                identity_integer_column(binary.left())
            }
            _ => None,
        },
        _ => None,
    }
}

fn signed_integer_literal(expression: &Expression) -> Option<i64> {
    use crate::protocol::UnaryOperator;
    fn number(expression: &Expression) -> Option<i128> {
        match expression {
            Expression::Literal(literal) if literal.literal_type() == LiteralType::Integer => {
                let LiteralValue::Number(value) = literal.value() else {
                    return None;
                };
                value.parse::<i128>().ok()
            }
            Expression::Unary(unary) => match unary.operator() {
                UnaryOperator::Plus => number(unary.operand()),
                UnaryOperator::Minus => number(unary.operand())?.checked_neg(),
                UnaryOperator::BitwiseNot => None,
            },
            _ => None,
        }
    }
    i64::try_from(number(expression)?).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SqlTruth {
    True,
    False,
    Unknown,
}

impl SqlTruth {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::True,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::False,
        }
    }
}

#[derive(Default)]
struct ColumnThresholds {
    integers: BTreeSet<i128>,
    prefixes: BTreeSet<String>,
}

fn collect_comparison_literals(
    condition: &BooleanRowConstraint,
    thresholds: &mut BTreeMap<ColumnRef, ColumnThresholds>,
) {
    match condition {
        BooleanRowConstraint::All(operands) | BooleanRowConstraint::Any(operands) => {
            for operand in operands.iter() {
                collect_comparison_literals(operand, thresholds);
            }
        }
        BooleanRowConstraint::NullTest { column, .. } => {
            thresholds.entry(column.clone()).or_default();
        }
        BooleanRowConstraint::IntegerComparison {
            column, literal, ..
        } => {
            thresholds
                .entry(column.clone())
                .or_default()
                .integers
                .insert(i128::from(*literal));
        }
        BooleanRowConstraint::StringPrefix { column, prefix, .. } => {
            thresholds
                .entry(column.clone())
                .or_default()
                .prefixes
                .insert(prefix.clone());
        }
        BooleanRowConstraint::Residual { .. } => {}
    }
}

/// Reprove the *same* physical row's joint TRUE or NOT TRUE requirements.
/// TRUE of every filter is the truth of their AND; NOT TRUE of every
/// filter is NOT TRUE of their OR, including SQL UNKNOWN. An empty assignment
/// search is unknown (e.g. a budget limit), not evidence of impossibility.
/// Detached string predicates have lost the required collation attestation.
pub(crate) fn conjoin_physical_row_truths(
    source: &str,
    truth: BooleanTruthCase,
    conditions: &[&BooleanRowConstraint],
    column_known: impl Fn(&ColumnRef) -> bool,
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<(BooleanRowConstraint, bool)> {
    if conditions.len() < 2
        || conditions
            .iter()
            .any(|condition| !condition.is_exact() || condition.contains_string_prefix())
    {
        return None;
    }
    let operands = BooleanOperands::new(
        conditions
            .iter()
            .map(|condition| (*condition).clone())
            .collect(),
    )?;
    let joint = match truth {
        BooleanTruthCase::True => BooleanRowConstraint::All(operands),
        BooleanTruthCase::NotTrue => BooleanRowConstraint::Any(operands),
    };
    let mut columns = Vec::new();
    joint.columns(&mut columns);
    if columns.is_empty()
        || columns
            .iter()
            .any(|column| column.relation() != Some(source) || !column_known(column))
    {
        return None;
    }
    let possible = possible_joint_truths(&joint, &integer_evidence, &|_| None, None);
    if possible.is_empty() {
        return None;
    }
    let satisfiable = match truth {
        BooleanTruthCase::True => possible.contains(&SqlTruth::True),
        BooleanTruthCase::NotTrue => {
            possible.contains(&SqlTruth::False) || possible.contains(&SqlTruth::Unknown)
        }
    };
    Some((joint, satisfiable))
}

/// Recheck independently proven source-row conditions against one physical
/// assignment. Exact FALSE/UNKNOWN are both SQL NOT TRUE, not Rust negation.
/// Returning None means type, collation, source binding or search evidence is
/// insufficient; only Some(false) justifies an unsatisfiable classification.
pub(crate) fn jointly_satisfiable_physical_truths(
    source: &str,
    requirements: &[(&BooleanRowConstraint, BooleanTruthCase)],
    column_known: impl Fn(&ColumnRef) -> bool,
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<bool> {
    if requirements.is_empty()
        || requirements
            .iter()
            .any(|(condition, _)| !condition.is_exact() || condition.contains_string_prefix())
    {
        return None;
    }
    let conditions = requirements
        .iter()
        .map(|(condition, _)| *condition)
        .collect::<Vec<_>>();
    let mut columns = Vec::new();
    for condition in &conditions {
        condition.columns(&mut columns);
    }
    if columns.is_empty()
        || columns
            .iter()
            .any(|column| column.relation() != Some(source) || !column_known(column))
    {
        return None;
    }
    let truths = possible_joint_truth_vectors(&conditions, &integer_evidence, &|_| None, None);
    if truths.is_empty() {
        return None;
    }
    Some(truths.iter().any(|row_truths| {
        row_truths
            .iter()
            .zip(requirements)
            .all(|(actual, (_, required))| match required {
                BooleanTruthCase::True => *actual == SqlTruth::True,
                BooleanTruthCase::NotTrue => *actual != SqlTruth::True,
            })
    }))
}

fn possible_joint_truths(
    constraint: &BooleanRowConstraint,
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: &impl Fn(&ColumnRef) -> Option<StringEvidence>,
    restrictions: Option<&BTreeMap<String, ColumnRestriction>>,
) -> BTreeSet<SqlTruth> {
    possible_joint_truth_vectors(
        &[constraint],
        integer_evidence,
        string_evidence,
        restrictions,
    )
    .into_iter()
    .filter_map(|truths| truths.into_iter().next())
    .collect()
}

/// Enumerate coupled truth regions for the *same row* across several source
/// predicates. The bounded domain representatives preserve SQL three-valued
/// truth; missing type evidence and assignment explosion remain unknown.
fn possible_joint_truth_vectors(
    conditions: &[&BooleanRowConstraint],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: &impl Fn(&ColumnRef) -> Option<StringEvidence>,
    restrictions: Option<&BTreeMap<String, ColumnRestriction>>,
) -> BTreeSet<Vec<SqlTruth>> {
    const MAX_ASSIGNMENTS: usize = 4096;
    let mut thresholds = BTreeMap::new();
    for condition in conditions {
        collect_comparison_literals(condition, &mut thresholds);
    }
    let mut assignments = vec![BTreeMap::new()];
    for (column, literals) in thresholds {
        let restriction = restrictions.and_then(|items| items.get(column.name()));
        let integer_bounds = integer_evidence(&column);
        let string_bounds = string_evidence(&column);
        if !literals.integers.is_empty() && !literals.prefixes.is_empty() {
            // A single source column cannot simultaneously be an integer and a
            // string under the scoped typed comparison semantics.
            return BTreeSet::new();
        }
        let mut values = BTreeSet::<Option<RowScalar>>::new();
        if let Some(allowed) = restriction.and_then(|item| item.accepted.as_ref()) {
            for value in allowed {
                let fits = match value {
                    RowScalar::Integer(value) if !literals.integers.is_empty() => integer_bounds
                        .is_some_and(|bounds| bounds.minimum <= *value && *value <= bounds.maximum),
                    RowScalar::String(value) if !literals.prefixes.is_empty() => string_bounds
                        .is_some_and(|bounds| {
                            bounds
                                .max_chars
                                .is_none_or(|max| value.chars().count() as u64 <= max)
                        }),
                    // A pure NULL test admits a source value of any scalar kind.
                    _ if literals.integers.is_empty() && literals.prefixes.is_empty() => true,
                    _ => false,
                };
                if fits {
                    values.insert(Some(value.clone()));
                }
            }
        } else if !literals.integers.is_empty() {
            let Some(bounds) = integer_bounds else {
                return BTreeSet::new();
            };
            values.insert(Some(RowScalar::Integer(bounds.minimum)));
            values.insert(Some(RowScalar::Integer(bounds.maximum)));
            for literal in literals.integers {
                for value in [literal - 1, literal, literal + 1] {
                    if bounds.minimum <= value && value <= bounds.maximum {
                        values.insert(Some(RowScalar::Integer(value)));
                    }
                }
            }
        } else if !literals.prefixes.is_empty() {
            let Some(bounds) = string_bounds else {
                return BTreeSet::new();
            };
            // Under attested binary/no-padding semantics, every ASCII prefix
            // pattern changes truth only at its prefix. Each literal prefix
            // and the empty string give representatives of the joint truth
            // regions, including overlapping and negated prefixes.
            values.insert(Some(RowScalar::String(String::new())));
            for prefix in literals.prefixes {
                if bounds
                    .max_chars
                    .is_none_or(|max| prefix.chars().count() as u64 <= max)
                {
                    values.insert(Some(RowScalar::String(prefix)));
                }
            }
        } else {
            // A NULL-only test only distinguishes NULL from non-NULL.
            values.insert(Some(RowScalar::Integer(0)));
        }
        if !restriction.is_some_and(|item| item.not_null) {
            values.insert(None);
        }
        if assignments.len().saturating_mul(values.len()) > MAX_ASSIGNMENTS {
            return BTreeSet::new();
        }
        let mut expanded = Vec::new();
        for assignment in assignments {
            for value in &values {
                let mut candidate = assignment.clone();
                candidate.insert(column.clone(), value.clone());
                expanded.push(candidate);
            }
        }
        assignments = expanded;
    }
    assignments
        .iter()
        .filter_map(|assignment| {
            conditions
                .iter()
                .map(|condition| eval_joint_truth(condition, assignment))
                .collect::<Option<Vec<_>>>()
        })
        .collect()
}

fn eval_joint_truth(
    condition: &BooleanRowConstraint,
    assignment: &BTreeMap<ColumnRef, Option<RowScalar>>,
) -> Option<SqlTruth> {
    match condition {
        BooleanRowConstraint::All(operands) => {
            let mut truth = SqlTruth::True;
            for operand in operands.iter() {
                truth = truth.and(eval_joint_truth(operand, assignment)?);
            }
            Some(truth)
        }
        BooleanRowConstraint::Any(operands) => {
            let mut truth = SqlTruth::False;
            for operand in operands.iter() {
                truth = truth.or(eval_joint_truth(operand, assignment)?);
            }
            Some(truth)
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            let is_null = assignment.get(column)?.is_none();
            Some(if is_null != *negated {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => {
            use crate::protocol::ComparisonOperator as Op;
            let value = match assignment.get(column)? {
                Some(RowScalar::Integer(value)) => *value,
                None => return Some(SqlTruth::Unknown),
                _ => return None,
            };
            let literal = i128::from(*literal);
            let result = match operator {
                Op::Eq => value == literal,
                Op::Neq => value != literal,
                Op::Lt => value < literal,
                Op::Lte => value <= literal,
                Op::Gt => value > literal,
                Op::Gte => value >= literal,
                Op::IsDistinctFrom | Op::IsNotDistinctFrom => return None,
            };
            Some(if result {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated,
        } => {
            let value = match assignment.get(column)? {
                Some(RowScalar::String(value)) => value,
                None => return Some(SqlTruth::Unknown),
                _ => return None,
            };
            let matching = value.starts_with(prefix) != *negated;
            Some(if matching {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::Residual { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_noninteger_conjuncts_preserve_integer_domain() {
        let a = ColumnRef::new(Some("t".to_string()), "a".to_string());
        let b = ColumnRef::new(Some("t".to_string()), "b".to_string());
        for other in [
            BooleanRowConstraint::NullTest {
                column: b.clone(),
                negated: false,
            },
            BooleanRowConstraint::StringPrefix {
                column: b.clone(),
                prefix: "ab".to_string(),
                negated: false,
            },
        ] {
            let mut witness = BooleanWitness {
                source_relation: "t".to_string(),
                condition: BooleanRowConstraint::All(
                    BooleanOperands::new(vec![
                        BooleanRowConstraint::IntegerComparison {
                            column: a.clone(),
                            operator: ComparisonOperator::Gt,
                            literal: 2,
                        },
                        other,
                    ])
                    .expect("two operands"),
                ),
                qualifying: BooleanWitnessDirection::Residual {
                    reason: "not yet checked".to_string(),
                },
                rejected: BooleanWitnessDirection::Residual {
                    reason: "not yet checked".to_string(),
                },
                integer_bounds: BTreeMap::from([(
                    a.clone(),
                    SignedIntegerEvidence {
                        minimum: i128::from(i32::MIN),
                        maximum: i128::from(i32::MAX),
                    },
                )]),
                string_bounds: BTreeMap::from([(b.clone(), StringEvidence { max_chars: Some(8) })]),
                comparison_assumptions: BTreeSet::from([
                    ComparisonAssumption::BinaryCollation,
                    ComparisonAssumption::NoCharPadding,
                ]),
                source_constraints: Vec::new(),
            };
            witness.recheck_truth_directions();
            assert!(matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            ));

            let domains = witness.qualifying_conjunctive_domains();
            let [domain] = domains.as_slice() else {
                panic!("expected one integer domain from the mixed conjunction");
            };
            assert_eq!(domain.column(), &a);
            assert_eq!(
                domain.domain(),
                &comparison_domain(
                    ComparisonOperator::Gt,
                    &LiteralExpression::new(
                        LiteralType::Integer,
                        LiteralValue::Number("2".to_string()),
                    ),
                )
            );
        }
    }
}
