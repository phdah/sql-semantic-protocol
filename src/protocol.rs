//! Parser-independent SQL Semantic Protocol domain values.
//!
//! The Rust model intentionally contains no sqlparser AST types. Unknown and unsupported
//! semantics remain explicit so consumers can distinguish incomplete analysis from known values.

use std::collections::BTreeSet;

use crate::constraints::RelationConstraintSet;
use crate::group_witness::GroupWitness;
use crate::window_witness::WindowWitness;

/// Current protocol version emitted by this crate.
pub const PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Root SQL Semantic Protocol document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protocol {
    protocol_version: &'static str,
    source: ProtocolSource,
    statements: Vec<ProtocolStatement>,
    relation_constraints: Vec<RelationConstraintSet>,
    comparison_declarations: Vec<ComparisonAssumption>,
}

impl Protocol {
    pub(crate) fn new(dialect: String, statements: Vec<ProtocolStatement>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            source: ProtocolSource { dialect },
            statements,
            relation_constraints: Vec::new(),
            comparison_declarations: Vec::new(),
        }
    }

    /// Declare warehouse comparison settings for single-input analysis.
    ///
    /// Callers must attest only settings known to hold for the target warehouse.
    pub fn declare_comparison_assumptions(&mut self, declared: &[ComparisonAssumption]) {
        self.comparison_declarations
            .extend(declared.iter().copied());
        self.comparison_declarations.sort();
        self.comparison_declarations.dedup();
        for statement in &mut self.statements {
            if let ProtocolStatement::Query(query) = statement {
                query.declare_comparison_assumptions(&self.comparison_declarations);
            }
        }
    }

    /// Return caller-declared comparison settings.
    pub fn comparison_declarations(&self) -> &[ComparisonAssumption] {
        &self.comparison_declarations
    }

    pub(crate) fn with_relation_constraints(
        mut self,
        relation_constraints: Vec<RelationConstraintSet>,
    ) -> Self {
        self.relation_constraints = relation_constraints;
        self
    }

    /// Return the protocol contract version.
    pub fn protocol_version(&self) -> &str {
        self.protocol_version
    }

    /// Return metadata describing the analyzed SQL source.
    pub fn source(&self) -> &ProtocolSource {
        &self.source
    }

    /// Return statements in original SQL statement order.
    pub fn statements(&self) -> &[ProtocolStatement] {
        &self.statements
    }

    /// Return canonical relation constraints discovered from the analyzed SQL.
    pub fn relation_constraints(&self) -> &[RelationConstraintSet] {
        &self.relation_constraints
    }
}

/// Metadata about the SQL source that produced a protocol document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSource {
    dialect: String,
}

impl ProtocolSource {
    /// Return the caller-provided SQL dialect name.
    pub fn dialect(&self) -> &str {
        &self.dialect
    }
}

/// Protocol representation of one parsed SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProtocolStatement {
    /// A query for which known semantics and analysis gaps are preserved independently.
    Query(QueryStatement),
    /// A parsed statement whose statement-level semantics are not modeled.
    Unsupported(UnsupportedStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowConditions {
    predicates: Box<Predicates>,
    column_domains: Vec<ColumnDomain>,
    exactness: ConditionExactness,
}

impl RowConditions {
    pub(crate) fn new(
        predicates: Predicates,
        mut column_domains: Vec<ColumnDomain>,
        exactness: ConditionExactness,
    ) -> Self {
        column_domains.sort_by(|left, right| left.column.cmp(&right.column));
        Self {
            predicates: Box::new(predicates),
            column_domains,
            exactness,
        }
    }
}

/// Partially analyzed query semantics.
///
/// Supported CTE and derived-table semantics are resolved through local scopes so physical joins,
/// source-column domains, and output lineage remain visible on the enclosing query. Sections whose
/// analysis has not been implemented are emitted conservatively and accompanied by diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryStatement {
    sources: Vec<SourceRelation>,
    dependencies: Vec<String>,
    joins: Vec<Join>,
    row_conditions: Box<RowConditions>,
    output: Output,
    aggregation: Option<Box<Aggregation>>,
    group_witness: Option<Box<GroupWitness>>,
    window_witness: Option<Box<WindowWitness>>,
    set_operation: Option<SetOperation>,
    produced_relation: Option<String>,
    write: Option<Box<WriteOperation>>,
    diagnostics: Vec<Diagnostic>,
}

