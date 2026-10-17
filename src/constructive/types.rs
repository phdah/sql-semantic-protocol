//! Canonical local source-row proof terms, count obligations and strengths.

use super::*;

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
    /// Typed SQL boolean constant.
    Boolean(bool),
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

/// Which complete physical input must be controlled to prove an absence.
///
/// Closing a candidate tuple class does not imply the entire relation is empty.
/// SQL set tuple equality treats NULLs as equal, unlike join predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedWorldCoverage {
    /// Enumerate every row in the entire physical source relation.
    EntireRelation,
    /// Enumerate all rows equal to one branch's candidate tuple.
    CandidateTuple {
        /// Originating set branch used to bind the positional tuple.
        branch_identity: String,
        /// Typed positional source columns determining tuple equivalence.
        columns: Vec<String>,
    },
}

/// Complete physical-row key assignment for one join. The same relation
/// appears just once in the source population, even when both aliases refer
/// to it. Values are assigned before either producer is materialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinPopulationPattern {
    /// Each row has the unique non-NULL integer key i in [0, n), with both
    /// inputs using the same sequence. An equijoin has exactly n matches.
    DistinctMatched,
    /// Every row on both sides has the same non-NULL integer key zero.
    /// A matching equijoin has left_rows * right_rows pairs, preserving bags.
    CommonMatched,
    /// Keys 0..groups occur exactly left_rows/groups and right_rows/groups
    /// times, respectively. SQL bags multiply those contributor counts.
    GroupedMatched { groups: u64 },
    /// Complete nonempty inputs with disjoint, non-NULL integer keys:
    /// every left key is 0 and every right key is 1. No TRUE ON pair exists.
    DisjointKeys,
    /// Two closed physical input bags each contain one matching key-zero row;
    /// their remaining rows have disjoint keys (left one, right two).
    /// Exactly one TRUE pair exists and each other physical row is unmatched.
    MixedMatchedUnmatched,
    /// One matching row on the left and two on the right both have key zero.
    /// Every remaining left row has key one and every remaining right row
    /// has key two. Exactly two TRUE pairs coexist with both unmatched sides.
    MixedDuplicateRight,
    /// One complete non-NULL physical source assignment: L keys [0,0,1,1],
    /// R keys [0,2]. Two TRUE join pairs select two left source rows by
    /// SEMI; the remaining left and right rows have no TRUE partners.
    /// NULL/UNKNOWN is independently certified by the DuckDB SQL fixture,
    /// not fabricated as an insertable physical key without metadata.
    MixedDuplicateLeft,
    /// Complete non-NULL physical assignment: L [0,0,1,1], R [2,2].
    /// No TRUE equality exists across the two closed input bags. Physical
    /// NULL insertability is not claimed; the separate SQL oracle checks
    /// UNKNOWN and anti membership for literal NULL source rows.
    DisjointFourTwo,
    /// The left population has no rows. The right may be nonempty.
    EmptyLeft,
    /// The right population has no rows. The left may be nonempty.
    EmptyRight,
}

