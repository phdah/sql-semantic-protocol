//! Set operations, branch semantics, and tuple witnesses.

use super::*;

/// A semantic SQL set operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOperator {
    /// UNION.
    Union,
    /// INTERSECT.
    Intersect,
    /// EXCEPT.
    Except,
}

impl SetOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Intersect => "intersect",
            Self::Except => "except",
        }
    }
}

/// Quantifier and alignment semantics for a SQL set operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetQuantifier {
    /// DISTINCT semantics, including an omitted SQL quantifier.
    Distinct,
    /// ALL semantics.
    All,
    /// BY NAME with the dialect-defined default duplicate treatment.
    ByName,
    /// ALL BY NAME.
    AllByName,
    /// DISTINCT BY NAME.
    DistinctByName,
}

impl SetQuantifier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Distinct => "distinct",
            Self::All => "all",
            Self::ByName => "by_name",
            Self::AllByName => "all_by_name",
            Self::DistinctByName => "distinct_by_name",
        }
    }

    pub(crate) fn uses_name_alignment(self) -> bool {
        matches!(self, Self::ByName | Self::AllByName | Self::DistinctByName)
    }
}

/// One operand in a recursive SQL set-operation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOperand {
    /// A query operand whose local semantics are represented by the surrounding query analysis.
    Query,
    /// A nested set operation.
    Operation(Box<SetOperation>),
}

/// The number of matching output tuples produced by an operation as a function of the
/// left and right operand tuple counts. Tuples compare with SQL IS NOT DISTINCT FROM
/// semantics, including NULL-to-NULL equality across every aligned output position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetMultiplicityRule {
    /// UNION ALL: left + right.
    Sum,
    /// UNION DISTINCT: one if either operand contains the tuple, otherwise zero.
    UnionDistinct,
    /// INTERSECT ALL: minimum of the two counts.
    Minimum,
    /// INTERSECT DISTINCT: one if both operands contain the tuple.
    IntersectDistinct,
    /// EXCEPT ALL: maximum of left minus right and zero.
    SaturatingDifference,
    /// EXCEPT DISTINCT: one if left contains the tuple and right does not.
    ExceptDistinct,
}

impl SetMultiplicityRule {
    /// Stable count rule for consumers to apply without re-interpreting SQL syntax.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::UnionDistinct => "union_distinct",
            Self::Minimum => "minimum",
            Self::IntersectDistinct => "intersect_distinct",
            Self::SaturatingDifference => "saturating_difference",
            Self::ExceptDistinct => "except_distinct",
        }
    }

    /// Evaluate a tuple's output multiplicity from nonnegative operand counts.
    pub fn evaluate(self, left: u64, right: u64) -> u64 {
        match self {
            Self::Sum => left.saturating_add(right),
            Self::UnionDistinct => u64::from(left > 0 || right > 0),
            Self::Minimum => left.min(right),
            Self::IntersectDistinct => u64::from(left > 0 && right > 0),
            Self::SaturatingDifference => left.saturating_sub(right),
            Self::ExceptDistinct => u64::from(left > 0 && right == 0),
        }
    }
}

/// Branch-local facts, retained separately to avoid combining incompatible source-row
/// alternatives into independent source-column domains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetBranch {
    identity: String,
    sources: Vec<SourceRelation>,
    predicates: Predicates,
    column_domains: Vec<ColumnDomain>,
    output: Output,
    condition_exactness: ConditionExactness,
    dependencies: Vec<String>,
    witness_boundary: Option<SetWitnessBoundary>,
    empty_input_preserving: bool,
}

impl SetBranch {
    pub(crate) fn new(
        identity: String,
        query: &QueryStatement,
        witness_boundary: Option<SetWitnessBoundary>,
    ) -> Self {
        Self {
            identity,
            sources: query.sources.clone(),
            predicates: (*query.row_conditions.predicates).clone(),
            column_domains: query.row_conditions.column_domains.clone(),
            output: query.output().clone(),
            condition_exactness: query.row_conditions.exactness.clone(),
            dependencies: query.dependencies.clone(),
            witness_boundary,
            empty_input_preserving: query.sources().len() == 1
                && (query.row_preserving_projection() || query.filter_only_row_shape())
                && query.aggregation().is_none()
                && query.set_operation().is_none()
                && query.window_witness().is_none()
                && query.subquery_witnesses().is_empty()
                && query.output().columns().iter().all(|column| {
                    matches!(
                        column.expression(),
                        Expression::Column(_) | Expression::Literal(_)
                    )
                }),
        }
    }

