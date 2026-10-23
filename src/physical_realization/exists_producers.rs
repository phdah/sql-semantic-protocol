//! Closed-world producer-chain SQL EXISTS proofs from canonical query semantics.
//!
//! Only complete attested physical leaves enter the evaluator. Every intermediate
//! relation is materialized by the protocol's own normalized predicates,
//! projections and join laws; no caller-provided intermediate outputs are trusted.

use std::collections::{BTreeMap, BTreeSet};

use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::protocol::{
    ComparisonOperator, Expression, JoinKind, LiteralValue, Output, Predicate, SourceRelation,
};

/// Complete source identities contributing to one intermediate or terminal row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalProducerRow {
    /// Exact projected SQL values, including duplicates and NULL.
    pub values: BTreeMap<String, LiteralValue>,
    /// Physical leaf identities in source-graph traversal order.
    pub source_row_ids: Vec<String>,
}

/// Exhaustive source-attested scalar domain, including SQL NULL and numeric bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalProducerDomain {
    /// Every distinct scalar value of a complete terminal column.
    pub values: Vec<LiteralValue>,
    /// Inclusive numeric bounds where all non-NULL values are signed integers.
    pub integer_bounds: Option<(i128, i128)>,
    /// Whether NULL occurs in the complete bag.
    pub null_allowed: bool,
}

/// Complete proof of a producer chain ending in correlated EXISTS/NOT EXISTS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalProducerExistsExact {
    /// Canonical physical-source and producer dependency DAG.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Every independently attested physical source relation.
    pub sources: Vec<String>,
    /// Complete intermediate and terminal output bags, keyed by canonical layer.
    pub layer_bags: BTreeMap<String, Vec<PhysicalProducerRow>>,
    /// Every physical identity rejected by a producer's WHERE/EXISTS predicate.
    pub rejected: BTreeMap<String, Vec<Vec<String>>>,
    /// Complete final SQL bag, retaining row and value multiplicities.
    pub terminal: Vec<PhysicalProducerRow>,
    /// Complete output domains derived from exactly the same terminal rows.
    pub terminal_domains: BTreeMap<String, PhysicalProducerDomain>,
}

/// Physical feasibility result for a fully resolved finite producer DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalProducerExistsProof {
    /// Every stage, source identity, comparison and terminal value is proven.
    Exact(Box<PhysicalProducerExistsExact>),
    /// Source rows contradict an authoritative physical schema or stable identity.
    Impossible { reason: String },
    /// A necessary execution law or producer/source mapping is unproved.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalProducerExistsProof {
    PhysicalProducerExistsProof::Residual {
        reason: reason.into(),
    }
}

// SQL object spellings may be adapter-qualified or quoted. A bare suffix is
// usable only after the caller proves it identifies exactly one relation.
fn canonical_tail(name: &str) -> String {
    name.rsplit('.')
        .next()
        .unwrap_or(name)
        .trim_matches('"')
        .trim_matches('`')
        .to_ascii_lowercase()
}

fn same_relation(left: &str, right: &str) -> bool {
    left == right || canonical_tail(left) == canonical_tail(right)
}

fn row_identity(row: &PhysicalWriteRow) -> Option<String> {
    match row.values().get("id")? {
        LiteralValue::Number(value) => Some(value.clone()),
        LiteralValue::Text(value) if !value.is_empty() => Some(value.clone()),
        _ => None,
    }
}

struct InputRow<'a> {
    relation: &'a SourceRelation,
    row: &'a PhysicalProducerRow,
}

fn lookup_column(
    relation: Option<&str>,
    name: &str,
    scope: &[InputRow<'_>],
) -> Result<LiteralValue, &'static str> {
    let choices = scope
        .iter()
        .filter(|source| {
            relation.is_none_or(|label| {
                source.relation.alias() == Some(label)
                    || source.relation.name() == label
                    || (source.relation.alias().is_none()
                        && same_relation(source.relation.name(), label))
            }) && source.row.values.contains_key(name)
        })
        .collect::<Vec<_>>();
    let [source] = choices.as_slice() else {
        return Err("column has no unique physical producer and value");
    };
    source.row.values.get(name).cloned().ok_or("source column missing")
}

