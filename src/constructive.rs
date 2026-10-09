//! Typed, source-independent constructive obligations.
//!
//! These obligations are *local* operator evidence, not a claim that an entire
//! dependency graph is writable at a physical leaf. TASK-68 owns that proof.
//! Intermediate boundaries must be realized through their producer before use.

use crate::boolean_witness::{BooleanRowConstraint, BooleanTruthCase, BooleanWitnessDirection};
use crate::bundle::{GroupBoundaryKind, ResolvedComposedSemantics};
use crate::group_witness::{GroupAggregate, GroupValueTest, GroupWitnessDirection};
use crate::join_witness::{JoinSide, JoinWitnessDirection, JoinWitnessShape};
use crate::protocol::{ColumnDomain, ColumnRef, ComparisonOperator, SetWitnessDirection};
use crate::subquery_witness::{
    SubqueryCorrelation, SubqueryMembershipCase, SubqueryMembershipDirection,
};
use crate::window_witness::{WindowOrderKey, WindowWitnessDirection};

/// A named candidate or partner row scoped to a relation instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowVariable {
    relation: String,
    instance: String,
    name: String,
}

impl RowVariable {
    /// Construct an identified source-row variable, rejecting empty identities.
    pub fn new(relation: &str, instance: &str, name: &str) -> Option<Self> {
        if relation.is_empty() || instance.is_empty() || name.is_empty() {
            return None;
        }
        Some(Self {
            relation: relation.to_string(),
            instance: instance.to_string(),
            name: name.to_string(),
        })
    }

    /// Relation identity, which is not necessarily a writable physical source.
    pub fn relation(&self) -> &str {
        &self.relation
    }
    /// Distinct relation instance (important for self-joins).
    pub fn instance(&self) -> &str {
        &self.instance
    }
    /// Variable name scoped to the witness case.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A typed scalar term; unknown functions are never silently represented as values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessTerm {
    /// Value of a source column on one bound row.
    Column { row: RowVariable, column: ColumnRef },
    /// Literal integer, whose source datatype must be verified separately.
    Integer(i64),
    /// SQL NULL, never equal under ordinary SQL equality.
    Null,
}

/// Logical condition that a row, tuple, or a collection of rows must satisfy.
///
/// The logical operators apply at one proof boundary; SQL row predicates use
/// three-valued truth, not Rust Boolean negation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessFormula {
    /// All operands apply jointly, including repeated columns on the same row.
    All(Vec<WitnessFormula>),
    /// Any one operand supplies a sufficient alternative.
    Any(Vec<WitnessFormula>),
    /// SQL NOT: UNKNOWN stays UNKNOWN.
    Not(Box<WitnessFormula>),
    /// Require SQL TRUE or NOT TRUE (FALSE or UNKNOWN) of one row predicate.
    RowTruth {
        row: RowVariable,
        predicate: BooleanRowConstraint,
        truth: BooleanTruthCase,
    },
    /// Compare typed terms under the given SQL comparison operator.
    Comparison {
        left: WitnessTerm,
        operator: ComparisonOperator,
        right: WitnessTerm,
    },
    /// Test whether a column value is (or is not) NULL.
    IsNull { term: WitnessTerm, negated: bool },
    /// Coupled multi-column tuple equality or inequality on named row variables.
    TupleComparison {
        left: Vec<WitnessTerm>,
        equal: bool,
        right: Vec<WitnessTerm>,
    },
    /// Exact prefix check with binary-collation assumptions established upstream.
    StringPrefix {
        term: WitnessTerm,
        prefix: String,
        negated: bool,
    },
}

/// Nonempty bounded row counts, with inclusive upper bounds and no overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountBounds {
    minimum: u64,
    maximum: Option<u64>,
}

impl CountBounds {
    /// Create valid inclusive cardinality bounds.
    pub fn new(minimum: u64, maximum: Option<u64>) -> Option<Self> {
        if maximum.is_some_and(|max| minimum > max) {
            return None;
        }
        Some(Self { minimum, maximum })
    }

    /// Minimum row count.
    pub fn minimum(self) -> u64 {
        self.minimum
    }
    /// Maximum row count when finite.
    pub fn maximum(self) -> Option<u64> {
        self.maximum
    }
}

/// Quantification of matching rows in a relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowQuantifier {
    /// A bounded number of candidate rows must satisfy the condition.
    Exists,
    /// Every candidate row must satisfy the condition; requires a closed-world scope.
    ForAll,
}

/// Whether a proof boundary is directly writable or must be realized by its producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessBoundary {
    relation: String,
    kind: GroupBoundaryKind,
    origin_layer_id: String,
}

impl WitnessBoundary {
    /// Create a typed boundary, retaining provenance for intermediate realization.
    pub fn new(relation: &str, kind: GroupBoundaryKind, origin_layer_id: &str) -> Option<Self> {
        if relation.is_empty() || origin_layer_id.is_empty() {
            return None;
        }
        Some(Self {
            relation: relation.to_string(),
            kind,
            origin_layer_id: origin_layer_id.to_string(),
        })
    }

    /// Relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }
    /// Physical, intermediate, or unresolved proof boundary.
    pub fn kind(&self) -> GroupBoundaryKind {
        self.kind
    }
    /// Layer introducing the obligation.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }
}

