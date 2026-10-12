//! DML write operations, mutation states, and assignments.

use super::*;

/// How a transformation changes its named target relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteKind {
    /// The transformation defines the complete relation contents represented by its query.
    Definition,
    /// The transformation appends rows without replacing pre-existing relation contents.
    Append,
    /// The transformation conditionally updates, inserts, or deletes existing relation rows.
    ConditionalMutation,
    /// UPDATE changes qualifying existing rows while preserving nonqualifying rows.
    Update,
    /// DELETE removes qualifying existing rows.
    Delete,
}

impl WriteKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Definition => "definition",
            Self::Append => "append",
            Self::ConditionalMutation => "conditional_mutation",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    /// Return whether this write fully defines the resulting relation.
    pub fn fully_defines_relation(self) -> bool {
        matches!(self, Self::Definition)
    }
}

/// Parser-independent semantics for a write into a named relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOperation {
    target: String,
    kind: WriteKind,
    target_columns: Vec<String>,
    match_condition: Option<Predicate>,
    merge_clauses: Vec<MergeClause>,
    selection: Option<Predicate>,
    assignments: Vec<MergeAssignment>,
    selection_domains: Vec<ColumnDomain>,
}

impl WriteOperation {
    pub(crate) fn definition(target: String) -> Self {
        Self {
            target,
            kind: WriteKind::Definition,
            target_columns: Vec::new(),
            match_condition: None,
            merge_clauses: Vec::new(),
            selection: None,
            assignments: Vec::new(),
            selection_domains: Vec::new(),
        }
    }

    pub(crate) fn append(target: String, target_columns: Vec<String>) -> Self {
        Self {
            target,
            kind: WriteKind::Append,
            target_columns,
            match_condition: None,
            merge_clauses: Vec::new(),
            selection: None,
            assignments: Vec::new(),
            selection_domains: Vec::new(),
        }
    }

    pub(crate) fn conditional_mutation(
        target: String,
        match_condition: Predicate,
        merge_clauses: Vec<MergeClause>,
    ) -> Self {
        Self {
            target,
            kind: WriteKind::ConditionalMutation,
            target_columns: Vec::new(),
            match_condition: Some(match_condition),
            merge_clauses,
            selection: None,
            assignments: Vec::new(),
            selection_domains: Vec::new(),
        }
    }

    pub(crate) fn update(
        target: String,
        selection: Option<Predicate>,
        assignments: Vec<MergeAssignment>,
        selection_domains: Vec<ColumnDomain>,
    ) -> Self {
        Self {
            target,
            kind: WriteKind::Update,
            target_columns: Vec::new(),
            match_condition: None,
            merge_clauses: Vec::new(),
            selection,
            assignments,
            selection_domains,
        }
    }

    pub(crate) fn delete(
        target: String,
        selection: Option<Predicate>,
        selection_domains: Vec<ColumnDomain>,
    ) -> Self {
        Self {
            target,
            kind: WriteKind::Delete,
            target_columns: Vec::new(),
            match_condition: None,
            merge_clauses: Vec::new(),
            selection,
            assignments: Vec::new(),
            selection_domains,
        }
    }

    /// Return the relation written by the transformation.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Return how the write changes the target relation.
    pub fn kind(&self) -> WriteKind {
        self.kind
    }

    /// Return explicit INSERT target columns in SQL order.
    pub fn target_columns(&self) -> &[String] {
        &self.target_columns
    }

    /// Return the MERGE match condition for conditional mutations.
    pub fn match_condition(&self) -> Option<&Predicate> {
        self.match_condition.as_ref()
    }

    /// Return normalized MERGE clauses in SQL order.
    pub fn merge_clauses(&self) -> &[MergeClause] {
        &self.merge_clauses
    }

    /// Return the standalone UPDATE/DELETE WHERE condition, if present.
    pub fn selection(&self) -> Option<&Predicate> {
        self.selection.as_ref()
    }

    /// Return standalone UPDATE assignments in SQL order.
    pub fn assignments(&self) -> &[MergeAssignment] {
        &self.assignments
    }

    /// Necessary domains on rows selected by standalone UPDATE/DELETE.
    pub fn selection_domains(&self) -> &[ColumnDomain] {
        &self.selection_domains
    }