impl QueryStatement {
    pub(crate) fn new(
        sources: Vec<SourceRelation>,
        dependencies: Vec<String>,
        joins: Vec<Join>,
        row_conditions: RowConditions,
        output: Output,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            sources,
            dependencies,
            joins,
            row_conditions: Box::new(row_conditions),
            output,
            aggregation: None,
            group_witness: None,
            window_witness: None,
            set_operation: None,
            produced_relation: None,
            write: None,
            diagnostics,
        }
    }

    pub(crate) fn with_aggregation(mut self, aggregation: Option<Aggregation>) -> Self {
        self.aggregation = aggregation.map(Box::new);
        self
    }

    pub(crate) fn with_group_witness(mut self) -> Self {
        self.group_witness = crate::group_witness::analyze(&self).map(Box::new);
        if self.group_witness.is_some() {
            self.output = crate::group_witness::refine_output(&self);
        }
        self
    }

    pub(crate) fn with_window_witness(mut self) -> Self {
        self.window_witness = crate::window_witness::analyze(&self).map(Box::new);
        if self
            .window_witness
            .as_ref()
            .is_some_and(|witness| witness.is_exact())
        {
            self.row_conditions.exactness =
                self.row_conditions.exactness.without_qualify_residual();
            self.output = crate::window_witness::refine_output(&self);
        }
        self
    }

    pub(crate) fn with_set_operation(mut self, set_operation: Option<SetOperation>) -> Self {
        self.set_operation = set_operation;
        self
    }

    pub(crate) fn with_produced_relation(mut self, produced_relation: Option<String>) -> Self {
        self.produced_relation = produced_relation;
        self
    }

    pub(crate) fn with_write(mut self, write: Option<WriteOperation>) -> Self {
        self.write = write.map(Box::new);
        self
    }

    /// Return direct relational inputs in first semantic appearance order.
    pub fn sources(&self) -> &[SourceRelation] {
        &self.sources
    }

    /// Return normalized physical upstream dependencies in lexicographic order.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Return joins visible to the query in SQL join order.
    ///
    /// Joins carried through referenced CTEs and derived tables retain their logical relation
    /// participants. Equality-column operands in their conditions resolve to physical source
    /// columns when every local projection hop is a plain column copy.
    pub fn joins(&self) -> &[Join] {
        &self.joins
    }

    /// Return WHERE, HAVING, and QUALIFY semantics known for the query.
    pub fn predicates(&self) -> &Predicates {
        &self.row_conditions.predicates
    }

    /// Return derived source-column value domains in deterministic column order.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.row_conditions.column_domains
    }

    /// Return whether row-membership conditions are represented exactly by domains and joins.
    pub fn condition_exactness(&self) -> &ConditionExactness {
        &self.row_conditions.exactness
    }

    pub(crate) fn declare_comparison_assumptions(&mut self, declared: &[ComparisonAssumption]) {
        self.row_conditions.exactness = self
            .row_conditions
            .exactness
            .clone()
            .with_declarations(declared);
    }

    /// Return final query output columns in SELECT-list order.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Return SELECT DISTINCT and GROUP BY semantics when they affect this query.
    pub fn aggregation(&self) -> Option<&Aggregation> {
        self.aggregation.as_deref()
    }

    /// Return typed qualifying and HAVING-rejected group witness plans, when HAVING exists.
    pub fn group_witness(&self) -> Option<&GroupWitness> {
        self.group_witness.as_deref()
    }

    /// Typed source-partition witness obligations for a QUALIFY rank filter.
    pub fn window_witness(&self) -> Option<&WindowWitness> {
        self.window_witness.as_deref()
    }

    /// Return the set-operation tree when this query combines multiple query operands.
    pub fn set_operation(&self) -> Option<&SetOperation> {
        self.set_operation.as_ref()
    }

    /// Return the named relation written by this transformation, if any.
    ///
    /// Bare queries produce anonymous results and therefore return `None`.
    pub fn produced_relation(&self) -> Option<&str> {
        self.produced_relation.as_deref()
    }

    /// Return relation-write semantics when this transformation writes a named relation.
    pub fn write(&self) -> Option<&WriteOperation> {
        self.write.as_deref()
    }

    /// Return diagnostics describing incomplete query semantics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// How a transformation changes its named target relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteKind {
    /// The transformation defines the complete relation contents represented by its query.
    Definition,
    /// The transformation appends rows without replacing pre-existing relation contents.
    Append,
    /// The transformation conditionally updates, inserts, or deletes existing relation rows.
    ConditionalMutation,
}

impl WriteKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Definition => "definition",
            Self::Append => "append",
            Self::ConditionalMutation => "conditional_mutation",
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
}

impl WriteOperation {
    pub(crate) fn definition(target: String) -> Self {
        Self {
            target,
            kind: WriteKind::Definition,
            target_columns: Vec::new(),
            match_condition: None,
            merge_clauses: Vec::new(),
        }
    }

    pub(crate) fn append(target: String, target_columns: Vec<String>) -> Self {
        Self {
            target,
            kind: WriteKind::Append,
            target_columns,
            match_condition: None,
            merge_clauses: Vec::new(),
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
    action: MergeAction,
}

impl MergeClause {
    pub(crate) fn new(
        match_kind: MergeMatchKind,
        predicate: Option<Predicate>,
        action: MergeAction,
    ) -> Self {
        Self {
            match_kind,
            predicate,
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
            output: query.output.clone(),
            condition_exactness: query.row_conditions.exactness.clone(),
            dependencies: query.dependencies.clone(),
            witness_boundary,
        }
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

/// SELECT-level duplicate elimination and grouping semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregation {
    distinct: bool,
    distinct_on: Vec<Expression>,
    group_by: Option<GroupBy>,
}

impl Aggregation {
    pub(crate) fn new(
        distinct: bool,
        distinct_on: Vec<Expression>,
        group_by: Option<GroupBy>,
    ) -> Self {
        Self {
            distinct,
            distinct_on,
            group_by,
        }
    }

    /// Return whether the SELECT eliminates duplicate output rows.
    pub fn distinct(&self) -> bool {
        self.distinct
    }

    /// Return DISTINCT ON expressions in SQL order.
    pub fn distinct_on(&self) -> &[Expression] {
        &self.distinct_on
    }

    /// Return GROUP BY semantics when grouping is present.
    pub fn group_by(&self) -> Option<&GroupBy> {
        self.group_by.as_ref()
    }
}

/// Normalized GROUP BY form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupBy {
    /// GROUP BY ALL.
    All,
    /// Explicit grouping expressions in SQL order.
    Expressions(Vec<GroupingExpression>),
}

/// One parser-independent grouping element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupingExpression {
    /// An ordinary GROUP BY expression.
    Expression(Expression),
    /// GROUPING SETS, with one vector per set.
    GroupingSets(Vec<Vec<Expression>>),
    /// ROLLUP, preserving parser-provided grouping levels.
    Rollup(Vec<Vec<Expression>>),
    /// CUBE, preserving parser-provided grouping levels.
    Cube(Vec<Vec<Expression>>),
}

/// Final columns produced by a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    columns: Vec<OutputColumn>,
}

impl Output {
    pub(crate) fn new(columns: Vec<OutputColumn>) -> Self {
        Self { columns }
    }

    /// Return final output columns in SELECT-list order.
    pub fn columns(&self) -> &[OutputColumn] {
        &self.columns
    }
}