impl JoinPopulationPattern {
    /// Stable canonical protocol identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DistinctMatched => "distinct_matched",
            Self::CommonMatched => "common_matched",
            Self::GroupedMatched { .. } => "grouped_matched",
            Self::DisjointKeys => "disjoint_keys",
            Self::MixedMatchedUnmatched => "mixed_matched_unmatched",
            Self::MixedDuplicateRight => "mixed_duplicate_right",
            Self::MixedDuplicateLeft => "mixed_duplicate_left",
            Self::DisjointFourTwo => "disjoint_four_two",
            Self::EmptyLeft => "empty_left",
            Self::EmptyRight => "empty_right",
        }
    }

    /// Distinct non-NULL join-key groups in a homogeneous grouped assignment.
    pub const fn group_count(self) -> Option<u64> {
        match self {
            Self::GroupedMatched { groups } => Some(groups),
            Self::DistinctMatched
            | Self::CommonMatched
            | Self::DisjointKeys
            | Self::MixedMatchedUnmatched
            | Self::MixedDuplicateRight
            | Self::MixedDuplicateLeft
            | Self::DisjointFourTwo
            | Self::EmptyLeft
            | Self::EmptyRight => None,
        }
    }

    /// Exact output rows under the SQL bag and outer/semijoin laws.
    /// A calculation returning None is unsupported, not evidence of zero.
    pub fn output_rows(
        self,
        kind: crate::protocol::JoinKind,
        left: u64,
        right: u64,
    ) -> Option<u64> {
        use crate::protocol::JoinKind;
        let pairs = match self {
            Self::DistinctMatched if left > 0 && left == right => left,
            Self::CommonMatched if left > 0 && right > 0 => left.checked_mul(right)?,
            Self::GroupedMatched { groups }
                if groups > 1
                    && left >= groups
                    && right >= groups
                    && left.is_multiple_of(groups)
                    && right.is_multiple_of(groups) =>
            {
                left.checked_div(groups)?.checked_mul(right)?
            }
            Self::DisjointKeys if left > 0 && right > 0 => 0,
            Self::MixedMatchedUnmatched if left >= 2 && right >= 2 => 1,
            Self::MixedDuplicateRight if left >= 2 && right >= 3 => 2,
            Self::MixedDuplicateLeft if left == 4 && right == 2 => 2,
            Self::DisjointFourTwo if left == 4 && right == 2 => 0,
            Self::EmptyLeft if left == 0 => 0,
            Self::EmptyRight if right == 0 => 0,
            _ => return None,
        };
        let (left_unmatched, right_unmatched) = match self {
            Self::MixedMatchedUnmatched => (left.checked_sub(1)?, right.checked_sub(1)?),
            Self::MixedDuplicateRight => (left.checked_sub(1)?, right.checked_sub(2)?),
            Self::MixedDuplicateLeft => (2, 1),
            _ if pairs == 0 => (left, right),
            _ => (0, 0),
        };
        match kind {
            JoinKind::Inner => Some(pairs),
            JoinKind::Left => pairs.checked_add(left_unmatched),
            JoinKind::Right => pairs.checked_add(right_unmatched),
            JoinKind::Full => pairs
                .checked_add(left_unmatched)?
                .checked_add(right_unmatched),
            // Partial populations have exactly one matched source row per
            // side. Semi joins select source rows, not pair multiplicities.
            JoinKind::LeftSemi => Some(match self {
                Self::MixedMatchedUnmatched | Self::MixedDuplicateRight => 1,
                Self::MixedDuplicateLeft => 2,
                _ if pairs > 0 => left,
                _ => 0,
            }),
            JoinKind::RightSemi => Some(match self {
                Self::MixedMatchedUnmatched => 1,
                Self::MixedDuplicateRight => 2,
                Self::MixedDuplicateLeft => 1,
                _ if pairs > 0 => right,
                _ => 0,
            }),
            JoinKind::LeftAnti => Some(left_unmatched),
            JoinKind::RightAnti => Some(right_unmatched),
            JoinKind::Cross | JoinKind::Unknown => None,
        }
    }
}

/// Closed-world two-source population whose ON predicate is uniform over
/// every physical pair. Non-NULL signed integer assignments are chosen
/// explicitly, so NULL-safe equality is never confused with ordinary SQL =.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformJoinLaw {
    /// CROSS JOIN has TRUE for every pair without a join key.
    Cartesian,
    /// Strict less-than of signed integer join columns.
    LessThan,
    /// Strict greater-than of signed integer join columns.
    GreaterThan,
    /// Inclusive less-than-or-equal of signed integer join columns.
    LessThanOrEqual,
    /// Inclusive greater-than-or-equal of signed integer join columns.
    GreaterThanOrEqual,
    /// SQL IS NOT DISTINCT FROM over non-NULL signed integer columns.
    NullSafeEquality,
    /// Equality disjoined with left-key IS NULL, evaluated with non-NULL keys.
    EqualityOrLeftNull,
}

