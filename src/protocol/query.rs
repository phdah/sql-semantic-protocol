//! Query semantics and row-condition metadata.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowConditions {
    pub(super) predicates: Box<Predicates>,
    pub(super) column_domains: Vec<ColumnDomain>,
    pub(super) exactness: ConditionExactness,
}

impl RowConditions {
    pub(crate) fn new(
        predicates: Predicates,
        mut column_domains: Vec<ColumnDomain>,
        exactness: ConditionExactness,
    ) -> Self {
        column_domains.sort_by(|left, right| left.column().cmp(right.column()));
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
    output: Box<Output>,
    aggregation: Option<Box<Aggregation>>,
    group_witness: Option<Box<GroupWitness>>,
    window_witness: Option<Box<WindowWitness>>,
    subquery_witnesses: Box<[crate::subquery_witness::SubqueryMembershipWitness]>,
    boolean_witness: Option<Box<BooleanWitness>>,
    row_preserving_projection: bool,
    filter_only_row_shape: bool,
    proven_single_row_output: bool,
    group_rows_match_surviving_groups: bool,
    plain_goal_output_shape: bool,
    filtered_join_goal_output_shape: bool,
    special_join_goal_output_shape: bool,
    ranked_goal_output_shape: bool,
    set_operation: Option<Box<SetOperation>>,
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
            output: Box::new(output),
            aggregation: None,
            group_witness: None,
            window_witness: None,
            subquery_witnesses: Vec::new().into_boxed_slice(),
            boolean_witness: None,
            row_preserving_projection: false,
            filter_only_row_shape: false,
            proven_single_row_output: false,
            group_rows_match_surviving_groups: false,
            plain_goal_output_shape: false,
            filtered_join_goal_output_shape: false,
            special_join_goal_output_shape: false,
            ranked_goal_output_shape: false,
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

    pub(crate) fn with_subquery_witnesses(mut self) -> Self {
        self.subquery_witnesses = crate::subquery_witness::analyze(&self).into_boxed_slice();
        self
    }

    pub(crate) fn with_boolean_witness(mut self, witness: Option<BooleanWitness>) -> Self {
        self.boolean_witness = witness.map(Box::new);
        self
    }

    pub(crate) fn with_row_preserving_projection(mut self, preserved: bool) -> Self {
        self.row_preserving_projection = preserved;
        self
    }

    pub(crate) fn row_preserving_projection(&self) -> bool {
        self.row_preserving_projection
    }

    pub(crate) fn with_filter_only_row_shape(mut self, proven: bool) -> Self {
        self.filter_only_row_shape = proven;
        self
    }

    /// Whether this query filters one direct table without further row shaping.
    ///
    /// The projection may drop or rename columns but never changes row count.
    /// This excludes limits, joins, set operators, grouping, DISTINCT and CTEs.
    pub fn filter_only_row_shape(&self) -> bool {
        self.filter_only_row_shape
    }

    pub(crate) fn with_proven_single_row_output(mut self, proven: bool) -> Self {
        self.proven_single_row_output = proven;
        self
    }

    /// Whether syntax and supported expressions prove exactly one output row.
    /// This is independent of whether any source table contains rows.
    pub fn proven_single_row_output(&self) -> bool {
        self.proven_single_row_output
    }

    pub(crate) fn with_group_row_correspondence(mut self, proven: bool) -> Self {
        self.group_rows_match_surviving_groups = proven;
        self
    }

    /// Whether SQL proves one final result row for every surviving ordinary GROUP BY group.
    /// DISTINCT, QUALIFY and result limits can invalidate this correspondence.
    pub fn group_rows_match_surviving_groups(&self) -> bool {
        self.group_rows_match_surviving_groups
    }

    pub(crate) fn with_goal_shapes(mut self, plain: bool, ranked: bool) -> Self {
        self.plain_goal_output_shape = plain;
        self.ranked_goal_output_shape = ranked;
        self
    }

    /// Whether this local query has no row-shaping beyond a plain FROM/JOIN projection.
    pub fn plain_goal_output_shape(&self) -> bool {
        self.plain_goal_output_shape
    }

    /// Store the parser-certified row shape for WHERE-bearing joins.
    pub(crate) fn with_filtered_join_goal_shape(mut self, proven: bool) -> Self {
        self.filtered_join_goal_output_shape = proven;
        self
    }

    /// Whether a filtered JOIN has no other row-shaping syntax such as LIMIT,
    /// DISTINCT, TOP or grouping. The physical planner certifies its WHERE.
    pub(crate) fn filtered_join_goal_output_shape(&self) -> bool {
        self.filtered_join_goal_output_shape
    }

    pub(crate) fn with_special_join_goal_shape(mut self, proven: bool) -> Self {
        self.special_join_goal_output_shape = proven;
        self
    }

    /// Whether a nonordinary JOIN's outer query is an unshaped, transparent
    /// projection; nested correlation is certified separately at the AST boundary.
    pub fn special_join_goal_output_shape(&self) -> bool {
        self.special_join_goal_output_shape
    }

    /// Whether QUALIFY is the sole row-shaping operation on a single direct table.
    pub fn ranked_goal_output_shape(&self) -> bool {
        self.ranked_goal_output_shape
    }

    pub(crate) fn with_group_witness(mut self) -> Self {
        self.group_witness = crate::group_witness::analyze(&self).map(Box::new);
        if self.group_witness.is_some() {
            self.output = Box::new(crate::group_witness::refine_output(&self));
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
            self.output = Box::new(crate::window_witness::refine_output(&self));
        }
        self
    }

    pub(crate) fn with_projected_window_witness(
        mut self,
        projected: Option<(WindowWitness, String)>,
    ) -> Self {
        if self.window_witness.is_some() {
            return self;
        }
        if let Some((witness, alias)) = projected {
            if witness.is_exact() {
                self.row_conditions.exactness = self
                    .row_conditions
                    .exactness
                    .without_projected_rank_where_residual();
                self.output = Box::new(crate::window_witness::refine_projected_output(
                    &self.output,
                    &alias,
                    &witness,
                ));
            }
            self.window_witness = Some(Box::new(witness));
        }
        self
    }

    pub(crate) fn with_set_operation(mut self, set_operation: Option<SetOperation>) -> Self {
        self.set_operation = set_operation.map(Box::new);
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
        if let Some(witness) = &mut self.boolean_witness {
            witness.declare_comparison_assumptions(declared);
        }
        self.refresh_empty_output_on_impossible();
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

    /// Operator-local typed source witnesses for EXISTS and IN subqueries.
    /// Whole-query condition exactness remains independent of these local proofs.
    pub fn subquery_witnesses(&self) -> &[crate::subquery_witness::SubqueryMembershipWitness] {
        &self.subquery_witnesses
    }

    /// A typed, coupled source-row predicate proof, independent of scalar domain exactness.
    pub fn boolean_witness(&self) -> Option<&BooleanWitness> {
        self.boolean_witness.as_deref()
    }

    pub(crate) fn restrict_boolean_witness(&mut self, sets: &[RelationConstraintSet]) {
        if let Some(witness) = &mut self.boolean_witness {
            witness.restrict_with_schema_constraints(sets);
        }
        self.refresh_empty_output_on_impossible();
    }

    /// After witness evidence changes without re-running analysis, an
    /// impossible qualifying direction admits no rows, so every projected
    /// output is empty. This mirrors the analysis-time impossible refinement
    /// without inventing per-column domains.
    fn refresh_empty_output_on_impossible(&mut self) {
        let directions = self
            .boolean_witness
            .iter()
            .map(|witness| witness.qualifying())
            .collect::<Vec<_>>();
        self.output.empty_on_impossible(directions);
    }

    /// Return the set-operation tree when this query combines multiple query operands.
    pub fn set_operation(&self) -> Option<&SetOperation> {
        self.set_operation.as_deref()
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