/// One final query output column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputColumn {
    name: String,
    expression: Expression,
    domain: ValueDomain,
    lineage: Vec<LineageSource>,
}

impl OutputColumn {
    pub(crate) fn new(
        name: String,
        expression: Expression,
        domain: ValueDomain,
        mut lineage: Vec<LineageSource>,
    ) -> Self {
        lineage.sort();
        lineage.dedup();
        Self {
            name,
            expression,
            domain,
            lineage,
        }
    }

    pub(crate) fn with_domain(mut self, domain: ValueDomain) -> Self {
        self.domain = domain;
        self
    }

    pub(crate) fn plain_copy_source(&self) -> Option<&LineageSource> {
        if !matches!(self.expression, Expression::Column(_)) {
            return None;
        }

        match self.lineage.as_slice() {
            [source] => Some(source),
            _ => None,
        }
    }

    /// Return the final output column name or unresolved projection label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the semantic expression that produces this output column.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the conservative value domain for this produced output value.
    pub fn domain(&self) -> &ValueDomain {
        &self.domain
    }

    /// Return physical source columns contributing to this output value.
    pub fn lineage(&self) -> &[LineageSource] {
        &self.lineage
    }
}

/// One physical source column contributing to an output value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineageSource {
    relation: String,
    column: String,
}

impl LineageSource {
    pub(crate) fn new(relation: String, column: String) -> Self {
        Self { relation, column }
    }

    /// Return the physical relation containing the source column.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the physical source column name.
    pub fn column(&self) -> &str {
        &self.column
    }
}

/// A direct relational input to a query.
///
/// Physical relations also appear in QueryStatement::dependencies. Local relations such as CTEs
/// and derived tables do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRelation {
    name: String,
    alias: Option<String>,
}

impl SourceRelation {
    pub(crate) fn new(name: String, alias: Option<String>) -> Self {
        Self { name, alias }
    }

    /// Return the relation name or stable local relation identity.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the SQL alias when one is present.
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// A relation participating in a join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRef {
    relation: String,
    alias: Option<String>,
}

impl RelationRef {
    pub(crate) fn new(relation: String, alias: Option<String>) -> Self {
        Self { relation, alias }
    }

    /// Return the relation name or stable local relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return the SQL alias when one is present.
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// Supported protocol join kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// INNER JOIN or JOIN.
    Inner,
    /// LEFT JOIN.
    Left,
    /// RIGHT JOIN.
    Right,
    /// FULL JOIN.
    Full,
    /// CROSS JOIN.
    Cross,
    /// LEFT SEMI JOIN.
    LeftSemi,
    /// RIGHT SEMI JOIN.
    RightSemi,
    /// LEFT ANTI JOIN.
    LeftAnti,
    /// RIGHT ANTI JOIN.
    RightAnti,
    /// The join exists but its exact kind is unsupported.
    Unknown,
}

impl JoinKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Inner => "inner",
            Self::Left => "left",
            Self::Right => "right",
            Self::Full => "full",
            Self::Cross => "cross",
            Self::LeftSemi => "left_semi",
            Self::RightSemi => "right_semi",
            Self::LeftAnti => "left_anti",
            Self::RightAnti => "right_anti",
            Self::Unknown => "unknown",
        }
    }
}

/// A normalized relationship between two query relations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    kind: JoinKind,
    left: RelationRef,
    right: RelationRef,
    condition: Option<Predicate>,
}

impl Join {
    pub(crate) fn new(
        kind: JoinKind,
        left: RelationRef,
        right: RelationRef,
        condition: Option<Predicate>,
    ) -> Self {
        Self {
            kind,
            left,
            right,
            condition,
        }
    }

    /// Return the normalized join kind.
    pub fn kind(&self) -> JoinKind {
        self.kind
    }

    /// Return the left relation participating in the join.
    pub fn left(&self) -> &RelationRef {
        &self.left
    }

    /// Return the right relation participating in the join.
    pub fn right(&self) -> &RelationRef {
        &self.right
    }

    /// Return the normalized join condition when it can be represented safely.
    ///
    /// Equality-column operands are expressed with physical source relation and column identities
    /// when lineage proves a plain-copy path through local relations. An unsafe mapping is retained
    /// as an unknown expression and accompanied by an `unresolved_join_column_lineage` diagnostic.
    pub fn condition(&self) -> Option<&Predicate> {
        self.condition.as_ref()
    }
}

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

/// Parser-independent expression semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expression {
    /// A column reference.
    Column(ColumnExpression),
    /// A typed literal.
    Literal(LiteralExpression),
    /// A function call whose argument semantics are understood.
    Function(FunctionExpression),
    /// A grouped aggregate function call.
    AggregateFunction(AggregateFunctionExpression),
    /// A window function call with a resolved parser-independent window specification.
    WindowFunction(WindowFunctionExpression),
    /// A CASE expression preserving branch semantics.
    Case(CaseExpression),
    /// A predicate used as a boolean-valued scalar expression.
    BooleanPredicate(Box<Predicate>),
    /// A supported unary operation.
    Unary(UnaryExpression),
    /// A supported binary operation.
    Binary(BinaryExpression),
    /// A scalar subquery evaluated as one expression value.
    ScalarSubquery(Box<ScalarSubqueryExpression>),
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the expression but does not support its semantics.
    Unsupported(UnsupportedSemantic),
}

/// Query semantics retained when a subquery appears inside an expression or predicate.
///
/// The summary is intentionally parser-independent and local to the nested query. Physical
/// dependencies remain explicit, correlated outer references are resolved to physical lineage
/// where possible, and nested diagnostics stay attached instead of disappearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubquerySemantics {
    dependencies: Vec<String>,
    correlations: Vec<LineageSource>,
    joins: Vec<Join>,
    output: Output,
    row_conditions: RowConditions,
    diagnostics: Vec<Diagnostic>,
}

