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
    LiteralExpression, LiteralType, LiteralValue, Predicate, SetMode, SourceRelation, ValueDomain,
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
    /// Ordinary comparison of a source column against literal SQL NULL.
    /// Its truth is UNKNOWN for both NULL and non-NULL values.
    UnknownNullComparison {
        /// Resolved source column.
        column: ColumnRef,
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
    /// Compare two signed integer source columns on the same source row.
    /// The two source datatypes must be independently certified and ordinary
    /// NULL comparisons retain SQL UNKNOWN.
    IntegerColumnComparison {
        /// Resolved left source column.
        left: ColumnRef,
        /// SQL comparison operator.
        operator: ComparisonOperator,
        /// Resolved right source column.
        right: ColumnRef,
    },
    /// Compare a string column against a literal under explicitly attested
    /// binary collation and no-padding rules.
    StringEquality {
        /// Resolved string-typed source column.
        column: ColumnRef,
        /// Equals, not-equals or their two-valued NULL-safe counterparts.
        operator: ComparisonOperator,
        /// Literal string value.
        literal: String,
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
            Self::NullTest { .. }
            | Self::UnknownNullComparison { .. }
            | Self::IntegerComparison { .. }
            | Self::IntegerColumnComparison { .. }
            | Self::StringEquality { .. }
            | Self::StringPrefix { .. } => true,
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
            Self::UnknownNullComparison { column } => Some(Self::UnknownNullComparison {
                column: mapped.get(column)?.clone(),
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
            Self::IntegerColumnComparison {
                left,
                operator,
                right,
            } => Some(Self::IntegerColumnComparison {
                left: mapped.get(left)?.clone(),
                operator: *operator,
                right: mapped.get(right)?.clone(),
            }),
            Self::StringEquality {
                column,
                operator,
                literal,
            } => Some(Self::StringEquality {
                column: mapped.get(column)?.clone(),
                operator: *operator,
                literal: literal.clone(),
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
            Self::IntegerComparison { .. }
            | Self::IntegerColumnComparison { .. }
            | Self::StringEquality { .. }
            | Self::StringPrefix { .. } => true,
            Self::NullTest { .. } | Self::UnknownNullComparison { .. } | Self::Residual { .. } => false,
        }
    }

    fn requires_binary_string_assumptions(&self) -> bool {
        match self {
            Self::All(operands) | Self::Any(operands) => {
                operands.iter().any(Self::requires_binary_string_assumptions)
            }
            Self::StringEquality { .. } | Self::StringPrefix { .. } => true,
            _ => false,
        }
    }

    // For independent signed-integer and NULL tests, integer threshold
    // representatives partition the complete three-valued truth regions.
    // Correlated columns and string comparisons only provide existence samples,
    // not complete absence proofs.
    fn has_complete_scalar_regions(&self) -> bool {
        match self {
            Self::All(operands) | Self::Any(operands) => {
                operands.iter().all(Self::has_complete_scalar_regions)
            }
            Self::NullTest { .. } | Self::UnknownNullComparison { .. } | Self::IntegerComparison { .. } => true,
            Self::IntegerColumnComparison { .. }
            | Self::StringEquality { .. }
            | Self::StringPrefix { .. }
            | Self::Residual { .. } => false,
        }
    }

    fn contains_column_comparison(&self) -> bool {
        match self {
            Self::All(operands) | Self::Any(operands) => {
                operands.iter().any(Self::contains_column_comparison)
            }
            Self::IntegerColumnComparison { .. } => true,
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
            | Self::UnknownNullComparison { column }
            | Self::IntegerComparison { column, .. }
            | Self::StringEquality { column, .. }
            | Self::StringPrefix { column, .. } => {
                output.push(column.clone());
            }
            Self::IntegerColumnComparison { left, right, .. } => {
                output.push(left.clone());
                output.push(right.clone());
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
    /// An exhaustive typed region proof excludes the requested truth outcome.
    Impossible,
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
                // A comparison against SQL NULL cannot qualify a row, and
                // contributes no independent value domain.
                BooleanRowConstraint::UnknownNullComparison { .. } => true,
                // Coupled source-column comparisons are not independent scalar
                // domains, even though they can be row-local exact witnesses.
                BooleanRowConstraint::IntegerColumnComparison { .. } => true,
                BooleanRowConstraint::StringEquality {
                    column,
                    operator,
                    literal,
                } => {
                    let derived = comparison_domain(
                        *operator,
                        &LiteralExpression::new(
                            LiteralType::String,
                            LiteralValue::Text(literal.clone()),
                        ),
                    );
                    domains
                        .entry(column.clone())
                        .and_modify(|existing| *existing = intersect_domains(existing, &derived))
                        .or_insert(derived);
                    true
                }
                BooleanRowConstraint::NullTest { column, negated } => {
                    let mode = if *negated {
                        SetMode::Exclude
                    } else {
                        SetMode::Include
                    };
                    let derived = ValueDomain::set(
                        mode,
                        vec![LiteralExpression::new(
                            LiteralType::Null,
                            LiteralValue::Null,
                        )],
                    );
                    domains
                        .entry(column.clone())
                        .and_modify(|existing| {
                            *existing = intersect_domains(existing, &derived);
                        })
                        .or_insert(derived);
                    true
                }
                // Pattern constraints have comparison-environment preconditions,
                // not a general independent scalar interval.
                BooleanRowConstraint::StringPrefix { .. } => true,
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
        // Equal datatypes do not attest the same collation or padding laws
        // across warehouse materialization boundaries.
        if self.condition.requires_binary_string_assumptions() {
            return None;
        }
        let mut columns = Vec::new();
        self.condition.columns(&mut columns);
        let mut mapping = BTreeMap::new();
        for column in columns {
            let std::collections::btree_map::Entry::Vacant(entry) = mapping.entry(column) else {
                continue;
            };
            let mapped = resolve(entry.key())?;
            if self.condition.requires_source_type_evidence()
                && !equivalent_type(entry.key(), &mapped)
            {
                return None;
            }
            entry.insert(mapped);
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
        let prefix_licensed = !self.condition.requires_binary_string_assumptions()
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
        let complete_scalar_regions = candidate
            && prefix_licensed
            && restrictions.is_some()
            && self.condition.has_complete_scalar_regions()
            && !cases.is_empty();
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
        } else if complete_scalar_regions {
            BooleanWitnessDirection::Impossible
        } else {
            BooleanWitnessDirection::Residual {
                reason: reason.to_string(),
            }
        };
        self.rejected = if cases.contains(&SqlTruth::False) || cases.contains(&SqlTruth::Unknown) {
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        } else if complete_scalar_regions {
            BooleanWitnessDirection::Impossible
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
                    RelationConstraint::ForeignKey(_) | RelationConstraint::CheckComparison(_) => {
                        return None
                    }
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
        Predicate::And(_)
        | Predicate::Or(_)
        | Predicate::Not(_)
        | Predicate::Between(_)
        | Predicate::In(_)
        | Predicate::LikePrefix(_)
        | Predicate::IsNull(_) => true,
        Predicate::Comparison(comparison) => {
            if matches!(
                comparison.operator(),
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ) || [comparison.left(), comparison.right()].iter().any(|expression| {
                matches!(expression, Expression::Literal(literal) if matches!(literal.value(), LiteralValue::Null))
            }) {
                return true;
            }
            // A lone direct column-to-literal comparison already has a scalar
            // domain; correlated columns require a coupled typed witness.
            if matches!(
                (comparison.left(), comparison.right()),
                (Expression::Column(_), Expression::Column(_))
            ) || [comparison.left(), comparison.right()].iter().any(|expression| {
                matches!(
                    expression,
                    Expression::Literal(value) if value.literal_type() == LiteralType::String
                )
            }) {
                return true;
            }
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
            normalize_null_test(test.expression(), test.negated(), sources, integer_evidence)
                .unwrap_or_else(|| residual("NULL test involves an unproven computed expression"))
        }
        Predicate::Comparison(comparison) => {
            // Ordinary SQL comparison with literal NULL yields UNKNOWN for
            // every source value. Keep that truth leaf inside IN/BETWEEN/NOT
            // instead of discarding an entire otherwise-proven predicate.
            if !matches!(
                comparison.operator(),
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ) {
                let column = match (comparison.left(), comparison.right()) {
                    (Expression::Column(column), Expression::Literal(literal))
                    | (Expression::Literal(literal), Expression::Column(column))
                        if matches!(literal.value(), LiteralValue::Null) => Some(column),
                    _ => None,
                };
                if let Some(column) = column {
                    return BooleanRowConstraint::UnknownNullComparison {
                        column: resolve_column(column, sources),
                    };
                }
            }
            // Null-safe comparison with a literal NULL is a two-valued NULL
            // test for any datatype; ordinary = NULL must stay UNKNOWN.
            if matches!(
                comparison.operator(),
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ) {
                let column = match (comparison.left(), comparison.right()) {
                    (Expression::Column(column), Expression::Literal(literal))
                        if matches!(literal.value(), LiteralValue::Null) =>
                    {
                        Some(column)
                    }
                    (Expression::Literal(literal), Expression::Column(column))
                        if matches!(literal.value(), LiteralValue::Null) =>
                    {
                        Some(column)
                    }
                    _ => None,
                };
                if let Some(column) = column {
                    return BooleanRowConstraint::NullTest {
                        column: resolve_column(column, sources),
                        negated: comparison.operator() == ComparisonOperator::IsDistinctFrom,
                    };
                }
            }
            // String comparisons are exact only for catalog-backed variable
            // strings and attested binary/no-padding comparison settings.
            if matches!(
                comparison.operator(),
                ComparisonOperator::Eq
                    | ComparisonOperator::Neq
                    | ComparisonOperator::IsDistinctFrom
                    | ComparisonOperator::IsNotDistinctFrom
            ) {
                let string_pair = match (comparison.left(), comparison.right()) {
                    (Expression::Column(column), Expression::Literal(literal))
                    | (Expression::Literal(literal), Expression::Column(column))
                        if literal.literal_type() == LiteralType::String =>
                    {
                        Some((column, literal))
                    }
                    _ => None,
                };
                if let Some((column, literal)) = string_pair {
                    let resolved = resolve_column(column, sources);
                    let LiteralValue::Text(value) = literal.value() else {
                        return residual("string comparison is not a canonical text literal");
                    };
                    if string_evidence(&resolved).is_none() {
                        return residual("string comparison requires variable-length string type");
                    }
                    return BooleanRowConstraint::StringEquality {
                        column: resolved,
                        operator: comparison.operator(),
                        literal: value.clone(),
                    };
                }
            }
            if let (Expression::Column(left), Expression::Column(right)) =
                (comparison.left(), comparison.right())
            {
                let left = resolve_column(left, sources);
                let right = resolve_column(right, sources);
                if sources.len() != 1
                    || left.relation() != Some(sources[0].name())
                    || right.relation() != Some(sources[0].name())
                {
                    // A self-join's two aliases refer to different row
                    // variables even when they share one physical table.
                    return residual(
                        "cross-relation comparison requires a relational witness, not one source row",
                    );
                }
                if integer_evidence(&left).is_none() || integer_evidence(&right).is_none() {
                    return residual(
                        "cross-column comparison requires signed integer type evidence",
                    );
                }
                return BooleanRowConstraint::IntegerColumnComparison {
                    left,
                    operator: comparison.operator(),
                    right,
                };
            }
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
        Predicate::Between(test) => {
            let (lower, upper, connective) = if test.negated() {
                (ComparisonOperator::Lt, ComparisonOperator::Gt, false)
            } else {
                (ComparisonOperator::Gte, ComparisonOperator::Lte, true)
            };
            let operands = vec![
                normalize_integer_comparison(
                    test.expression(),
                    lower,
                    test.lower(),
                    sources,
                    integer_evidence,
                    string_evidence,
                ),
                normalize_integer_comparison(
                    test.expression(),
                    upper,
                    test.upper(),
                    sources,
                    integer_evidence,
                    string_evidence,
                ),
            ];
            let Some(operands) = BooleanOperands::new(operands) else {
                return residual("BETWEEN requires both comparison bounds");
            };
            if connective {
                BooleanRowConstraint::All(operands)
            } else {
                BooleanRowConstraint::Any(operands)
            }
        }
        Predicate::In(test) => {
            // SQL IN is OR of equalities; NOT IN is AND of inequalities.
            // A NULL member deliberately stays residual unless the solver can
            // prove its UNKNOWN effect rather than silently removing it.
            let operator = if test.negated() {
                ComparisonOperator::Neq
            } else {
                ComparisonOperator::Eq
            };
            let mut operands = test.values().iter().map(|value| {
                normalize_integer_comparison(
                    test.expression(),
                    operator,
                    value,
                    sources,
                    integer_evidence,
                    string_evidence,
                )
            });
            let Some(first) = operands.next() else {
                return residual("empty IN list has no proven row semantics");
            };
            let Some(second) = operands.next() else {
                return first;
            };
            let mut children = vec![first, second];
            children.extend(operands);
            let Some(children) = BooleanOperands::new(children) else {
                return residual("IN list requires at least one value");
            };
            if test.negated() {
                BooleanRowConstraint::All(children)
            } else {
                BooleanRowConstraint::Any(children)
            }
        }
        Predicate::Not(negation) => negate_constraint(normalize(
            negation.operand(),
            sources,
            integer_evidence,
            string_evidence,
        )),
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
        _ => residual("computed, function, collation, cast or other predicate is not invertible"),
    }
}

// A NULL test is two-valued even when its input is nullable. Only
// independently certified, total integer expressions may be reduced to
// source-column NULL tests. Ordinary TRY_CAST or opaque function calls can
// fail or change NULL behavior and deliberately do not enter this proof.
fn normalize_null_test(
    expression: &Expression,
    negated: bool,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<BooleanRowConstraint> {
    let test = |column: &crate::protocol::ColumnExpression, negated| {
        BooleanRowConstraint::NullTest {
            column: resolve_column(column, sources),
            negated,
        }
    };
    if let Expression::Column(column) = expression {
        return Some(test(column, negated));
    }
    if let Some((column, _)) = affine_integer_operand(expression, sources, integer_evidence) {
        if integer_evidence(&resolve_column(column, sources)).is_some() {
            return Some(test(column, negated));
        }
    }
    let Expression::Function(function) = expression else {
        return None;
    };
    if function.distinct() {
        return None;
    }
    if function.name().eq_ignore_ascii_case("COALESCE")
        && function.arguments().len() >= 2
    {
        let mut columns = Vec::new();
        for argument in function.arguments() {
            match argument {
                Expression::Literal(literal)
                    if matches!(literal.value(), LiteralValue::Null) => {}
                Expression::Column(column)
                    if integer_evidence(&resolve_column(column, sources)).is_some() =>
                {
                    columns.push(test(column, negated));
                }
                _ => return None,
            }
        }
        return match columns.len() {
            0 => None,
            1 => columns.pop(),
            _ => BooleanOperands::new(columns).map(|operands| {
                if negated {
                    BooleanRowConstraint::Any(operands)
                } else {
                    BooleanRowConstraint::All(operands)
                }
            }),
        };
    }
    if function.name().eq_ignore_ascii_case("NULLIF") {
        let [Expression::Column(left), Expression::Column(right)] = function.arguments() else {
            return None;
        };
        let left_column = resolve_column(left, sources);
        let right_column = resolve_column(right, sources);
        integer_evidence(&left_column)?;
        integer_evidence(&right_column)?;
        let comparison = BooleanRowConstraint::IntegerColumnComparison {
            left: left_column,
            right: right_column,
            operator: if negated {
                ComparisonOperator::IsDistinctFrom
            } else {
                ComparisonOperator::IsNotDistinctFrom
            },
        };
        let operands = BooleanOperands::new(vec![test(left, negated), comparison])?;
        return Some(if negated {
            BooleanRowConstraint::All(operands)
        } else {
            BooleanRowConstraint::Any(operands)
        });
    }
    None
}

// Negation is pushed to proven leaves. De Morgan's laws preserve SQL
// UNKNOWN, while swapping comparison operators keeps NULL-sensitive
// comparisons unknown and NULL-safe comparisons two-valued.
fn negate_constraint(condition: BooleanRowConstraint) -> BooleanRowConstraint {
    match condition {
        BooleanRowConstraint::All(operands) => BooleanRowConstraint::Any(
            BooleanOperands::new(operands.iter().cloned().map(negate_constraint).collect())
                .expect("negation preserves the operand count"),
        ),
        BooleanRowConstraint::Any(operands) => BooleanRowConstraint::All(
            BooleanOperands::new(operands.iter().cloned().map(negate_constraint).collect())
                .expect("negation preserves the operand count"),
        ),
        BooleanRowConstraint::NullTest { column, negated } => BooleanRowConstraint::NullTest {
            column,
            negated: !negated,
        },
        // SQL NOT UNKNOWN remains UNKNOWN.
        BooleanRowConstraint::UnknownNullComparison { column } => {
            BooleanRowConstraint::UnknownNullComparison { column }
        },
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => {
            let operator = negated_comparison_operator(operator);
            BooleanRowConstraint::IntegerComparison {
                column,
                operator,
                literal,
            }
        }
        BooleanRowConstraint::IntegerColumnComparison {
            left,
            operator,
            right,
        } => BooleanRowConstraint::IntegerColumnComparison {
            left,
            operator: negated_comparison_operator(operator),
            right,
        },
        BooleanRowConstraint::StringEquality {
            column,
            operator,
            literal,
        } => BooleanRowConstraint::StringEquality {
            column,
            operator: negated_comparison_operator(operator),
            literal,
        },
        BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated,
        } => BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated: !negated,
        },
        BooleanRowConstraint::Residual { reason } => BooleanRowConstraint::Residual { reason },
    }
}

fn negated_comparison_operator(operator: ComparisonOperator) -> ComparisonOperator {
    match operator {
        ComparisonOperator::Eq => ComparisonOperator::Neq,
        ComparisonOperator::Neq => ComparisonOperator::Eq,
        ComparisonOperator::Lt => ComparisonOperator::Gte,
        ComparisonOperator::Lte => ComparisonOperator::Gt,
        ComparisonOperator::Gt => ComparisonOperator::Lte,
        ComparisonOperator::Gte => ComparisonOperator::Lt,
        ComparisonOperator::IsDistinctFrom => ComparisonOperator::IsNotDistinctFrom,
        ComparisonOperator::IsNotDistinctFrom => ComparisonOperator::IsDistinctFrom,
    }
}

fn normalize_integer_comparison(
    left: &Expression,
    operator: ComparisonOperator,
    right: &Expression,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: &impl Fn(&ColumnRef) -> Option<StringEvidence>,
) -> BooleanRowConstraint {
    normalize(
        &Predicate::Comparison(crate::protocol::ComparisonPredicate::new(
            left.clone(),
            operator,
            right.clone(),
        )),
        sources,
        integer_evidence,
        string_evidence,
    )
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

#[derive(Default, Clone)]
struct ColumnThresholds {
    integers: BTreeSet<i128>,
    prefixes: BTreeSet<String>,
    strings: BTreeSet<String>,
    compared_columns: BTreeSet<ColumnRef>,
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
        BooleanRowConstraint::NullTest { column, .. }
        | BooleanRowConstraint::UnknownNullComparison { column } => {
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
        BooleanRowConstraint::IntegerColumnComparison { left, right, .. } => {
            thresholds
                .entry(left.clone())
                .or_default()
                .compared_columns
                .insert(right.clone());
            thresholds
                .entry(right.clone())
                .or_default()
                .compared_columns
                .insert(left.clone());
        }
        BooleanRowConstraint::StringEquality { column, literal, .. } => {
            thresholds
                .entry(column.clone())
                .or_default()
                .strings
                .insert(literal.clone());
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
            .any(|condition| !condition.is_exact() || condition.requires_binary_string_assumptions())
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
    if !satisfiable && joint.contains_column_comparison() {
        // Region representatives establish concrete existence, but cannot
        // establish universal absence for correlated source columns.
        return None;
    }
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
            .any(|(condition, _)| !condition.is_exact() || condition.requires_binary_string_assumptions())
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
    let satisfiable = truths.iter().any(|row_truths| {
        row_truths
            .iter()
            .zip(requirements)
            .all(|(actual, (_, required))| match required {
                BooleanTruthCase::True => *actual == SqlTruth::True,
                BooleanTruthCase::NotTrue => *actual != SqlTruth::True,
            })
    });
    if !satisfiable
        && conditions
            .iter()
            .any(|condition| condition.contains_column_comparison())
    {
        return None;
    }
    Some(satisfiable)
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
    // Candidate values for correlated comparisons must include thresholds
    // referenced by the opposite column. This is a witness search, not an
    // exhaustive proof of impossible correlated assignments.
    let initial = thresholds.clone();
    for (column, region) in &initial {
        for other in &region.compared_columns {
            if let Some(other_region) = initial.get(other) {
                thresholds
                    .get_mut(column)
                    .expect("collected correlated column")
                    .integers
                    .extend(other_region.integers.iter().copied());
            }
        }
    }
    let mut assignments = vec![BTreeMap::new()];
    for (column, literals) in thresholds {
        let restriction = restrictions.and_then(|items| items.get(column.name()));
        let integer_bounds = integer_evidence(&column);
        let string_bounds = string_evidence(&column);
        let integer_required =
            !literals.integers.is_empty() || !literals.compared_columns.is_empty();
        let string_required = !literals.prefixes.is_empty() || !literals.strings.is_empty();
        if integer_required && string_required {
            // A single source column cannot simultaneously be an integer and a
            // string under the scoped typed comparison semantics.
            return BTreeSet::new();
        }
        let mut values = BTreeSet::<Option<RowScalar>>::new();
        if let Some(allowed) = restriction.and_then(|item| item.accepted.as_ref()) {
            for value in allowed {
                let fits = match value {
                    RowScalar::Integer(value) if integer_required => integer_bounds
                        .is_some_and(|bounds| bounds.minimum <= *value && *value <= bounds.maximum),
                    RowScalar::String(value) if string_required => string_bounds
                        .is_some_and(|bounds| {
                            bounds
                                .max_chars
                                .is_none_or(|max| value.chars().count() as u64 <= max)
                        }),
                    // A pure NULL test admits a source value of any scalar kind.
                    _ if !integer_required && !string_required => true,
                    _ => false,
                };
                if fits {
                    values.insert(Some(value.clone()));
                }
            }
        } else if integer_required {
            let Some(bounds) = integer_bounds else {
                return BTreeSet::new();
            };
            let mut pivots = BTreeSet::from([bounds.minimum, bounds.maximum]);
            pivots.extend(literals.integers);
            for other in &literals.compared_columns {
                let Some(other_bounds) = integer_evidence(other) else {
                    return BTreeSet::new();
                };
                pivots.insert(other_bounds.minimum);
                pivots.insert(other_bounds.maximum);
                if let Some(allowed) = restrictions
                    .and_then(|items| items.get(other.name()))
                    .and_then(|item| item.accepted.as_ref())
                {
                    for value in allowed {
                        if let RowScalar::Integer(value) = value {
                            pivots.insert(*value);
                        }
                    }
                }
            }
            for pivot in pivots {
                for value in [pivot.checked_sub(1), Some(pivot), pivot.checked_add(1)]
                    .into_iter()
                    .flatten()
                {
                    if bounds.minimum <= value && value <= bounds.maximum {
                        values.insert(Some(RowScalar::Integer(value)));
                    }
                }
            }
        } else if string_required {
            let Some(bounds) = string_bounds else {
                return BTreeSet::new();
            };
            let mut candidates = BTreeSet::from([
                String::new(),
                "!".to_string(),
                "z".to_string(),
                "0".to_string(),
            ]);
            candidates.extend(literals.strings);
            for prefix in literals.prefixes {
                candidates.insert(prefix.clone());
                candidates.insert(format!("{prefix}!"));
                candidates.insert(format!("{prefix}0"));
            }
            for value in candidates {
                if bounds
                    .max_chars
                    .is_none_or(|max| value.chars().count() as u64 <= max)
                {
                    values.insert(Some(RowScalar::String(value)));
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
        BooleanRowConstraint::UnknownNullComparison { column } => {
            assignment.get(column)?;
            Some(SqlTruth::Unknown)
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
                Some(RowScalar::Integer(value)) => Some(*value),
                None => None,
                _ => return None,
            };
            let literal = i128::from(*literal);
            let result = match operator {
                Op::IsDistinctFrom => value != Some(literal),
                Op::IsNotDistinctFrom => value == Some(literal),
                ordinary => {
                    let Some(value) = value else {
                        return Some(SqlTruth::Unknown);
                    };
                    match ordinary {
                        Op::Eq => value == literal,
                        Op::Neq => value != literal,
                        Op::Lt => value < literal,
                        Op::Lte => value <= literal,
                        Op::Gt => value > literal,
                        Op::Gte => value >= literal,
                        Op::IsDistinctFrom | Op::IsNotDistinctFrom => unreachable!("handled above"),
                    }
                }
            };
            Some(if result {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::IntegerColumnComparison {
            left,
            operator,
            right,
        } => {
            use crate::protocol::ComparisonOperator as Op;
            let read = |column: &ColumnRef| -> Option<Option<i128>> {
                match assignment.get(column)? {
                    Some(RowScalar::Integer(value)) => Some(Some(*value)),
                    None => Some(None),
                    _ => None,
                }
            };
            let lhs = read(left)?;
            let rhs = read(right)?;
            let result = match operator {
                Op::IsDistinctFrom => lhs != rhs,
                Op::IsNotDistinctFrom => lhs == rhs,
                ordinary => {
                    let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                        return Some(SqlTruth::Unknown);
                    };
                    match ordinary {
                        Op::Eq => lhs == rhs,
                        Op::Neq => lhs != rhs,
                        Op::Lt => lhs < rhs,
                        Op::Lte => lhs <= rhs,
                        Op::Gt => lhs > rhs,
                        Op::Gte => lhs >= rhs,
                        Op::IsDistinctFrom | Op::IsNotDistinctFrom => unreachable!("handled above"),
                    }
                }
            };
            Some(if result {
                SqlTruth::True
            } else {
                SqlTruth::False
            })
        }
        BooleanRowConstraint::StringEquality {
            column,
            operator,
            literal,
        } => {
            use crate::protocol::ComparisonOperator as Op;
            let value = match assignment.get(column)? {
                Some(RowScalar::String(value)) => Some(value.as_str()),
                None => None,
                _ => return None,
            };
            let result = match operator {
                Op::IsDistinctFrom => value != Some(literal.as_str()),
                Op::IsNotDistinctFrom => value == Some(literal.as_str()),
                Op::Eq | Op::Neq => {
                    let Some(value) = value else {
                        return Some(SqlTruth::Unknown);
                    };
                    let equal = value == literal;
                    if *operator == Op::Eq { equal } else { !equal }
                }
                _ => return None,
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
            let domain = domains
                .iter()
                .find(|domain| domain.column() == &a)
                .expect("exact integer comparison must retain its domain");
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
