//! Source-complete physical evaluation of bounded, lexically scoped ordinary CTEs.
//!
//! Only a parser-independent CteProgram is consumed. All input bags are complete
//! caller-attested physical populations, never independent CTE inserts.

use std::collections::{BTreeMap, BTreeSet};

use super::write_states::{physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan};
use super::*;
use crate::data_type::DataType;
use crate::protocol::{CteColumnRef, CteInput, CteStage, LiteralValue};

/// Exact proof of each physical leaf, lexical producer and final SQL bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCteExact {
    /// Reference-based physical source graph; local CTE stages are in stage_bags.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Distinct physical leaves, never one insert target per CTE reference.
    pub sources: Vec<String>,
    /// Complete initial source rows with occurrence identities.
    pub source_bags: BTreeMap<String, Vec<PhysicalProducerRow>>,
    /// Every CTE producer and terminal stage with lexical source identities.
    pub stage_bags: BTreeMap<String, Vec<PhysicalProducerRow>>,
    /// Rejected candidate contributor occurrences at each join stage.
    pub rejected: BTreeMap<String, Vec<Vec<String>>>,
    /// Explicit FALSE candidate pairs by stage.
    pub false_pairs: BTreeMap<String, Vec<Vec<String>>>,
    /// Explicit SQL UNKNOWN candidate pairs by stage.
    pub unknown_pairs: BTreeMap<String, Vec<Vec<String>>>,
    /// Complete terminal multiset, preserving duplicates and NULL values.
    pub terminal: Vec<PhysicalProducerRow>,
    /// Finite typed domains computed from the exact same terminal rows.
    pub terminal_domains: BTreeMap<String, PhysicalProducerDomain>,
}

/// CTE result strength, including sound absence proofs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalCteProof {
    /// All physical and producer rows have been constructed from canonical facts.
    Exact(Box<PhysicalCteExact>),
    /// Schema contradiction or a typed requested tuple absent from a complete bag.
    Impossible { reason: String },
    /// Missing/ambiguous evidence or an unsupported operator cannot be certified.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalCteProof {
    PhysicalCteProof::Residual { reason: reason.into() }
}

fn signed_integer(data_type: &DataType) -> bool {
    match data_type {
        DataType::SignedInteger { bits: Some(bits) } => *bits > 0 && *bits <= 128,
        DataType::Nullable(inner) => signed_integer(inner),
        _ => false,
    }
}

fn physical_rows(
    state: &PhysicalWriteState,
) -> Vec<PhysicalProducerRow> {
    state.rows().iter().enumerate().map(|(ordinal, row)| {
        PhysicalProducerRow {
            values: row.values().clone(),
            source_row_ids: vec![format!("{}#{ordinal}", state.relation())],
        }
    }).collect()
}

fn value<'a>(
    reference: &CteColumnRef,
    scope: &'a [(&CteInput, &PhysicalProducerRow)],
) -> Result<&'a LiteralValue, &'static str> {
    let mut matches = scope.iter().filter_map(|(input, row)| {
        if reference.relation().is_some_and(|alias| alias != input.alias()) {
            return None;
        }
        row.values.get(reference.column())
    });
    let first = matches.next().ok_or("source column has no producer")?;
    if matches.next().is_some() {
        return Err("unqualified column has ambiguous source instances");
    }
    Ok(first)
}

fn equal(
    left: &LiteralValue,
    right: &LiteralValue,
) -> Result<Option<bool>, &'static str> {
    match (left, right) {
        (LiteralValue::Null, _) | (_, LiteralValue::Null) => Ok(None),
        (LiteralValue::Number(a), LiteralValue::Number(b)) => {
            let a = a.parse::<i128>().map_err(|_| "unproved numeric CTE equality")?;
            let b = b.parse::<i128>().map_err(|_| "unproved numeric CTE equality")?;
            Ok(Some(a == b))
        }
        _ => Err("CTE equality requires attested integer comparison"),
    }
}