fn scalar(
    expression: &Expression,
    scope: &[InputRow<'_>],
    binary_text_laws_attested: bool,
) -> Result<LiteralValue, &'static str> {
    match expression {
        Expression::Literal(literal) => Ok(literal.value().clone()),
        Expression::Column(column) => lookup_column(column.relation(), column.name(), scope),
        Expression::BooleanPredicate(predicate) => Ok(match predicate_truth(
            predicate,
            scope,
            binary_text_laws_attested,
        )? {
            Some(value) => LiteralValue::Boolean(value),
            None => LiteralValue::Null,
        }),
        Expression::Case(case) if case.operand().is_none() => {
            for branch in case.branches() {
                if matches!(
                    scalar(branch.condition(), scope, binary_text_laws_attested)?,
                    LiteralValue::Boolean(true)
                ) {
                    return scalar(branch.result(), scope, binary_text_laws_attested);
                }
            }
            match case.else_result() {
                Some(expression) => scalar(expression, scope, binary_text_laws_attested),
                None => Ok(LiteralValue::Null),
            }
        }
        _ => Err("unsupported computed producer projection"),
    }
}

fn comparison(
    operator: ComparisonOperator,
    left: &LiteralValue,
    right: &LiteralValue,
    binary_text_laws_attested: bool,
) -> Result<Option<bool>, &'static str> {
    if matches!(left, LiteralValue::Null) || matches!(right, LiteralValue::Null) {
        return Ok(None);
    }
    let order = match (left, right) {
        (LiteralValue::Number(left), LiteralValue::Number(right)) => {
            left.parse::<i128>()
                .map_err(|_| "non-integer producer comparison")?
                .cmp(&right.parse::<i128>().map_err(|_| "non-integer producer comparison")?)
        }
        (LiteralValue::Boolean(left), LiteralValue::Boolean(right)) => left.cmp(right),
        (LiteralValue::Text(left), LiteralValue::Text(right)) if binary_text_laws_attested => {
            left.cmp(right)
        }
        (LiteralValue::Text(_), LiteralValue::Text(_)) => {
            return Err("binary text collation and no-padding laws require attestation");
        }
        _ => return Err("incompatible producer comparison types"),
    };
    let result = match operator {
        ComparisonOperator::Eq => order.is_eq(),
        ComparisonOperator::Neq => order.is_ne(),
        ComparisonOperator::Lt => order.is_lt(),
        ComparisonOperator::Lte => order.is_le(),
        ComparisonOperator::Gt => order.is_gt(),
        ComparisonOperator::Gte => order.is_ge(),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
            return Err("unattested NULL-safe comparison law");
        }
    };
    Ok(Some(result))
}

fn predicate_truth(
    predicate: &Predicate,
    scope: &[InputRow<'_>],
    binary_text_laws_attested: bool,
) -> Result<Option<bool>, &'static str> {
    match predicate {
        Predicate::Comparison(cmp) => comparison(
            cmp.operator(),
            &scalar(cmp.left(), scope, binary_text_laws_attested)?,
            &scalar(cmp.right(), scope, binary_text_laws_attested)?,
            binary_text_laws_attested,
        ),
        Predicate::And(and) => {
            let mut has_unknown = false;
            for item in and.operands() {
                match predicate_truth(item, scope, binary_text_laws_attested)? {
                    Some(false) => return Ok(Some(false)),
                    Some(true) => {}
                    None => has_unknown = true,
                }
            }
            Ok(if has_unknown { None } else { Some(true) })
        }
        Predicate::Or(or) => {
            let mut has_unknown = false;
            for item in or.operands() {
                match predicate_truth(item, scope, binary_text_laws_attested)? {
                    Some(true) => return Ok(Some(true)),
                    Some(false) => {}
                    None => has_unknown = true,
                }
            }
            Ok(if has_unknown { None } else { Some(false) })
        }
        Predicate::Exists(exists) => {
            let nested = exists.subquery();
            if !nested.row_shape_preserves_candidates()
                || nested.sources().len() != 1
                || !nested.joins().is_empty()
                || !nested.diagnostics().is_empty()
            {
                return Err("nested producer EXISTS lacks row-existence equivalence");
            }
            // Inner population resolution is supplied by eval_query, not a
            // guessed independently constructed subquery sample.
            let inner_relation = &nested.sources()[0];
            let inner = scope.iter().filter(|source| {
                source.relation.alias() == inner_relation.alias()
                    && same_relation(source.relation.name(), inner_relation.name())
            });
            let mut matching = false;
            for source in inner {
                let mut refs = scope.iter().filter(|item| {
                    !std::ptr::eq(*item, source)
                }).map(|item| InputRow { relation: item.relation, row: item.row }).collect::<Vec<_>>();
                refs.push(InputRow { relation: source.relation, row: source.row });
                if nested.predicates().where_predicate().is_none_or(|predicate| {
                    predicate_truth(predicate, &refs, binary_text_laws_attested)
                        .ok() == Some(Some(true))
                }) {
                    matching = true;
                    break;
                }
            }
            Ok(Some(if exists.negated() { !matching } else { matching }))
        }
        _ => Err("unsupported producer WHERE or JOIN predicate"),
    }
}

