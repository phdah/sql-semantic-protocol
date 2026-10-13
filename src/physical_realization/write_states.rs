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

/// An exact closed-world column value domain, including SQL NULL and bag multiplicity.
///
/// Values are in first-occurrence row order. A domain with no values is
/// exactly empty, not an unbounded open-world approximation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalWriteColumnDomain {
    column: String,
    values: Vec<(LiteralValue, u64)>,
}

impl PhysicalWriteColumnDomain {
    /// Column name from the attested physical schema.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Exact frequency of each scalar value in the complete physical bag.
    pub fn values(&self) -> &[(LiteralValue, u64)] {
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

    /// Derive exact per-column domains from a complete schema-backed population.
    ///
    /// Returns None if the schema does not identify this relation or if any
    /// physical row is missing/exceeding the declared set of columns. This
    /// method does not replace datatype or constraint validation.
    pub fn exact_domains(&self, schema: &RelationSchema) -> Option<Vec<PhysicalWriteColumnDomain>> {
        if schema.relation() != self.relation {
            return None;
        }
        let mut domains = schema
            .columns()
            .iter()
            .map(|column| PhysicalWriteColumnDomain {
                column: column.name().to_string(),
                values: Vec::new(),
            })
            .collect::<Vec<_>>();
        for row in &self.rows {
            if row.values.len() != domains.len() {
                return None;
            }
            for domain in &mut domains {
                let value = row.values.get(&domain.column)?;
                if let Some((_, count)) = domain.values.iter_mut().find(|(v, _)| v == value) {
                    *count = count.checked_add(1)?;
                } else {
                    domain.values.push((value.clone(), 1));
                }
            }
        }
        Some(domains)
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

/// Why an ordered source-state construction cannot yet be certified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalWriteStateGap {
    /// Physical input/target population was not supplied as a complete snapshot.
    MissingPopulation,
    /// Required typed schema evidence was not available.
    MissingSchema,
    /// Operator-local scalar, predicate, DML, or lifecycle proof is incomplete.
    UnprovedOperation,
    /// Authoritative physical uniqueness, nullability, default, or FK law is missing.
    UnprovedConstraint,
    /// Unsupported SQL, DDL, or transaction/session semantics.
    UnsupportedStatement,
    /// Source/target alias, multiplicity or shared relation identity is unresolved.
    UnresolvedSourceIdentity,
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
        /// Canonical category of missing evidence.
        gap: PhysicalWriteStateGap,
        /// Explicit missing proof.
        reason: String,
    },
}

#[derive(Debug)]
enum StateFailure {
    Impossible(String),
    Residual(PhysicalWriteStateGap, String),
}

type StateResult<T> = Result<T, StateFailure>;

fn residual(reason: impl Into<String>) -> StateFailure {
    residual_gap(PhysicalWriteStateGap::UnprovedOperation, reason)
}

fn residual_gap(gap: PhysicalWriteStateGap, reason: impl Into<String>) -> StateFailure {
    StateFailure::Residual(gap, reason.into())
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
            gap: PhysicalWriteStateGap::UnsupportedStatement,
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
        StateFailure::Residual(gap, reason) => PhysicalWriteStatePlan::Residual {
            layer_id,
            gap,
            reason,
        },
    }
}

mod constraints;
mod evaluator;
mod operators;
mod projection;
mod type_validation;

use constraints::{validate_all_constraints, validate_state};
use operators::{append_rows, define_rows, merge_rows, mutate_rows};
use type_validation::validate_value;