/// One required fact. None of these grant physical-source realizability on their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessObligation {
    /// SQL three-valued row condition, with shared row identity.
    Predicate(WitnessFormula),
    /// Row count and universal/existential predicate on a closed candidate set.
    Rows {
        boundary: WitnessBoundary,
        quantifier: RowQuantifier,
        bounds: CountBounds,
        predicate: WitnessFormula,
        closed_world: bool,
    },
    /// Explicit absence of a matching partner (not merely an absent example row).
    NoMatchingPartner {
        candidate: RowVariable,
        partner: RowVariable,
        comparison: ComparisonOperator,
        left: ColumnRef,
        right: ColumnRef,
        closed_world: bool,
    },
    /// One matched join pair with a typed comparison.
    JoinPair {
        left_row: RowVariable,
        right_row: RowVariable,
        left: ColumnRef,
        right: ColumnRef,
        comparison: ComparisonOperator,
        null_extended: Option<JoinSide>,
    },
    /// Complete group construction at the named boundary, including contributor tests.
    Group {
        boundary: WitnessBoundary,
        key: Vec<ColumnRef>,
        aggregate: GroupAggregate,
        distinct: bool,
        argument: Option<ColumnRef>,
        rows: CountBounds,
        non_null: CountBounds,
        tests: Vec<GroupValueTest>,
    },
    /// Rank a candidate within a fully controlled partition.
    Ranked {
        boundary: WitnessBoundary,
        candidate: RowVariable,
        partition_by: Vec<ColumnRef>,
        order_by: Vec<WindowOrderKey>,
        preceding: CountBounds,
        strict_unique: bool,
        closed_world: bool,
    },
    /// A membership case with explicit NULL and correlated-key laws.
    Membership {
        /// Candidate physical outer row.
        outer: RowVariable,
        /// Candidate inner row set, potentially empty.
        inner: WitnessBoundary,
        /// Existing typed EXISTS/IN/NOT IN witness law.
        case: SubqueryMembershipCase,
        /// Joint correlated equalities, not independently sampled.
        correlations: Vec<SubqueryCorrelation>,
        /// Optional IN membership key.
        membership_key: Option<SubqueryCorrelation>,
        /// Exact typed inner source-column constraints from upstream analysis.
        inner_domains: Vec<ColumnDomain>,
        /// All eligible inner rows must be controlled for absence and NULL evidence.
        closed_world: bool,
    },
    /// Exact candidate-tuple multiplicity in one independent set operand.
    SetTuple {
        /// Stable identity of this leaf branch in the operation tree.
        branch_identity: String,
        /// Physical boundary; an intermediate requires producer proof instead.
        boundary: WitnessBoundary,
        /// Positional projection of the candidate tuple onto source columns.
        tuple_columns: Vec<String>,
        /// Exact number of equal tuples in the branch under IS NOT DISTINCT FROM.
        matching_rows: u64,
        /// Typed branch-domain requirements; never reparse SQL in consumers.
        column_domains: Vec<ColumnDomain>,
        /// Exact absence is required when matching_rows is zero.
        closed_world: bool,
    },
    /// Multiplicity of the candidate tuple after a complete set-operation tree.
    SetResultTuple {
        /// Exact output count of this tuple, not the full output cardinality.
        matching_rows: u64,
        /// SQL set equality treats corresponding NULLs as equal.
        nulls_equal: bool,
    },
    /// Final output count requirement.
    OutputRows {
        layer_id: String,
        bounds: CountBounds,
    },
    /// Before/after relation state count requirement; requires ordered state composition.
    StateRows {
        relation: String,
        before: CountBounds,
        after: CountBounds,
    },
    /// A computed boundary must be realized through its named upstream producer.
    Producer {
        boundary: WitnessBoundary,
        /// Physical leaf relations that the producer must be realized from.
        physical_sources: Vec<String>,
    },
}

/// Whether a case is proved sufficient, necessary, or equivalent to classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofStrength {
    /// Fulfilling the obligations guarantees the classification.
    Sufficient,
    /// Every classified row must fulfill the obligations.
    Necessary,
    /// The obligations are both necessary and sufficient.
    Equivalent,
}

/// A jointly enforced conjunction of typed witness obligations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessCase {
    obligations: Vec<WitnessObligation>,
    strength: ProofStrength,
}

impl WitnessCase {
    /// Construct a nonempty case; reject directly conflicting cardinality and NULL facts.
    ///
    /// This local check is deliberately incomplete: success does not prove cross-row
    /// satisfiability, type compatibility, or producer realizability.
    pub fn new(obligations: Vec<WitnessObligation>, strength: ProofStrength) -> Option<Self> {
        let mut unique = Vec::new();
        for obligation in obligations {
            if !unique.contains(&obligation) {
                unique.push(obligation);
            }
        }
        let obligations = unique;
        if obligations.is_empty()
            || obligations.iter().any(invalid_obligation)
            || directly_conflicts(&obligations)
        {
            return None;
        }
        Some(Self {
            obligations,
            strength,
        })
    }
    /// Requirements that must hold together.
    pub fn obligations(&self) -> &[WitnessObligation] {
        &self.obligations
    }
    /// Strength of the proof relative to the classified result.
    pub fn strength(&self) -> ProofStrength {
        self.strength
    }
}

fn invalid_formula(formula: &WitnessFormula) -> bool {
    match formula {
        WitnessFormula::All(children) | WitnessFormula::Any(children) => {
            children.is_empty() || children.iter().any(invalid_formula)
        }
        WitnessFormula::Not(child) => invalid_formula(child),
        WitnessFormula::RowTruth { predicate, .. } => !predicate.is_exact(),
        WitnessFormula::TupleComparison { left, right, .. } => {
            left.is_empty() || left.len() != right.len()
        }
        WitnessFormula::Comparison { .. }
        | WitnessFormula::IsNull { .. }
        | WitnessFormula::StringPrefix { .. } => false,
    }
}