fn projection(
    output: &Output,
    scope: &[InputRow<'_>],
    ids: Vec<String>,
    binary_text_laws_attested: bool,
) -> Result<PhysicalProducerRow, &'static str> {
    let values = output.columns().iter().map(|column| {
        Ok((
            column.name().to_string(),
            scalar(column.expression(), scope, binary_text_laws_attested)?,
        ))
    }).collect::<Result<BTreeMap<_, _>, &'static str>>()?;
    Ok(PhysicalProducerRow { values, source_row_ids: ids })
}

fn certified_query_shape(query: &QueryStatement) -> bool {
    query.aggregation().is_none()
        && query.set_operation().is_none()
        && query.window_witness().is_none()
        && query.predicates().having_predicate().is_none()
        && query.predicates().qualify_predicate().is_none()
        && query.diagnostics().is_empty()
        && query.sources().len() >= 1
        && query.sources().len() <= 2
        && if query.joins().is_empty() {
            query.filter_only_row_shape() || query.row_preserving_projection()
        } else {
            query.joins().len() == 1
                && matches!(query.joins()[0].kind(), JoinKind::Inner)
                && query.joins()[0].execution().is_none()
                && (query.plain_goal_output_shape() || query.filtered_join_goal_output_shape())
        }
}

fn complete_domain(
    rows: &[PhysicalProducerRow],
    column: &str,
) -> Option<PhysicalProducerDomain> {
    let mut values = Vec::new();
    let mut numeric = Vec::new();
    let mut null_allowed = false;
    let mut all_numeric = true;
    for row in rows {
        let value = row.values.get(column)?;
        if !values.contains(value) {
            values.push(value.clone());
        }
        match value {
            LiteralValue::Null => null_allowed = true,
            LiteralValue::Number(value) => numeric.push(value.parse::<i128>().ok()?),
            _ => all_numeric = false,
        }
    }
    let integer_bounds = if all_numeric && !numeric.is_empty() {
        Some((*numeric.iter().min()?, *numeric.iter().max()?))
    } else {
        None
    };
    Some(PhysicalProducerDomain { values, integer_bounds, null_allowed })
}

fn input<'a>(
    produced: &'a BTreeMap<String, Vec<PhysicalProducerRow>>,
    name: &str,
) -> Result<&'a Vec<PhysicalProducerRow>, &'static str> {
    let candidates = produced.iter().filter(|(key, _)| same_relation(key, name))
        .map(|(_, rows)| rows).collect::<Vec<_>>();
    let [rows] = candidates.as_slice() else {
        return Err("producer relation is missing or has ambiguous source identity");
    };
    Ok(rows)
}

fn execute_query(
    query: &QueryStatement,
    produced: &BTreeMap<String, Vec<PhysicalProducerRow>>,
    binary_text_laws_attested: bool,
) -> Result<(Vec<PhysicalProducerRow>, Vec<Vec<String>>), &'static str> {
    if !certified_query_shape(query) {
        return Err("query has unsupported or unproved row-shaping semantics");
    }
    let sources = query.sources();
    let left = input(produced, sources[0].name())?;
    let right = match sources.get(1) {
        Some(relation) => Some(input(produced, relation.name())?),
        None => None,
    };
    let mut terminal = Vec::new();
    let mut rejected = Vec::new();
    for outer in left {
        let partners = match right {
            Some(rows) => rows.iter().map(Some).collect::<Vec<_>>(),
            None => vec![None],
        };
        for partner in partners {
            let mut scope = vec![InputRow { relation: &sources[0], row: outer }];
            let mut ids = outer.source_row_ids.clone();
            if let Some(inner) = partner {
                scope.push(InputRow { relation: &sources[1], row: inner });
                ids.extend(inner.source_row_ids.iter().cloned());
            }
            if let Some(join) = query.joins().first() {
                let on = join.condition().ok_or("INNER JOIN ON is not normalized")?;
                if predicate_truth(on, &scope, binary_text_laws_attested)? != Some(true) {
                    rejected.push(ids);
                    continue;
                }
            }
            let survives = match query.predicates().where_predicate() {
                Some(Predicate::Exists(exists)) => {
                    let nested = exists.subquery();
                    let [nested_source] = nested.sources() else {
                        return Err("producer EXISTS requires one complete nested source");
                    };
                    if !nested.row_shape_preserves_candidates()
                        || !nested.joins().is_empty()
                        || !nested.diagnostics().is_empty()
                    {
                        return Err("producer EXISTS row shape is not certified");
                    }
                    let inner_rows = input(produced, nested_source.name())?;
                    let mut any = false;
                    for inner in inner_rows {
                        let mut both = vec![InputRow { relation: &sources[0], row: outer }];
                        both.push(InputRow { relation: nested_source, row: inner });
                        if nested.predicates().where_predicate().is_none_or(|predicate| {
                            predicate_truth(predicate, &both, binary_text_laws_attested)
                                .ok() == Some(Some(true))
                        }) {
                            any = true;
                            break;
                        }
                    }
                    if exists.negated() { !any } else { any }
                }
                Some(predicate) => predicate_truth(predicate, &scope, binary_text_laws_attested)? == Some(true),
                None => true,
            };
            if survives {
                terminal.push(projection(query.output(), &scope, ids, binary_text_laws_attested)?);
            } else {
                rejected.push(ids);
            }
        }
    }
    Ok((terminal, rejected))
}

