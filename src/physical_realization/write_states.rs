//! Exact ordered mutations over complete caller-controlled physical snapshots.
//!
//! The snapshots are proof inputs, not rows sampled from an open-world table.
//! The interpreter only executes parser-independent, row-local semantics; an
//! unknown expression, unsupported statement or schema obligation stops the
//! entire program before it can be advertised as a feasible physical state.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::AnalysisBundle;
use crate::constraints::{ConstraintEnforcement, ConstraintValue, RelationConstraint};
use crate::data_type::DataType;
use crate::protocol::{
    BinaryOperator, ComparisonOperator, Expression, LiteralValue, MergeAction, MergeMatchKind,
    Predicate, ProtocolStatement, QueryStatement, UnaryOperator, WriteDefinitionMode, WriteKind,
    WriteRowCounts,
};
use crate::relation::RelationSchema;

/// All columns of one concrete physical row, including explicit SQL NULLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalWriteRow {
    values: BTreeMap<String, LiteralValue>,
}

impl PhysicalWriteRow {
    /// Construct a row with named, nonempty column identities.
    pub fn new(values: BTreeMap<String, LiteralValue>) -> Option<Self> {
        if values.keys().any(|name| name.trim().is_empty()) {
            None
        } else {
            Some(Self { values })
        }
    }

    /// Stable column-to-value map, without implicit defaults.
    pub fn values(&self) -> &BTreeMap<String, LiteralValue> {
        &self.values
    }
}

/// An entire closed-world relation population, not an open-world row sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalWriteState {
    relation: String,
    rows: Vec<PhysicalWriteRow>,
}

impl PhysicalWriteState {
    /// Construct a physical population with a named relation.
    pub fn new(relation: impl Into<String>, rows: Vec<PhysicalWriteRow>) -> Option<Self> {
        let relation = relation.into();
        if relation.trim().is_empty() {
            None
        } else {
            Some(Self { relation, rows })
        }
    }

    /// Canonical physical relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Complete rows in deterministic caller-supplied order; duplicates remain distinct.
    pub fn rows(&self) -> &[PhysicalWriteRow] {
        &self.rows
    }
}

/// Exact target state on either side of one committed SQL write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalWriteTransition {
    layer_id: String,
    kind: WriteKind,
    before: PhysicalWriteState,
    after: PhysicalWriteState,
    affected: WriteRowCounts,
}

impl PhysicalWriteTransition {
    /// Canonical layer responsible for this transition.
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }
    /// Parser-independent write kind.
    pub fn kind(&self) -> WriteKind {
        self.kind
    }
    /// Complete target population before executing this statement.
    pub fn before(&self) -> &PhysicalWriteState {
        &self.before
    }
    /// Complete target population after executing this statement.
    pub fn after(&self) -> &PhysicalWriteState {
        &self.after
    }
    /// Exact logical action counts, not estimated source-row counts.
    pub fn affected(&self) -> WriteRowCounts {
        self.affected
    }
}

/// Result of interpreting a sequence against closed-world physical inputs.
///
/// Feasible carries every materialized intermediate target state. Impossible
/// requires an actual contradiction; insufficient evidence is always Residual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalWriteStatePlan {
    /// Fully realized ordered writes and final source/target snapshots.
    Feasible {
        /// Committed transitions in SQL statement order.
        transitions: Vec<PhysicalWriteTransition>,
        /// Complete final physical populations in stable relation order.
        final_states: Vec<PhysicalWriteState>,
    },
    /// Concrete snapshots violate a necessary write invariant.
    Impossible {
        /// Layer at which a contradiction was proved, if applicable.
        layer_id: Option<String>,
        /// Explanation of the contradiction.
        reason: String,
    },
    /// The program contains an unproved expression, law or data prerequisite.
    Residual {
        /// Layer requiring further evidence, if applicable.
        layer_id: Option<String>,
        /// Explicit missing proof.
        reason: String,
    },
}

#[derive(Debug)]
enum StateFailure {
    Impossible(String),
    Residual(String),
}

type StateResult<T> = Result<T, StateFailure>;

fn residual(reason: impl Into<String>) -> StateFailure {
    StateFailure::Residual(reason.into())
}

fn impossible(reason: impl Into<String>) -> StateFailure {
    StateFailure::Impossible(reason.into())
}

