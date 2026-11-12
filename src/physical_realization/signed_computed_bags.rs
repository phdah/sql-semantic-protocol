//! Exact finite source-bag proof for bounded signed addition and ABS filters.
//!
//! Parser-independent, canonical expressions only. Full caller-attested source
//! populations and declared signed widths are required; arithmetic that may
//! overflow the declared width is unproved, not silently widened by DuckDB.

use std::collections::BTreeMap;

use super::exists_bags::{output_domain, scalar, PhysicalExistsColumnDomain};
use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::data_type::DataType;
use crate::protocol::{BinaryOperator, ComparisonOperator, LiteralValue};

/// One complete source-row occurrence partition. Duplicate values are retained
/// as distinct ordinals in the original complete physical population.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalSignedComputedExact {
    /// Physical relation whose whole population was inspected.
    pub source: String,
    /// Original source-row positions where the predicate is SQL TRUE.
    pub selected_ordinals: Vec<usize>,
    /// Original source-row positions where the predicate is SQL NOT TRUE.
    pub rejected_ordinals: Vec<usize>,
    /// Separate SQL UNKNOWN occurrences, a subset of rejected ordinals.
    pub unknown_ordinals: Vec<usize>,
    /// Complete terminal projected bag, including duplicate and NULL values.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Precise typed, NULL-aware output domain for the complete terminal bag.
    pub terminal_domain: PhysicalExistsColumnDomain,
}

/// Finite proof, contradictory source evidence, or explicitly missing law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalSignedComputedProof {
    /// Complete schema-backed source and overflow-safe row selection.
    Exact(PhysicalSignedComputedExact),
    /// The provided complete population contradicts the attested schema.
    Impossible { reason: String },
    /// Query shape, datatype width or arithmetic safety is unproved.
    Residual { reason: String },
}

fn residual(reason: &str) -> PhysicalSignedComputedProof {
    PhysicalSignedComputedProof::Residual {
        reason: reason.to_string(),
    }
}

fn integer_literal(expression: &Expression) -> Option<i128> {
    let Expression::Literal(literal) = expression else {
        return None;
    };
    match literal.value() {
        LiteralValue::Number(number)
            if literal.literal_type() == crate::protocol::LiteralType::Integer =>
        {
            number.parse::<i128>().ok()
        }
        _ => None,
    }
}

fn source_column<'a>(
    expression: &'a Expression,
    source_name: &str,
    source_alias: Option<&str>,
) -> Option<&'a str> {
    let Expression::Column(column) = expression else {
        return None;
    };
    if column
        .relation()
        .is_some_and(|relation| relation != source_name && Some(relation) != source_alias)
    {
        return None;
    }
    Some(column.name())
}