fn invalid_obligation(obligation: &WitnessObligation) -> bool {
    match obligation {
        WitnessObligation::Predicate(formula) => invalid_formula(formula),
        WitnessObligation::Rows {
            quantifier,
            bounds,
            predicate,
            closed_world,
            ..
        } => {
            invalid_formula(predicate)
                || (matches!(quantifier, RowQuantifier::Exists) && bounds.minimum() == 0)
                || (matches!(quantifier, RowQuantifier::ForAll) && !closed_world)
        }
        WitnessObligation::NoMatchingPartner { closed_world, .. } => !closed_world,
        WitnessObligation::JoinPair { null_extended, .. } => null_extended.is_some(),
        WitnessObligation::Group { rows, non_null, .. } => {
            non_null.minimum() > rows.maximum().unwrap_or(u64::MAX)
        }
        WitnessObligation::Ranked {
            strict_unique,
            closed_world,
            order_by,
            ..
        } => !strict_unique || !closed_world || order_by.is_empty(),
        WitnessObligation::Membership { closed_world, .. } => !closed_world,
        WitnessObligation::SetTuple {
            branch_identity,
            tuple_columns,
            matching_rows,
            closed_world,
            ..
        } => {
            branch_identity.is_empty()
                || tuple_columns.is_empty()
                || (*matching_rows == 0 && !closed_world)
        }
        WitnessObligation::SetResultTuple { nulls_equal, .. } => !nulls_equal,
        WitnessObligation::OutputRows { layer_id, .. } => layer_id.is_empty(),
        WitnessObligation::StateRows { relation, .. } => relation.is_empty(),
        WitnessObligation::Producer {
            boundary,
            physical_sources,
        } => {
            boundary.kind() != GroupBoundaryKind::Intermediate
                || physical_sources.iter().any(String::is_empty)
        }
    }
}

fn directly_conflicts(obligations: &[WitnessObligation]) -> bool {
    for (index, first) in obligations.iter().enumerate() {
        for second in obligations.iter().skip(index + 1) {
            match (first, second) {
                (
                    WitnessObligation::OutputRows {
                        layer_id: l,
                        bounds: a,
                    },
                    WitnessObligation::OutputRows {
                        layer_id: r,
                        bounds: b,
                    },
                ) if l == r => {
                    if disjoint(*a, *b) {
                        return true;
                    }
                }
                (
                    WitnessObligation::StateRows {
                        relation: l,
                        before: ab,
                        after: aa,
                    },
                    WitnessObligation::StateRows {
                        relation: r,
                        before: bb,
                        after: ba,
                    },
                ) if l == r => {
                    if disjoint(*ab, *bb) || disjoint(*aa, *ba) {
                        return true;
                    }
                }
                (
                    WitnessObligation::Predicate(WitnessFormula::RowTruth {
                        row: a,
                        predicate: ap,
                        truth: at,
                    }),
                    WitnessObligation::Predicate(WitnessFormula::RowTruth {
                        row: b,
                        predicate: bp,
                        truth: bt,
                    }),
                ) if a == b && ap == bp && at != bt => return true,
                _ => {}
            }
        }
    }
    false
}

fn disjoint(left: CountBounds, right: CountBounds) -> bool {
    left.maximum.is_some_and(|max| max < right.minimum)
        || right.maximum.is_some_and(|max| max < left.minimum)
}

/// Classification is independent in the qualifying and rejected directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessDirection {
    /// One of these separately sufficient constructions is available.
    Feasible(Vec<WitnessCase>),
    /// The specified classification is impossible at this local operator.
    Impossible,
    /// No constructive proof exists; never assume this means impossible.
    Residual { reason: String },
}

impl WitnessDirection {
    /// Validate that every feasible direction has at least one sufficient case.
    pub fn feasible(cases: Vec<WitnessCase>) -> Option<Self> {
        if cases.is_empty()
            || cases.iter().any(|case| {
                !matches!(
                    case.strength(),
                    ProofStrength::Sufficient | ProofStrength::Equivalent
                )
            })
        {
            return None;
        }
        Some(Self::Feasible(cases))
    }

    /// Conjoin sufficient cases at the same proof boundary, preserving shared row variables.
    ///
    /// A finite 256-pair bound prevents explosive proof enumeration. Unknown
    /// satisfiability remains residual, while proven direct contradictions are impossible.
    pub fn all(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Impossible, _) | (_, Self::Impossible) => Self::Impossible,
            (Self::Residual { .. }, _) | (_, Self::Residual { .. }) => {
                Self::residual("conjunctive_operand_not_proven")
            }
            (Self::Feasible(left), Self::Feasible(right)) => {
                if left.len().checked_mul(right.len()).is_none_or(|n| n > 256) {
                    return Self::residual("conjunctive_case_limit");
                }
                let fully_equivalent = left.len() == 1 && right.len() == 1
                    && left[0].strength() == ProofStrength::Equivalent
                    && right[0].strength() == ProofStrength::Equivalent;
                let mut cases = Vec::new();
                let mut unproved = false;
                for a in left {
                    for b in right {
                        // Sufficient constructions are examples, not exhaustive
                        // model sets. Two different examples cannot be merged
                        // without a proof of joint satisfiability.
                        if a.obligations() != b.obligations() {
                            unproved = true;
                            continue;
                        }
                        let strength = if a.strength() == ProofStrength::Equivalent
                            && b.strength() == ProofStrength::Equivalent
                        {
                            ProofStrength::Equivalent
                        } else {
                            ProofStrength::Sufficient
                        };
                        if let Some(case) = WitnessCase::new(a.obligations().to_vec(), strength) {
                            if !cases.contains(&case) {
                                cases.push(case);
                            }
                        }
                    }
                }
                if !cases.is_empty() {
                    Self::Feasible(cases)
                } else if fully_equivalent && directly_conflicts(
                    &left[0].obligations().iter().cloned()
                        .chain(right[0].obligations().iter().cloned())
                        .collect::<Vec<_>>()
                ) {
                    Self::Impossible
                } else if unproved {
                    Self::residual("conjunctive_realization_unproven")
                } else {
                    Self::residual("conjunctive_case_satisfiability_unproven")
                }
            }
        }
    }

    /// Disjoin sufficient alternatives; one proven branch suffices even when the other is residual.
    pub fn any(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Feasible(a), Self::Feasible(b)) => {
                let mut cases = a.clone();
                for case in b {
                    if !cases.contains(case) {
                        cases.push(case.clone());
                    }
                }
                Self::Feasible(cases)
            }
            (Self::Feasible(_), _) => self.clone(),
            (_, Self::Feasible(_)) => other.clone(),
            (Self::Impossible, Self::Impossible) => Self::Impossible,
            _ => Self::residual("disjunctive_operand_not_proven"),
        }
    }

    fn residual(reason: &str) -> Self {
        Self::Residual {
            reason: reason.to_string(),
        }
    }
}