impl SubquerySemantics {
    pub(crate) fn new(
        mut dependencies: Vec<String>,
        mut correlations: Vec<LineageSource>,
        joins: Vec<Join>,
        output: Output,
        row_conditions: RowConditions,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        dependencies.sort();
        dependencies.dedup();
        correlations.sort();
        correlations.dedup();
        Self {
            dependencies,
            correlations,
            joins,
            output,
            row_conditions,
            diagnostics,
        }
    }

    /// Return physical relations read by the nested query.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Return physical outer-scope columns referenced by the nested query.
    pub fn correlations(&self) -> &[LineageSource] {
        &self.correlations
    }

    /// Return joins whose row-membership equalities belong to the nested query.
    pub fn joins(&self) -> &[Join] {
        &self.joins
    }

    /// Return projected nested-query output semantics.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Return WHERE, HAVING, and QUALIFY semantics inside the nested query.
    pub fn predicates(&self) -> &Predicates {
        &self.row_conditions.predicates
    }

    /// Return source-column domains derived inside the nested query.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.row_conditions.column_domains
    }

    /// Return row-condition exactness for the nested query scope.
    pub fn condition_exactness(&self) -> &ConditionExactness {
        &self.row_conditions.exactness
    }

    /// Return diagnostics scoped to the nested query.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// A scalar subquery used as an expression value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarSubqueryExpression {
    subquery: SubquerySemantics,
}

impl ScalarSubqueryExpression {
    pub(crate) fn new(subquery: SubquerySemantics) -> Self {
        Self { subquery }
    }

    /// Return the nested query semantics.
    pub fn subquery(&self) -> &SubquerySemantics {
        &self.subquery
    }
}

/// A column expression with an optional relation qualifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnExpression {
    relation: Option<String>,
    name: String,
}

impl ColumnExpression {
    pub(crate) fn new(relation: Option<String>, name: String) -> Self {
        Self { relation, name }
    }

    /// Return the relation qualifier when one is present.
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    /// Return the column name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A typed literal expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralExpression {
    literal_type: LiteralType,
    value: LiteralValue,
}

impl LiteralExpression {
    pub(crate) fn new(literal_type: LiteralType, value: LiteralValue) -> Self {
        Self {
            literal_type,
            value,
        }
    }

    /// Return the protocol literal type.
    pub fn literal_type(&self) -> LiteralType {
        self.literal_type
    }

    /// Return the literal value.
    pub fn value(&self) -> &LiteralValue {
        &self.value
    }
}

/// Literal types defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralType {
    /// SQL NULL.
    Null,
    /// Boolean literal.
    Boolean,
    /// Integer numeric literal.
    Integer,
    /// Decimal or exponent numeric literal.
    Decimal,
    /// Text string literal.
    String,
    /// Date literal.
    Date,
    /// Time literal.
    Time,
    /// Timestamp literal.
    Timestamp,
    /// Interval literal.
    Interval,
}

impl LiteralType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::String => "string",
            Self::Date => "date",
            Self::Time => "time",
            Self::Timestamp => "timestamp",
            Self::Interval => "interval",
        }
    }
}

/// Literal payload used by LiteralExpression.
///
/// Number values are canonical JSON-number text validated before protocol construction.
/// Text carries string-like SQL literals, including date/time literals whose type is retained by
/// LiteralExpression::literal_type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LiteralValue {
    /// SQL NULL.
    Null,
    /// Boolean value.
    Boolean(bool),
    /// Canonical JSON-number text.
    Number(String),
    /// String-like literal value.
    Text(String),
}

/// Reference to a column whose scalar value domain was analyzed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColumnRef {
    relation: Option<String>,
    name: String,
}

impl ColumnRef {
    pub(crate) fn new(relation: Option<String>, name: String) -> Self {
        Self { relation, name }
    }

    /// Return the resolved relation name when one is known.
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    /// Return the referenced column name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A derived value domain for one referenced column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDomain {
    column: ColumnRef,
    domain: ValueDomain,
}

impl ColumnDomain {
    pub(crate) fn new(column: ColumnRef, domain: ValueDomain) -> Self {
        Self { column, domain }
    }

    /// Return the constrained column.
    pub fn column(&self) -> &ColumnRef {
        &self.column
    }

    /// Return the conservative scalar domain derived for the column.
    pub fn domain(&self) -> &ValueDomain {
        &self.domain
    }
}

/// Conservative scalar values that may satisfy the analyzed predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueDomain {
    /// No useful scalar restriction is known.
    Unbounded,
    /// One or more ordered ranges constrain the value.
    Ranges(RangesDomain),
    /// A finite inclusion or exclusion set constrains the value.
    Set(SetDomain),
    /// No value can satisfy the known constraints.
    Empty,
    /// A scalar domain cannot be derived safely from the known semantics.
    Unknown(UnknownDomain),
}

impl ValueDomain {
    pub(crate) fn ranges(ranges: Vec<ValueRange>) -> Self {
        if ranges.is_empty() {
            Self::Empty
        } else {
            Self::Ranges(RangesDomain { ranges })
        }
    }

    pub(crate) fn set(mode: SetMode, mut values: Vec<LiteralExpression>) -> Self {
        values.sort_by_key(literal_sort_key);
        values.dedup();

        if values.is_empty() {
            return match mode {
                SetMode::Include => Self::Empty,
                SetMode::Exclude => Self::Unbounded,
            };
        }

        Self::Set(SetDomain { mode, values })
    }

    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        let reason = if reason.trim().is_empty() {
            "column domain could not be derived safely".to_string()
        } else {
            reason
        };
        Self::Unknown(UnknownDomain { reason })
    }

    /// Return whether SQL NULL is admitted by this domain.
    ///
    /// A None result means the domain itself is unknown, so NULL membership cannot be proven.
    pub fn admits_null(&self) -> Option<bool> {
        match self {
            Self::Unbounded => Some(true),
            Self::Ranges(_) => Some(false),
            Self::Set(domain) => {
                let contains_null = domain
                    .values
                    .iter()
                    .any(|literal| matches!(literal.value(), LiteralValue::Null));
                Some(match domain.mode {
                    SetMode::Include => contains_null,
                    SetMode::Exclude => !contains_null,
                })
            }
            Self::Empty => Some(false),
            Self::Unknown(_) => None,
        }
    }
}