    /// Derive conservative before/after effects without assuming target data exists.
    ///
    /// A caller must provide an initial target snapshot and verify source multiplicities,
    /// key constraints, and SQL three-valued predicate evaluation. No sampled rows are
    /// invented. The unconditional DELETE case alone proves an empty final target and
    /// idempotence regardless of the initial rows.
    pub fn state_effect(&self) -> Option<WriteStateEffect> {
        let initial = WriteInitialState::CallerSupplied;
        let unbounded = WriteAffectedRows {
            minimum: 0,
            maximum: None,
        };
        let mut reasons = vec![WriteUncertainty::AffectedRowsUnknown];
        let (branches, post_state, idempotence) = match self.kind {
            WriteKind::Definition => return None,
            WriteKind::Append => {
                if self.target_columns.is_empty() {
                    reasons.push(WriteUncertainty::ImplicitTargetColumns);
                }
                reasons.push(WriteUncertainty::ConstraintConflictsUnverified);
                (
                    vec![WriteEffectBranch {
                        match_kind: None,
                        predicate: None,
                        domains: Vec::new(),
                        action: WriteEffectAction::InsertQuery,
                    }],
                    WritePostState::ApplyToInitial,
                    WriteIdempotence::Unproven,
                )
            }
            WriteKind::Update => {
                if self.selection.is_some() {
                    reasons.push(WriteUncertainty::PredicateExactnessUnverified);
                }
                reasons.push(WriteUncertainty::ConstraintConflictsUnverified);
                (
                    vec![WriteEffectBranch {
                        match_kind: None,
                        predicate: self.selection.clone(),
                        domains: self.selection_domains.clone(),
                        action: WriteEffectAction::Mutation(MergeAction::Update {
                            assignments: self.assignments.clone(),
                        }),
                    }],
                    WritePostState::ApplyToInitial,
                    WriteIdempotence::Unproven,
                )
            }
            WriteKind::Delete => {
                let unconditional = self.selection.is_none();
                if !unconditional {
                    reasons.push(WriteUncertainty::PredicateExactnessUnverified);
                }
                (
                    vec![WriteEffectBranch {
                        match_kind: None,
                        predicate: self.selection.clone(),
                        domains: self.selection_domains.clone(),
                        action: WriteEffectAction::Mutation(MergeAction::Delete),
                    }],
                    if unconditional {
                        WritePostState::Empty
                    } else {
                        WritePostState::ApplyToInitial
                    },
                    if unconditional {
                        WriteIdempotence::Proven
                    } else {
                        WriteIdempotence::Unproven
                    },
                )
            }
            WriteKind::ConditionalMutation => {
                if self
                    .merge_clauses
                    .iter()
                    .any(|clause| matches!(clause.action(), MergeAction::Unsupported(_)))
                {
                    reasons.push(WriteUncertainty::UnsupportedAction);
                }
                reasons.push(WriteUncertainty::PredicateExactnessUnverified);
                reasons.push(WriteUncertainty::MatchMultiplicityUnknown);
                reasons.push(WriteUncertainty::ConstraintConflictsUnverified);
                (
                    self.merge_clauses
                        .iter()
                        .map(|clause| WriteEffectBranch {
                            match_kind: Some(clause.match_kind()),
                            predicate: clause.predicate().cloned(),
                            domains: clause.domains().to_vec(),
                            action: WriteEffectAction::Mutation(clause.action().clone()),
                        })
                        .collect(),
                    WritePostState::ApplyToInitial,
                    WriteIdempotence::Unproven,
                )
            }
        };
        Some(WriteStateEffect {
            initial,
            affected_rows: unbounded,
            post_state,
            idempotence,
            branches,
            reasons,
            target_columns: self.target_columns.clone(),
            match_condition: self.match_condition.clone(),
            cardinality_rule: match self.kind {
                WriteKind::Append => WriteCardinalityRule::Append,
                WriteKind::Update => WriteCardinalityRule::Preserve,
                WriteKind::Delete => WriteCardinalityRule::SubtractDeletes,
                WriteKind::ConditionalMutation => WriteCardinalityRule::Merge,
                WriteKind::Definition => return None,
            },
        })
    }
}

