//! Source-complete physical evaluation of bounded, lexically scoped ordinary CTEs.
//!
//! Only a parser-independent CteProgram is consumed. All input bags are complete
//! caller-attested physical populations, never independent CTE inserts.

use std::collections::{BTreeMap, BTreeSet};

use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::data_type::DataType;
use crate::protocol::{CteColumnRef, CteInput, CteStage, LiteralValue};

/// Typed node reference in one complete lexical CTE execution graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PhysicalCteRef {
    /// A complete, caller-attested physical relation.
    Source(String),
    /// One lexical CTE definition or the terminal SELECT.
    Producer(String),
}

/// One SQL source instance binding to an existing physical or local producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCteInput {
    /// Local SQL alias; distinct even when multiple aliases share one producer.
    pub alias: String,
    /// Canonical input identity, not an independently insertable CTE table.
    pub reference: PhysicalCteRef,
}

/// One producer-first physical or lexical node of the CTE execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCteNode {
    /// Identity of the physical source or lexical producer.
    pub id: PhysicalCteRef,
    /// SQL input instances, which can share one producer.
    pub inputs: Vec<PhysicalCteInput>,
}

/// Exact proof of each physical leaf, lexical producer and final SQL bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCteExact {
    /// Producer-first graph with distinct physical leaves, stages and SQL aliases.
    pub nodes: Vec<PhysicalCteNode>,
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
    PhysicalCteProof::Residual {
        reason: reason.into(),
    }
}

fn signed_integer(data_type: &DataType) -> bool {
    match data_type {
        DataType::SignedInteger { bits: Some(bits) } => *bits > 0 && *bits <= 128,
        DataType::Nullable(inner) => signed_integer(inner),
        _ => false,
    }
}

fn physical_rows(state: &PhysicalWriteState) -> Vec<PhysicalProducerRow> {
    state
        .rows()
        .iter()
        .enumerate()
        .map(|(ordinal, row)| PhysicalProducerRow {
            values: row.values().clone(),
            source_row_ids: vec![format!("{}#{ordinal}", state.relation())],
        })
        .collect()
}