/// Ordered ranges comprising a value domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangesDomain {
    ranges: Vec<ValueRange>,
}

impl RangesDomain {
    /// Return ranges in deterministic derivation order.
    pub fn ranges(&self) -> &[ValueRange] {
        &self.ranges
    }
}

/// One interval in an ordered value domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRange {
    lower: Option<Bound>,
    upper: Option<Bound>,
}

impl ValueRange {
    pub(crate) fn new(lower: Option<Bound>, upper: Option<Bound>) -> Self {
        Self { lower, upper }
    }

    /// Return the lower bound, or None when the range is unbounded below.
    pub fn lower(&self) -> Option<&Bound> {
        self.lower.as_ref()
    }

    /// Return the upper bound, or None when the range is unbounded above.
    pub fn upper(&self) -> Option<&Bound> {
        self.upper.as_ref()
    }
}

/// One inclusive or exclusive literal range bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    value: LiteralExpression,
    inclusive: bool,
}

impl Bound {
    pub(crate) fn new(value: LiteralExpression, inclusive: bool) -> Self {
        Self { value, inclusive }
    }

    /// Return the literal value used by this bound.
    pub fn value(&self) -> &LiteralExpression {
        &self.value
    }

    /// Return whether the bound includes its literal value.
    pub fn inclusive(&self) -> bool {
        self.inclusive
    }
}

/// Set-domain interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetMode {
    /// Only the listed values are allowed.
    Include,
    /// Every value except the listed values is allowed.
    Exclude,
}

impl SetMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Include => "include",
            Self::Exclude => "exclude",
        }
    }
}

/// A finite inclusion or exclusion domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetDomain {
    mode: SetMode,
    values: Vec<LiteralExpression>,
}

impl SetDomain {
    /// Return whether values are included or excluded.
    pub fn mode(&self) -> SetMode {
        self.mode
    }

    /// Return deterministically ordered literal values.
    pub fn values(&self) -> &[LiteralExpression] {
        &self.values
    }
}

/// Reason a safe scalar value domain could not be derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDomain {
    reason: String,
}

impl UnknownDomain {
    /// Return why domain derivation could not be completed safely.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

fn literal_sort_key(literal: &LiteralExpression) -> (&'static str, String) {
    let value = match literal.value() {
        LiteralValue::Null => "null".to_string(),
        LiteralValue::Boolean(value) => value.to_string(),
        LiteralValue::Number(value) | LiteralValue::Text(value) => value.clone(),
    };
    (literal.literal_type().as_str(), value)
}

/// One argument to an aggregate function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateArgument {
    /// A scalar expression argument.
    Expression(Expression),
    /// An unqualified wildcard such as COUNT(*).
    Wildcard,
    /// A qualified wildcard such as COUNT(table.*).
    QualifiedWildcard(String),
}

/// A normalized grouped aggregate function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateFunctionExpression {
    name: String,
    arguments: Vec<AggregateArgument>,
    distinct: bool,
    filter: Option<Box<Predicate>>,
}

impl AggregateFunctionExpression {
    pub(crate) fn new(
        name: String,
        arguments: Vec<AggregateArgument>,
        distinct: bool,
        filter: Option<Predicate>,
    ) -> Self {
        Self {
            name,
            arguments,
            distinct,
            filter: filter.map(Box::new),
        }
    }

    /// Return the aggregate function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return normalized aggregate arguments in SQL order.
    pub fn arguments(&self) -> &[AggregateArgument] {
        &self.arguments
    }

    /// Return whether the aggregate argument list uses DISTINCT.
    pub fn distinct(&self) -> bool {
        self.distinct
    }

    /// Return the aggregate FILTER predicate, if present.
    pub fn filter(&self) -> Option<&Predicate> {
        self.filter.as_deref()
    }
}

/// One normalized CASE expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseExpression {
    operand: Option<Box<Expression>>,
    branches: Vec<CaseBranch>,
    else_result: Option<Box<Expression>>,
    else_source_domains: CaseSourceDomains,
}

impl CaseExpression {
    pub(crate) fn new(
        operand: Option<Expression>,
        branches: Vec<CaseBranch>,
        else_result: Option<Expression>,
        else_source_domains: CaseSourceDomains,
    ) -> Self {
        Self {
            operand: operand.map(Box::new),
            branches,
            else_result: else_result.map(Box::new),
            else_source_domains,
        }
    }

    /// Return the optional simple-CASE operand.
    pub fn operand(&self) -> Option<&Expression> {
        self.operand.as_deref()
    }

    /// Return CASE branches in SQL order.
    pub fn branches(&self) -> &[CaseBranch] {
        &self.branches
    }

    /// Return the ELSE result, if present.
    pub fn else_result(&self) -> Option<&Expression> {
        self.else_result.as_deref()
    }

    /// Return source-column domains selecting the explicit or implicit ELSE branch.
    ///
    /// Local-relation copies preserve these domains, and composed layers remap them to physical
    /// sources only through proven plain-copy identity paths. Query-level filters are represented
    /// separately in column domains and are not folded into CASE branch reachability.
    pub fn else_source_domains(&self) -> &CaseSourceDomains {
        &self.else_source_domains
    }
}

/// One WHEN/THEN branch in a CASE expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseBranch {
    condition: Expression,
    result: Expression,
    source_domains: CaseSourceDomains,
}

impl CaseBranch {
    pub(crate) fn new(
        condition: Expression,
        result: Expression,
        source_domains: CaseSourceDomains,
    ) -> Self {
        Self {
            condition,
            result,
            source_domains,
        }
    }