    /// Whether the parsed branch is a direct one-to-one read or filter of one
    /// named relation with no row-creating aggregate, subquery, or window.
    /// This is a zero-input proof, not a count or tuple-membership witness.
    pub fn empty_input_preserving(&self) -> bool {
        self.empty_input_preserving
    }

    /// Deterministic location of this leaf, such as body:left or body:right.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Source relations referenced within this branch.
    pub fn sources(&self) -> &[SourceRelation] {
        &self.sources
    }

    /// Predicates specific to this branch.
    pub fn predicates(&self) -> &Predicates {
        &self.predicates
    }

    /// Source-column domains specific to this branch, not an intersection with other branches.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.column_domains
    }

    /// Positionally aligned source expressions and lineage in this branch.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Whether the branch's row filters were proven, independently of set membership.
    pub fn condition_exactness(&self) -> &ConditionExactness {
        &self.condition_exactness
    }

    /// Transitive physical dependencies for checking witness independence.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Boundary eligible for exact tuple-count obligations, if proven.
    pub fn witness_boundary(&self) -> Option<&SetWitnessBoundary> {
        self.witness_boundary.as_ref()
    }
}

/// A relation boundary where matching output-tuple counts can be controlled exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetWitnessBoundary {
    relation: String,
    tuple_columns: Vec<String>,
    intermediate: bool,
}

impl SetWitnessBoundary {
    pub(crate) fn new(relation: String, tuple_columns: Vec<String>, intermediate: bool) -> Self {
        Self {
            relation,
            tuple_columns,
            intermediate,
        }
    }

    /// Source table or named intermediate relation read by this branch.
    pub fn relation(&self) -> &str {
        &self.relation
    }
    /// Positional input columns corresponding to the candidate output tuple.
    pub fn tuple_columns(&self) -> &[String] {
        &self.tuple_columns
    }
    /// Whether this boundary requires upstream producer realization rather than direct source loading.
    pub fn is_intermediate(&self) -> bool {
        self.intermediate
    }
}

/// Exact tuple-count requirement for one independent branch input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetWitnessObligation {
    branch_identity: String,
    boundary: SetWitnessBoundary,
    matching_tuple_count: u64,
}

impl SetWitnessObligation {
    /// Stable leaf identity in the parent set-operation tree.
    pub fn branch_identity(&self) -> &str {
        &self.branch_identity
    }
    /// Physical or intermediate boundary to which the requirement applies.
    pub fn boundary(&self) -> &SetWitnessBoundary {
        &self.boundary
    }
    /// Exact number of rows satisfying branch predicates and matching the output tuple using NULL-safe equality.
    pub fn matching_tuple_count(&self) -> u64 {
        self.matching_tuple_count
    }
}

/// A complete, mutually dependent set of row-count obligations for one output tuple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetWitnessCase {
    output_tuple_count: u64,
    obligations: Vec<SetWitnessObligation>,
}

impl SetWitnessCase {
    /// Result multiplicity under the operation tree when obligations hold.
    pub fn output_tuple_count(&self) -> u64 {
        self.output_tuple_count
    }
    /// All obligations must hold simultaneously; zero counts are closed-world absence proofs.
    pub fn obligations(&self) -> &[SetWitnessObligation] {
        &self.obligations
    }