/// Required initial state before applying a partial write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteInitialState {
    /// A complete caller-provided target snapshot, never an inferred empty table.
    CallerSupplied,
}

/// What can be asserted about the final target without supplying initial rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePostState {
    /// Unconditional DELETE guarantees an empty target.
    Empty,
    /// Apply the ordered write effects to the actual initial snapshot.
    ApplyToInitial,
}

/// Whether repeating the statement is proved to leave the same final state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteIdempotence {
    /// SQL semantics prove idempotence independent of initial row values.
    Proven,
    /// No idempotence proof is available.
    Unproven,
}

/// Conservative interval for affected target rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteAffectedRows {
    minimum: u64,
    maximum: Option<u64>,
}

impl WriteAffectedRows {
    /// Inclusive minimum affected row count.
    pub fn minimum(&self) -> u64 {
        self.minimum
    }
    /// Inclusive maximum, or None when no bound is provable.
    pub fn maximum(&self) -> Option<u64> {
        self.maximum
    }
}

/// A residual obligation before a generator can verify the write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteUncertainty {
    /// Affected cardinality depends on actual target and source rows.
    AffectedRowsUnknown,
    /// Schema constraints and conflicts have not been checked against the actual target snapshot.
    ConstraintConflictsUnverified,
    /// Multiple source matches and clause overlap require execution-time verification.
    MatchMultiplicityUnknown,
    /// Predicate truth under NULL and dialect comparison rules is not proved exact.
    PredicateExactnessUnverified,
    /// An unsupported MERGE branch cannot be constructed or verified.
    UnsupportedAction,
    /// Implicit INSERT target column order requires target-schema evidence.
    ImplicitTargetColumns,
}

impl WriteUncertainty {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AffectedRowsUnknown => "affected_rows_unknown",
            Self::ConstraintConflictsUnverified => "constraint_conflicts_unverified",
            Self::MatchMultiplicityUnknown => "match_multiplicity_unknown",
            Self::PredicateExactnessUnverified => "predicate_exactness_unverified",
            Self::UnsupportedAction => "unsupported_action",
            Self::ImplicitTargetColumns => "implicit_target_columns",
        }
    }
}

/// A mutation branch with explicit SQL match and predicate context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEffectBranch {
    match_kind: Option<MergeMatchKind>,
    predicate: Option<Predicate>,
    domains: Vec<ColumnDomain>,
    action: WriteEffectAction,
}

impl WriteEffectBranch {
    /// MERGE matched/unmatched classification; None for standalone DML.
    pub fn match_kind(&self) -> Option<MergeMatchKind> {
        self.match_kind
    }
    /// Additional branch/WHERE predicate; only SQL TRUE selects rows.
    pub fn predicate(&self) -> Option<&Predicate> {
        self.predicate.as_ref()
    }
    /// Necessary (not sufficient) predicate domains for the branch.
    pub fn domains(&self) -> &[ColumnDomain] {
        &self.domains
    }

    /// Written action, with normalized expressions and conservative domains.
    pub fn action(&self) -> &WriteEffectAction {
        &self.action
    }
}

/// Source-backed INSERT or normalized row mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteEffectAction {
    /// INSERT SELECT takes its inserted rows from the owning query output.
    InsertQuery,
    /// An UPDATE, DELETE, or MERGE action, including explicit unsupported actions.
    Mutation(MergeAction),
}

/// Algebraic relation between target row counts, conditional on legal, executed DML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteCardinalityRule {
    /// Post = pre + inserted (INSERT SELECT).
    Append,
    /// Post = pre (UPDATE, including predicates rejecting all rows).
    Preserve,
    /// Post = pre - deleted (DELETE).
    SubtractDeletes,
    /// Post = pre + inserted - deleted (MERGE).
    Merge,
}

impl WriteCardinalityRule {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Append => "initial_plus_inserted",
            Self::Preserve => "initial",
            Self::SubtractDeletes => "initial_minus_deleted",
            Self::Merge => "initial_plus_inserted_minus_deleted",
        }
    }
}