type BagMap = BTreeMap<String, Vec<PhysicalProducerRow>>;
type Rejected = Vec<Vec<String>>;
type StageExecution = (Vec<PhysicalProducerRow>, Rejected, Rejected, Rejected);

fn stage(
    program: &CteStage,
    bags: &BagMap,
    types: &BTreeMap<String, BTreeMap<String, bool>>,
) -> Result<(StageExecution, BTreeMap<String, bool>), &'static str> {
    let [first, others @ ..] = program.inputs() else {
        return Err("CTE needs a physical or lexical input");
    };
    let rows = bags.get(first.relation()).ok_or("CTE input source or producer absent")?;
    let right = match others {
        [] => None,
        [second] => Some((
            second,
            bags.get(second.relation()).ok_or("joined CTE source missing")?
        )),
        _ => return Err("CTE join arity unproved"),
    };
    let mut output_types = BTreeMap::new();
    for projected in program.projections() {
        let candidates = program.inputs().iter().filter(|input| {
            projected.source().relation().is_none_or(|alias| alias == input.alias())
                && types.get(input.relation()).and_then(|m| m.get(projected.source().column())).is_some()
        }).collect::<Vec<_>>();
        let [input] = candidates.as_slice() else {
            return Err("CTE projected column lacks a unique typed producer");
        };
        let is_integer = *types.get(input.relation())
            .and_then(|m| m.get(projected.source().column()))
            .ok_or("CTE projection lacks attested datatype")?;
        if !is_integer {
            return Err("CTE projection needs attested integer columns");
        }
        if output_types.insert(projected.name().to_string(), is_integer).is_some() {
            return Err("duplicate CTE output name");
        }
    }
    if let Some((left, right_key)) = program.equality() {
        for key in [left, right_key] {
            let candidates = program.inputs().iter().filter(|input| {
                key.relation().is_none_or(|alias| alias == input.alias())
                    && types.get(input.relation()).and_then(|m| m.get(key.column())).is_some()
            }).collect::<Vec<_>>();
            let [input] = candidates.as_slice() else {
                return Err("CTE join key lacks a unique typed producer");
            };
            if !types.get(input.relation()).and_then(|m| m.get(key.column())).copied().unwrap_or(false) {
                return Err("CTE equality requires attested integer keys");
            }
        }
    } else if right.is_some() {
        return Err("CTE two-input shape has no normalized equality");
    }

    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    let mut false_pairs = Vec::new();
    let mut unknown_pairs = Vec::new();
    for outer in rows {
        let candidates = right.as_ref().map_or_else(
            || vec![None],
            |(_, rows)| rows.iter().map(Some).collect::<Vec<_>>(),
        );
        for inner in candidates {
            let mut scope = vec![(first, outer)];
            let mut source_ids = outer.source_row_ids.clone();
            if let (Some((input, _)), Some(row)) = (&right, inner) {
                scope.push((*input, row));
                source_ids.extend(row.source_row_ids.iter().cloned());
            }
            if let Some((a, b)) = program.equality() {
                match equal(value(a, &scope)?, value(b, &scope)?)? {
                    Some(true) => {}
                    Some(false) => {
                        false_pairs.push(source_ids.clone());
                        rejected.push(source_ids);
                        continue;
                    }
                    None => {
                        unknown_pairs.push(source_ids.clone());
                        rejected.push(source_ids);
                        continue;
                    }
                }
            }
            let mut values = BTreeMap::new();
            for projection in program.projections() {
                let column = value(projection.source(), &scope)?.clone();
                if values.insert(projection.name().to_string(), column).is_some() {
                    return Err("duplicate CTE output label");
                }
            }
            selected.push(PhysicalProducerRow { values, source_row_ids: source_ids });
        }
    }
    Ok(((selected, rejected, false_pairs, unknown_pairs), output_types))
}