    /// Rebind an exact local tuple-count law to certified physical leaves.
    /// The mapper must prove complete, identity-preserving source branches;
    /// an intermediate boundary never becomes writable by assumption.
    pub(crate) fn with_physical_boundaries(
        &self,
        map: &dyn Fn(&SetWitnessBoundary) -> Option<SetWitnessBoundary>,
    ) -> Option<Self> {
        let obligations = self
            .obligations
            .iter()
            .map(|obligation| {
                let boundary = map(&obligation.boundary)?;
                if boundary.is_intermediate()
                    || boundary.tuple_columns().len() != obligation.boundary.tuple_columns().len()
                {
                    return None;
                }
                Some(SetWitnessObligation {
                    branch_identity: obligation.branch_identity.clone(),
                    boundary,
                    matching_tuple_count: obligation.matching_tuple_count,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            output_tuple_count: self.output_tuple_count,
            obligations,
        })
    }
}

/// Proof outcome for a qualifying or non-qualifying tuple witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetWitnessDirection {
    /// Exact count plans, each sufficient on its own to establish the advertised result.
    Exact(Vec<SetWitnessCase>),
    /// A stable, explicit reason preventing generator-safe witness construction.
    Residual {
        reason: &'static str,
        origin: Option<String>,
    },
}

/// One UNION, INTERSECT, or EXCEPT operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetOperation {
    operator: SetOperator,
    quantifier: SetQuantifier,
    left: SetOperand,
    right: SetOperand,
    branches: Vec<SetBranch>,
    set_level_safe: bool,
}

impl SetOperation {
    pub(crate) fn new(
        operator: SetOperator,
        quantifier: SetQuantifier,
        left: SetOperand,
        right: SetOperand,
    ) -> Self {
        Self {
            operator,
            quantifier,
            left,
            right,
            branches: Vec::new(),
            set_level_safe: false,
        }
    }

    pub(crate) fn with_set_level_safety(mut self, safe: bool) -> Self {
        self.set_level_safe = safe;
        self
    }

    pub(crate) fn with_branches(mut self, branches: Vec<SetBranch>) -> Self {
        self.branches = branches;
        self
    }

    /// Return a typed duplicate-count rule, or None when BY NAME alignment is not modeled.
    pub fn multiplicity_rule(&self) -> Option<SetMultiplicityRule> {
        if self.quantifier.uses_name_alignment() {
            return None;
        }
        match (self.operator, self.quantifier) {
            (SetOperator::Union, SetQuantifier::All) => Some(SetMultiplicityRule::Sum),
            (SetOperator::Union, SetQuantifier::Distinct) => {
                Some(SetMultiplicityRule::UnionDistinct)
            }
            (SetOperator::Intersect, SetQuantifier::All) => Some(SetMultiplicityRule::Minimum),
            (SetOperator::Intersect, SetQuantifier::Distinct) => {
                Some(SetMultiplicityRule::IntersectDistinct)
            }
            (SetOperator::Except, SetQuantifier::All) => {
                Some(SetMultiplicityRule::SaturatingDifference)
            }
            (SetOperator::Except, SetQuantifier::Distinct) => {
                Some(SetMultiplicityRule::ExceptDistinct)
            }
            (
                _,
                SetQuantifier::ByName | SetQuantifier::AllByName | SetQuantifier::DistinctByName,
            ) => None,
        }
    }