    /// Return the WHEN condition or simple-CASE match value.
    pub fn condition(&self) -> &Expression {
        &self.condition
    }

    /// Return the THEN result expression.
    pub fn result(&self) -> &Expression {
        &self.result
    }

    /// Return source-column domains selecting this branch after earlier branches fail.
    ///
    /// Local-relation copies preserve these domains, and composed layers remap them to physical
    /// sources only through proven plain-copy identity paths. Query-level filters are represented
    /// separately in column domains and are not folded into CASE branch reachability.
    pub fn source_domains(&self) -> &CaseSourceDomains {
        &self.source_domains
    }
}

/// One conjunction of physical source-column domains that can select a CASE branch.
///
/// Alternatives inside CaseSourceDomains::Reachable are combined with logical OR. Domains inside
/// one alternative are combined with logical AND.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSourceDomainAlternative {
    column_domains: Vec<ColumnDomain>,
}

impl CaseSourceDomainAlternative {
    pub(crate) fn new(mut column_domains: Vec<ColumnDomain>) -> Self {
        column_domains.sort_by(|left, right| left.column.cmp(&right.column));
        Self { column_domains }
    }

    /// Return conjunctive physical source-column domains in deterministic column order.
    pub fn column_domains(&self) -> &[ColumnDomain] {
        &self.column_domains
    }
}

/// Source-column domains controlling CASE branch selection.
///
/// Reachability reflects CASE control flow, including prior branches, but does not intersect
/// query-level WHERE, HAVING, or QUALIFY domains. During composition, reachable domains are
/// rewritten to physical sources only through proven identity columns; unsafe hops become Unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CaseSourceDomains {
    /// The branch is reachable through one or more disjunctive domain alternatives.
    Reachable {
        /// Domain alternatives in deterministic derivation order.
        alternatives: Vec<CaseSourceDomainAlternative>,
    },
    /// Earlier CASE branches make this branch impossible to select.
    Unreachable,
    /// Branch selection cannot be reduced safely to physical source-column domains.
    Unknown(UnknownDomain),
}

impl CaseSourceDomains {
    pub(crate) fn reachable(alternatives: Vec<CaseSourceDomainAlternative>) -> Self {
        if alternatives.is_empty() {
            Self::Unreachable
        } else {
            Self::Reachable { alternatives }
        }
    }

    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        let reason = if reason.trim().is_empty() {
            "CASE branch source domains could not be derived safely".to_string()
        } else {
            reason
        };
        Self::Unknown(UnknownDomain { reason })
    }
}

/// A normalized function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionExpression {
    name: String,
    arguments: Vec<Expression>,
    distinct: bool,
}

impl FunctionExpression {
    pub(crate) fn new(name: String, arguments: Vec<Expression>, distinct: bool) -> Self {
        Self {
            name,
            arguments,
            distinct,
        }
    }

    /// Return the function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return normalized function arguments in SQL order.
    pub fn arguments(&self) -> &[Expression] {
        &self.arguments
    }

    /// Return whether the function argument list uses DISTINCT.
    pub fn distinct(&self) -> bool {
        self.distinct
    }
}

/// A normalized window function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFunctionExpression {
    function: FunctionExpression,
    window: WindowSpecification,
}

impl WindowFunctionExpression {
    pub(crate) fn new(function: FunctionExpression, window: WindowSpecification) -> Self {
        Self { function, window }
    }

    /// Return the normalized function call.
    pub fn function(&self) -> &FunctionExpression {
        &self.function
    }

    /// Return the resolved window specification.
    pub fn window(&self) -> &WindowSpecification {
        &self.window
    }
}

/// A resolved parser-independent window specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSpecification {
    name: Option<String>,
    partition_by: Vec<Expression>,
    order_by: Vec<WindowOrderExpression>,
    frame: Option<WindowFrame>,
}

impl WindowSpecification {
    pub(crate) fn new(
        name: Option<String>,
        partition_by: Vec<Expression>,
        order_by: Vec<WindowOrderExpression>,
        frame: Option<WindowFrame>,
    ) -> Self {
        Self {
            name,
            partition_by,
            order_by,
            frame,
        }
    }

    /// Return the referenced named window, if the function used one.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return normalized PARTITION BY expressions in SQL order.
    pub fn partition_by(&self) -> &[Expression] {
        &self.partition_by
    }

    /// Return normalized ORDER BY expressions in SQL order.
    pub fn order_by(&self) -> &[WindowOrderExpression] {
        &self.order_by
    }

    /// Return the explicit frame, if one was specified.
    pub fn frame(&self) -> Option<&WindowFrame> {
        self.frame.as_ref()
    }
}

/// One expression in a window ORDER BY clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOrderExpression {
    expression: Expression,
    ascending: Option<bool>,
    nulls_first: Option<bool>,
}

impl WindowOrderExpression {
    pub(crate) fn new(
        expression: Expression,
        ascending: Option<bool>,
        nulls_first: Option<bool>,
    ) -> Self {
        Self {
            expression,
            ascending,
            nulls_first,
        }
    }

    /// Return the ordering expression.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return explicit ascending or descending ordering, if specified.
    pub fn ascending(&self) -> Option<bool> {
        self.ascending
    }

    /// Return explicit NULLS FIRST or NULLS LAST ordering, if specified.
    pub fn nulls_first(&self) -> Option<bool> {
        self.nulls_first
    }
}

/// Units used by an explicit window frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFrameUnits {
    /// ROWS frame semantics.
    Rows,
    /// RANGE frame semantics.
    Range,
    /// GROUPS frame semantics.
    Groups,
}

impl WindowFrameUnits {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Range => "range",
            Self::Groups => "groups",
        }
    }
}

/// One explicit window frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFrame {
    units: WindowFrameUnits,
    start_bound: WindowFrameBound,
    end_bound: WindowFrameBound,
}