/// Origin of one typed operator proof in the resolved dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessOperator {
    /// Coupled WHERE predicate.
    Boolean,
    /// Join matched/unmatched row membership.
    Join,
    /// Grouped HAVING.
    Group,
    /// Window ranking.
    Window,
    /// Subquery membership.
    Subquery,
    /// SQL set operation.
    Set,
}

impl WitnessOperator {
    /// Stable schema tag.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Join => "join",
            Self::Group => "group",
            Self::Window => "window",
            Self::Subquery => "subquery",
            Self::Set => "set",
        }
    }
}

/// Canonical typed local witness with two independent proof directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructiveWitness {
    operator: WitnessOperator,
    origin_layer_id: String,
    qualifying: WitnessDirection,
    rejected: WitnessDirection,
}

impl ConstructiveWitness {
    /// Operator that supplied the local proof.
    pub fn operator(&self) -> WitnessOperator {
        self.operator
    }
    /// Layer supplying provenance.
    pub fn origin_layer_id(&self) -> &str {
        &self.origin_layer_id
    }
    /// Matching classification, independently proven.
    pub fn qualifying(&self) -> &WitnessDirection {
        &self.qualifying
    }
    /// Rejected classification, independently proven.
    pub fn rejected(&self) -> &WitnessDirection {
        &self.rejected
    }

    /// SQL AND of two classifications introduced at the same operator boundary.
    ///
    /// TRUE requires both inputs TRUE; FALSE or UNKNOWN requires at least
    /// one input NOT TRUE. Both directions retain sufficient witness cases.
    pub fn logical_and(&self, other: &Self) -> Option<Self> {
        if self.origin_layer_id != other.origin_layer_id {
            return None;
        }
        Some(Self {
            operator: WitnessOperator::Boolean,
            origin_layer_id: self.origin_layer_id.clone(),
            qualifying: self.qualifying.all(&other.qualifying),
            rejected: self.rejected.any(&other.rejected),
        })
    }

    /// SQL OR of two classifications introduced at the same operator boundary.
    pub fn logical_or(&self, other: &Self) -> Option<Self> {
        if self.origin_layer_id != other.origin_layer_id {
            return None;
        }
        Some(Self {
            operator: WitnessOperator::Boolean,
            origin_layer_id: self.origin_layer_id.clone(),
            qualifying: self.qualifying.any(&other.qualifying),
            rejected: self.rejected.all(&other.rejected),
        })
    }

    /// SQL NOT at this boundary. A nullable expression's NOT TRUE is not
    /// equivalent to FALSE: an UNKNOWN input remains UNKNOWN under SQL NOT.
    ///
    /// An explicitly proven two-valued expression permits swapping directions.
    /// Otherwise qualifying is residual and known TRUE cases remain sufficient
    /// for a rejected NOT, but an impossible TRUE does not prove NOT impossible.
    pub fn logical_not(&self, proven_two_valued: bool) -> Self {
        let (qualifying, rejected) = if proven_two_valued {
            (self.rejected.clone(), self.qualifying.clone())
        } else {
            (
                WitnessDirection::residual("sql_not_requires_false_vs_unknown_proof"),
                match &self.qualifying {
                    WitnessDirection::Feasible(_) => self.qualifying.clone(),
                    _ => WitnessDirection::residual("nullable_not_rejection_unproven"),
                },
            )
        };
        Self {
            operator: WitnessOperator::Boolean,
            origin_layer_id: self.origin_layer_id.clone(),
            qualifying,
            rejected,
        }
    }
}