/// Execute supported, independently materialized writes against complete
/// physical source and target snapshots. The returned rows are *constructed*
/// from normalized protocol expressions without reparsing SQL.
///
/// All initial physical relations must have authoritative typed schema
/// evidence. Reads use the state after preceding writes, never independent
/// assignments to intermediate producers. Unsupported DDL, functions,
/// transactions, unknown comparisons and unverified constraints stay residual.
pub fn physical_ordered_write_state_plan(
    bundle: &AnalysisBundle,
    initial_states: &[PhysicalWriteState],
) -> PhysicalWriteStatePlan {
    let mut states = BTreeMap::<String, PhysicalWriteState>::new();
    for state in initial_states {
        if states
            .insert(state.relation.clone(), state.clone())
            .is_some()
        {
            return PhysicalWriteStatePlan::Impossible {
                layer_id: None,
                reason: "a physical relation has more than one initial population".to_string(),
            };
        }
        if let Err(error) = validate_state(bundle, state) {
            return failure(None, error);
        }
    }
    // An unsupported statement cannot disappear from a multi-statement
    // program simply because the bundle has no TransformationLayer for it.
    if bundle.inputs().iter().any(|input| {
        input
            .statements()
            .iter()
            .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_)))
    }) {
        return PhysicalWriteStatePlan::Residual {
            layer_id: None,
            reason: "the program contains an unsupported SQL statement or DDL/transaction law"
                .to_string(),
        };
    }
    if let Err(error) = validate_all_constraints(bundle, &states) {
        return failure(None, error);
    }
    let mut transitions = Vec::new();
    for layer in bundle.layers() {
        let Some(kind) = layer.write_kind() else {
            continue;
        };
        let Some(query) = bundle
            .inputs()
            .iter()
            .find(|input| input.id() == layer.input_id())
            .and_then(|input| input.statements().get(layer.statement_index()))
            .and_then(|statement| match statement {
                ProtocolStatement::Query(query) => Some(query),
                ProtocolStatement::Unsupported(_) => None,
            })
        else {
            return failure(
                Some(layer.id()),
                residual("missing canonical write statement"),
            );
        };
        let Some(write) = query.write() else {
            return failure(
                Some(layer.id()),
                residual("missing normalized write operation"),
            );
        };
        let target = write.target();
        let before = states.get(target).cloned();
        let result = match kind {
            WriteKind::Definition => define_rows(bundle, query, &states, target, before.as_ref()),
            WriteKind::Append => append_rows(bundle, query, &states, target, before.as_ref()),
            WriteKind::Update => {
                mutate_rows(bundle, query, &states, target, before.as_ref(), false)
            }
            WriteKind::Delete => mutate_rows(bundle, query, &states, target, before.as_ref(), true),
            WriteKind::ConditionalMutation => {
                merge_rows(bundle, query, &states, target, before.as_ref())
            }
        };
        let (after, counts) = match result {
            Ok(value) => value,
            Err(error) => return failure(Some(layer.id()), error),
        };
        let effect = write.state_effect();
        let old_count = before.as_ref().map_or(0, |state| state.rows.len());
        let Ok(old_count) = u64::try_from(old_count) else {
            return failure(Some(layer.id()), residual("initial row count overflow"));
        };
        if let Some(effect) = effect {
            let Ok(expected) = effect.resulting_rows(old_count, counts) else {
                return failure(
                    Some(layer.id()),
                    impossible("write action violates cardinality law"),
                );
            };
            if usize::try_from(expected).ok() != Some(after.rows.len()) {
                return failure(
                    Some(layer.id()),
                    impossible("constructed rows contradict DML count law"),
                );
            }
        } else if kind != WriteKind::Definition {
            return failure(
                Some(layer.id()),
                residual("missing canonical write count law"),
            );
        }
        if let Err(error) = validate_state(bundle, &after) {
            return failure(Some(layer.id()), error);
        }
        let before = before.unwrap_or_else(|| PhysicalWriteState {
            relation: target.to_string(),
            rows: Vec::new(),
        });
        states.insert(target.to_string(), after.clone());
        if let Err(error) = validate_all_constraints(bundle, &states) {
            return failure(Some(layer.id()), error);
        }
        transitions.push(PhysicalWriteTransition {
            layer_id: layer.id().to_string(),
            kind,
            before,
            after,
            affected: counts,
        });
    }
    PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states: states.into_values().collect(),
    }
}

fn failure(layer_id: Option<&str>, error: StateFailure) -> PhysicalWriteStatePlan {
    let layer_id = layer_id.map(str::to_string);
    match error {
        StateFailure::Impossible(reason) => PhysicalWriteStatePlan::Impossible { layer_id, reason },
        StateFailure::Residual(reason) => PhysicalWriteStatePlan::Residual { layer_id, reason },
    }
}

fn validate_state(bundle: &AnalysisBundle, state: &PhysicalWriteState) -> StateResult<()> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == state.relation)
        .ok_or_else(|| residual(format!("missing complete schema for {}", state.relation)))?;
    for row in &state.rows {
        if row.values.len() != schema.columns().len() {
            return Err(impossible(
                "physical row column count contradicts declared schema",
            ));
        }
        for column in schema.columns() {
            let value = row
                .values
                .get(column.name())
                .ok_or_else(|| impossible("physical row omits a declared column"))?;
            if !value_conforms(value, column.data_type()) {
                return Err(residual(format!(
                    "unproved physical value/type compatibility: {}.{}",
                    state.relation,
                    column.name()
                )));
            }
        }
    }
    Ok(())
}