    /// Construct generator-consumable positive and negative tuple-count proofs.
    ///
    /// Only independent, row-preserving branch boundaries qualify. Repeated underlying
    /// dependencies, unmapped projections and unsupported nested semantics stay residual.
    /// A zero count requires proving that no other matching rows exist at that boundary.
    pub fn witness_directions(&self) -> (SetWitnessDirection, SetWitnessDirection) {
        let residual = |reason: &'static str, origin: Option<String>| {
            (
                SetWitnessDirection::Residual {
                    reason,
                    origin: origin.clone(),
                },
                SetWitnessDirection::Residual { reason, origin },
            )
        };
        if !self.set_level_safe {
            return residual("set_level_membership_modifier", Some("body".to_string()));
        }
        if self.branches.is_empty() || self.branches.len() > 4 {
            return residual("unsupported_branch_count", Some("body".to_string()));
        }
        if !self.has_supported_tree() {
            return residual("unsupported_alignment", Some("body".to_string()));
        }
        if self.branches.iter().any(|branch| {
            branch.output().columns().len() != self.branches[0].output().columns().len()
        }) || self.branches[0].output().columns().is_empty()
        {
            return residual("unresolved_positional_alignment", Some("body".to_string()));
        }
        let mut physical_dependencies = std::collections::BTreeSet::new();
        for branch in &self.branches {
            if !branch.condition_exactness().is_exact() {
                return residual(
                    "inexact_branch_conditions",
                    Some(branch.identity().to_string()),
                );
            }
            if branch.witness_boundary().is_none() || branch.dependencies().is_empty() {
                return residual(
                    "unresolved_branch_boundary",
                    Some(branch.identity().to_string()),
                );
            }
            for relation in branch.dependencies() {
                if !physical_dependencies.insert(relation) {
                    return residual(
                        "shared_physical_dependency",
                        Some(branch.identity().to_string()),
                    );
                }
            }
        }
        // Enumerate small, finite bag counts rather than assuming DISTINCT semantics.
        // The 0/1/2 cases include duplicates and cancellation for EXCEPT ALL.
        let total = 3_usize.pow(self.branches.len() as u32);
        let mut qualifying = Vec::new();
        let mut non_qualifying = Vec::new();
        for number in 0..total {
            let mut n = number;
            let counts = (0..self.branches.len())
                .map(|_| {
                    let count = (n % 3) as u64;
                    n /= 3;
                    count
                })
                .collect::<Vec<_>>();
            if !self.candidate_domains_compatible(&counts) {
                continue;
            }
            let mut cursor = 0;
            let Some(output_tuple_count) = self.count_for_leaves(&counts, &mut cursor) else {
                continue;
            };
            if cursor != counts.len() {
                continue;
            }
            let obligations = self
                .branches
                .iter()
                .zip(counts)
                .map(|(branch, count)| SetWitnessObligation {
                    branch_identity: branch.identity().to_string(),
                    boundary: branch
                        .witness_boundary()
                        .expect("validated branch boundary")
                        .clone(),
                    matching_tuple_count: count,
                })
                .collect();
            let case = SetWitnessCase {
                output_tuple_count,
                obligations,
            };
            if output_tuple_count > 0 {
                qualifying.push(case);
            } else {
                non_qualifying.push(case);
            }
        }
        let direction = |cases: Vec<SetWitnessCase>, reason| {
            if cases.is_empty() {
                SetWitnessDirection::Residual {
                    reason,
                    origin: Some("body".to_string()),
                }
            } else {
                SetWitnessDirection::Exact(cases)
            }
        };
        (
            direction(qualifying, "no_feasible_qualifying_counts"),
            direction(non_qualifying, "no_feasible_non_qualifying_counts"),
        )
    }

    fn has_supported_tree(&self) -> bool {
        self.multiplicity_rule().is_some()
            && [self.left(), self.right()]
                .iter()
                .all(|operand| match operand {
                    SetOperand::Query => true,
                    SetOperand::Operation(operation) => operation.has_supported_tree(),
                })
    }

    fn count_for_leaves(&self, counts: &[u64], cursor: &mut usize) -> Option<u64> {
        let eval = |operand: &SetOperand, cursor: &mut usize| match operand {
            SetOperand::Query => {
                let count = counts.get(*cursor).copied();
                *cursor += 1;
                count
            }
            SetOperand::Operation(operation) => operation.count_for_leaves(counts, cursor),
        };
        let left = eval(&self.left, cursor)?;
        let right = eval(&self.right, cursor)?;
        Some(self.multiplicity_rule()?.evaluate(left, right))
    }

    fn candidate_domains_compatible(&self, counts: &[u64]) -> bool {
        for index in 0..self.branches[0].output().columns().len() {
            let mut domain = ValueDomain::Unbounded;
            for (branch, count) in self.branches.iter().zip(counts) {
                if *count == 0 {
                    continue;
                }
                domain = crate::domain::intersect_set_operation_domains(
                    &domain,
                    branch.output().columns()[index].domain(),
                );
                if matches!(domain, ValueDomain::Empty) {
                    return false;
                }
            }
        }
        true
    }

    /// Return all leaf branches with deterministic identities; nested branches are flattened.
    pub fn branches(&self) -> &[SetBranch] {
        &self.branches
    }

    /// Return the set operator.
    pub fn operator(&self) -> SetOperator {
        self.operator
    }

    /// Return duplicate-treatment and alignment semantics.
    pub fn quantifier(&self) -> SetQuantifier {
        self.quantifier
    }

    /// Return the left operand.
    pub fn left(&self) -> &SetOperand {
        &self.left
    }

    /// Return the right operand.
    pub fn right(&self) -> &SetOperand {
        &self.right
    }
}