impl UniformJoinLaw {
    /// Stable name for the public population contract.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cartesian => "cartesian",
            Self::LessThan => "less_than",
            Self::GreaterThan => "greater_than",
            Self::LessThanOrEqual => "less_than_or_equal",
            Self::GreaterThanOrEqual => "greater_than_or_equal",
            Self::NullSafeEquality => "null_safe_equality",
            Self::EqualityOrLeftNull => "equality_or_left_null",
        }
    }

    /// TRUE for every source pair under the complete constant-key assignment.
    /// Returns None for an ill-formed Cartesian/key combination.
    pub const fn pair_matches(self, left: Option<i64>, right: Option<i64>) -> Option<bool> {
        match (self, left, right) {
            (Self::Cartesian, None, None) => Some(true),
            (Self::LessThan, Some(left), Some(right)) => Some(left < right),
            (Self::GreaterThan, Some(left), Some(right)) => Some(left > right),
            (Self::LessThanOrEqual, Some(left), Some(right)) => Some(left <= right),
            (Self::GreaterThanOrEqual, Some(left), Some(right)) => Some(left >= right),
            (Self::NullSafeEquality | Self::EqualityOrLeftNull, Some(left), Some(right)) => {
                Some(left == right)
            }
            _ => None,
        }
    }
}

/// A complete, ordered pair-selection law. Indices identify individual
/// physical source rows, not just values or projected SQL columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiniteJoinLaw {
    /// For each left value choose the unique maximum non-NULL right value <= it.
    /// Ties are not certified without explicit engine ordering evidence.
    AsOfNearestPriorInclusive,
    /// Evaluate equality separately for each left row and emit all TRUE partners.
    CorrelatedInnerEquality,
    /// Evaluate equality per left row, null-extending only absent right bags.
    CorrelatedOuterEquality,
}

impl FiniteJoinLaw {
    /// Stable machine-readable proof-law name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AsOfNearestPriorInclusive => "asof_nearest_prior_inclusive",
            Self::CorrelatedInnerEquality => "correlated_inner_equality",
            Self::CorrelatedOuterEquality => "correlated_outer_equality",
        }
    }

    /// Compute the entire selected output bag with physical row identities.
    /// A missing right index represents a synthetic NULL projection, not a
    /// source row. Return None for a noncertifiable ASOF tie.
    pub fn selected_rows(
        self,
        left: &[Option<i64>],
        right: &[Option<i64>],
    ) -> Option<Vec<(usize, Option<usize>)>> {
        let mut selected = Vec::new();
        for (left_index, left_value) in left.iter().enumerate() {
            let mut matches = Vec::new();
            for (right_index, right_value) in right.iter().enumerate() {
                let matched = match (left_value, right_value) {
                    (Some(left), Some(right)) => match self {
                        Self::AsOfNearestPriorInclusive => right <= left,
                        Self::CorrelatedInnerEquality | Self::CorrelatedOuterEquality => left == right,
                    },
                    _ => false,
                };
                if matched {
                    matches.push(right_index);
                }
            }
            match self {
                Self::AsOfNearestPriorInclusive => {
                    let nearest = matches
                        .iter()
                        .copied()
                        .max_by_key(|index| right[*index]);
                    if let Some(nearest) = nearest {
                        if matches.iter().filter(|index| right[**index] == right[nearest]).count() != 1 {
                            return None;
                        }
                        selected.push((left_index, Some(nearest)));
                    } else {
                        selected.push((left_index, None));
                    }
                }
                Self::CorrelatedInnerEquality => {
                    selected.extend(matches.into_iter().map(|index| (left_index, Some(index))));
                }
                Self::CorrelatedOuterEquality => {
                    if matches.is_empty() {
                        selected.push((left_index, None));
                    } else {
                        selected.extend(matches.into_iter().map(|index| (left_index, Some(index))));
                    }
                }
            }
        }
        Some(selected)
    }

    /// All real physical source pairs which are not selected, including ASOF
    /// eligible-but-nonnearest predecessors and SQL UNKNOWN NULL pairs.
    pub fn unselected_pairs(
        self,
        left: &[Option<i64>],
        right: &[Option<i64>],
    ) -> Option<Vec<(usize, usize)>> {
        let selected = self.selected_rows(left, right)?;
        Some((0..left.len())
            .flat_map(|l| (0..right.len()).map(move |r| (l, r)))
            .filter(|(l, r)| !selected.contains(&(*l, Some(*r))))
            .collect())
    }
}

/// A second or later equality in a complete composite join-key assignment.
/// Both sides are physical source columns controlled by the same key-group values
/// as the primary equality. This is a complete bag law, not a sampled pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinKeyEquality {
    left: ColumnRef,
    right: ColumnRef,
}