fn value<'a>(
    reference: &CteColumnRef,
    scope: &'a [(&CteInput, &PhysicalProducerRow)],
) -> Result<&'a LiteralValue, &'static str> {
    let mut matches = scope.iter().filter_map(|(input, row)| {
        if reference
            .relation()
            .is_some_and(|alias| alias != input.alias())
        {
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

fn equal(left: &LiteralValue, right: &LiteralValue) -> Result<Option<bool>, &'static str> {
    match (left, right) {
        (LiteralValue::Null, _) | (_, LiteralValue::Null) => Ok(None),
        (LiteralValue::Number(a), LiteralValue::Number(b)) => {
            let a = a
                .parse::<i128>()
                .map_err(|_| "unproved numeric CTE equality")?;
            let b = b
                .parse::<i128>()
                .map_err(|_| "unproved numeric CTE equality")?;
            Ok(Some(a == b))
        }
        _ => Err("CTE equality requires attested integer comparison"),
    }
}

type BagMap = BTreeMap<String, Vec<PhysicalProducerRow>>;

/// One fully evaluated lexical producer, keeping each candidate direction distinct.
struct StageExecution {
    selected: Vec<PhysicalProducerRow>,
    rejected: Vec<Vec<String>>,
    false_pairs: Vec<Vec<String>>,
    unknown_pairs: Vec<Vec<String>>,
    output_types: BTreeMap<String, bool>,
}

fn recursive_stage(program: &CteStage) -> Result<StageExecution, &'static str> {
    let recursion = program.recursion().ok_or("not a recursive stage")?;
    // The source-free seed and strictly increasing checked step are sufficient
    // for a finite, iteration-by-iteration complete physical proof.
    // Materialization is deliberately bounded; a larger finite result is
    // Residual, never a silently truncated Exact bag.
    let values = recursion.finite_values().ok_or(
        "recursive induction is too large, overflows SQL integer width, or lacks a finite proof",
    )?;
    let selected = values
        .iter()
        .enumerate()
        .map(|(ordinal, value)| PhysicalProducerRow {
            values: BTreeMap::from([(
                recursion.column().to_string(),
                LiteralValue::Number(value.to_string()),
            )]),
            source_row_ids: vec![format!("{}#{ordinal}", program.id())],
        })
        .collect::<Vec<_>>();
    let last_ordinal = values.len().checked_sub(1).ok_or("empty recursive seed")?;
    let stopped = format!("{}#{last_ordinal}", program.id());
    // A positive step gives strictly distinct values, so DISTINCT and ALL
    // construct the same occurrences. The set mode remains explicitly
    // represented on the canonical recursive producer for consumers.
    Ok(StageExecution {
        selected,
        rejected: vec![vec![stopped.clone()]],
        false_pairs: vec![vec![stopped]],
        unknown_pairs: Vec::new(),
        output_types: BTreeMap::from([(recursion.column().to_string(), true)]),
    })
}

fn stage(
    program: &CteStage,
    bags: &BagMap,
    types: &BTreeMap<String, BTreeMap<String, bool>>,
) -> Result<StageExecution, &'static str> {
    if program.recursion().is_some() {
        return recursive_stage(program);
    }
    let [first, others @ ..] = program.inputs() else {
        return Err("CTE needs a physical or lexical input");
    };
    let rows = bags
        .get(first.relation())
        .ok_or("CTE input source or producer absent")?;
    let right = match others {
        [] => None,
        [second] => Some((
            second,
            bags.get(second.relation())
                .ok_or("joined CTE source missing")?,
        )),
        _ => return Err("CTE join arity unproved"),
    };
    let mut output_types = BTreeMap::new();
    for projected in program.projections() {
        let candidates = program
            .inputs()
            .iter()
            .filter(|input| {
                projected
                    .source()
                    .relation()
                    .is_none_or(|alias| alias == input.alias())
                    && types
                        .get(input.relation())
                        .and_then(|m| m.get(projected.source().column()))
                        .is_some()
            })
            .collect::<Vec<_>>();
        let [input] = candidates.as_slice() else {
            return Err("CTE projected column lacks a unique typed producer");
        };
        let is_integer = *types
            .get(input.relation())
            .and_then(|m| m.get(projected.source().column()))
            .ok_or("CTE projection lacks attested datatype")?;
        if !is_integer {
            return Err("CTE projection needs attested integer columns");
        }
        if output_types
            .insert(projected.name().to_string(), is_integer)
            .is_some()
        {
            return Err("duplicate CTE output name");
        }
    }
    if let Some((left, right_key)) = program.equality() {
        for key in [left, right_key] {
            let candidates = program
                .inputs()
                .iter()
                .filter(|input| {
                    key.relation().is_none_or(|alias| alias == input.alias())
                        && types
                            .get(input.relation())
                            .and_then(|m| m.get(key.column()))
                            .is_some()
                })
                .collect::<Vec<_>>();
            let [input] = candidates.as_slice() else {
                return Err("CTE join key lacks a unique typed producer");
            };
            if !types
                .get(input.relation())
                .and_then(|m| m.get(key.column()))
                .copied()
                .unwrap_or(false)
            {
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
                if values
                    .insert(projection.name().to_string(), column)
                    .is_some()
                {
                    return Err("duplicate CTE output label");
                }
            }
            selected.push(PhysicalProducerRow {
                values,
                source_row_ids: source_ids,
            });
        }
    }
    Ok(StageExecution {
        selected,
        rejected,
        false_pairs,
        unknown_pairs,
        output_types,
    })
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
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalCteProof::Impossible { reason }
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let Some(layer) = bundle
        .layers()
        .iter()
        .find(|layer| layer.id() == target_layer_id)
    else {
        return residual("missing canonical CTE terminal layer");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("CTE terminal has no canonical query");
    };
    let Some(program) = query.cte_program() else {
        return residual("CTE shape lacks a certified canonical producer program");
    };
    let Some(final_stage) = program.stages().last() else {
        return residual("empty CTE producer program");
    };
    let projected = final_stage
        .projections()
        .iter()
        .map(|column| column.name())
        .collect::<Vec<_>>();
    let analyzed = query
        .output()
        .columns()
        .iter()
        .map(|column| column.name())
        .collect::<Vec<_>>();
    if final_stage.id() != program.terminal() || projected != analyzed {
        return residual("CTE terminal projection differs from canonical output columns");
    }
    // Include leaves of shadowed but still independently evaluated CTEs.
    // The terminal dependency graph deliberately excludes these unused
    // producers, but their own complete physical bags remain required.
    let stage_ids = program
        .stages()
        .iter()
        .map(|stage| stage.id().to_string())
        .collect::<BTreeSet<_>>();
    let source_names = program
        .stages()
        .iter()
        .flat_map(|stage| stage.inputs())
        .filter(|input| !stage_ids.contains(input.relation()))
        .map(|input| input.relation().to_string())
        .collect::<BTreeSet<_>>();
    if !query
        .dependencies()
        .iter()
        .all(|name| source_names.contains(name))
    {
        return residual("CTE lexical leaves disagree with canonical physical dependencies");
    }
    if source_names.len() != snapshots.len() {
        return residual(
            "CTE proof requires every lexical physical leaf, including unused producers",
        );
    }
    let mut bags = BagMap::new();
    let mut types = BTreeMap::new();
    let mut source_bags = BTreeMap::new();
    for state in snapshots {
        if !source_names.contains(state.relation()) || bags.contains_key(state.relation()) {
            return residual("physical source is absent from canonical dependency graph");
        }
        let Some(schema) = bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation() == state.relation())
        else {
            return residual("CTE source lacks canonical typed schema");
        };
        let source_types = schema
            .columns()
            .iter()
            .map(|col| (col.name().to_string(), signed_integer(col.data_type())))
            .collect::<BTreeMap<_, _>>();
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
        let StageExecution {
            selected: rows,
            rejected: failures,
            false_pairs: falses,
            unknown_pairs: unknowns,
            output_types,
        } = match stage(producer, &bags, &types) {
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
        let Some(domain) = super::exists_producers::complete_domain(&terminal, column.name())
        else {
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
        // SQL integer membership compares numeric values, not their literal
        // spellings. NULL matches NULL for tuple membership (IS NOT DISTINCT
        // FROM), even though a SQL JOIN equality treats NULL as UNKNOWN.
        let present = terminal.iter().any(|row| {
            row.values
                .iter()
                .all(|(name, actual)| match (actual, wanted.get(name)) {
                    (LiteralValue::Null, Some(LiteralValue::Null)) => true,
                    (LiteralValue::Number(a), Some(LiteralValue::Number(b))) => a
                        .parse::<i128>()
                        .ok()
                        .zip(b.parse::<i128>().ok())
                        .is_some_and(|(a, b)| a == b),
                    _ => false,
                })
        });
        if !present {
            return PhysicalCteProof::Impossible {
                reason: "requested tuple is absent from complete physical CTE terminal bag"
                    .to_string(),
            };
        }
    }
    let mut nodes = source_names
        .iter()
        .map(|name| PhysicalCteNode {
            id: PhysicalCteRef::Source(name.clone()),
            inputs: Vec::new(),
        })
        .collect::<Vec<_>>();
    for producer in program.stages() {
        let inputs = producer
            .inputs()
            .iter()
            .map(|input| {
                let reference = if stage_ids.contains(input.relation()) {
                    PhysicalCteRef::Producer(input.relation().to_string())
                } else {
                    PhysicalCteRef::Source(input.relation().to_string())
                };
                PhysicalCteInput {
                    alias: input.alias().to_string(),
                    reference,
                }
            })
            .collect();
        nodes.push(PhysicalCteNode {
            id: PhysicalCteRef::Producer(producer.id().to_string()),
            inputs,
        });
    }
    PhysicalCteProof::Exact(Box::new(PhysicalCteExact {
        nodes,
        sources: source_names.into_iter().collect(),
        source_bags,
        stage_bags,
        rejected,
        false_pairs,
        unknown_pairs,
        terminal,
        terminal_domains,
    }))
}