/// Evaluate every producer from complete, schema-validated physical leaves.
///
/// `wanted` requests typed membership. Absence from the fully evaluated
/// terminal is Impossible, while malformed tuples and unproven operators are
/// Residual. A shared CTE is evaluated once, then referenced by both aliases.
pub fn physical_finite_cte_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalCteProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => return PhysicalCteProof::Impossible { reason },
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("missing canonical CTE terminal layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("CTE terminal has no canonical query");
    };
    let Some(program) = query.cte_program() else {
        return residual("CTE shape lacks a certified canonical producer program");
    };
    if walker.sources.len() != snapshots.len() {
        return residual("CTE proof requires every physical source and no extra populations");
    }
    let mut bags = BagMap::new();
    let mut types = BTreeMap::new();
    let mut source_bags = BTreeMap::new();
    for state in snapshots {
        if !walker.sources.contains(state.relation()) || bags.contains_key(state.relation()) {
            return residual("physical source is absent from canonical dependency graph");
        }
        let Some(schema) = bundle.source_schemas().iter().find(|s| s.relation() == state.relation()) else {
            return residual("CTE source lacks canonical typed schema");
        };
        let source_types = schema.columns().iter().map(|col| {
            (col.name().to_string(), signed_integer(col.data_type()))
        }).collect::<BTreeMap<_, _>>();
        let rows = physical_rows(state);
        source_bags.insert(state.relation().to_string(), rows.clone());
        bags.insert(state.relation().to_string(), rows);
        types.insert(state.relation().to_string(), source_types);
    }

    let mut stage_bags = BTreeMap::new();
    let mut rejected = BTreeMap::new();
    let mut false_pairs = BTreeMap::new();
    let mut unknown_pairs = BTreeMap::new();
    for producer in program.stages() {
        if bags.contains_key(producer.id()) {
            return residual("CTE lexical producer collides with existing physical identity");
        }
        let ((rows, failures, falses, unknowns), output_types) = match stage(producer, &bags, &types) {
            Ok(value) => value,
            Err(reason) => return residual(format!("{}: {reason}", producer.id())),
        };
        bags.insert(producer.id().to_string(), rows.clone());
        types.insert(producer.id().to_string(), output_types);
        stage_bags.insert(producer.id().to_string(), rows);
        rejected.insert(producer.id().to_string(), failures);
        false_pairs.insert(producer.id().to_string(), falses);
        unknown_pairs.insert(producer.id().to_string(), unknowns);
    }
    let Some(terminal) = stage_bags.get(program.terminal()).cloned() else {
        return residual("missing CTE terminal output");
    };
    let mut terminal_domains = BTreeMap::new();
    for column in query.output().columns() {
        let Some(domain) = super::exists_producers::complete_domain(&terminal, column.name()) else {
            return residual("CTE final output domain lacks complete values");
        };
        terminal_domains.insert(column.name().to_string(), domain);
    }
    if let Some(wanted) = wanted {
        if wanted.len() != terminal_domains.len()
            || wanted.iter().any(|(column, value)| {
                !terminal_domains.contains_key(column)
                    || !matches!(value, LiteralValue::Null | LiteralValue::Number(_))
                    || matches!(value, LiteralValue::Number(raw) if raw.parse::<i128>().is_err())
            })
        {
            return residual("requested CTE tuple lacks supported typed integer columns");
        }
        if !terminal.iter().any(|row| &row.values == wanted) {
            return PhysicalCteProof::Impossible {
                reason: "requested tuple is absent from complete physical CTE terminal bag".to_string(),
            };
        }
    }
    PhysicalCteProof::Exact(Box::new(PhysicalCteExact {
        nodes: walker.nodes,
        sources: walker.sources.into_iter().collect(),
        source_bags, stage_bags, rejected, false_pairs, unknown_pairs, terminal, terminal_domains
    }))
}