impl JoinKeyEquality {
    /// Construct a key component already resolved to physical columns.
    pub fn new(left: ColumnRef, right: ColumnRef) -> Self {
        Self { left, right }
    }

    /// Column in the left physical source instance.
    pub fn left(&self) -> &ColumnRef {
        &self.left
    }

    /// Column in the right physical source instance.
    pub fn right(&self) -> &ColumnRef {
        &self.right
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
        /// Side receiving NULL values when the unmatched candidate is preserved.
        null_extended: Option<JoinSide>,
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
    /// Fully specified equality-key and join-bag population through
    /// physical source leaves, including SQL NULL-extension and no-partner
    /// consequences. Every involved physical relation must also be closed.
    JoinPopulation {
        left_row: RowVariable,
        right_row: RowVariable,
        left_key: ColumnRef,
        right_key: ColumnRef,
        /// Additional key components sharing the same complete key-group assignment.
        /// Each pair is equal on matched rows, with ordinary SQL NULL not matching.
        additional_keys: Vec<JoinKeyEquality>,
        join_kind: crate::protocol::JoinKind,
        pattern: JoinPopulationPattern,
        left_rows: u64,
        right_rows: u64,
        output_rows: u64,
        closed_world: bool,
    },
    /// Every pair of two complete physical bags has one proved SQL truth
    /// outcome under an admitted portable Boolean predicate. Source rows
    /// share the indicated typed constant key in each bag, or have no key
    /// for CROSS. The relationship is verified before case construction.
    UniformJoinPopulation {
        left_row: RowVariable,
        right_row: RowVariable,
        left_key: Option<ColumnRef>,
        right_key: Option<ColumnRef>,
        law: UniformJoinLaw,
        left_value: Option<i64>,
        right_value: Option<i64>,
        left_rows: u64,
        right_rows: u64,
        output_rows: u64,
        closed_world: bool,
    },
    /// Every per-row selection and NOT TRUE complement is fixed by a complete
    /// typed two-source value bag. Source row indices are stable identities;
    /// an unmatched output has no physical right-row index.
    FiniteJoinPopulation {
        left_row: RowVariable,
        right_row: RowVariable,
        left_key: ColumnRef,
        right_key: ColumnRef,
        law: FiniteJoinLaw,
        left_values: Vec<Option<i64>>,
        right_values: Vec<Option<i64>>,
        output_rows: u64,
        closed_world: bool,
    },
    /// A three-instance INNER chain, whose two comparisons are simultaneously
    /// realized by assigning one common, non-NULL integer key value to every
    /// row in each *unique* controlled physical bag. Aliases can share a bag.
    /// Counts multiply only because every row passes both ON equalities.
    JoinChainPopulation {
        left_row: RowVariable,
        middle_row: RowVariable,
        right_row: RowVariable,
        left_key: ColumnRef,
        middle_left_key: ColumnRef,
        middle_right_key: ColumnRef,
        right_key: ColumnRef,
        matched_value: i64,
        left_rows: u64,
        middle_rows: u64,
        right_rows: u64,
        output_rows: u64,
        closed_world: bool,
    },
    /// A complete operator-specific population at controlled physical leaves.
    /// Its typed witness carries every required key, multiplicity and NULL-aware
    /// branch count; the surrounding case must also close each leaf relation.
    OperatorPopulation {
        /// Canonical physical operator construction, never an intermediate write.
        witness: crate::outcome_proofs::OutcomeWitness,
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
    /// A complete row-number population at one physical input. Partition
    /// values are the integers 0..partitions, repeated rows_per_partition
    /// times each; order values are 0..rows_per_partition within each
    /// partition, so ties cannot affect ROW_NUMBER/QUALIFY. The exact
    /// surviving output cardinality is partitions * min(rows_per_partition,
    /// rank_limit). A local ROW_NUMBER witness must justify this rule.
    RankPopulation {
        boundary: WitnessBoundary,
        partition_key: ColumnRef,
        order: WindowOrderKey,
        partitions: u64,
        rows_per_partition: u64,
        rank_limit: u64,
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
    /// Exact negative evidence requires a complete physical closed-world scope.
    ClosedWorld {
        /// The actual writable physical source whose rows are controlled.
        boundary: WitnessBoundary,
        /// Entire relation or one complete candidate-tuple equivalence class.
        coverage: ClosedWorldCoverage,
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