/// Logical counts of actual inserted, updated and deleted target rows.
///
/// These must be verified against execution, rather than inferred from SQL
/// source-row counts: MERGE source rows may not match 1:1, and conflicting
/// writes may fail without producing any final state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteRowCounts {
    inserted: u64,
    updated: u64,
    deleted: u64,
}

impl WriteRowCounts {
    /// Record verified logical target-row counts.
    pub fn new(inserted: u64, updated: u64, deleted: u64) -> Self {
        Self {
            inserted,
            updated,
            deleted,
        }
    }

    /// Rows appended to the target.
    pub fn inserted(self) -> u64 {
        self.inserted
    }
    /// Existing rows updated, not newly inserted.
    pub fn updated(self) -> u64 {
        self.updated
    }
    /// Existing rows removed.
    pub fn deleted(self) -> u64 {
        self.deleted
    }
}

/// Why verified DML counts cannot satisfy the stated target-state contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteCountError {
    /// Count of action kinds inconsistent with SQL mutation class.
    InvalidActionCounts,
    /// UPDATE/DELETE cannot affect more distinct target rows than existed.
    ExceedsInitialRows,
    /// A DELETE without WHERE must remove every pre-existing row.
    UnconditionalDeleteMismatch,
    /// The final row count cannot fit in an unsigned 64-bit integer.
    Overflow,
}

impl std::fmt::Display for WriteCountError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::InvalidActionCounts => "observed action counts contradict the DML kind",
            Self::ExceedsInitialRows => "affected existing rows exceed the initial target size",
            Self::UnconditionalDeleteMismatch => {
                "unconditional DELETE must remove every initial row"
            }
            Self::Overflow => "computed final row count exceeds u64",
        };
        formatter.write_str(reason)
    }
}

impl std::error::Error for WriteCountError {}

/// A verifiable DML effect, not an assertion that the whole target is generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteStateEffect {
    initial: WriteInitialState,
    target_columns: Vec<String>,
    match_condition: Option<Predicate>,
    affected_rows: WriteAffectedRows,
    post_state: WritePostState,
    idempotence: WriteIdempotence,
    branches: Vec<WriteEffectBranch>,
    reasons: Vec<WriteUncertainty>,
    cardinality_rule: WriteCardinalityRule,
}

impl WriteStateEffect {
    /// Ordered target columns for an INSERT SELECT result, or empty for other writes.
    pub fn target_columns(&self) -> &[String] {
        &self.target_columns
    }
    /// MERGE ON condition (None for non-MERGE writes).
    pub fn match_condition(&self) -> Option<&Predicate> {
        self.match_condition.as_ref()
    }

    /// Initial target state precondition.
    pub fn initial(&self) -> WriteInitialState {
        self.initial
    }
    /// Affected row count bounds.
    pub fn affected_rows(&self) -> &WriteAffectedRows {
        &self.affected_rows
    }
    /// Proven final state, or a delta requiring the initial snapshot.
    pub fn post_state(&self) -> WritePostState {
        self.post_state
    }
    /// Proven or unproven idempotence.
    pub fn idempotence(&self) -> WriteIdempotence {
        self.idempotence
    }
    /// Ordered operation branches.
    pub fn branches(&self) -> &[WriteEffectBranch] {
        &self.branches
    }
    /// Explicit outstanding verification obligations.
    pub fn reasons(&self) -> &[WriteUncertainty] {
        &self.reasons
    }

    /// Conditional row-count conservation rule, independent of action feasibility.
    pub fn cardinality_rule(&self) -> WriteCardinalityRule {
        self.cardinality_rule
    }