/// Execute and certify each normalized layer of a source-to-EXISTS producer DAG.
///
/// Caller supplies only complete physical leaves. Text comparisons require
/// explicit binary-collation and no-padding attestation; noncertified SQL
/// shapes return Residual rather than inferring a terminal from snapshots.
pub fn physical_finite_producer_exists_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
    binary_text_laws_attested: bool,
) -> PhysicalProducerExistsProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalProducerExistsProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    if walker.sources.len() != snapshots.len() {
        return residual("every physical leaf must have exactly one complete input bag");
    }
    let mut produced = BTreeMap::new();
    for state in snapshots {
        if !walker.sources.contains(state.relation()) || produced.contains_key(state.relation()) {
            return residual("source bag does not uniquely match canonical graph leaf");
        }
        let mut identities = BTreeSet::new();
        let mut rows = Vec::new();
        for row in state.rows() {
            let Some(id) = row_identity(row) else {
                return PhysicalProducerExistsProof::Impossible {
                    reason: "physical source requires a stable non-NULL id".to_string(),
                };
            };
            if !identities.insert(id.clone()) {
                return PhysicalProducerExistsProof::Impossible {
                    reason: "duplicate physical source row identity".to_string(),
                };
            }
            rows.push(PhysicalProducerRow {
                values: row.values().clone(),
                source_row_ids: vec![format!("{}#{id}", state.relation())],
            });
        }
        produced.insert(state.relation().to_string(), rows);
    }
    let mut layer_bags = BTreeMap::new();
    let mut rejected = BTreeMap::new();
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let Some(layer) = walker.layers.get(id.as_str()).copied() else {
            return residual("canonical graph producer layer is missing");
        };
        let Some(query) = query_for(bundle, layer) else {
            return residual("producer has no query semantics");
        };
        let (rows, failed) = match execute_query(query, &produced, binary_text_laws_attested) {
            Ok(value) => value,
            Err(reason) => return residual(format!("{id}: {reason}")),
        };
        for name in node.produced_relations() {
            if produced.insert(name.clone(), rows.clone()).is_some() {
                return residual("multiple producers claim one physical relation");
            }
        }
        rejected.insert(id.clone(), failed);
        layer_bags.insert(id.clone(), rows);
    }
    let Some(terminal) = layer_bags.get(target_layer_id).cloned() else {
        return residual("terminal producer layer not evaluated");
    };
    let Some(target) = walker.layers.get(target_layer_id).copied() else {
        return residual("terminal canonical layer not found");
    };
    let Some(query) = query_for(bundle, target) else {
        return residual("terminal has no canonical query");
    };
    let mut terminal_domains = BTreeMap::new();
    for column in query.output().columns() {
        let Some(domain) = complete_domain(&terminal, column.name()) else {
            return residual("terminal column is not physically materialized");
        };
        terminal_domains.insert(column.name().to_string(), domain);
    }
    PhysicalProducerExistsProof::Exact(Box::new(PhysicalProducerExistsExact {
        nodes: walker.nodes,
        sources: walker.sources.into_iter().collect(),
        layer_bags,
        rejected,
        terminal,
        terminal_domains,
    }))
}