impl WindowFrame {
    pub(crate) fn new(
        units: WindowFrameUnits,
        start_bound: WindowFrameBound,
        end_bound: WindowFrameBound,
    ) -> Self {
        Self {
            units,
            start_bound,
            end_bound,
        }
    }

    /// Return the frame units.
    pub fn units(&self) -> WindowFrameUnits {
        self.units
    }

    /// Return the starting frame bound.
    pub fn start_bound(&self) -> &WindowFrameBound {
        &self.start_bound
    }

    /// Return the ending frame bound.
    pub fn end_bound(&self) -> &WindowFrameBound {
        &self.end_bound
    }
}

/// One normalized window-frame boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowFrameBound {
    /// CURRENT ROW.
    CurrentRow,
    /// UNBOUNDED PRECEDING.
    UnboundedPreceding,
    /// A bounded PRECEDING offset.
    Preceding(Box<Expression>),
    /// UNBOUNDED FOLLOWING.
    UnboundedFollowing,
    /// A bounded FOLLOWING offset.
    Following(Box<Expression>),
}

/// A normalized unary operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpression {
    operator: UnaryOperator,
    operand: Box<Expression>,
}

impl UnaryExpression {
    pub(crate) fn new(operator: UnaryOperator, operand: Expression) -> Self {
        Self {
            operator,
            operand: Box::new(operand),
        }
    }

    /// Return the unary operator.
    pub fn operator(&self) -> UnaryOperator {
        self.operator
    }

    /// Return the unary operand.
    pub fn operand(&self) -> &Expression {
        &self.operand
    }
}

/// Unary expression operators defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Unary plus.
    Plus,
    /// Unary minus.
    Minus,
    /// Bitwise NOT.
    BitwiseNot,
}

impl UnaryOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::BitwiseNot => "bitwise_not",
        }
    }
}

/// A normalized binary operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpression {
    operator: BinaryOperator,
    left: Box<Expression>,
    right: Box<Expression>,
}

impl BinaryExpression {
    pub(crate) fn new(operator: BinaryOperator, left: Expression, right: Expression) -> Self {
        Self {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Return the binary operator.
    pub fn operator(&self) -> BinaryOperator {
        self.operator
    }

    /// Return the left operand.
    pub fn left(&self) -> &Expression {
        &self.left
    }

    /// Return the right operand.
    pub fn right(&self) -> &Expression {
        &self.right
    }
}

/// Binary expression operators defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    /// Addition.
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
    /// Modulo.
    Modulo,
    /// String concatenation.
    StringConcat,
    /// Bitwise AND.
    BitwiseAnd,
    /// Bitwise OR.
    BitwiseOr,
    /// Bitwise XOR.
    BitwiseXor,
}

impl BinaryOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Modulo => "modulo",
            Self::StringConcat => "string_concat",
            Self::BitwiseAnd => "bitwise_and",
            Self::BitwiseOr => "bitwise_or",
            Self::BitwiseXor => "bitwise_xor",
        }
    }
}

/// Predicate semantics known by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Predicate {
    /// A comparison between two expressions.
    Comparison(ComparisonPredicate),
    /// Logical conjunction preserving SQL tree order.
    And(LogicalPredicate),
    /// Logical disjunction preserving SQL tree order.
    Or(LogicalPredicate),
    /// Logical negation.
    Not(NotPredicate),
    /// SQL IS NULL or IS NOT NULL.
    IsNull(IsNullPredicate),
    /// SQL IN or NOT IN with an explicit value list.
    In(InPredicate),
    /// SQL EXISTS or NOT EXISTS with nested query semantics.
    Exists(ExistsPredicate),
    /// SQL IN or NOT IN whose values come from a subquery.
    InSubquery(InSubqueryPredicate),
    /// SQL BETWEEN or NOT BETWEEN.
    Between(BetweenPredicate),
    /// An expression interpreted in boolean predicate context.
    BooleanExpression(Expression),
    /// Semantics exist but cannot be resolved precisely from available information.
    Unknown(UnknownSemantic),
    /// The producer recognizes the feature but does not support its semantics yet.
    Unsupported(UnsupportedSemantic),
}

/// A comparison predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonPredicate {
    left: Expression,
    operator: ComparisonOperator,
    right: Expression,
}

impl ComparisonPredicate {
    pub(crate) fn new(left: Expression, operator: ComparisonOperator, right: Expression) -> Self {
        Self {
            left,
            operator,
            right,
        }
    }

    /// Return the left comparison expression.
    pub fn left(&self) -> &Expression {
        &self.left
    }

    /// Return the comparison operator.
    pub fn operator(&self) -> ComparisonOperator {
        self.operator
    }

    /// Return the right comparison expression.
    pub fn right(&self) -> &Expression {
        &self.right
    }
}

/// Comparison operators defined by protocol v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    /// Equal.
    Eq,
    /// Not equal.
    Neq,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// IS DISTINCT FROM.
    IsDistinctFrom,
    /// IS NOT DISTINCT FROM.
    IsNotDistinctFrom,
}

impl ComparisonOperator {
    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Eq => Self::Eq,
            Self::Neq => Self::Neq,
            Self::Lt => Self::Gt,
            Self::Lte => Self::Gte,
            Self::Gt => Self::Lt,
            Self::Gte => Self::Lte,
            Self::IsDistinctFrom => Self::IsDistinctFrom,
            Self::IsNotDistinctFrom => Self::IsNotDistinctFrom,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Neq => "neq",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::IsDistinctFrom => "is_distinct_from",
            Self::IsNotDistinctFrom => "is_not_distinct_from",
        }
    }
}

/// Operands for an AND or OR predicate.
///
/// Construction is crate-private so protocol values emitted by the analyzer always contain at
/// least the two operands required by protocol v0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalPredicate {
    operands: Vec<Predicate>,
}

impl LogicalPredicate {
    pub(crate) fn pair(left: Predicate, right: Predicate) -> Self {
        Self {
            operands: vec![left, right],
        }
    }

