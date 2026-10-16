//! Coupled source-row predicates that cannot be represented by independent column domains.
//!
//! Proof is deliberately narrow. An exact direction constrains a *single row* at the
//! originating relation boundary, retaining the logical tree and SQL's three-valued
//! WHERE semantics. No independent Cartesian sampling or SQL reparsing is implied.

use std::collections::{BTreeMap, BTreeSet};

mod numeric;

use numeric::{
    normalize_affine_comparison, normalize_modulo_range_comparison, total_integer_column,
};

use crate::constraints::{
    ConstraintEnforcement, ConstraintValue, RelationConstraint, RelationConstraintSet,
};

use crate::data_type::DataType;
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
    /// Compare two variable-length string source columns on the same source
    /// row under explicitly attested binary collation and no-padding rules.
    /// Only the equality family is modeled; text ordering has no proven
    /// collation order and stays residual.
    StringColumnComparison {
        /// Resolved left string-typed source column.
        left: ColumnRef,
        /// Equals, not-equals or their two-valued NULL-safe counterparts.
        operator: ComparisonOperator,
        /// Resolved right string-typed source column.
        right: ColumnRef,
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
            | Self::StringColumnComparison { .. }
            | Self::StringPrefix { .. } => true,
            Self::Residual { .. } => false,
        }
    }

    /// Whether this canonical predicate constrains the given physical column.
    /// Joint operator construction uses this to keep independently selected
    /// non-key values separate from join key assignments.
    pub(crate) fn references_column(&self, column: &ColumnRef) -> bool {
        let mut columns = Vec::new();
        self.columns(&mut columns);
        columns.contains(column)
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
            Self::StringColumnComparison {
                left,
                operator,
                right,
            } => Some(Self::StringColumnComparison {
                left: mapped.get(left)?.clone(),
                operator: *operator,
                right: mapped.get(right)?.clone(),
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
            | Self::StringColumnComparison { .. }
            | Self::StringPrefix { .. } => true,
            Self::NullTest { .. } | Self::UnknownNullComparison { .. } | Self::Residual { .. } => {
                false
            }
        }
    }

    fn requires_binary_string_assumptions(&self) -> bool {
        match self {
            Self::All(operands) | Self::Any(operands) => operands
                .iter()
                .any(Self::requires_binary_string_assumptions),
            Self::StringEquality { .. }
            | Self::StringColumnComparison { .. }
            | Self::StringPrefix { .. } => true,
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
            Self::NullTest { .. }
            | Self::UnknownNullComparison { .. }
            | Self::IntegerComparison { .. } => true,
            // A source value compared to itself has complete TRUE/FALSE/UNKNOWN
            // regions, unlike two independently ranged correlated columns.
            // The same holds for one text value compared to itself under
            // attested binary equality; distinct text columns only sample.
            Self::IntegerColumnComparison { left, right, .. } => left == right,
            Self::StringColumnComparison { left, right, .. } => left == right,
            Self::StringEquality { .. } | Self::StringPrefix { .. } | Self::Residual { .. } => {
                false
            }
        }
    }

    fn contains_column_comparison(&self) -> bool {
        match self {
            Self::All(operands) | Self::Any(operands) => {
                operands.iter().any(Self::contains_column_comparison)
            }
            Self::IntegerColumnComparison { left, right, .. } => left != right,
            Self::StringColumnComparison { left, right, .. } => left != right,
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
            Self::StringColumnComparison { left, right, .. } => {
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
                BooleanRowConstraint::StringColumnComparison { .. } => true,
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

    /// Transport a typed integer predicate through a caller-certified
    /// lossless widening materialization. The producer must independently
    /// prove every expression and hop are total and injective; the physical
    /// source width replaces (rather than inherits) the wider intermediate
    /// bounds before either truth direction is rechecked.
    pub(crate) fn mapped_to_physical_widened(
        &self,
        resolve: impl FnMut(&ColumnRef) -> Option<ColumnRef>,
        mut physical_integer: impl FnMut(&ColumnRef) -> Option<SignedIntegerEvidence>,
    ) -> Option<Self> {
        if !self.condition.is_exact()
            || self.condition.requires_binary_string_assumptions()
            || self.integer_bounds.is_empty()
        {
            return None;
        }
        let mut mapped = self.mapped_to_physical_certified(resolve, |_, _| true)?;
        let mut integer_bounds = BTreeMap::new();
        for column in mapped.integer_bounds.keys() {
            integer_bounds.insert(column.clone(), physical_integer(column)?);
        }
        mapped.integer_bounds = integer_bounds;
        mapped.recheck_truth_directions();
        Some(mapped)
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
        let symbolic_qualifying_impossible = candidate
            && prefix_licensed
            && restrictions.is_some()
            && !cases.contains(&SqlTruth::True)
            && restrictions.as_ref().is_some_and(|items| {
                true_region_empty(
                    &self.condition,
                    &|column| self.integer_bounds.get(column).copied(),
                    Some(items),
                )
            });
        let symbolic_rejected_impossible = candidate
            && prefix_licensed
            && restrictions.is_some()
            && !cases.contains(&SqlTruth::False)
            && !cases.contains(&SqlTruth::Unknown)
            && restrictions.as_ref().is_some_and(|items| {
                not_true_constraint(&self.condition).is_some_and(|not_true| {
                    true_region_empty(
                        &not_true,
                        &|column| self.integer_bounds.get(column).copied(),
                        Some(items),
                    )
                })
            });
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
        } else if complete_scalar_regions || symbolic_qualifying_impossible {
            BooleanWitnessDirection::Impossible
        } else {
            BooleanWitnessDirection::Residual {
                reason: reason.to_string(),
            }
        };
        self.rejected = if cases.contains(&SqlTruth::False) || cases.contains(&SqlTruth::Unknown) {
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        } else if complete_scalar_regions || symbolic_rejected_impossible {
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
    column_datatype: impl Fn(&ColumnRef) -> Option<DataType>,
) -> Option<BooleanWitness> {
    analyze_candidate(
        predicate,
        sources,
        integer_evidence,
        string_evidence,
        column_datatype,
        false,
    )
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
    column_datatype: impl Fn(&ColumnRef) -> Option<DataType>,
) -> Option<BooleanWitness> {
    analyze_candidate(
        predicate,
        sources,
        integer_evidence,
        string_evidence,
        column_datatype,
        true,
    )
}

fn analyze_candidate(
    predicate: Option<&Predicate>,
    sources: &[SourceRelation],
    integer_evidence: impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: impl Fn(&ColumnRef) -> Option<StringEvidence>,
    column_datatype: impl Fn(&ColumnRef) -> Option<DataType>,
    physical_scalar: bool,
) -> Option<BooleanWitness> {
    let predicate = predicate?;
    if !(is_witness_candidate(predicate)
        || physical_scalar && matches!(predicate, Predicate::Comparison(_) | Predicate::IsNull(_)))
    {
        return None;
    }
    let [source] = sources else {
        return None;
    };
    let condition = normalize(
        predicate,
        sources,
        &integer_evidence,
        &string_evidence,
        &column_datatype,
    );
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
        | Predicate::LikePrefix(_) => true,
        // Plain column NULL tests already participate in the canonical
        // scalar domain proof. Creating a second local witness can turn an
        // exact transitive copy into a spurious composition-domain residual.
        Predicate::IsNull(test) => !matches!(test.expression(), Expression::Column(_)),
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
            ) || [comparison.left(), comparison.right()]
                .iter()
                .any(|expression| {
                    matches!(
                        expression,
                        Expression::Literal(value) if value.literal_type() == LiteralType::String
                    )
                })
            {
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
                            | Expression::Function(_)
                            | Expression::Case(_)
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
    column_datatype: &impl Fn(&ColumnRef) -> Option<DataType>,
) -> BooleanRowConstraint {
    match predicate {
        Predicate::And(logical) | Predicate::Or(logical) => {
            let operands = logical
                .operands()
                .iter()
                .map(|item| {
                    normalize(
                        item,
                        sources,
                        integer_evidence,
                        string_evidence,
                        column_datatype,
                    )
                })
                .collect();
            match BooleanOperands::new(operands) {
                Some(operands) if matches!(predicate, Predicate::And(_)) => {
                    BooleanRowConstraint::All(operands)
                }
                Some(operands) => BooleanRowConstraint::Any(operands),
                None => residual("logical predicate has fewer than two operands"),
            }
        }
        Predicate::IsNull(test) => normalize_null_test(
            test.expression(),
            test.negated(),
            sources,
            integer_evidence,
            string_evidence,
            column_datatype,
        )
        .unwrap_or_else(|| {
            residual(&format!(
                "NULL test involves an unproven computed expression: {}",
                crate::protocol::expression_residual_label(test.expression()),
            ))
        }),
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
                        if matches!(literal.value(), LiteralValue::Null) =>
                    {
                        Some(column)
                    }
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
                // Attested binary equality between two variable-length text
                // columns is an exact source-row obligation once the caller
                // declares binary collation and no CHAR padding. Ordering
                // comparisons have no proven collation order and stay
                // residual, as do columns without variable-length evidence
                // (notably fixed-width CHAR, whose padding law is unproven).
                if matches!(
                    comparison.operator(),
                    ComparisonOperator::Eq
                        | ComparisonOperator::Neq
                        | ComparisonOperator::IsDistinctFrom
                        | ComparisonOperator::IsNotDistinctFrom
                ) && string_evidence(&left).is_some()
                    && string_evidence(&right).is_some()
                {
                    return BooleanRowConstraint::StringColumnComparison {
                        left,
                        operator: comparison.operator(),
                        right,
                    };
                }
                if matches!(
                    comparison.operator(),
                    ComparisonOperator::Lt
                        | ComparisonOperator::Lte
                        | ComparisonOperator::Gt
                        | ComparisonOperator::Gte
                ) && (string_evidence(&left).is_some() || string_evidence(&right).is_some())
                {
                    return residual("text ordering comparison requires a proven collation order");
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
            if let Some(computed) = normalize_case_integer_comparison(
                comparison.left(),
                comparison.operator(),
                comparison.right(),
                sources,
                integer_evidence,
                string_evidence,
                column_datatype,
            ) {
                return computed;
            }
            if let Some(computed) = normalize_computed_integer_comparison(
                comparison.left(),
                comparison.operator(),
                comparison.right(),
                sources,
                integer_evidence,
            ) {
                return computed;
            }
            if let Some(bounded) = normalize_modulo_range_comparison(
                comparison.left(),
                comparison.operator(),
                comparison.right(),
                sources,
                integer_evidence,
            ) {
                return bounded;
            }
            if let Some(inverted) = normalize_affine_comparison(
                comparison.left(),
                comparison.operator(),
                comparison.right(),
                sources,
                integer_evidence,
            ) {
                return inverted;
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
                return residual(&numeric::unresolved_arithmetic_reason(
                    comparison.left(),
                    comparison.right(),
                    sources,
                    column_datatype,
                ));
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
                    column_datatype,
                ),
                normalize_integer_comparison(
                    test.expression(),
                    upper,
                    test.upper(),
                    sources,
                    integer_evidence,
                    string_evidence,
                    column_datatype,
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
                    column_datatype,
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
            column_datatype,
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
        Predicate::BooleanExpression(expression) => residual(&format!(
            "boolean expression has no certified truth law: {}",
            crate::protocol::expression_residual_label(expression),
        )),
        _ => residual("computed, function, collation, cast or other predicate is not invertible"),
    }
}

/// Normalize a CASE-valued integer comparison, preserving first-TRUE branch
/// selection as a two-valued guard and each result's SQL three-valued truth.
/// This avoids erroneously letting an unselected nullable arm leak UNKNOWN.
fn normalize_case_integer_comparison(
    left: &Expression,
    operator: ComparisonOperator,
    right: &Expression,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    string_evidence: &impl Fn(&ColumnRef) -> Option<StringEvidence>,
    column_datatype: &impl Fn(&ColumnRef) -> Option<DataType>,
) -> Option<BooleanRowConstraint> {
    let (case, operator, value) = match (left, right) {
        (Expression::Case(case), other) => (case, operator, signed_integer_literal(other)?),
        (other, Expression::Case(case)) => {
            (case, operator.reversed(), signed_integer_literal(other)?)
        }
        _ => return None,
    };
    if matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    ) {
        return None;
    }

    let simple_operand = match case.operand() {
        None => None,
        Some(Expression::Column(column)) => {
            let column = resolve_column(column, sources);
            integer_evidence(&column)?;
            Some(column)
        }
        Some(_) => return None,
    };
    let mut prior_not_true = Vec::new();
    let mut arms = Vec::new();
    let mut anchor = simple_operand.clone();
    for branch in case.branches() {
        let guard = match &simple_operand {
            None => {
                let Expression::BooleanPredicate(predicate) = branch.condition() else {
                    return None;
                };
                normalize(
                    predicate,
                    sources,
                    integer_evidence,
                    string_evidence,
                    column_datatype,
                )
            }
            Some(column) => BooleanRowConstraint::IntegerComparison {
                column: column.clone(),
                operator: ComparisonOperator::Eq,
                literal: signed_integer_literal(branch.condition())?,
            },
        };
        let guard_not_true = not_true_constraint(&guard)?;
        if anchor.is_none() {
            let mut columns = Vec::new();
            guard.columns(&mut columns);
            anchor = columns.into_iter().next();
        }
        let anchor_column = anchor.as_ref()?;
        let selected_guard = negate_constraint(guard_not_true.clone());
        let result_comparison = case_arm_integer_comparison(
            branch.result(),
            operator,
            value,
            anchor_column,
            sources,
            integer_evidence,
        )?;
        let mut conjunction = prior_not_true.clone();
        conjunction.push(selected_guard);
        conjunction.push(result_comparison);
        arms.push(conjoin_all(conjunction)?);
        prior_not_true.push(guard_not_true);
    }
    let anchor = anchor?;
    let null_else = Expression::Literal(LiteralExpression::new(
        LiteralType::Null,
        LiteralValue::Null,
    ));
    let else_result = case.else_result().unwrap_or(&null_else);
    let comparison = case_arm_integer_comparison(
        else_result,
        operator,
        value,
        &anchor,
        sources,
        integer_evidence,
    )?;
    prior_not_true.push(comparison);
    arms.push(conjoin_all(prior_not_true)?);
    match arms.len() {
        0 => None,
        1 => arms.pop(),
        _ => BooleanOperands::new(arms).map(BooleanRowConstraint::Any),
    }
}

fn case_arm_integer_comparison(
    expression: &Expression,
    operator: ComparisonOperator,
    value: i64,
    anchor: &ColumnRef,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<BooleanRowConstraint> {
    if let Expression::Column(column) = expression {
        let column = resolve_column(column, sources);
        integer_evidence(&column)?;
        return Some(BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal: value,
        });
    }
    if let Expression::Literal(literal) = expression {
        if matches!(literal.value(), LiteralValue::Null) {
            return Some(BooleanRowConstraint::UnknownNullComparison {
                column: anchor.clone(),
            });
        }
    }
    let integer = signed_integer_literal(expression)?;
    let matches = match operator {
        ComparisonOperator::Eq => integer == value,
        ComparisonOperator::Neq => integer != value,
        ComparisonOperator::Lt => integer < value,
        ComparisonOperator::Lte => integer <= value,
        ComparisonOperator::Gt => integer > value,
        ComparisonOperator::Gte => integer >= value,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => return None,
    };
    let operands = BooleanOperands::new(vec![
        BooleanRowConstraint::NullTest {
            column: anchor.clone(),
            negated: false,
        },
        BooleanRowConstraint::NullTest {
            column: anchor.clone(),
            negated: true,
        },
    ])?;
    Some(if matches {
        BooleanRowConstraint::Any(operands)
    } else {
        BooleanRowConstraint::All(operands)
    })
}

/// Normalize the provably reversible integer subset of COALESCE and
/// NULLIF comparisons. Preserving UNKNOWN on NULLIF's equal-value branch is
/// essential: a plain conjunction would incorrectly turn NOT UNKNOWN TRUE.
fn normalize_computed_integer_comparison(
    left: &Expression,
    operator: ComparisonOperator,
    right: &Expression,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<BooleanRowConstraint> {
    let (computed, operator, comparison_literal) = match (left, right) {
        (Expression::Function(function), other) => {
            (function, operator, signed_integer_literal(other)?)
        }
        (other, Expression::Function(function)) => (
            function,
            operator.reversed(),
            signed_integer_literal(other)?,
        ),
        _ => return None,
    };
    if computed.distinct() {
        return None;
    }
    let [Expression::Column(column), fallback] = computed.arguments() else {
        return None;
    };
    let source = resolve_column(column, sources);
    integer_evidence(&source)?;
    let fallback_literal = signed_integer_literal(fallback)?;
    let comparison = BooleanRowConstraint::IntegerComparison {
        column: source.clone(),
        operator,
        literal: comparison_literal,
    };

    if computed.name().eq_ignore_ascii_case("COALESCE") {
        // COALESCE(a, fallback) never returns NULL when fallback is a
        // certified non-NULL integer literal. The NULL arm evaluates the
        // comparison over fallback, not over source a.
        let fallback_matches = match operator {
            ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => {
                fallback_literal == comparison_literal
            }
            ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => {
                fallback_literal != comparison_literal
            }
            ComparisonOperator::Lt => fallback_literal < comparison_literal,
            ComparisonOperator::Lte => fallback_literal <= comparison_literal,
            ComparisonOperator::Gt => fallback_literal > comparison_literal,
            ComparisonOperator::Gte => fallback_literal >= comparison_literal,
        };
        let null_arm = BooleanRowConstraint::NullTest {
            column: source,
            negated: !fallback_matches,
        };
        return BooleanOperands::new(vec![null_arm, comparison]).map(|operands| {
            if fallback_matches {
                BooleanRowConstraint::Any(operands)
            } else {
                BooleanRowConstraint::All(operands)
            }
        });
    }

    if !computed.name().eq_ignore_ascii_case("NULLIF")
        || matches!(
            operator,
            ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
        )
    {
        return None;
    }
    // When a == fallback, NULLIF returns SQL NULL. Preserve the UNKNOWN
    // comparison truth under that condition, rather than incorrectly
    // classifying it as FALSE (which would make NOT wrongly qualify).
    let equal_arm = BooleanRowConstraint::All(BooleanOperands::new(vec![
        BooleanRowConstraint::IntegerComparison {
            column: source.clone(),
            operator: ComparisonOperator::Eq,
            literal: fallback_literal,
        },
        BooleanRowConstraint::UnknownNullComparison {
            column: source.clone(),
        },
    ])?);
    let other_arm = BooleanRowConstraint::All(BooleanOperands::new(vec![
        BooleanRowConstraint::IntegerComparison {
            column: source,
            operator: ComparisonOperator::Neq,
            literal: fallback_literal,
        },
        comparison,
    ])?);
    BooleanOperands::new(vec![equal_arm, other_arm]).map(BooleanRowConstraint::Any)
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
    string_evidence: &impl Fn(&ColumnRef) -> Option<StringEvidence>,
    column_datatype: &impl Fn(&ColumnRef) -> Option<DataType>,
) -> Option<BooleanRowConstraint> {
    let test =
        |column: &crate::protocol::ColumnExpression, negated| BooleanRowConstraint::NullTest {
            column: resolve_column(column, sources),
            negated,
        };
    if let Expression::Column(column) = expression {
        return Some(test(column, negated));
    }
    if let Some(column) = total_integer_column(expression, sources, integer_evidence) {
        return Some(test(column, negated));
    }
    if let Expression::SignedIntegerCast(cast) = expression {
        let Expression::Column(source) = cast.expression() else {
            return None;
        };
        if !matches!(
            cast.cast_kind(),
            crate::protocol::SignedIntegerCastKind::TryCast
                | crate::protocol::SignedIntegerCastKind::SafeCast
        ) {
            // A potentially failing ordinary CAST has no certified result for
            // every catalog-admissible row. Its IS NULL test is not total.
            return None;
        }
        let column = resolve_column(source, sources);
        integer_evidence(&column)?;
        let (minimum, maximum) = cast.target_width().limits();
        let minimum = i64::try_from(minimum).ok()?;
        let maximum = i64::try_from(maximum).ok()?;
        // TRY/SAFE_CAST of an attested signed integer introduces NULL
        // precisely for source NULL or an out-of-range integer. Preserve
        // the complementary two-valued IS NOT NULL obligation as well.
        let comparisons = if negated {
            vec![
                test(source, true),
                BooleanRowConstraint::IntegerComparison {
                    column: column.clone(),
                    operator: ComparisonOperator::Gte,
                    literal: minimum,
                },
                BooleanRowConstraint::IntegerComparison {
                    column,
                    operator: ComparisonOperator::Lte,
                    literal: maximum,
                },
            ]
        } else {
            vec![
                test(source, false),
                BooleanRowConstraint::IntegerComparison {
                    column: column.clone(),
                    operator: ComparisonOperator::Lt,
                    literal: minimum,
                },
                BooleanRowConstraint::IntegerComparison {
                    column,
                    operator: ComparisonOperator::Gt,
                    literal: maximum,
                },
            ]
        };
        let children = BooleanOperands::new(comparisons)?;
        return Some(if negated {
            BooleanRowConstraint::All(children)
        } else {
            BooleanRowConstraint::Any(children)
        });
    }
    // Searched CASE chooses the first arm whose guard is SQL TRUE; UNKNOWN or
    // FALSE guards fall through to the next arm or the ELSE result. For the
    // supported total integer / NULL result class, NULL tests of the selected
    // value remain exact. Guards may be NULL-sensitive: fallthrough uses the
    // FALSE-or-UNKNOWN region rather than a Rust negation.
    if let Expression::Case(case) = expression {
        if case.branches().is_empty() {
            return None;
        }
        // Simple CASE compares the operand with three-valued equality: a NULL
        // operand never matches, so it falls through like an UNKNOWN guard.
        // Synthesize the equality guard for column operands and signed
        // integer WHEN values; anything else stays residual.
        let simple_operand = match case.operand() {
            None => None,
            Some(operand) => {
                let Expression::Column(column) = operand else {
                    return None;
                };
                let resolved = resolve_column(column, sources);
                integer_evidence(&resolved)?;
                Some(resolved)
            }
        };
        let mut prior_not_true = Vec::new();
        let mut alternatives = Vec::new();
        for branch in case.branches() {
            let guard = match &simple_operand {
                None => {
                    let Expression::BooleanPredicate(predicate) = branch.condition() else {
                        return None;
                    };
                    normalize(
                        predicate,
                        sources,
                        integer_evidence,
                        string_evidence,
                        column_datatype,
                    )
                }
                Some(operand) => {
                    let literal = signed_integer_literal(branch.condition())?;
                    BooleanRowConstraint::IntegerComparison {
                        column: operand.clone(),
                        operator: ComparisonOperator::Eq,
                        literal,
                    }
                }
            };
            let guard_not_true = not_true_constraint(&guard)?;
            let nullness =
                case_result_null_test(branch.result(), negated, sources, integer_evidence)?;
            match nullness {
                CaseArmNullness::Provable(constraint) => {
                    let mut conjunction = prior_not_true.clone();
                    conjunction.push(guard.clone());
                    conjunction.push(constraint);
                    alternatives.push(conjoin_all(conjunction)?);
                }
                CaseArmNullness::Vacuous => {
                    // The test holds for every row selecting this arm.
                    let mut conjunction = prior_not_true.clone();
                    conjunction.push(guard.clone());
                    alternatives.push(conjoin_all(conjunction)?);
                }
                CaseArmNullness::Impossible => {}
            }
            // Arm selection depends only on guards, never on the arm result,
            // so the fallthrough guard is recorded even for dropped arms.
            prior_not_true.push(guard_not_true);
        }
        // An implicit ELSE is SQL NULL.
        let else_nullness = match case.else_result() {
            Some(result) => case_result_null_test(result, negated, sources, integer_evidence)?,
            None => {
                if negated {
                    CaseArmNullness::Impossible
                } else {
                    CaseArmNullness::Vacuous
                }
            }
        };
        match else_nullness {
            CaseArmNullness::Impossible => {}
            CaseArmNullness::Vacuous => {
                alternatives.push(conjoin_all(prior_not_true)?);
            }
            CaseArmNullness::Provable(constraint) => {
                let mut conjunction = prior_not_true;
                conjunction.push(constraint);
                alternatives.push(conjoin_all(conjunction)?);
            }
        }
        return match alternatives.len() {
            0 => None,
            1 => Some(alternatives.pop()?),
            _ => BooleanOperands::new(alternatives).map(BooleanRowConstraint::Any),
        };
    }
    let Expression::Function(function) = expression else {
        return None;
    };
    if function.distinct() {
        return None;
    }
    if function.name().eq_ignore_ascii_case("COALESCE") && function.arguments().len() >= 2 {
        let mut columns = Vec::new();
        let mut anchor: Option<ColumnRef> = None;
        let mut never_null_literal = false;
        for argument in function.arguments() {
            match argument {
                Expression::Literal(literal) if matches!(literal.value(), LiteralValue::Null) => {}
                Expression::Literal(_) => never_null_literal = true,
                Expression::Column(column)
                    if integer_evidence(&resolve_column(column, sources)).is_some() =>
                {
                    if anchor.is_none() {
                        anchor = Some(resolve_column(column, sources));
                    }
                    columns.push(test(column, negated));
                }
                _ => return None,
            }
        }
        if never_null_literal {
            // A non-NULL literal argument can never be NULL, so it decides
            // the outcome alone: IS NULL can never be TRUE and IS NOT NULL
            // can never be NOT TRUE. Encode the empty TRUE region as an
            // explicit contradiction (IS NULL) or the full TRUE region as a
            // tautology (IS NOT NULL) over the first column, so joint-truth
            // analysis proves the direction exactly instead of residualizing.
            let anchor = anchor?;
            let pair = BooleanOperands::new(vec![
                BooleanRowConstraint::NullTest {
                    column: anchor.clone(),
                    negated: false,
                },
                BooleanRowConstraint::NullTest {
                    column: anchor,
                    negated: true,
                },
            ])?;
            return Some(if negated {
                BooleanRowConstraint::Any(pair)
            } else {
                BooleanRowConstraint::All(pair)
            });
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
        let [left, right] = function.arguments() else {
            return None;
        };
        // NULLIF(a, b) is NULL exactly when the two arguments are NULL-equal,
        // and otherwise returns the left argument. Both truth directions stay
        // exact for two source columns or an attested integer literal.
        let (left_column, comparison) = match (left, right) {
            (Expression::Column(left), Expression::Column(right)) => {
                let left_column = resolve_column(left, sources);
                let right_column = resolve_column(right, sources);
                integer_evidence(&left_column)?;
                integer_evidence(&right_column)?;
                let operator = if negated {
                    ComparisonOperator::IsDistinctFrom
                } else {
                    ComparisonOperator::IsNotDistinctFrom
                };
                (
                    left_column.clone(),
                    BooleanRowConstraint::IntegerColumnComparison {
                        left: left_column,
                        operator,
                        right: right_column,
                    },
                )
            }
            (Expression::Column(left), other) => {
                let value = signed_integer_literal(other)?;
                let left_column = resolve_column(left, sources);
                integer_evidence(&left_column)?;
                let operator = if negated {
                    ComparisonOperator::Neq
                } else {
                    ComparisonOperator::Eq
                };
                let comparison = BooleanRowConstraint::IntegerComparison {
                    column: left_column.clone(),
                    operator,
                    literal: value,
                };
                (left_column, comparison)
            }
            _ => return None,
        };
        let operands = BooleanOperands::new(vec![
            BooleanRowConstraint::NullTest {
                column: left_column,
                negated,
            },
            comparison,
        ])?;
        return Some(if negated {
            BooleanRowConstraint::All(operands)
        } else {
            BooleanRowConstraint::Any(operands)
        });
    }
    None
}

/// How a CASE arm result's NULL status constrains the source row.
enum CaseArmNullness {
    /// The IS NULL / IS NOT NULL test reduces to a source-column constraint.
    Provable(BooleanRowConstraint),
    /// The NULL test holds for every admissible arm result (e.g. a NULL
    /// literal under IS NULL), so the arm adds no value conjunct.
    Vacuous,
    /// The NULL test can never hold for this arm (e.g. a NULL literal under
    /// IS NOT NULL), so the arm contributes no qualifying region.
    Impossible,
}

/// Reduce an arm result's IS NULL / IS NOT NULL to a source constraint, a
/// constant TRUE, or a constant FALSE. Only NULL literals and total integer
/// results are supported; anything else stays an explicit residual.
fn case_result_null_test(
    result: &Expression,
    negated: bool,
    sources: &[SourceRelation],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<CaseArmNullness> {
    if let Expression::Literal(literal) = result {
        return Some(
            match (matches!(literal.value(), LiteralValue::Null), negated) {
                (true, false) | (false, true) => CaseArmNullness::Vacuous,
                (true, true) | (false, false) => CaseArmNullness::Impossible,
            },
        );
    }
    let column = total_integer_column(result, sources, integer_evidence)?;
    Some(CaseArmNullness::Provable(BooleanRowConstraint::NullTest {
        column: resolve_column(column, sources),
        negated,
    }))
}

/// Collapse a conjunction to a single constraint when only one operand
/// remains, keeping `All` only for two or more.
fn conjoin_all(mut operands: Vec<BooleanRowConstraint>) -> Option<BooleanRowConstraint> {
    match operands.len() {
        0 => None,
        1 => Some(operands.pop()?),
        _ => Some(BooleanRowConstraint::All(BooleanOperands::new(operands)?)),
    }
}

/// The FALSE-or-UNKNOWN region of a normalized predicate: SQL's NOT TRUE.
/// NULL-sensitive comparisons fold their UNKNOWN arm back in, so this is
/// broader than a three-valued negation. Returns `None` when the subtree
/// cannot express that region (opaque or already residual guards).
fn not_true_constraint(condition: &BooleanRowConstraint) -> Option<BooleanRowConstraint> {
    /// The NOT TRUE region of one single-column comparison: the plain
    /// negated comparison when the operator is two-valued, otherwise the
    /// negated comparison OR the column being NULL (the UNKNOWN arm folds
    /// back into the accepted region).
    fn not_true_of_comparison(
        column: &ColumnRef,
        operator: ComparisonOperator,
        negated: BooleanRowConstraint,
    ) -> Option<BooleanRowConstraint> {
        if two_valued_comparison(operator) {
            Some(negated)
        } else {
            BooleanOperands::new(vec![
                BooleanRowConstraint::NullTest {
                    column: column.clone(),
                    negated: false,
                },
                negated,
            ])
            .map(BooleanRowConstraint::Any)
        }
    }
    match condition {
        BooleanRowConstraint::NullTest { column, negated } => {
            Some(BooleanRowConstraint::NullTest {
                column: column.clone(),
                negated: !negated,
            })
        }
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => not_true_of_comparison(
            column,
            *operator,
            BooleanRowConstraint::IntegerComparison {
                column: column.clone(),
                operator: negated_comparison_operator(*operator),
                literal: *literal,
            },
        ),
        BooleanRowConstraint::IntegerColumnComparison {
            left,
            operator,
            right,
        } => {
            let negated = BooleanRowConstraint::IntegerColumnComparison {
                left: left.clone(),
                operator: negated_comparison_operator(*operator),
                right: right.clone(),
            };
            if two_valued_comparison(*operator) {
                Some(negated)
            } else {
                BooleanOperands::new(vec![
                    BooleanRowConstraint::NullTest {
                        column: left.clone(),
                        negated: false,
                    },
                    BooleanRowConstraint::NullTest {
                        column: right.clone(),
                        negated: false,
                    },
                    negated,
                ])
                .map(BooleanRowConstraint::Any)
            }
        }
        BooleanRowConstraint::StringEquality {
            column,
            operator,
            literal,
        } => not_true_of_comparison(
            column,
            *operator,
            BooleanRowConstraint::StringEquality {
                column: column.clone(),
                operator: negated_comparison_operator(*operator),
                literal: literal.clone(),
            },
        ),
        BooleanRowConstraint::StringColumnComparison {
            left,
            operator,
            right,
        } => {
            let negated = BooleanRowConstraint::StringColumnComparison {
                left: left.clone(),
                operator: negated_comparison_operator(*operator),
                right: right.clone(),
            };
            if two_valued_comparison(*operator) {
                Some(negated)
            } else {
                BooleanOperands::new(vec![
                    BooleanRowConstraint::NullTest {
                        column: left.clone(),
                        negated: false,
                    },
                    BooleanRowConstraint::NullTest {
                        column: right.clone(),
                        negated: false,
                    },
                    negated,
                ])
                .map(BooleanRowConstraint::Any)
            }
        }
        BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated,
        } => BooleanOperands::new(vec![
            BooleanRowConstraint::NullTest {
                column: column.clone(),
                negated: false,
            },
            BooleanRowConstraint::StringPrefix {
                column: column.clone(),
                prefix: prefix.clone(),
                negated: !negated,
            },
        ])
        .map(BooleanRowConstraint::Any),
        BooleanRowConstraint::UnknownNullComparison { .. }
        | BooleanRowConstraint::Residual { .. } => None,
        // De Morgan over SQL's NOT TRUE: NOT TRUE(a AND b) = NOT TRUE(a) OR
        // NOT TRUE(b), and the converse for OR.
        BooleanRowConstraint::All(operands) => {
            let mapped = operands
                .iter()
                .map(not_true_constraint)
                .collect::<Option<Vec<_>>>()?;
            BooleanOperands::new(mapped).map(BooleanRowConstraint::Any)
        }
        BooleanRowConstraint::Any(operands) => {
            let mapped = operands
                .iter()
                .map(not_true_constraint)
                .collect::<Option<Vec<_>>>()?;
            BooleanOperands::new(mapped).map(BooleanRowConstraint::All)
        }
    }
}

fn two_valued_comparison(operator: ComparisonOperator) -> bool {
    matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    )
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
        }
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
        BooleanRowConstraint::StringColumnComparison {
            left,
            operator,
            right,
        } => BooleanRowConstraint::StringColumnComparison {
            left,
            operator: negated_comparison_operator(operator),
            right,
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

pub(crate) fn negated_comparison_operator(operator: ComparisonOperator) -> ComparisonOperator {
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
    column_datatype: &impl Fn(&ColumnRef) -> Option<DataType>,
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
        column_datatype,
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
            let (minimum, maximum) = cast.target_width().limits();
            (bounds.minimum >= minimum && bounds.maximum <= maximum).then_some(column)
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
    let (width_minimum, width_maximum) = cast.target_width().limits();
    let minimum = bounds.minimum.checked_add(delta)?;
    let maximum = bounds.maximum.checked_add(delta)?;
    (bounds.minimum >= width_minimum
        && bounds.maximum <= width_maximum
        && minimum >= width_minimum
        && maximum <= width_maximum)
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
    compared_text_columns: BTreeSet<ColumnRef>,
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
        BooleanRowConstraint::StringEquality {
            column, literal, ..
        } => {
            thresholds
                .entry(column.clone())
                .or_default()
                .strings
                .insert(literal.clone());
        }
        BooleanRowConstraint::StringColumnComparison { left, right, .. } => {
            // Record the text correlation for candidate sharing below; the
            // empty marker forces the string (never integer) value path.
            // It is already a default candidate, so no truth region is added.
            thresholds
                .entry(left.clone())
                .or_default()
                .compared_text_columns
                .insert(right.clone());
            thresholds
                .entry(right.clone())
                .or_default()
                .compared_text_columns
                .insert(left.clone());
            thresholds
                .entry(left.clone())
                .or_default()
                .strings
                .insert(String::new());
            thresholds
                .entry(right.clone())
                .or_default()
                .strings
                .insert(String::new());
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
        || conditions.iter().any(|condition| {
            !condition.is_exact() || condition.requires_binary_string_assumptions()
        })
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
        // The representative search may exhaust its assignment budget before
        // seeing any rows. A separate symbolic proof can still establish
        // impossibility without relying on that incomplete enumeration.
        let proven = match truth {
            BooleanTruthCase::True => true_region_empty(&joint, &integer_evidence, None),
            BooleanTruthCase::NotTrue => not_true_constraint(&joint)
                .is_some_and(|not_true| true_region_empty(&not_true, &integer_evidence, None)),
        };
        return proven.then_some((joint, false));
    }
    let satisfiable = match truth {
        BooleanTruthCase::True => possible.contains(&SqlTruth::True),
        BooleanTruthCase::NotTrue => {
            possible.contains(&SqlTruth::False) || possible.contains(&SqlTruth::Unknown)
        }
    };
    if !satisfiable && joint.contains_column_comparison() {
        // Bounded representatives alone cannot assert universal absence for
        // correlated columns. A symbolic integer-order invariant can.
        let proven = match truth {
            BooleanTruthCase::True => true_region_empty(&joint, &integer_evidence, None),
            BooleanTruthCase::NotTrue => not_true_constraint(&joint)
                .is_some_and(|not_true| true_region_empty(&not_true, &integer_evidence, None)),
        };
        if !proven {
            return None;
        }
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
        || requirements.iter().any(|(condition, _)| {
            !condition.is_exact() || condition.requires_binary_string_assumptions()
        })
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
        // No sampled assignment may also mean that enumeration hit its
        // deterministic limit. Only a symbolic contradiction proves absence.
        return requirements_jointly_unsatisfiable(requirements, &integer_evidence)
            .then_some(false);
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
        && !requirements_jointly_unsatisfiable(requirements, &integer_evidence)
    {
        return None;
    }
    Some(satisfiable)
}

/// Whether the requested row requirements admit no shared physical assignment.
///
/// Each `NotTrue` requirement is rewritten with `not_true_constraint` so the
/// joint obligation becomes one conjunction whose SQL TRUE region must hold.
/// `None` from that rewrite, residual/string leaves, missing type evidence,
/// or deterministic resource caps all fail closed to `false` (unproven).
fn requirements_jointly_unsatisfiable(
    requirements: &[(&BooleanRowConstraint, BooleanTruthCase)],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> bool {
    let mut conjuncts = Vec::new();
    for (condition, truth) in requirements {
        match truth {
            BooleanTruthCase::True => conjuncts.push((*condition).clone()),
            BooleanTruthCase::NotTrue => {
                let Some(not_true) = not_true_constraint(condition) else {
                    return false;
                };
                conjuncts.push(not_true);
            }
        }
    }
    let Some(all) = BooleanOperands::new(conjuncts).map(BooleanRowConstraint::All) else {
        return false;
    };
    true_region_empty(&all, integer_evidence, None)
}

/// Whether no shared non-null-aware integer assignment makes `condition` TRUE.
///
/// This is a sound symbolic complement to bounded enumeration: enumeration
/// proves existence, this proves universal absence from total-order invariants,
/// exhaustive type bounds, and accepted-value evidence. Anything else fails
/// closed to `false`. Deterministic caps keep the proof bounded.
fn true_region_empty(
    condition: &BooleanRowConstraint,
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    restrictions: Option<&BTreeMap<String, ColumnRestriction>>,
) -> bool {
    // Mixed conjunctions/disjunctions require each disjunctive branch to be
    // unsatisfiable under the *same* conjunctive requirements. Distribute only
    // within a fixed budget; an unsupported subtree remains unproven.
    if let Some(cases) = bounded_conjunctive_cases(condition) {
        return !cases.is_empty()
            && cases
                .iter()
                .all(|leaves| joint_integer_contradiction(leaves, integer_evidence, restrictions));
    }
    match condition {
        BooleanRowConstraint::All(operands) => {
            if operands
                .iter()
                .any(|child| true_region_empty(child, integer_evidence, restrictions))
            {
                return true;
            }
            let mut leaves = Vec::new();
            if !collect_conjunctive_leaves(condition, &mut leaves) {
                return false;
            }
            joint_integer_contradiction(&leaves, integer_evidence, restrictions)
        }
        BooleanRowConstraint::Any(operands) => operands
            .iter()
            .all(|child| true_region_empty(child, integer_evidence, restrictions)),
        BooleanRowConstraint::UnknownNullComparison { .. } => true,
        BooleanRowConstraint::Residual { .. }
        | BooleanRowConstraint::StringEquality { .. }
        | BooleanRowConstraint::StringPrefix { .. }
        | BooleanRowConstraint::NullTest { .. } => false,
        BooleanRowConstraint::IntegerComparison { .. }
        | BooleanRowConstraint::IntegerColumnComparison { .. }
        | BooleanRowConstraint::StringColumnComparison { .. } => {
            joint_integer_contradiction(&[condition], integer_evidence, restrictions)
        }
    }
}

/// Convert a bounded Boolean TRUE region into conjunctive cases.
///
/// Each case represents one alternative satisfying assignment. An entire
/// expression is impossible only if every case is contradictory. Exhausting
/// either budget or encountering an unproved leaf fails closed to `None`.
fn bounded_conjunctive_cases(
    condition: &BooleanRowConstraint,
) -> Option<Vec<Vec<&BooleanRowConstraint>>> {
    const MAX_CASES: usize = 64;
    const MAX_ATOMS_PER_CASE: usize = 128;
    match condition {
        BooleanRowConstraint::All(operands) => {
            let mut cases = vec![Vec::new()];
            for child in operands.iter() {
                let child_cases = bounded_conjunctive_cases(child)?;
                if cases.len().saturating_mul(child_cases.len()) > MAX_CASES {
                    return None;
                }
                let mut expanded = Vec::new();
                for prior in &cases {
                    for alternative in &child_cases {
                        if prior.len().saturating_add(alternative.len()) > MAX_ATOMS_PER_CASE {
                            return None;
                        }
                        let mut case = prior.clone();
                        case.extend(alternative.iter().copied());
                        expanded.push(case);
                    }
                }
                cases = expanded;
            }
            Some(cases)
        }
        BooleanRowConstraint::Any(operands) => {
            let mut cases = Vec::new();
            for child in operands.iter() {
                let child_cases = bounded_conjunctive_cases(child)?;
                if cases.len().saturating_add(child_cases.len()) > MAX_CASES {
                    return None;
                }
                cases.extend(child_cases);
            }
            Some(cases)
        }
        BooleanRowConstraint::NullTest { .. }
        | BooleanRowConstraint::UnknownNullComparison { .. }
        | BooleanRowConstraint::IntegerComparison { .. }
        | BooleanRowConstraint::IntegerColumnComparison { .. }
        | BooleanRowConstraint::StringColumnComparison { .. } => Some(vec![vec![condition]]),
        BooleanRowConstraint::Residual { .. }
        | BooleanRowConstraint::StringEquality { .. }
        | BooleanRowConstraint::StringPrefix { .. } => None,
    }
}

/// Flatten nested `All` into leaves. Returns `false` when a disjunctive or
/// non-conjunctive shape needs case splitting beyond this bounded prover.
fn collect_conjunctive_leaves<'a>(
    condition: &'a BooleanRowConstraint,
    leaves: &mut Vec<&'a BooleanRowConstraint>,
) -> bool {
    match condition {
        BooleanRowConstraint::All(operands) => operands
            .iter()
            .all(|child| collect_conjunctive_leaves(child, leaves)),
        BooleanRowConstraint::Any(_)
        | BooleanRowConstraint::Residual { .. }
        | BooleanRowConstraint::StringEquality { .. }
        | BooleanRowConstraint::StringPrefix { .. } => false,
        BooleanRowConstraint::NullTest { .. }
        | BooleanRowConstraint::UnknownNullComparison { .. }
        | BooleanRowConstraint::IntegerComparison { .. }
        | BooleanRowConstraint::IntegerColumnComparison { .. }
        | BooleanRowConstraint::StringColumnComparison { .. } => {
            leaves.push(condition);
            true
        }
    }
}

/// Joint contradiction over one conjunction of integer/null leaves.
///
/// TRUE of an ordinary comparison requires non-NULL operands; NULL-safe
/// `IsDistinctFrom`/`IsNotDistinctFrom` are two-valued. Equalities use
/// union-find, orders use integer difference constraints with a CONST node
/// (`<` becomes `<= -1`), type bounds contribute edges, and accepted values
/// contribute disjointness. A negative cycle or direct null/equality conflict
/// proves no shared TRUE row exists.
fn joint_integer_contradiction(
    leaves: &[&BooleanRowConstraint],
    integer_evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
    restrictions: Option<&BTreeMap<String, ColumnRestriction>>,
) -> bool {
    const MAX_ATOMS: usize = 128;
    const MAX_COLUMNS: usize = 32;
    if leaves.is_empty() || leaves.len() > MAX_ATOMS {
        return false;
    }
    let mut columns: Vec<ColumnRef> = Vec::new();
    for leaf in leaves {
        let mut found = Vec::new();
        leaf.columns(&mut found);
        for column in found {
            if !columns.contains(&column) {
                columns.push(column);
            }
        }
    }
    if columns.is_empty() || columns.len() > MAX_COLUMNS {
        return false;
    }
    // Every integer leaf needs catalog type evidence; otherwise the
    // contradiction would assume an unproven domain.
    for column in &columns {
        let mut needs_integer = false;
        for leaf in leaves {
            match leaf {
                BooleanRowConstraint::IntegerComparison { column: target, .. }
                    if target == column =>
                {
                    needs_integer = true;
                }
                BooleanRowConstraint::IntegerColumnComparison { left, right, .. }
                    if left == column || right == column =>
                {
                    needs_integer = true;
                }
                BooleanRowConstraint::IntegerComparison { .. }
                | BooleanRowConstraint::IntegerColumnComparison { .. }
                | BooleanRowConstraint::NullTest { .. }
                | BooleanRowConstraint::UnknownNullComparison { .. }
                | BooleanRowConstraint::All(_)
                | BooleanRowConstraint::Any(_)
                | BooleanRowConstraint::StringEquality { .. }
                | BooleanRowConstraint::StringColumnComparison { .. }
                | BooleanRowConstraint::StringPrefix { .. }
                | BooleanRowConstraint::Residual { .. } => {}
            }
        }
        if needs_integer && integer_evidence(column).is_none() {
            return false;
        }
    }

    let mut must_null: BTreeSet<ColumnRef> = BTreeSet::new();
    let mut must_non_null: BTreeSet<ColumnRef> = BTreeSet::new();
    for leaf in leaves {
        match leaf {
            BooleanRowConstraint::NullTest { column, negated } => {
                if *negated {
                    must_non_null.insert(column.clone());
                } else {
                    must_null.insert(column.clone());
                }
            }
            BooleanRowConstraint::UnknownNullComparison { .. } => return true,
            BooleanRowConstraint::IntegerComparison {
                column, operator, ..
            } if !matches!(operator, ComparisonOperator::IsDistinctFrom) => {
                // Ordinary comparisons require non-NULL for TRUE. IS NOT
                // DISTINCT FROM against a non-NULL literal is TRUE only for
                // the equal non-NULL value; IS DISTINCT FROM stays TRUE for
                // NULL and must not force non-NULL.
                must_non_null.insert(column.clone());
            }
            BooleanRowConstraint::IntegerColumnComparison {
                left,
                right,
                operator,
            } => {
                if matches!(
                    operator,
                    ComparisonOperator::Eq
                        | ComparisonOperator::Neq
                        | ComparisonOperator::Lt
                        | ComparisonOperator::Lte
                        | ComparisonOperator::Gt
                        | ComparisonOperator::Gte
                ) {
                    must_non_null.insert(left.clone());
                    must_non_null.insert(right.clone());
                }
                if left == right
                    && matches!(
                        operator,
                        ComparisonOperator::Neq
                            | ComparisonOperator::Lt
                            | ComparisonOperator::Gt
                            | ComparisonOperator::IsDistinctFrom
                    )
                {
                    return true;
                }
            }
            BooleanRowConstraint::StringColumnComparison {
                left,
                right,
                operator,
            } => {
                // Byte equality is reflexive under any attested binary law:
                // ordinary comparisons need both sides non-NULL for TRUE,
                // while NULL-safe operators are two-valued.
                if matches!(operator, ComparisonOperator::Eq | ComparisonOperator::Neq) {
                    must_non_null.insert(left.clone());
                    must_non_null.insert(right.clone());
                }
                if left == right
                    && matches!(
                        operator,
                        ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom
                    )
                {
                    return true;
                }
            }
            BooleanRowConstraint::IntegerComparison { .. }
            | BooleanRowConstraint::All(_)
            | BooleanRowConstraint::Any(_)
            | BooleanRowConstraint::StringEquality { .. }
            | BooleanRowConstraint::StringPrefix { .. }
            | BooleanRowConstraint::Residual { .. } => {}
        }
    }
    // IS DISTINCT FROM against a literal is TRUE for NULL, handled above
    // by not forcing non-NULL.
    if must_null.intersection(&must_non_null).next().is_some() {
        return true;
    }

    // Equality union-find over typed integer and attested binary-text columns.
    // Any inconsistent
    // access fails closed to unproven rather than panicking.
    let index_of = |column: &ColumnRef| columns.iter().position(|item| item == column);
    let mut parent: Vec<usize> = (0..columns.len()).collect();
    fn find(parent: &mut [usize], mut node: usize) -> Option<usize> {
        while parent.get(node).is_some_and(|root| *root != node) {
            let next = *parent.get(node)?;
            let grandparent = *parent.get(next).unwrap_or(&next);
            *parent.get_mut(node)? = grandparent;
            node = grandparent;
        }
        parent.get(node).copied()
    }
    fn union(parent: &mut [usize], left: usize, right: usize) -> bool {
        let (Some(left), Some(right)) = (find(parent, left), find(parent, right)) else {
            return false;
        };
        if left != right {
            if let Some(slot) = parent.get_mut(left) {
                *slot = right;
            } else {
                return false;
            }
        }
        true
    }
    for leaf in leaves {
        if let BooleanRowConstraint::IntegerColumnComparison {
            left,
            right,
            operator,
        }
        | BooleanRowConstraint::StringColumnComparison {
            left,
            right,
            operator,
        } = leaf
        {
            if matches!(
                operator,
                ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom
            ) {
                let (Some(left), Some(right)) = (index_of(left), index_of(right)) else {
                    return false;
                };
                if !union(&mut parent, left, right) {
                    return false;
                }
            }
        }
    }
    // Disequality against an equality group is contradictory.
    for leaf in leaves {
        if let BooleanRowConstraint::IntegerColumnComparison {
            left,
            right,
            operator,
        }
        | BooleanRowConstraint::StringColumnComparison {
            left,
            right,
            operator,
        } = leaf
        {
            if matches!(
                operator,
                ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom
            ) {
                let (Some(left), Some(right)) = (index_of(left), index_of(right)) else {
                    return false;
                };
                let (Some(left_root), Some(right_root)) =
                    (find(&mut parent, left), find(&mut parent, right))
                else {
                    return false;
                };
                if left_root == right_root {
                    return true;
                }
            }
        }
    }
    // Column forced to a literal while forbidden the same literal.
    let mut forced: BTreeMap<ColumnRef, i128> = BTreeMap::new();
    for leaf in leaves {
        if let BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } = leaf
        {
            if matches!(
                operator,
                ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom
            ) {
                let value = i128::from(*literal);
                if let Some(prior) = forced.get(column) {
                    if *prior != value {
                        return true;
                    }
                } else {
                    forced.insert(column.clone(), value);
                }
            }
        }
    }
    for leaf in leaves {
        let forbidden = match leaf {
            BooleanRowConstraint::IntegerComparison {
                column,
                operator,
                literal,
            } if matches!(
                operator,
                ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom
            ) =>
            {
                Some((column, operator, i128::from(*literal)))
            }
            _ => None,
        };
        if let Some((column, operator, literal)) = forbidden {
            if forced.get(column).is_some_and(|value| *value == literal) {
                // A forced non-NULL equality makes the forbidden literal
                // FALSE; NULL would make ordinary Neq UNKNOWN, but the
                // forced equality already requires non-NULL.
                if *operator == ComparisonOperator::Neq {
                    if !must_null.contains(column) {
                        return true;
                    }
                } else if forced.contains_key(column) {
                    // IS DISTINCT FROM is TRUE for NULL, but the forced
                    // IS NOT DISTINCT FROM/Eq arm already pins non-NULL.
                    return true;
                }
            }
        }
    }
    // Accepted-value disjointness and forced-literal membership.
    if let Some(items) = restrictions {
        let accepted_integers = |column: &ColumnRef| -> Option<BTreeSet<i128>> {
            let item = items.get(column.name())?;
            let allowed = item.accepted.as_ref()?;
            let mut values = BTreeSet::new();
            for value in allowed {
                match value {
                    RowScalar::Integer(value) => {
                        values.insert(*value);
                    }
                    RowScalar::String(_) => return None,
                }
            }
            Some(values)
        };
        for (column, value) in &forced {
            if let Some(allowed) = accepted_integers(column) {
                if !allowed.contains(value) && !must_null.contains(column) {
                    // A forced non-NULL literal outside the enforced
                    // accepted set can never be TRUE.
                    return true;
                }
            }
        }
        // Equality groups with disjoint accepted sets and a non-NULL
        // requirement cannot share one TRUE row (both-NULL would fail the
        // ordinary equality or order conjuncts already recorded).
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for position in 0..columns.len() {
            let Some(root) = find(&mut parent, position) else {
                return false;
            };
            groups.entry(root).or_default().push(position);
        }
        for members in groups.values() {
            let mut shared: Option<BTreeSet<i128>> = None;
            let mut has_accepted = false;
            let mut group_must_non_null = false;
            for position in members {
                let column = &columns[*position];
                if must_non_null.contains(column)
                    || items.get(column.name()).is_some_and(|item| item.not_null)
                {
                    group_must_non_null = true;
                }
                if let Some(allowed) = accepted_integers(column) {
                    has_accepted = true;
                    shared = Some(match shared {
                        Some(current) => current.intersection(&allowed).copied().collect(),
                        None => allowed,
                    });
                }
            }
            if has_accepted && group_must_non_null && shared.is_some_and(|values| values.is_empty())
            {
                return true;
            }
        }
    }

    // Difference constraints: edge `from -> to` with weight `w` means
    // `to - from <= w`. CONST (index len) is fixed at zero.
    let const_index = columns.len();
    let node_count = columns.len() + 1;
    let mut edges: Vec<(usize, usize, i128)> = Vec::new();
    let push = |edges: &mut Vec<(usize, usize, i128)>, from: usize, to: usize, weight: i128| {
        edges.push((from, to, weight));
    };
    for leaf in leaves {
        match leaf {
            BooleanRowConstraint::IntegerComparison {
                column,
                operator,
                literal,
            } => {
                let Some(position) = index_of(column) else {
                    return false;
                };
                let value = i128::from(*literal);
                let (Some(lower), Some(upper)) = (value.checked_sub(1), value.checked_add(1))
                else {
                    return false;
                };
                match operator {
                    ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => {
                        push(&mut edges, const_index, position, value);
                        push(&mut edges, position, const_index, -value);
                    }
                    ComparisonOperator::Lt => push(&mut edges, const_index, position, lower),
                    ComparisonOperator::Lte => push(&mut edges, const_index, position, value),
                    ComparisonOperator::Gt => push(&mut edges, position, const_index, -upper),
                    ComparisonOperator::Gte => push(&mut edges, position, const_index, -value),
                    ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => {}
                }
            }
            BooleanRowConstraint::IntegerColumnComparison {
                left,
                right,
                operator,
            } => {
                let (Some(left), Some(right)) = (index_of(left), index_of(right)) else {
                    return false;
                };
                match operator {
                    ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => {
                        push(&mut edges, right, left, 0);
                        push(&mut edges, left, right, 0);
                    }
                    ComparisonOperator::Lt => push(&mut edges, right, left, -1),
                    ComparisonOperator::Lte => push(&mut edges, right, left, 0),
                    ComparisonOperator::Gt => push(&mut edges, left, right, -1),
                    ComparisonOperator::Gte => push(&mut edges, left, right, 0),
                    ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => {}
                }
            }
            BooleanRowConstraint::NullTest { .. }
            | BooleanRowConstraint::UnknownNullComparison { .. }
            | BooleanRowConstraint::All(_)
            | BooleanRowConstraint::Any(_)
            | BooleanRowConstraint::StringEquality { .. }
            | BooleanRowConstraint::StringColumnComparison { .. }
            | BooleanRowConstraint::StringPrefix { .. }
            | BooleanRowConstraint::Residual { .. } => {}
        }
    }
    for (position, column) in columns.iter().enumerate() {
        // Pure NULL tests are valid on non-integer columns. Only the columns
        // used by integer comparisons require type bounds (checked above).
        if let Some(bounds) = integer_evidence(column) {
            push(&mut edges, const_index, position, bounds.maximum);
            push(&mut edges, position, const_index, -bounds.minimum);
        }
    }
    // Bellman-Ford from a virtual super source (all distances zero):
    // a further relaxation on iteration N proves a negative cycle.
    // Out-of-range access fails closed to unproven rather than panicking.
    let mut distance = vec![0_i128; node_count];
    for _ in 0..node_count {
        let mut relaxed = false;
        for (from, to, weight) in &edges {
            let (Some(current_from), Some(current_to)) = (distance.get(*from), distance.get(*to))
            else {
                return false;
            };
            let Some(candidate) = current_from.checked_add(*weight) else {
                return false;
            };
            if candidate < *current_to {
                let Some(slot) = distance.get_mut(*to) else {
                    return false;
                };
                *slot = candidate;
                relaxed = true;
            }
        }
        if !relaxed {
            return false;
        }
    }
    true
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
    // Text equalities can form transitive chains (a = b AND b = c). Share
    // candidate literals and LIKE prefixes across the entire connected
    // component; a single pass misses satisfying assignments at distance >1.
    // Only finite collected values propagate, so this reaches a fixed point.
    loop {
        let snapshot = thresholds.clone();
        let mut changed = false;
        for (column, region) in &snapshot {
            for other in &region.compared_text_columns {
                if let Some(other_region) = snapshot.get(other) {
                    let shared = thresholds
                        .get_mut(column)
                        .expect("collected correlated text column");
                    let before_strings = shared.strings.len();
                    let before_prefixes = shared.prefixes.len();
                    shared.strings.extend(other_region.strings.iter().cloned());
                    shared
                        .prefixes
                        .extend(other_region.prefixes.iter().cloned());
                    changed |= shared.strings.len() != before_strings
                        || shared.prefixes.len() != before_prefixes;
                }
            }
        }
        if !changed {
            break;
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
                    RowScalar::String(value) if string_required => {
                        string_bounds.is_some_and(|bounds| {
                            bounds
                                .max_chars
                                .is_none_or(|max| value.chars().count() as u64 <= max)
                        })
                    }
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
                    if *operator == Op::Eq {
                        equal
                    } else {
                        !equal
                    }
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
        BooleanRowConstraint::StringColumnComparison {
            left,
            operator,
            right,
        } => {
            use crate::protocol::ComparisonOperator as Op;
            // Byte equality under attested binary collation. Ordinary
            // operators yield UNKNOWN when either side is NULL; NULL-safe
            // operators are two-valued.
            let read = |column: &ColumnRef| -> Option<Option<&str>> {
                match assignment.get(column)? {
                    Some(RowScalar::String(value)) => Some(Some(value.as_str())),
                    None => Some(None),
                    _ => None,
                }
            };
            let lhs = read(left)?;
            let rhs = read(right)?;
            let result = match operator {
                Op::IsDistinctFrom => lhs != rhs,
                Op::IsNotDistinctFrom => lhs == rhs,
                Op::Eq | Op::Neq => {
                    let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                        return Some(SqlTruth::Unknown);
                    };
                    (lhs == rhs) == (*operator == Op::Eq)
                }
                Op::Lt | Op::Lte | Op::Gt | Op::Gte => return None,
            };
            Some(if result {
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
    fn symbolic_physical_contradiction_survives_exhausted_sample_budget() {
        let a = ColumnRef::new(Some("t".to_string()), "a".to_string());
        let b = ColumnRef::new(Some("t".to_string()), "b".to_string());
        let mut conditions = vec![
            BooleanRowConstraint::IntegerColumnComparison {
                left: a.clone(),
                operator: ComparisonOperator::Gt,
                right: b.clone(),
            },
            BooleanRowConstraint::IntegerColumnComparison {
                left: b,
                operator: ComparisonOperator::Gt,
                right: a,
            },
        ];
        // Fourteen NULL-only columns push the Cartesian representative search
        // above 4096 assignments, while the strict cycle is still impossible.
        for position in 0..14 {
            conditions.push(BooleanRowConstraint::NullTest {
                column: ColumnRef::new(Some("t".to_string()), format!("nullable_{position}")),
                negated: false,
            });
        }
        let references = conditions.iter().collect::<Vec<_>>();
        let requirements = references
            .iter()
            .map(|item| (*item, BooleanTruthCase::True))
            .collect::<Vec<_>>();
        let known = |column: &ColumnRef| column.relation() == Some("t");
        let bounds = |column: &ColumnRef| {
            // NULL-only string columns must not require integer evidence.
            if column.name().starts_with("nullable_") {
                None
            } else {
                Some(SignedIntegerEvidence {
                    minimum: -1,
                    maximum: 1,
                })
            }
        };
        assert_eq!(
            conjoin_physical_row_truths("t", BooleanTruthCase::True, &references, known, bounds,)
                .map(|(_, satisfiable)| satisfiable),
            Some(false)
        );
        assert_eq!(
            jointly_satisfiable_physical_truths("t", &requirements, known, bounds),
            Some(false)
        );
    }

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