/// Only exact signed addition of one source column and one integer constant,
/// or ABS of one source column, can be evaluated. No other function or nested
/// expression inherits this safety proof.
fn computed_operand<'a>(
    expression: &'a Expression,
    relation: &str,
    alias: Option<&str>,
) -> Option<(&'a str, SignedComputation)> {
    match expression {
        Expression::Binary(binary) if binary.operator() == BinaryOperator::Add => {
            if let (Some(column), Some(literal)) = (
                source_column(binary.left(), relation, alias),
                integer_literal(binary.right()),
            ) {
                return Some((column, SignedComputation::Add(literal)));
            }
            let (Some(literal), Some(column)) = (
                integer_literal(binary.left()),
                source_column(binary.right(), relation, alias),
            ) else {
                return None;
            };
            Some((column, SignedComputation::Add(literal)))
        }
        Expression::Function(function)
            if function.name().eq_ignore_ascii_case("abs") && !function.distinct() =>
        {
            let [expression] = function.arguments() else {
                return None;
            };
            source_column(expression, relation, alias)
                .map(|column| (column, SignedComputation::Abs))
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum SignedComputation {
    Add(i128),
    Abs,
}

fn signed_width(bundle: &AnalysisBundle, relation: &str, column: &str) -> Option<u16> {
    let column = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == relation)?
        .columns()
        .iter()
        .find(|entry| entry.name() == column)?;
    let mut data_type = column.data_type();
    while let DataType::Nullable(inner) = data_type {
        data_type = inner;
    }
    match data_type {
        DataType::SignedInteger {
            bits: Some(bits @ (16 | 32 | 64)),
        } => Some(*bits),
        _ => None,
    }
}

/// Prove original S027/S032-style computed predicates against a complete
/// typed source population without trusting a particular warehouse to widen
/// overflow. An overflow risk yields Residual, even if a native execution
/// happened to succeed. Unrelated projected columns stay open-world Unbounded
/// in the analyzer; only the *complete supplied bag* has this finite domain.
pub fn physical_finite_signed_computed_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    source: &PhysicalWriteState,
) -> PhysicalSignedComputedProof {
    match physical_closed_world_schema_plan(bundle, std::slice::from_ref(source)) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalSignedComputedProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(&reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    if walker.sources.len() != 1 || !walker.sources.contains(source.relation()) {
        return residual("requires exactly one direct complete physical source");
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing canonical query layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("target is not a canonical SELECT");
    };
    let [relation] = query.sources() else {
        return residual("computed predicate requires one direct source");
    };
    let [output] = query.output().columns() else {
        return residual("requires a single copied output column");
    };
    let Some(projected) = output.plain_copy_source() else {
        return residual("output must be a plain physical source-column copy");
    };
    if !query.filter_only_row_shape()
        || query.aggregation().is_some()
        || !query.joins().is_empty()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.diagnostics().is_empty()
        || relation.name() != source.relation()
        || projected.relation() != source.relation()
    {
        return residual("computed query has an unsupported producer or row shape");
    }
    let Some(Predicate::Comparison(compare)) = query.predicates().where_predicate() else {
        return residual("computed predicate must be a single comparison");
    };
    let (column, operation, limit, op) = if let Some((column, operation)) =
        computed_operand(compare.left(), relation.name(), relation.alias())
    {
        let Some(limit) = integer_literal(compare.right()) else {
            return residual("comparison bound is not a signed integer literal");
        };
        (column, operation, limit, compare.operator())
    } else if let Some((column, operation)) =
        computed_operand(compare.right(), relation.name(), relation.alias())
    {
        let Some(limit) = integer_literal(compare.left()) else {
            return residual("comparison bound is not a signed integer literal");
        };
        (column, operation, limit, compare.operator().reversed())
    } else {
        return residual("arithmetic expression lacks a certified canonical operator");
    };
    let Some(bits) = signed_width(bundle, relation.name(), column) else {
        return residual("source integer storage width is not authoritatively established");
    };
    if signed_width(bundle, relation.name(), projected.column()).is_none() {
        return residual("projected column lacks an authoritative signed integer type");
    }
    let magnitude = 1_i128 << (bits - 1);
    let (minimum, maximum) = (-magnitude, magnitude - 1);
    if !(minimum..=maximum).contains(&limit)
        || matches!(operation, SignedComputation::Add(value) if !(minimum..=maximum).contains(&value))
    {
        return residual("numeric constant exceeds the declared source integer width");
    }

    let mut selected_ordinals = Vec::new();
    let mut rejected_ordinals = Vec::new();
    let mut unknown_ordinals = Vec::new();
    let mut terminal = Vec::new();
    for (ordinal, row) in source.rows().iter().enumerate() {
        let value = match scalar(row, column) {
            Ok(value) => value,
            Err(reason) => return residual(reason),
        };
        let Some(value) = value else {
            rejected_ordinals.push(ordinal);
            unknown_ordinals.push(ordinal);
            continue;
        };
        let computed = match operation {
            SignedComputation::Add(n) => value.checked_add(n),
            SignedComputation::Abs => value.checked_abs(),
        };
        let Some(computed) = computed.filter(|n| *n >= minimum && *n <= maximum) else {
            return residual("signed arithmetic may overflow source width; widening or error is vendor-dependent");
        };
        let true_row = match op {
            ComparisonOperator::Eq => computed == limit,
            ComparisonOperator::Neq => computed != limit,
            ComparisonOperator::Lt => computed < limit,
            ComparisonOperator::Lte => computed <= limit,
            ComparisonOperator::Gt => computed > limit,
            ComparisonOperator::Gte => computed >= limit,
            ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                return residual("NULL-safe comparison is outside the signed arithmetic proof");
            }
        };
        if true_row {
            let Some(out_value) = row.values().get(projected.column()) else {
                return residual("projected source value is absent");
            };
            if !matches!(out_value, LiteralValue::Number(_) | LiteralValue::Null) {
                return residual("projected source value is not a nullable integer");
            }
            terminal.push(BTreeMap::from([(
                output.name().to_string(),
                out_value.clone(),
            )]));
            selected_ordinals.push(ordinal);
        } else {
            rejected_ordinals.push(ordinal);
        }
    }
    let Some(terminal_domain) = output_domain(&terminal, output.name()) else {
        return residual("no exact nullable signed terminal domain");
    };
    PhysicalSignedComputedProof::Exact(PhysicalSignedComputedExact {
        source: source.relation().to_string(),
        selected_ordinals,
        rejected_ordinals,
        unknown_ordinals,
        terminal,
        terminal_domain,
    })
}