    /// Check target-row conservation against caller-verified action counts.
    ///
    /// A successful result establishes only the required final row *count* if
    /// SQL execution successfully performed these actions. It does not verify
    /// branch predicates, key conflicts, exact row values, or transaction success.
    /// In particular it is unsafe to supply source-row counts as MERGE updates.
    pub fn resulting_rows(
        &self,
        initial: u64,
        counts: WriteRowCounts,
    ) -> Result<u64, WriteCountError> {
        let (inserted, updated, deleted) = (counts.inserted(), counts.updated(), counts.deleted());
        let target_changed = updated
            .checked_add(deleted)
            .ok_or(WriteCountError::Overflow)?;
        if target_changed > initial {
            return Err(WriteCountError::ExceedsInitialRows);
        }
        match self.cardinality_rule {
            WriteCardinalityRule::Append if updated != 0 || deleted != 0 => {
                return Err(WriteCountError::InvalidActionCounts);
            }
            WriteCardinalityRule::Preserve if inserted != 0 || deleted != 0 => {
                return Err(WriteCountError::InvalidActionCounts);
            }
            WriteCardinalityRule::SubtractDeletes if inserted != 0 || updated != 0 => {
                return Err(WriteCountError::InvalidActionCounts);
            }
            WriteCardinalityRule::Append
            | WriteCardinalityRule::Preserve
            | WriteCardinalityRule::SubtractDeletes
            | WriteCardinalityRule::Merge => {}
        }
        if self.post_state == WritePostState::Empty && deleted != initial {
            return Err(WriteCountError::UnconditionalDeleteMismatch);
        }
        initial
            .checked_sub(deleted)
            .and_then(|remaining| remaining.checked_add(inserted))
            .ok_or(WriteCountError::Overflow)
    }
}

/// MERGE clause match category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMatchKind {
    /// WHEN MATCHED.
    Matched,
    /// WHEN NOT MATCHED.
    NotMatched,
    /// WHEN NOT MATCHED BY TARGET.
    NotMatchedByTarget,
    /// WHEN NOT MATCHED BY SOURCE.
    NotMatchedBySource,
}

impl MergeMatchKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::NotMatched => "not_matched",
            Self::NotMatchedByTarget => "not_matched_by_target",
            Self::NotMatchedBySource => "not_matched_by_source",
        }
    }
}

/// One normalized MERGE clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeClause {
    match_kind: MergeMatchKind,
    predicate: Option<Predicate>,
    domains: Vec<ColumnDomain>,
    action: MergeAction,
}

impl MergeClause {
    pub(crate) fn new(
        match_kind: MergeMatchKind,
        predicate: Option<Predicate>,
        domains: Vec<ColumnDomain>,
        action: MergeAction,
    ) -> Self {
        Self {
            match_kind,
            predicate,
            domains,
            action,
        }
    }

    /// Return which MERGE rows this clause can match.
    pub fn match_kind(&self) -> MergeMatchKind {
        self.match_kind
    }

    /// Return the optional additional clause predicate.
    pub fn predicate(&self) -> Option<&Predicate> {
        self.predicate.as_ref()
    }

    /// Necessary source-column domains for this branch (not match proof).
    pub fn domains(&self) -> &[ColumnDomain] {
        &self.domains
    }

    /// Return the action executed by the clause.
    pub fn action(&self) -> &MergeAction {
        &self.action
    }
}

/// One normalized MERGE action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeAction {
    /// INSERT target columns and value rows.
    Insert {
        /// Explicit target columns in SQL order.
        columns: Vec<String>,
        /// Inserted value rows with normalized expressions and conservative outcome domains.
        values: Vec<Vec<WriteValue>>,
    },
    /// UPDATE assignments.
    Update {
        /// Assignments in SQL order.
        assignments: Vec<MergeAssignment>,
    },
    /// DELETE matching rows.
    Delete,
    /// The parser recognized an action form that cannot be represented safely.
    Unsupported(UnsupportedSemantic),
}

/// One value written by a DML action together with its conservative outcome domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteValue {
    expression: Expression,
    domain: ValueDomain,
}

impl WriteValue {
    pub(crate) fn new(expression: Expression, domain: ValueDomain) -> Self {
        Self { expression, domain }
    }

    /// Return the normalized expression that produces the written value.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the strongest safely derivable value domain for the written value.
    pub fn domain(&self) -> &ValueDomain {
        &self.domain
    }
}

/// One normalized MERGE UPDATE assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeAssignment {
    target: String,
    value: WriteValue,
}

impl MergeAssignment {
    pub(crate) fn new(target: String, value: WriteValue) -> Self {
        Self { target, value }
    }

    /// Return the target column or tuple representation.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Return the value expression and conservative domain assigned to the target.
    pub fn value(&self) -> &WriteValue {
        &self.value
    }
}