fn validate_all_constraints(
    bundle: &AnalysisBundle,
    states: &BTreeMap<String, PhysicalWriteState>,
) -> StateResult<()> {
    for set in bundle.relation_constraints() {
        let Some(state) = states.get(set.relation()) else {
            continue;
        };
        if !set.diagnostics().is_empty() {
            return Err(residual("unresolved canonical constraint diagnostics"));
        }
        for constraint in set.constraints() {
            if !constraint
                .evidence()
                .iter()
                .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
            {
                return Err(residual("constraint enforcement not attested"));
            }
            match constraint {
                RelationConstraint::NotNull(rule) => {
                    for row in &state.rows {
                        if matches!(row.values.get(rule.column()), Some(LiteralValue::Null)) {
                            return Err(impossible("enforced NOT NULL violated"));
                        }
                    }
                }
                RelationConstraint::PrimaryKey(key) | RelationConstraint::UniqueKey(key) => {
                    for (index, row) in state.rows.iter().enumerate() {
                        if matches!(constraint, RelationConstraint::PrimaryKey(_))
                            && key
                                .columns()
                                .iter()
                                .any(|col| matches!(row.values.get(col), Some(LiteralValue::Null)))
                        {
                            return Err(impossible("enforced PRIMARY KEY cannot contain NULL"));
                        }
                        if key
                            .columns()
                            .iter()
                            .any(|col| matches!(row.values.get(col), Some(LiteralValue::Null)))
                        {
                            continue;
                        }
                        for prior in &state.rows[..index] {
                            if key.columns().iter().any(|col| {
                                matches!(prior.values.get(col), Some(LiteralValue::Null))
                            }) {
                                continue;
                            }
                            if equal_key_rows(row, prior, key.columns(), key.columns())? {
                                return Err(impossible(
                                    "enforced key has duplicate physical values",
                                ));
                            }
                        }
                    }
                }
                RelationConstraint::ForeignKey(key) => {
                    let referenced = states.get(key.referenced_relation()).ok_or_else(|| {
                        residual("foreign-key target population is not controlled")
                    })?;
                    for row in &state.rows {
                        if key
                            .columns()
                            .iter()
                            .any(|col| matches!(row.values.get(col), Some(LiteralValue::Null)))
                        {
                            continue;
                        }
                        let mut found = false;
                        for other in &referenced.rows {
                            if equal_key_rows(row, other, key.columns(), key.referenced_columns())?
                            {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            // Deferrability and statement-end enforcement timing
                            // have not been modeled, so do not call this impossible.
                            return Err(residual("unproved foreign-key reference or deferral"));
                        }
                    }
                }
                RelationConstraint::AcceptedValues(accepted) => {
                    for row in &state.rows {
                        let value = row
                            .values
                            .get(accepted.column())
                            .ok_or_else(|| residual("accepted-values column missing"))?;
                        if *value == LiteralValue::Null {
                            continue;
                        }
                        let mut found = false;
                        for candidate in accepted.values() {
                            if constraint_scalar_equal(value, candidate)? {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return Err(impossible("enforced accepted-values constraint violated"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn equal_key_rows(
    left: &PhysicalWriteRow,
    right: &PhysicalWriteRow,
    left_columns: &[String],
    right_columns: &[String],
) -> StateResult<bool> {
    for (left_name, right_name) in left_columns.iter().zip(right_columns) {
        let a = left
            .values
            .get(left_name)
            .ok_or_else(|| residual("key column missing"))?;
        let b = right
            .values
            .get(right_name)
            .ok_or_else(|| residual("referenced key column missing"))?;
        if !scalar_equality(a, b)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn constraint_scalar_equal(value: &LiteralValue, candidate: &ConstraintValue) -> StateResult<bool> {
    match (value, candidate) {
        (LiteralValue::Boolean(a), ConstraintValue::Boolean(b)) => Ok(a == b),
        (LiteralValue::Text(a), ConstraintValue::String(b)) => {
            if a == b {
                Ok(true)
            } else {
                Err(residual("accepted string comparison needs collation proof"))
            }
        }
        (LiteralValue::Number(a), ConstraintValue::Integer(b)) => a
            .parse::<i128>()
            .map(|a| a == i128::from(*b))
            .map_err(|_| residual("unsupported numeric constraint comparison")),
        (LiteralValue::Number(a), ConstraintValue::UnsignedInteger(b)) => a
            .parse::<u128>()
            .map(|a| a == u128::from(*b))
            .map_err(|_| residual("unsupported unsigned constraint comparison")),
        (LiteralValue::Number(a), ConstraintValue::Number(b)) => {
            if a == b {
                Ok(true)
            } else {
                Err(residual("unproved decimal accepted-value equality"))
            }
        }
        (LiteralValue::Null, ConstraintValue::Null) => Ok(true),
        _ => Err(residual("unproved accepted-value coercion")),
    }
}

fn value_conforms(value: &LiteralValue, kind: &DataType) -> bool {
    match (value, kind) {
        (LiteralValue::Null, _) => true,
        (value, DataType::Nullable(inner)) => value_conforms(value, inner),
        (LiteralValue::Boolean(_), DataType::Boolean) => true,
        (LiteralValue::Number(number), DataType::SignedInteger { bits }) => {
            let Ok(value) = number.parse::<i128>() else {
                return false;
            };
            match bits {
                Some(16) => i16::try_from(value).is_ok(),
                Some(32) => i32::try_from(value).is_ok(),
                Some(64) | None => i64::try_from(value).is_ok(),
                _ => false,
            }
        }
        (LiteralValue::Number(number), DataType::UnsignedInteger { bits }) => {
            let Ok(value) = number.parse::<u128>() else {
                return false;
            };
            match bits {
                Some(16) => u16::try_from(value).is_ok(),
                Some(32) => u32::try_from(value).is_ok(),
                Some(64) | None => u64::try_from(value).is_ok(),
                _ => false,
            }
        }
        (LiteralValue::Text(_), DataType::String { fixed: false, .. }) => true,
        _ => false,
    }
}

fn initial_target<'a>(
    target: &str,
    before: Option<&'a PhysicalWriteState>,
) -> StateResult<&'a PhysicalWriteState> {
    before.ok_or_else(|| residual(format!("missing complete initial target state: {target}")))
}

fn define_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let write = query
        .write()
        .ok_or_else(|| residual("missing definition operation"))?;
    match write.definition_mode() {
        Some(WriteDefinitionMode::CreateTable) if before.is_some() => {
            return Err(impossible(
                "CREATE TABLE conflicts with a prior physical target",
            ));
        }
        Some(WriteDefinitionMode::CreateTable | WriteDefinitionMode::ReplaceTable) => {}
        Some(WriteDefinitionMode::CreateView | WriteDefinitionMode::ReplaceView) => {
            return Err(residual(
                "logical CREATE VIEW is not a writable physical table",
            ));
        }
        None => return Err(residual("missing CREATE/REPLACE lifecycle evidence")),
    }
    let schema = schema_for(bundle, target)?;
    let projected = evaluate_simple_select(bundle, query, states)?;
    let columns = query
        .output()
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect::<Vec<_>>();
    if columns.len() != schema.columns().len()
        || columns.iter().collect::<BTreeSet<_>>().len() != columns.len()
        || schema
            .columns()
            .iter()
            .any(|c| !columns.contains(&c.name().to_string()))
    {
        return Err(residual(
            "definition projection does not determine all target columns",
        ));
    }
    let rows = projected
        .into_iter()
        .map(|values| {
            if values.len() != columns.len() {
                return Err(residual("invalid definition projection arity"));
            }
            Ok(PhysicalWriteRow {
                values: columns.iter().cloned().zip(values).collect(),
            })
        })
        .collect::<StateResult<Vec<_>>>()?;
    let before_rows = before.map_or(0, |state| state.rows.len());
    let deleted = u64::try_from(before_rows).map_err(|_| residual("replaced table overflow"))?;
    let inserted = u64::try_from(rows.len()).map_err(|_| residual("definition result overflow"))?;
    Ok((
        PhysicalWriteState {
            relation: target.to_string(),
            rows,
        },
        WriteRowCounts::new(inserted, 0, deleted),
    ))
}

fn append_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let Some(write) = query.write() else {
        return Err(residual("missing INSERT write"));
    };
    let schema = schema_for(bundle, target)?;
    let columns = if write.target_columns().is_empty() {
        schema
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect::<Vec<_>>()
    } else {
        write.target_columns().to_vec()
    };
    // Defaults and generated columns require independent canonical evidence.
    if columns.len() != schema.columns().len()
        || columns.iter().collect::<BTreeSet<_>>().len() != columns.len()
        || schema
            .columns()
            .iter()
            .any(|c| !columns.contains(&c.name().to_string()))
    {
        return Err(residual(
            "implicit/default/partial INSERT target columns not proved",
        ));
    }
    let produced = evaluate_simple_select(bundle, query, states)?;
    let mut next = before.clone();
    for output_row in produced {
        if output_row.len() != columns.len() {
            return Err(residual("INSERT SELECT projection arity is not proved"));
        }
        let values = columns.iter().cloned().zip(output_row).collect();
        next.rows.push(PhysicalWriteRow { values });
    }
    let inserted = u64::try_from(next.rows.len() - before.rows.len())
        .map_err(|_| residual("inserted count overflow"))?;
    Ok((next, WriteRowCounts::new(inserted, 0, 0)))
}

fn mutate_rows(
    _bundle: &AnalysisBundle,
    query: &QueryStatement,
    _states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
    delete: bool,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let write = query
        .write()
        .ok_or_else(|| residual("missing mutation write"))?;
    let mut kept = Vec::new();
    let mut affected = 0_u64;
    for row in &before.rows {
        let context = Context {
            target: Some((target, row)),
            target_alias: write.target_alias(),
            source: None,
        };
        if !selected(write.selection(), &context, &[])? {
            kept.push(row.clone());
            continue;
        }
        affected = affected
            .checked_add(1)
            .ok_or_else(|| residual("affected count overflow"))?;
        if !delete {
            kept.push(apply_assignments(row, write.assignments(), &context, &[])?);
        }
    }
    let counts = if delete {
        WriteRowCounts::new(0, 0, affected)
    } else {
        WriteRowCounts::new(0, affected, 0)
    };
    Ok((
        PhysicalWriteState {
            relation: target.to_string(),
            rows: kept,
        },
        counts,
    ))
}

fn merge_rows(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
    target: &str,
    before: Option<&PhysicalWriteState>,
) -> StateResult<(PhysicalWriteState, WriteRowCounts)> {
    let before = initial_target(target, before)?;
    let write = query
        .write()
        .ok_or_else(|| residual("missing MERGE write"))?;
    let Some(match_condition) = write.match_condition() else {
        return Err(residual("missing MERGE match condition"));
    };
    let [source] = query.sources() else {
        return Err(residual("MERGE must have one independent physical source"));
    };
    if source.name() == target {
        return Err(residual(
            "MERGE self-source needs joint physical alias proof",
        ));
    }
    schema_for(bundle, source.name())?;
    let source_rows = states
        .get(source.name())
        .ok_or_else(|| residual("missing complete MERGE source population"))?;
    let aliases = [source.alias().unwrap_or(source.name()), source.name()];
    let mut matched_sources = BTreeSet::new();
    let mut updates = 0_u64;
    let mut deletes = 0_u64;
    let mut inserts = Vec::new();
    let mut retained = Vec::new();
    for target_row in &before.rows {
        let matches = source_rows
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, source_row)| {
                let context = Context {
                    target: Some((target, target_row)),
                    target_alias: write.target_alias(),
                    source: Some((source.name(), source_row)),
                };
                match selected(Some(match_condition), &context, &aliases) {
                    Ok(true) => Some(Ok(index)),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<StateResult<Vec<_>>>()?;
        if matches.len() > 1 {
            return Err(residual(
                "multiple MERGE source matches require deterministic branch/multiplicity law",
            ));
        }
        let mut result = Some(target_row.clone());
        if let Some(index) = matches.first().copied() {
            matched_sources.insert(index);
            let source_row = &source_rows.rows[index];
            let context = Context {
                target: Some((target, target_row)),
                target_alias: write.target_alias(),
                source: Some((source.name(), source_row)),
            };
            for clause in write.merge_clauses() {
                if clause.match_kind() != MergeMatchKind::Matched
                    || !selected(clause.predicate(), &context, &aliases)?
                {
                    continue;
                }
                match clause.action() {
                    MergeAction::Update { assignments } => {
                        result = Some(apply_assignments(
                            target_row,
                            assignments,
                            &context,
                            &aliases,
                        )?);
                        updates += 1;
                    }
                    MergeAction::Delete => {
                        result = None;
                        deletes += 1;
                    }
                    MergeAction::Insert { .. } | MergeAction::Unsupported(_) => {
                        return Err(residual("unproved MATCHED MERGE action"));
                    }
                }
                break; // SQL's first-applicable branch, not independent actions.
            }
        } else {
            let context = Context {
                target: Some((target, target_row)),
                target_alias: write.target_alias(),
                source: None,
            };
            for clause in write.merge_clauses() {
                if clause.match_kind() != MergeMatchKind::NotMatchedBySource
                    || !selected(clause.predicate(), &context, &aliases)?
                {
                    continue;
                }
                match clause.action() {
                    MergeAction::Update { assignments } => {
                        result = Some(apply_assignments(
                            target_row,
                            assignments,
                            &context,
                            &aliases,
                        )?);
                        updates += 1;
                    }
                    MergeAction::Delete => {
                        result = None;
                        deletes += 1;
                    }
                    _ => return Err(residual("unproved BY SOURCE MERGE action")),
                }
                break;
            }
        }
        if let Some(row) = result {
            retained.push(row);
        }
    }
    for (index, source_row) in source_rows.rows.iter().enumerate() {
        if matched_sources.contains(&index) {
            continue;
        }
        let context = Context {
            target: None,
            target_alias: write.target_alias(),
            source: Some((source.name(), source_row)),
        };
        for clause in write.merge_clauses() {
            if !matches!(
                clause.match_kind(),
                MergeMatchKind::NotMatched | MergeMatchKind::NotMatchedByTarget
            ) || !selected(clause.predicate(), &context, &aliases)?
            {
                continue;
            }
            match clause.action() {
                MergeAction::Insert { columns, values } => {
                    let schema = schema_for(bundle, target)?;
                    if columns.len() != schema.columns().len() || values.len() != 1 {
                        return Err(residual(
                            "MERGE INSERT requires complete target-column tuple",
                        ));
                    }
                    let mut row_values = BTreeMap::new();
                    for (column, value) in columns.iter().zip(&values[0]) {
                        if row_values
                            .insert(
                                column.clone(),
                                eval(value.expression(), &context, &aliases)?,
                            )
                            .is_some()
                        {
                            return Err(impossible("duplicate MERGE target column"));
                        }
                    }
                    inserts.push(PhysicalWriteRow { values: row_values });
                }
                _ => return Err(residual("unproved NOT MATCHED MERGE action")),
            }
            break;
        }
    }
    let inserted = u64::try_from(inserts.len()).map_err(|_| residual("inserted count overflow"))?;
    retained.extend(inserts);
    Ok((
        PhysicalWriteState {
            relation: target.to_string(),
            rows: retained,
        },
        WriteRowCounts::new(inserted, updates, deletes),
    ))
}

fn schema_for<'a>(bundle: &'a AnalysisBundle, relation: &str) -> StateResult<&'a RelationSchema> {
    bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == relation)
        .ok_or_else(|| residual(format!("missing schema for {relation}")))
}

fn evaluate_simple_select(
    _bundle: &AnalysisBundle,
    query: &QueryStatement,
    states: &BTreeMap<String, PhysicalWriteState>,
) -> StateResult<Vec<Vec<LiteralValue>>> {
    if !query.filter_only_row_shape()
        || query.set_operation().is_some()
        || query.aggregation().is_some()
        || !query.joins().is_empty()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
    {
        return Err(residual(
            "INSERT source is not a proven row-local, one-source SELECT",
        ));
    }
    let [source] = query.sources() else {
        return Err(residual("INSERT SELECT must read one physical relation"));
    };
    let rows = states
        .get(source.name())
        .ok_or_else(|| residual("missing complete INSERT source population"))?;
    let aliases = [source.alias().unwrap_or(source.name()), source.name()];
    let mut result = Vec::new();
    for row in &rows.rows {
        let context = Context {
            target: None,
            target_alias: None,
            source: Some((source.name(), row)),
        };
        if !selected(query.predicates().where_predicate(), &context, &aliases)? {
            continue;
        }
        let values = query
            .output()
            .columns()
            .iter()
            .map(|column| eval(column.expression(), &context, &aliases))
            .collect::<StateResult<Vec<_>>>()?;
        result.push(values);
    }
    Ok(result)
}

struct Context<'a> {
    target: Option<(&'a str, &'a PhysicalWriteRow)>,
    target_alias: Option<&'a str>,
    source: Option<(&'a str, &'a PhysicalWriteRow)>,
}

fn column<'a>(
    context: &'a Context<'_>,
    aliases: &[&str],
    relation: Option<&str>,
    name: &str,
) -> StateResult<&'a LiteralValue> {
    match relation {
        Some(qualifier) => {
            if let Some((source, row)) = context.source {
                if qualifier == source || aliases.contains(&qualifier) {
                    return row
                        .values
                        .get(name)
                        .ok_or_else(|| residual("source column missing"));
                }
            }
            if let Some((target, row)) = context.target {
                if qualifier == target || context.target_alias == Some(qualifier) {
                    return row
                        .values
                        .get(name)
                        .ok_or_else(|| residual("target column missing"));
                }
            }
            Err(residual(
                "column qualifier/target alias not canonically resolved",
            ))
        }
        None => {
            let target = context.target.and_then(|(_, row)| row.values.get(name));
            let source = context.source.and_then(|(_, row)| row.values.get(name));
            match (target, source) {
                (Some(_), Some(_)) => Err(residual("ambiguous unqualified source/target column")),
                (Some(value), None) | (None, Some(value)) => Ok(value),
                (None, None) => Err(residual("column missing from complete source snapshots")),
            }
        }
    }
}

fn eval(
    expression: &Expression,
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<LiteralValue> {
    match expression {
        Expression::Literal(literal) => Ok(literal.value().clone()),
        Expression::Column(col) => {
            Ok(column(context, aliases, col.relation(), col.name())?.clone())
        }
        Expression::Unary(unary) => {
            let value = eval(unary.operand(), context, aliases)?;
            match (unary.operator(), value) {
                (_, LiteralValue::Null) => Ok(LiteralValue::Null),
                (UnaryOperator::Plus, LiteralValue::Number(value)) => {
                    Ok(LiteralValue::Number(value))
                }
                (UnaryOperator::Minus, LiteralValue::Number(value)) => {
                    let number = value
                        .parse::<i128>()
                        .map_err(|_| residual("noninteger unary negation"))?;
                    Ok(LiteralValue::Number(
                        number
                            .checked_neg()
                            .ok_or_else(|| residual("numeric overflow"))?
                            .to_string(),
                    ))
                }
                _ => Err(residual("unproved unary scalar semantics")),
            }
        }
        Expression::Binary(binary) => {
            let left = eval(binary.left(), context, aliases)?;
            let right = eval(binary.right(), context, aliases)?;
            if left == LiteralValue::Null || right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            let (LiteralValue::Number(left), LiteralValue::Number(right)) = (left, right) else {
                return Err(residual("binary expression requires proved numeric types"));
            };
            let a = left
                .parse::<i128>()
                .map_err(|_| residual("noninteger arithmetic"))?;
            let b = right
                .parse::<i128>()
                .map_err(|_| residual("noninteger arithmetic"))?;
            let result = match binary.operator() {
                BinaryOperator::Add => a.checked_add(b),
                BinaryOperator::Subtract => a.checked_sub(b),
                BinaryOperator::Multiply => a.checked_mul(b),
                BinaryOperator::Divide | BinaryOperator::Modulo if b == 0 => {
                    return Err(residual("division by zero"))
                }
                BinaryOperator::Divide => a.checked_div(b),
                BinaryOperator::Modulo => a.checked_rem(b),
                _ => return Err(residual("unproved binary expression law")),
            }
            .ok_or_else(|| residual("numeric overflow"))?;
            Ok(LiteralValue::Number(result.to_string()))
        }
        Expression::BooleanPredicate(predicate) => {
            match predicate_truth(predicate, context, aliases)? {
                Truth::True => Ok(LiteralValue::Boolean(true)),
                Truth::False => Ok(LiteralValue::Boolean(false)),
                Truth::Unknown => Ok(LiteralValue::Null),
            }
        }
        _ => Err(residual(
            "unsupported scalar expression cannot be physically evaluated",
        )),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

fn selected(
    predicate: Option<&Predicate>,
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<bool> {
    predicate.map_or(Ok(true), |predicate| {
        Ok(predicate_truth(predicate, context, aliases)? == Truth::True)
    })
}

fn predicate_truth(
    predicate: &Predicate,
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<Truth> {
    match predicate {
        Predicate::Comparison(comparison) => {
            let a = eval(comparison.left(), context, aliases)?;
            let b = eval(comparison.right(), context, aliases)?;
            compare(&a, comparison.operator(), &b)
        }
        Predicate::IsNull(test) => {
            let value = eval(test.expression(), context, aliases)?;
            Ok(if (value == LiteralValue::Null) != test.negated() {
                Truth::True
            } else {
                Truth::False
            })
        }
        Predicate::And(logical) => {
            let mut truth = Truth::True;
            for predicate in logical.operands() {
                match predicate_truth(predicate, context, aliases)? {
                    Truth::False => truth = Truth::False,
                    Truth::Unknown if truth != Truth::False => truth = Truth::Unknown,
                    Truth::True | Truth::Unknown => {}
                }
            }
            Ok(truth)
        }
        Predicate::Or(logical) => {
            let mut truth = Truth::False;
            for predicate in logical.operands() {
                match predicate_truth(predicate, context, aliases)? {
                    Truth::True => truth = Truth::True,
                    Truth::Unknown if truth != Truth::True => truth = Truth::Unknown,
                    Truth::False | Truth::Unknown => {}
                }
            }
            Ok(truth)
        }
        Predicate::Not(not) => Ok(match predicate_truth(not.operand(), context, aliases)? {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        }),
        Predicate::Between(between) => {
            let value = eval(between.expression(), context, aliases)?;
            let lo = eval(between.lower(), context, aliases)?;
            let hi = eval(between.upper(), context, aliases)?;
            let lower = compare(&value, ComparisonOperator::Gte, &lo)?;
            let upper = compare(&value, ComparisonOperator::Lte, &hi)?;
            let truth = and_truth(lower, upper);
            Ok(if between.negated() {
                negate_truth(truth)
            } else {
                truth
            })
        }
        Predicate::In(test) => {
            let value = eval(test.expression(), context, aliases)?;
            let mut truth = Truth::False;
            for expression in test.values() {
                let candidate = eval(expression, context, aliases)?;
                truth = or_truth(truth, compare(&value, ComparisonOperator::Eq, &candidate)?);
            }
            Ok(if test.negated() {
                negate_truth(truth)
            } else {
                truth
            })
        }
        Predicate::BooleanExpression(expression) => match eval(expression, context, aliases)? {
            LiteralValue::Boolean(true) => Ok(Truth::True),
            LiteralValue::Boolean(false) => Ok(Truth::False),
            LiteralValue::Null => Ok(Truth::Unknown),
            _ => Err(residual("non-boolean predicate expression")),
        },
        _ => Err(residual(
            "unsupported predicate requires operator-local proof",
        )),
    }
}

fn negate_truth(truth: Truth) -> Truth {
    match truth {
        Truth::True => Truth::False,
        Truth::False => Truth::True,
        Truth::Unknown => Truth::Unknown,
    }
}
fn and_truth(a: Truth, b: Truth) -> Truth {
    if a == Truth::False || b == Truth::False {
        Truth::False
    } else if a == Truth::Unknown || b == Truth::Unknown {
        Truth::Unknown
    } else {
        Truth::True
    }
}
fn or_truth(a: Truth, b: Truth) -> Truth {
    if a == Truth::True || b == Truth::True {
        Truth::True
    } else if a == Truth::Unknown || b == Truth::Unknown {
        Truth::Unknown
    } else {
        Truth::False
    }
}

fn compare(a: &LiteralValue, operator: ComparisonOperator, b: &LiteralValue) -> StateResult<Truth> {
    if matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    ) {
        // IS DISTINCT FROM treats NULL = NULL as not distinct.
        let equal = scalar_equality(a, b)?;
        let distinct = !equal;
        let truth = if operator == ComparisonOperator::IsDistinctFrom {
            distinct
        } else {
            !distinct
        };
        return Ok(if truth { Truth::True } else { Truth::False });
    }
    if *a == LiteralValue::Null || *b == LiteralValue::Null {
        return Ok(Truth::Unknown);
    }
    let order = match (a, b) {
        (LiteralValue::Number(left), LiteralValue::Number(right)) => {
            let left = left
                .parse::<i128>()
                .map_err(|_| residual("unproved decimal comparison"))?;
            let right = right
                .parse::<i128>()
                .map_err(|_| residual("unproved decimal comparison"))?;
            left.cmp(&right)
        }
        (LiteralValue::Boolean(left), LiteralValue::Boolean(right)) => left.cmp(right),
        (LiteralValue::Text(_), LiteralValue::Text(_)) => {
            return Err(residual(
                "string comparison requires binary collation and padding proof",
            ));
        }
        _ => return Err(residual("implicit comparison coercion is not proved")),
    };
    let ok = match operator {
        ComparisonOperator::Eq => order.is_eq(),
        ComparisonOperator::Neq => order.is_ne(),
        ComparisonOperator::Lt => order.is_lt(),
        ComparisonOperator::Lte => order.is_le(),
        ComparisonOperator::Gt => order.is_gt(),
        ComparisonOperator::Gte => order.is_ge(),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
            return Err(residual("unexpected distinct comparison boundary"))
        }
    };
    Ok(if ok { Truth::True } else { Truth::False })
}

fn scalar_equality(a: &LiteralValue, b: &LiteralValue) -> StateResult<bool> {
    match (a, b) {
        (LiteralValue::Null, LiteralValue::Null) => Ok(true),
        (LiteralValue::Null, _) | (_, LiteralValue::Null) => Ok(false),
        (LiteralValue::Text(_), LiteralValue::Text(_)) => {
            Err(residual("unverified text collation"))
        }
        (LiteralValue::Number(_), LiteralValue::Number(_))
        | (LiteralValue::Boolean(_), LiteralValue::Boolean(_)) => {
            Ok(compare(a, ComparisonOperator::Eq, b)? == Truth::True)
        }
        _ => Err(residual("unproved coercion in distinct comparison")),
    }
}

fn apply_assignments(
    before: &PhysicalWriteRow,
    assignments: &[crate::protocol::MergeAssignment],
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<PhysicalWriteRow> {
    let mut next = before.clone();
    let mut written = BTreeSet::new();
    for assignment in assignments {
        let target = assignment.target();
        let name = if let Some((qualifier, column)) = target.rsplit_once('.') {
            let Some((canonical, _)) = context.target else {
                return Err(residual("update assignment has no target"));
            };
            if qualifier != canonical && context.target_alias != Some(qualifier) {
                return Err(residual("unresolved target column alias"));
            }
            column
        } else {
            target
        };
        if !before.values.contains_key(name) {
            return Err(residual("assignment targets an unknown physical column"));
        }
        if !written.insert(name) {
            return Err(residual("overlapping UPDATE assignments"));
        }
        let value = eval(assignment.value().expression(), context, aliases)?;
        // SQL evaluates all SET expressions against the original row, not
        // the previous SET value in the same statement.
        next.values.insert(name.to_string(), value);
    }
    Ok(next)
}