    /// Return logical operands in SQL evaluation-tree order.
    pub fn operands(&self) -> &[Predicate] {
        &self.operands
    }
}

/// A logical NOT predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotPredicate {
    operand: Box<Predicate>,
}

impl NotPredicate {
    pub(crate) fn new(operand: Predicate) -> Self {
        Self {
            operand: Box::new(operand),
        }
    }

    /// Return the predicate being negated.
    pub fn operand(&self) -> &Predicate {
        &self.operand
    }
}

/// An IS NULL predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsNullPredicate {
    expression: Expression,
    negated: bool,
}

impl IsNullPredicate {
    pub(crate) fn new(expression: Expression, negated: bool) -> Self {
        Self {
            expression,
            negated,
        }
    }

    /// Return the tested expression.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return whether the SQL form is IS NOT NULL.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// An IN-list predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InPredicate {
    expression: Expression,
    values: Vec<Expression>,
    negated: bool,
}

impl InPredicate {
    pub(crate) fn new(expression: Expression, values: Vec<Expression>, negated: bool) -> Self {
        Self {
            expression,
            values,
            negated,
        }
    }

    /// Return the expression tested for membership.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return IN-list values in SQL order.
    pub fn values(&self) -> &[Expression] {
        &self.values
    }

    /// Return whether the SQL form is NOT IN.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// An EXISTS or NOT EXISTS predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistsPredicate {
    subquery: SubquerySemantics,
    negated: bool,
}

impl ExistsPredicate {
    pub(crate) fn new(subquery: SubquerySemantics, negated: bool) -> Self {
        Self { subquery, negated }
    }

    /// Return the nested query semantics.
    pub fn subquery(&self) -> &SubquerySemantics {
        &self.subquery
    }

    /// Return whether the SQL form is NOT EXISTS.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// An IN or NOT IN predicate whose candidate values come from a subquery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InSubqueryPredicate {
    expression: Expression,
    subquery: SubquerySemantics,
    negated: bool,
}

impl InSubqueryPredicate {
    pub(crate) fn new(expression: Expression, subquery: SubquerySemantics, negated: bool) -> Self {
        Self {
            expression,
            subquery,
            negated,
        }
    }

    /// Return the expression tested for membership.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the nested query that supplies candidate values.
    pub fn subquery(&self) -> &SubquerySemantics {
        &self.subquery
    }

    /// Return whether the SQL form is NOT IN.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// A BETWEEN predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BetweenPredicate {
    expression: Expression,
    lower: Expression,
    upper: Expression,
    negated: bool,
}

impl BetweenPredicate {
    pub(crate) fn new(
        expression: Expression,
        lower: Expression,
        upper: Expression,
        negated: bool,
    ) -> Self {
        Self {
            expression,
            lower,
            upper,
            negated,
        }
    }

    /// Return the constrained expression.
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// Return the lower bound expression.
    pub fn lower(&self) -> &Expression {
        &self.lower
    }

    /// Return the upper bound expression.
    pub fn upper(&self) -> &Expression {
        &self.upper
    }

    /// Return whether the SQL form is NOT BETWEEN.
    pub fn negated(&self) -> bool {
        self.negated
    }
}

/// Semantic value whose precise meaning cannot currently be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSemantic {
    reason: String,
}

impl UnknownSemantic {
    pub(crate) fn new(reason: String) -> Self {
        Self { reason }
    }

    /// Return the reason the semantic value could not be resolved.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Semantic feature recognized by the parser but not supported by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSemantic {
    feature: String,
    reason: Option<String>,
}

impl UnsupportedSemantic {
    pub(crate) fn new(feature: String, reason: Option<String>) -> Self {
        Self { feature, reason }
    }

    /// Return the stable name of the unsupported semantic feature.
    pub fn feature(&self) -> &str {
        &self.feature
    }

    /// Return an optional human-readable reason for the unsupported feature.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// A parsed statement that cannot yet be represented semantically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedStatement {
    category: String,
    diagnostics: Vec<Diagnostic>,
}

impl UnsupportedStatement {
    pub(crate) fn new(category: String, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            category,
            diagnostics,
        }
    }

    /// Return the broad statement category reported by the analyzer.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return diagnostics explaining why the statement is unsupported.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Analyzer diagnostic attached to protocol semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    severity: DiagnosticSeverity,
    code: String,
    area: DiagnosticArea,
    message: String,
}

impl Diagnostic {
    pub(crate) fn new(
        severity: DiagnosticSeverity,
        code: String,
        area: DiagnosticArea,
        message: String,
    ) -> Self {
        Self {
            severity,
            code,
            area,
            message,
        }
    }

    /// Return the diagnostic severity.
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Return the stable diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Return the semantic area affected by the diagnostic.
    pub fn area(&self) -> DiagnosticArea {
        self.area
    }

    /// Return the human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Severity of a protocol diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// Informational diagnostic.
    Info,
    /// Incomplete semantics that consumers should account for.
    Warning,
    /// Semantic problem that prevents reliable interpretation of the affected area.
    Error,
}

impl DiagnosticSeverity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Semantic area affected by a protocol diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticArea {
    /// Whole-statement semantics.
    Statement,
    /// Source-relation semantics.
    Source,
    /// Join semantics.
    Join,
    /// Predicate semantics.
    Predicate,
    /// Column-domain semantics.
    Domain,
    /// Output-column semantics.
    Output,
    /// Expression semantics.
    Expression,
    /// Function semantics.
    Function,
    /// Semantics that do not fit a more specific area.
    Other,
}

impl DiagnosticArea {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Statement => "statement",
            Self::Source => "source",
            Self::Join => "join",
            Self::Predicate => "predicate",
            Self::Domain => "domain",
            Self::Output => "output",
            Self::Expression => "expression",
            Self::Function => "function",
            Self::Other => "other",
        }
    }
}