/// Return typed intermediate-source obligations that cannot be satisfied by
/// inserting rows directly into a derived relation.
///
/// Consumers must discharge these against the producer graph, with the physical
/// leaf dependencies in `ResolvedComposedSemantics::dependencies`. A pending
/// producer is not a proof that physical realization is possible (TASK-68).
pub fn local_pending_producers(semantics: &ResolvedComposedSemantics) -> Vec<WitnessObligation> {
    let mut boundaries = Vec::new();
    let mut add = |relation: Option<&str>, kind: GroupBoundaryKind, layer_id: &str| {
        if kind != GroupBoundaryKind::Intermediate {
            return;
        }
        if let Some(boundary) = relation.and_then(|r| WitnessBoundary::new(r, kind, layer_id)) {
            if !boundaries.contains(&boundary) {
                boundaries.push(boundary);
            }
        }
    };
    for witness in semantics.boolean_witnesses() {
        add(
            Some(witness.witness().source_relation()),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for witness in semantics.group_witnesses() {
        add(
            witness.witness().boundary(),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for witness in semantics.window_witnesses() {
        add(
            witness.witness().boundary(),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for witness in semantics.subquery_witnesses() {
        add(
            witness.witness().inner_relation(),
            witness.boundary_kind(),
            witness.origin_layer_id(),
        );
    }
    for item in semantics.set_operations() {
        let operation = item.operation();
        for branch in operation.branches() {
            if let Some(boundary) = branch.witness_boundary() {
                if boundary.is_intermediate() {
                    add(
                        Some(boundary.relation()),
                        GroupBoundaryKind::Intermediate,
                        item.origin_layer_id(),
                    );
                }
            }
        }
    }
    boundaries
        .sort_by(|a, b| (&a.origin_layer_id, &a.relation).cmp(&(&b.origin_layer_id, &b.relation)));
    boundaries
        .into_iter()
        .map(|boundary| WitnessObligation::Producer {
            boundary,
            physical_sources: semantics.dependencies().to_vec(),
        })
        .collect()
}

/// Translate existing operator-local proofs into one deterministic typed API.
///
/// No physical-source DAG realization is inferred. Operator families not yet
/// translated remain visibly residual, even when their legacy local witness
/// is exact, rather than being silently upgraded to full constructive proof.
pub fn local_constructive_witnesses(
    semantics: &ResolvedComposedSemantics,
) -> Vec<ConstructiveWitness> {
    let mut proofs = Vec::new();
    for source in semantics.boolean_witnesses() {
        let witness = source.witness();
        let row = RowVariable::new(
            witness.source_relation(),
            witness.source_relation(),
            "candidate",
        );
        let map = |direction: &BooleanWitnessDirection| match (
            row.as_ref(),
            direction,
            source.boundary_kind(),
        ) {
            (Some(row), BooleanWitnessDirection::Exact(truth), GroupBoundaryKind::Physical) => {
                let case = WitnessCase::new(
                    vec![WitnessObligation::Predicate(WitnessFormula::RowTruth {
                        row: row.clone(),
                        predicate: witness.condition().clone(),
                        truth: *truth,
                    })],
                    ProofStrength::Sufficient,
                );
                case.and_then(|c| WitnessDirection::feasible(vec![c]))
                    .unwrap_or_else(|| WitnessDirection::residual("invalid_boolean_case"))
            }
            (_, BooleanWitnessDirection::Residual { reason }, _) => {
                WitnessDirection::residual(reason)
            }
            (_, _, _) => WitnessDirection::residual("requires_physical_source_realization"),
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Boolean,
            origin_layer_id: source.origin_layer_id().to_string(),
            qualifying: map(witness.qualifying()),
            rejected: map(witness.rejected()),
        });
    }
    for source in semantics.join_witnesses() {
        let translate = |direction: &JoinWitnessDirection| match direction {
            JoinWitnessDirection::Impossible => WitnessDirection::Impossible,
            JoinWitnessDirection::Residual { reason } => WitnessDirection::residual(reason),
            JoinWitnessDirection::Exact(cases) => {
                let (Some(left), Some(right), Some(op)) =
                    (source.left(), source.right(), source.comparison())
                else {
                    return WitnessDirection::residual("missing_join_endpoint");
                };
                let (Some(left_row), Some(right_row)) = (
                    RowVariable::new(left.relation(), left.relation_instance(), "left"),
                    RowVariable::new(right.relation(), right.relation_instance(), "right"),
                ) else {
                    return WitnessDirection::residual("invalid_join_relation_instance");
                };
                let mut results = Vec::new();
                for join_case in cases {
                    let obligation = match join_case.shape() {
                        JoinWitnessShape::Matched => WitnessObligation::JoinPair {
                            left_row: left_row.clone(),
                            right_row: right_row.clone(),
                            left: ColumnRef::new(
                                Some(left.relation().to_string()),
                                left.column().to_string(),
                            ),
                            right: ColumnRef::new(
                                Some(right.relation().to_string()),
                                right.column().to_string(),
                            ),
                            comparison: op,
                            null_extended: join_case.null_extended_side(),
                        },
                        JoinWitnessShape::LeftUnmatched => WitnessObligation::NoMatchingPartner {
                            candidate: left_row.clone(),
                            partner: right_row.clone(),
                            comparison: op,
                            left: ColumnRef::new(
                                Some(left.relation().to_string()),
                                left.column().to_string(),
                            ),
                            right: ColumnRef::new(
                                Some(right.relation().to_string()),
                                right.column().to_string(),
                            ),
                            closed_world: true,
                        },
                        JoinWitnessShape::RightUnmatched => WitnessObligation::NoMatchingPartner {
                            candidate: right_row.clone(),
                            partner: left_row.clone(),
                            comparison: op.reversed(),
                            left: ColumnRef::new(
                                Some(right.relation().to_string()),
                                right.column().to_string(),
                            ),
                            right: ColumnRef::new(
                                Some(left.relation().to_string()),
                                left.column().to_string(),
                            ),
                            closed_world: true,
                        },
                    };
                    let Some(case) = WitnessCase::new(vec![obligation], ProofStrength::Sufficient)
                    else {
                        return WitnessDirection::residual("invalid_join_case");
                    };
                    results.push(case);
                }
                WitnessDirection::feasible(results)
                    .unwrap_or_else(|| WitnessDirection::residual("empty_join_case"))
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Join,
            origin_layer_id: source.origin_layer_id().to_string(),
            qualifying: translate(source.qualifying()),
            rejected: translate(source.rejected()),
        });
    }
    for item in semantics.group_witnesses() {
        let witness = item.witness();
        let translate = |direction: &GroupWitnessDirection| -> WitnessDirection {
            match direction {
                GroupWitnessDirection::Residual { reason } => WitnessDirection::residual(reason),
                GroupWitnessDirection::Exact(cases) if cases.is_empty() => {
                    WitnessDirection::Impossible
                }
                GroupWitnessDirection::Exact(cases) => {
                    if item.boundary_kind() != GroupBoundaryKind::Physical {
                        return WitnessDirection::residual("requires_physical_source_realization");
                    }
                    let Some(boundary_name) = witness.boundary() else {
                        return WitnessDirection::residual("missing_group_boundary");
                    };
                    let Some(boundary) = WitnessBoundary::new(
                        boundary_name,
                        item.boundary_kind(),
                        item.origin_layer_id(),
                    ) else {
                        return WitnessDirection::residual("invalid_group_boundary");
                    };
                    let Some(aggregate) = witness.aggregate() else {
                        return WitnessDirection::residual("missing_group_aggregate");
                    };
                    let mut translated = Vec::new();
                    for case in cases {
                        let (Some(rows), Some(non_null)) = (
                            CountBounds::new(case.min_rows(), case.max_rows()),
                            CountBounds::new(case.min_non_null(), case.max_non_null()),
                        ) else {
                            return WitnessDirection::residual("invalid_group_cardinality");
                        };
                        if non_null.minimum() > rows.maximum().unwrap_or(u64::MAX) {
                            return WitnessDirection::residual("unsatisfiable_group_cardinality");
                        }
                        let Some(translated_case) = WitnessCase::new(
                            vec![WitnessObligation::Group {
                                boundary: boundary.clone(),
                                key: witness.group_keys().to_vec(),
                                aggregate,
                                distinct: witness.distinct(),
                                argument: witness.argument().cloned(),
                                rows,
                                non_null,
                                tests: case.tests().to_vec(),
                            }],
                            ProofStrength::Sufficient,
                        ) else {
                            return WitnessDirection::residual("invalid_group_case");
                        };
                        translated.push(translated_case);
                    }
                    WitnessDirection::feasible(translated)
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_group_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Group,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(witness.qualifying()),
            rejected: translate(witness.rejected()),
        });
    }
    for item in semantics.window_witnesses() {
        let witness = item.witness();
        let translate = |direction: &WindowWitnessDirection| -> WitnessDirection {
            match direction {
                WindowWitnessDirection::Impossible => WitnessDirection::Impossible,
                WindowWitnessDirection::Residual { reason } => WitnessDirection::residual(reason),
                WindowWitnessDirection::Exact(case) => {
                    if item.boundary_kind() != GroupBoundaryKind::Physical {
                        return WitnessDirection::residual("requires_physical_source_realization");
                    }
                    let Some(boundary_name) = witness.boundary() else {
                        return WitnessDirection::residual("missing_window_boundary");
                    };
                    let (Some(boundary), Some(candidate), Some(preceding)) = (
                        WitnessBoundary::new(
                            boundary_name,
                            item.boundary_kind(),
                            item.origin_layer_id(),
                        ),
                        RowVariable::new(boundary_name, boundary_name, "candidate"),
                        CountBounds::new(case.min_preceding(), case.max_preceding()),
                    ) else {
                        return WitnessDirection::residual("invalid_window_case");
                    };
                    let Some(translated) = WitnessCase::new(
                        vec![WitnessObligation::Ranked {
                            boundary,
                            candidate,
                            partition_by: witness.partition_by().to_vec(),
                            order_by: witness.order_by().to_vec(),
                            preceding,
                            strict_unique: true,
                            closed_world: true,
                        }],
                        ProofStrength::Sufficient,
                    ) else {
                        return WitnessDirection::residual("invalid_window_case");
                    };
                    WitnessDirection::feasible(vec![translated])
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_window_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Window,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(witness.qualifying()),
            rejected: translate(witness.rejected()),
        });
    }
    for item in semantics.subquery_witnesses() {
        let witness = item.witness();
        let translate = |direction: &SubqueryMembershipDirection| -> WitnessDirection {
            match direction {
                SubqueryMembershipDirection::Residual { reason } => {
                    WitnessDirection::residual(reason)
                }
                SubqueryMembershipDirection::Exact(cases) if cases.is_empty() => {
                    WitnessDirection::Impossible
                }
                SubqueryMembershipDirection::Exact(cases) => {
                    if item.boundary_kind() != GroupBoundaryKind::Physical {
                        return WitnessDirection::residual("requires_physical_source_realization");
                    }
                    let Some(inner_name) = witness.inner_relation() else {
                        return WitnessDirection::residual("missing_inner_source_identity");
                    };
                    let mut outer_relations = semantics
                        .dependencies()
                        .iter()
                        .filter(|relation| relation.as_str() != inner_name);
                    let (Some(outer_name), None) = (outer_relations.next(), outer_relations.next())
                    else {
                        return WitnessDirection::residual("unresolved_outer_source_identity");
                    };
                    let (Some(outer), Some(inner)) = (
                        RowVariable::new(outer_name, witness.outer_relation(), "candidate"),
                        WitnessBoundary::new(
                            inner_name,
                            item.boundary_kind(),
                            item.origin_layer_id(),
                        ),
                    ) else {
                        return WitnessDirection::residual("invalid_subquery_identity");
                    };
                    let mut translated = Vec::new();
                    for case in cases {
                        let Some(translated_case) = WitnessCase::new(
                            vec![WitnessObligation::Membership {
                                outer: outer.clone(),
                                inner: inner.clone(),
                                case: *case,
                                correlations: witness.correlations().to_vec(),
                                membership_key: witness.membership_key().cloned(),
                                inner_domains: witness.inner_column_domains().to_vec(),
                                closed_world: true,
                            }],
                            ProofStrength::Sufficient,
                        ) else {
                            return WitnessDirection::residual("invalid_membership_case");
                        };
                        translated.push(translated_case);
                    }
                    WitnessDirection::feasible(translated)
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_membership_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Subquery,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(witness.qualifying()),
            rejected: translate(witness.rejected()),
        });
    }
    for item in semantics.set_operations() {
        let operation = item.operation();
        let (qualifying, rejected) = operation.witness_directions();
        let translate = |direction: SetWitnessDirection| -> WitnessDirection {
            match direction {
                SetWitnessDirection::Residual { reason, .. } => WitnessDirection::residual(reason),
                SetWitnessDirection::Exact(cases) if cases.is_empty() => {
                    WitnessDirection::Impossible
                }
                SetWitnessDirection::Exact(cases) => {
                    let mut translated = Vec::new();
                    for case in cases {
                        let mut obligations = Vec::new();
                        for original in case.obligations() {
                            let source = original.boundary();
                            let kind = if source.is_intermediate() {
                                GroupBoundaryKind::Intermediate
                            } else {
                                GroupBoundaryKind::Physical
                            };
                            // Producer realization is a separate task. A local tuple
                            // count cannot be turned into a direct write at a CTE.
                            if kind != GroupBoundaryKind::Physical {
                                return WitnessDirection::residual(
                                    "requires_physical_source_realization",
                                );
                            }
                            let Some(boundary) = WitnessBoundary::new(
                                source.relation(),
                                kind,
                                item.origin_layer_id(),
                            ) else {
                                return WitnessDirection::residual("invalid_set_boundary");
                            };
                            let Some(branch) = operation
                                .branches()
                                .iter()
                                .find(|branch| branch.identity() == original.branch_identity())
                            else {
                                return WitnessDirection::residual("missing_set_branch_evidence");
                            };
                            obligations.push(WitnessObligation::SetTuple {
                                branch_identity: original.branch_identity().to_string(),
                                boundary,
                                tuple_columns: source.tuple_columns().to_vec(),
                                matching_rows: original.matching_tuple_count(),
                                column_domains: branch.column_domains().to_vec(),
                                closed_world: true,
                            });
                        }
                        obligations.push(WitnessObligation::SetResultTuple {
                            matching_rows: case.output_tuple_count(),
                            nulls_equal: true,
                        });
                        let Some(translated_case) =
                            WitnessCase::new(obligations, ProofStrength::Sufficient)
                        else {
                            return WitnessDirection::residual("invalid_set_case");
                        };
                        translated.push(translated_case);
                    }
                    WitnessDirection::feasible(translated)
                        .unwrap_or_else(|| WitnessDirection::residual("invalid_set_cases"))
                }
            }
        };
        proofs.push(ConstructiveWitness {
            operator: WitnessOperator::Set,
            origin_layer_id: item.origin_layer_id().to_string(),
            qualifying: translate(qualifying),
            rejected: translate(rejected),
        });
    }
    proofs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound(min: u64, max: Option<u64>) -> CountBounds {
        CountBounds::new(min, max).expect("valid test bounds")
    }

    fn counted(rows: u64) -> WitnessDirection {
        WitnessDirection::feasible(vec![WitnessCase::new(
            vec![WitnessObligation::OutputRows {
                layer_id: "terminal".to_owned(),
                bounds: bound(rows, Some(rows)),
            }],
            ProofStrength::Equivalent,
        )
        .expect("valid proof")])
        .expect("feasible")
    }

    #[test]
    fn conjunction_detects_shared_conflicting_counts_but_disjunction_preserves_cases() {
        let a = counted(1);
        let b = counted(2);
        assert_eq!(a.all(&b), WitnessDirection::Impossible);
        let WitnessDirection::Feasible(options) = a.any(&b) else {
            panic!("disjoint cases are alternatives");
        };
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].strength(), ProofStrength::Equivalent);
        assert_eq!(counted(1).all(&counted(1)), counted(1).all(&counted(1)));
    }

    #[test]
    fn distinct_sufficient_examples_are_not_a_joint_satisfiability_proof() {
        let sufficient = |count| WitnessDirection::feasible(vec![
            WitnessCase::new(
                vec![WitnessObligation::OutputRows {
                    layer_id: "terminal".to_string(),
                    bounds: bound(count, Some(count)),
                }],
                ProofStrength::Sufficient,
            ).expect("case")
        ]).expect("direction");
        assert!(matches!(
            sufficient(1).all(&sufficient(2)),
            WitnessDirection::Residual { .. }
        ));
        assert!(matches!(
            sufficient(1).all(&sufficient(1)),
            WitnessDirection::Feasible(_)
        ));
    }

    #[test]
    fn residual_is_not_impossible_and_sufficient_or_branch_survives_it() {
        let residual = WitnessDirection::Residual {
            reason: "unknown".into(),
        };
        assert_eq!(
            residual.all(&counted(1)),
            WitnessDirection::Residual {
                reason: "conjunctive_operand_not_proven".to_owned(),
            }
        );
        assert_eq!(residual.any(&counted(1)), counted(1));
        assert_eq!(
            residual.all(&WitnessDirection::Impossible),
            WitnessDirection::Impossible
        );
    }

    #[test]
    fn malformed_quantifiers_tuples_and_closed_world_absence_are_unrepresentable() {
        let boundary = WitnessBoundary::new("raw.orders", GroupBoundaryKind::Physical, "layer")
            .expect("boundary");
        let row = RowVariable::new("raw.orders", "o", "candidate").expect("row");
        let value = WitnessTerm::Integer(1);
        let tautology = WitnessFormula::IsNull {
            term: value.clone(),
            negated: true,
        };
        let case = |obligation| WitnessCase::new(vec![obligation], ProofStrength::Sufficient);
        assert!(case(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: bound(0, Some(1)),
            predicate: tautology.clone(),
            closed_world: false,
        })
        .is_none());
        assert!(case(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::Exists,
            bounds: bound(0, Some(1)),
            predicate: tautology.clone(),
            closed_world: true,
        })
        .is_none());
        assert!(case(WitnessObligation::Predicate(
            WitnessFormula::TupleComparison {
                left: vec![value.clone()],
                equal: true,
                right: vec![value.clone(), value.clone()],
            }
        ))
        .is_none());
        assert!(case(WitnessObligation::NoMatchingPartner {
            candidate: row.clone(),
            partner: row.clone(),
            comparison: ComparisonOperator::Eq,
            left: ColumnRef::new(Some("raw.orders".into()), "id".into()),
            right: ColumnRef::new(Some("raw.orders".into()), "id".into()),
            closed_world: false,
        })
        .is_none());
        assert!(case(WitnessObligation::Producer {
            boundary,
            physical_sources: vec!["raw.orders".into()],
        })
        .is_none());
    }

    #[test]
    fn sql_not_does_not_turn_unknown_into_false() {
        let w = ConstructiveWitness {
            operator: WitnessOperator::Boolean,
            origin_layer_id: "layer".into(),
            qualifying: counted(1),
            rejected: counted(2),
        };
        let nullable = w.logical_not(false);
        assert!(matches!(
            nullable.qualifying(),
            WitnessDirection::Residual { .. }
        ));
        assert_eq!(nullable.rejected(), w.qualifying());
        let two_valued = w.logical_not(true);
        assert_eq!(two_valued.qualifying(), w.rejected());
        assert_eq!(two_valued.rejected(), w.qualifying());
    }

    #[test]
    fn and_or_propagate_classifications_independently() {
        let w = ConstructiveWitness {
            operator: WitnessOperator::Boolean,
            origin_layer_id: "layer".into(),
            qualifying: counted(1),
            rejected: counted(2),
        };
        let both = w.logical_and(&w).expect("same layer");
        assert!(matches!(both.qualifying(), WitnessDirection::Feasible(_)));
        assert!(matches!(both.rejected(), WitnessDirection::Feasible(_)));
        let either = w.logical_or(&w).expect("same layer");
        assert_eq!(either.qualifying(), w.qualifying());
        assert_eq!(either.rejected(), w.rejected());
        let other = ConstructiveWitness {
            origin_layer_id: "different".into(),
            ..w.clone()
        };
        assert!(w.logical_or(&other).is_none());
    }

    #[test]
    fn invalid_bounds_and_empty_names_cannot_be_created() {
        assert!(CountBounds::new(3, Some(2)).is_none());
        assert!(RowVariable::new("orders", "", "candidate").is_none());
        assert!(WitnessBoundary::new("x", GroupBoundaryKind::Physical, "").is_none());
        assert!(WitnessCase::new(Vec::new(), ProofStrength::Sufficient).is_none());
        assert!(WitnessDirection::feasible(Vec::new()).is_none());
    }

    #[test]
    fn contradictory_output_and_state_counts_are_rejected() {
        assert!(WitnessCase::new(
            vec![
                WitnessObligation::OutputRows {
                    layer_id: "l".into(),
                    bounds: bound(3, Some(3))
                },
                WitnessObligation::OutputRows {
                    layer_id: "l".into(),
                    bounds: bound(0, Some(2))
                },
            ],
            ProofStrength::Sufficient
        )
        .is_none());
        assert!(WitnessCase::new(
            vec![
                WitnessObligation::StateRows {
                    relation: "t".into(),
                    before: bound(0, Some(0)),
                    after: bound(2, Some(3))
                },
                WitnessObligation::StateRows {
                    relation: "t".into(),
                    before: bound(0, Some(0)),
                    after: bound(4, Some(5))
                },
            ],
            ProofStrength::Sufficient
        )
        .is_none());
    }

    #[test]
    fn unknown_is_not_equivalent_to_boolean_false() {
        let row = RowVariable::new("orders", "o", "candidate").expect("valid row");
        let predicate = BooleanRowConstraint::NullTest {
            column: ColumnRef::new(Some("orders".to_string()), "amount".to_string()),
            negated: false,
        };
        let truth = |truth| {
            WitnessObligation::Predicate(WitnessFormula::RowTruth {
                row: row.clone(),
                predicate: predicate.clone(),
                truth,
            })
        };
        assert!(WitnessCase::new(
            vec![
                truth(BooleanTruthCase::True),
                truth(BooleanTruthCase::NotTrue),
            ],
            ProofStrength::Sufficient
        )
        .is_none());
    }

    #[test]
    fn necessity_does_not_claim_sufficient_constructibility() {
        let case = WitnessCase::new(
            vec![WitnessObligation::OutputRows {
                layer_id: "x".into(),
                bounds: bound(1, Some(1)),
            }],
            ProofStrength::Necessary,
        )
        .expect("valid necessary condition");
        assert!(WitnessDirection::feasible(vec![case]).is_none());
    }
}
