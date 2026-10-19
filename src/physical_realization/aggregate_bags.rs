//! TASK-121 closed-world basic integer aggregate laws.
//!
//! A complete, schema-attested physical bag is the evidence. We interpret only
//! canonical parsed aggregate expressions, never input SQL or presumed row
//! counts. Missing type, scale, collation and overflow laws fail closed.

use super::write_states::{
    physical_closed_world_schema_plan, PhysicalWriteState, PhysicalWriteStatePlan,
};
use super::*;
use crate::data_type::DataType;
use crate::protocol::{
    AggregateArgument, ColumnExpression, ComparisonOperator, LiteralExpression, LiteralType,
    LiteralValue, Predicate, SetMode, ValueDomain,
};

/// Exact membership of a group, keeping duplicates as separate source rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalAggregateGroup {
    /// GROUP BY key. None is either SQL NULL or the synthetic global group.
    pub key: Option<i128>,
    /// All physical rows contributing to this group, including NULL inputs.
    pub source_row_ids: Vec<String>,
    /// Non-NULL contributors for each named aggregate (COUNT(*) includes all).
    pub aggregate_row_ids: BTreeMap<String, Vec<String>>,
    /// SQL HAVING truth for this entire group (None without HAVING).
    pub having_truth: Option<PhysicalAggregateTruth>,
}

/// Exact SQL three-valued truth of a group condition or aggregate FILTER.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalAggregateTruth {
    /// The condition is satisfied.
    True,
    /// The condition is false.
    False,
    /// The condition is UNKNOWN due to SQL NULL.
    Unknown,
}

impl PhysicalAggregateTruth {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::True,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::False,
        }
    }

    fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}

/// One fully evaluated source-backed aggregate query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFiniteAggregateExact {
    /// Canonical graph reached from the requested terminal layer.
    pub nodes: Vec<PhysicalPlanNode>,
    /// Physical relation whose complete bag was supplied.
    pub source: String,
    /// True for SQL GROUP BY, false for the global singleton group.
    pub grouped: bool,
    /// All groups including groups not surviving any NULL aggregate values.
    pub groups: Vec<PhysicalAggregateGroup>,
    /// Complete resulting terminal bag with SQL NULL values preserved.
    pub terminal: Vec<BTreeMap<String, LiteralValue>>,
    /// Canonical exact finite domains of each terminal output column.
    pub terminal_domains: BTreeMap<String, ValueDomain>,
}

/// Proof strength, with contradiction distinct from absent execution evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalFiniteAggregateProof {
    /// Fully closed-world input and exact SQL terminal bag.
    Exact(Box<PhysicalFiniteAggregateExact>),
    /// Enforced physical schema evidence contradicts the supplied population.
    Impossible { reason: String },
    /// Unproven operator or runtime law. Never an empty-output proof.
    Residual { reason: String },
}

fn residual(reason: impl Into<String>) -> PhysicalFiniteAggregateProof {
    PhysicalFiniteAggregateProof::Residual {
        reason: reason.into(),
    }
}

fn signed_integer(data_type: &DataType) -> bool {
    match data_type {
        DataType::SignedInteger { bits } => bits.is_none_or(|bits| bits <= 128),
        DataType::Nullable(inner) => signed_integer(inner),
        _ => false,
    }
}

fn matches_source(column: &ColumnExpression, name: &str, alias: Option<&str>) -> bool {
    column
        .relation()
        .is_none_or(|reference| reference == name || Some(reference) == alias)
}

fn numeric(value: &LiteralValue) -> Result<Option<i128>, &'static str> {
    match value {
        LiteralValue::Null => Ok(None),
        LiteralValue::Number(v) => v
            .parse::<i128>()
            .map(Some)
            .map_err(|_| "source value is not an exact signed integer"),
        _ => Err("source value has no attested signed integer execution law"),
    }
}


fn source_scalar(
    expression: &Expression,
    row: Option<&PhysicalWriteRow>,
    schema: &crate::relation::RelationSchema,
    source: &str,
    alias: Option<&str>,
) -> Result<LiteralValue, &'static str> {
    match expression {
        Expression::Column(column) if matches_source(column, source, alias) => {
            let Some(declared) = schema.columns().iter().find(|c| c.name() == column.name()) else {
                return Err("filter column lacks physical schema");
            };
            if !signed_integer(declared.data_type()) {
                return Err("filter column lacks a signed integer comparison law");
            }
            match row {
                Some(row) => row.values().get(column.name()).cloned()
                    .ok_or("filter column absent from complete source row"),
                None => Ok(LiteralValue::Null),
            }
        }
        Expression::Literal(literal) => match literal.literal_type() {
            LiteralType::Integer | LiteralType::Null | LiteralType::Boolean => {
                Ok(literal.value().clone())
            }
            _ => Err("FILTER requires an exact integer/NULL/Boolean literal"),
        },
        _ => Err("FILTER scalar is not a typed physical integer expression"),
    }
}

/// Validate the entire predicate tree even for empty physical bags or absent groups.
/// Without this preflight an unsupported expression could incorrectly prove an
/// empty GROUP BY output or a FILTER with no contributing rows.
fn predicate_shape<F>(predicate: &Predicate, scalar: &F) -> Result<(), &'static str>
where
    F: Fn(&Expression) -> Result<LiteralValue, &'static str>,
{
    match predicate {
        Predicate::Comparison(c) => { scalar(c.left())?; scalar(c.right())?; }
        Predicate::IsNull(c) => { scalar(c.expression())?; }
        Predicate::And(c) | Predicate::Or(c) => {
            for item in c.operands() { predicate_shape(item, scalar)?; }
        }
        Predicate::Not(c) => predicate_shape(c.operand(), scalar)?,
        Predicate::BooleanExpression(Expression::BooleanPredicate(c)) => {
            predicate_shape(c, scalar)?;
        }
        Predicate::BooleanExpression(e) => {
            if !matches!(scalar(e)?, LiteralValue::Boolean(_) | LiteralValue::Null) {
                return Err("predicate expression is not Boolean");
            }
        }
        _ => return Err("predicate requires unproved SQL execution semantics"),
    }
    Ok(())
}

fn predicate_truth<F>(predicate: &Predicate, scalar: &F) -> Result<PhysicalAggregateTruth, &'static str>
where
    F: Fn(&Expression) -> Result<LiteralValue, &'static str>,
{
    use PhysicalAggregateTruth::{False, True, Unknown};
    match predicate {
        Predicate::Comparison(c) => {
            let lhs = scalar(c.left())?;
            let rhs = scalar(c.right())?;
            if lhs == LiteralValue::Null || rhs == LiteralValue::Null {
                return Ok(match c.operator() {
                    ComparisonOperator::IsDistinctFrom =>
                        if lhs == rhs { False } else { True },
                    ComparisonOperator::IsNotDistinctFrom =>
                        if lhs == rhs { True } else { False },
                    _ => Unknown,
                });
            }
            let (Some(left), Some(right)) = (numeric(&lhs)?, numeric(&rhs)?) else {
                return Err("integer predicate operand is unproved");
            };
            let applies = match c.operator() {
                ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => left == right,
                ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => left != right,
                ComparisonOperator::Lt => left < right,
                ComparisonOperator::Lte => left <= right,
                ComparisonOperator::Gt => left > right,
                ComparisonOperator::Gte => left >= right,
            };
            Ok(if applies { True } else { False })
        }
        Predicate::IsNull(c) => {
            let is_null = scalar(c.expression())? == LiteralValue::Null;
            Ok(if is_null != c.negated() { True } else { False })
        }
        Predicate::And(c) => {
            let mut result = True;
            for child in c.operands() { result = result.and(predicate_truth(child, scalar)?); }
            Ok(result)
        }
        Predicate::Or(c) => {
            let mut result = False;
            for child in c.operands() { result = result.or(predicate_truth(child, scalar)?); }
            Ok(result)
        }
        Predicate::Not(c) => Ok(predicate_truth(c.operand(), scalar)?.not()),
        Predicate::BooleanExpression(Expression::BooleanPredicate(c)) =>
            predicate_truth(c, scalar),
        Predicate::BooleanExpression(e) => match scalar(e)? {
            LiteralValue::Boolean(true) => Ok(True),
            LiteralValue::Boolean(false) => Ok(False),
            LiteralValue::Null => Ok(Unknown),
            _ => Err("predicate does not evaluate to Boolean"),
        },
        _ => Err("predicate has no complete signed integer Boolean law"),
    }
}

fn having_scalar(
    expression: &Expression,
    projected: &BTreeMap<String, LiteralValue>,
    key: Option<i128>,
    group_key: Option<&str>,
    members: &[usize],
    snapshot: &PhysicalWriteState,
    schema: &crate::relation::RelationSchema,
    source: &str,
    alias: Option<&str>,
) -> Result<LiteralValue, &'static str> {
    match expression {
        Expression::AggregateFunction(function) =>
            aggregate_value(function, members, snapshot, schema, source, alias).map(|(v, _)| v),
        Expression::Column(column) => {
            if column.relation().is_some() && !matches_source(column, source, alias) {
                return Err("HAVING expression references an unproved producer");
            }
            if group_key == Some(column.name()) {
                return Ok(key.map(|k| LiteralValue::Number(k.to_string())).unwrap_or(LiteralValue::Null));
            }
            if column.relation().is_none() && !schema.columns().iter().any(|c| c.name() == column.name()) {
                return projected.get(column.name()).cloned()
                    .ok_or("HAVING alias does not name an output aggregate");
            }
            Err("HAVING column is neither the grouping key nor an unambiguous aggregate alias")
        }
        Expression::Literal(literal) => match literal.literal_type() {
            LiteralType::Integer | LiteralType::Null | LiteralType::Boolean => Ok(literal.value().clone()),
            _ => Err("HAVING literal comparison lacks a portable type law"),
        },
        _ => Err("HAVING contains an unproved computed scalar expression"),
    }
}

fn aggregate_value(
    function: &crate::protocol::AggregateFunctionExpression,
    members: &[usize],
    snapshot: &PhysicalWriteState,
    schema: &crate::relation::RelationSchema,
    source: &str,
    alias: Option<&str>,
) -> Result<(LiteralValue, Vec<String>), &'static str> {
    if function.distinct() && !function.name().eq_ignore_ascii_case("count") {
        return Err("only COUNT(DISTINCT column) has a certified integer law");
    }
    if function.distinct() && matches!(function.arguments(), [AggregateArgument::Wildcard]) {
        return Err("COUNT(DISTINCT *) has no portable certified law");
    }
    if let Some(filter) = function.filter() {
        predicate_shape(filter, &|expr| source_scalar(expr, None, schema, source, alias))?;
    }
    let count_all = function.name().eq_ignore_ascii_case("count")
        && matches!(function.arguments(), [AggregateArgument::Wildcard]);
    let column = if count_all {
        None
    } else {
        let [AggregateArgument::Expression(Expression::Column(column))] = function.arguments()
        else {
            return Err("only COUNT(*) or a single physical scalar argument is certified");
        };
        if !matches_source(column, source, alias) {
            return Err("aggregate argument is not a direct physical source column");
        }
        let schema_column = schema
            .columns()
            .iter()
            .find(|item| item.name() == column.name())
            .ok_or("aggregate argument has no source schema")?;
        if !signed_integer(schema_column.data_type()) {
            return Err("aggregate source needs an exact signed integer type law");
        }
        Some(column.name())
    };

    let mut numbers = Vec::new();
    let mut contributors = Vec::new();
    let mut distinct_numbers = BTreeSet::new();
    for index in members {
        let row = &snapshot.rows()[*index];
        let id = format!("{source}#{index}");
        if let Some(filter) = function.filter() {
            let scalar = |expr: &Expression| source_scalar(expr, Some(row), schema, source, alias);
            if predicate_truth(filter, &scalar)? != PhysicalAggregateTruth::True {
                continue;
            }
        }
        if count_all {
            contributors.push(id);
            continue;
        }
        let value = row
            .values()
            .get(column.ok_or("missing aggregate column")?)
            .ok_or("aggregate source row lacks a declared column")?;
        if let Some(value) = numeric(value)? {
            if !function.distinct() || distinct_numbers.insert(value) {
                numbers.push(value);
                contributors.push(id);
            }
        }
    }

    let value = match function.name().to_ascii_lowercase().as_str() {
        "count" => LiteralValue::Number(contributors.len().to_string()),
        "sum" | "avg" => {
            if numbers.is_empty() {
                LiteralValue::Null
            } else {
                let sum = numbers
                    .iter()
                    .try_fold(0_i128, |acc, n| acc.checked_add(*n))
                    .ok_or("aggregate accumulator overflow without an attested wider SQL type")?;
                if function.name().eq_ignore_ascii_case("sum") {
                    LiteralValue::Number(sum.to_string())
                } else {
                    let count = i128::try_from(numbers.len())
                        .map_err(|_| "aggregate contributor count exceeds signed integer range")?;
                    if sum % count != 0 {
                        return Err(
                            "nonintegral AVG needs an attested decimal scale and rounding law",
                        );
                    }
                    LiteralValue::Number((sum / count).to_string())
                }
            }
        }
        "min" => numbers
            .iter()
            .min()
            .map(|n| LiteralValue::Number(n.to_string()))
            .unwrap_or(LiteralValue::Null),
        "max" => numbers
            .iter()
            .max()
            .map(|n| LiteralValue::Number(n.to_string()))
            .unwrap_or(LiteralValue::Null),
        _ => return Err("aggregate function has no portable closed-world proof"),
    };
    Ok((value, contributors))
}

/// Prove a direct single-source GROUP BY or global aggregation on the complete
/// physical integer bag. Both SQL's empty-global singleton and empty-grouped
/// zero-row result are evaluated from the same source identity. Runtime-specific
/// AVG scale, overflowing SUM and unsupported operators stay Residual.
pub fn physical_finite_aggregate_bag_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    snapshots: &[PhysicalWriteState],
) -> PhysicalFiniteAggregateProof {
    match physical_closed_world_schema_plan(bundle, snapshots) {
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            return PhysicalFiniteAggregateProof::Impossible { reason };
        }
        PhysicalWriteStatePlan::Residual { reason, .. } => return residual(reason),
        PhysicalWriteStatePlan::Feasible { .. } => {}
    }
    let mut walker = Walker::new(bundle);
    if let Err(gap) = walker.visit(target_layer_id) {
        return residual(gap.as_str());
    }
    let Some(layer) = walker.layers.get(target_layer_id).copied() else {
        return residual("unknown aggregate layer identity");
    };
    let Some(query) = query_for(bundle, layer) else {
        return residual("aggregate layer is not a parsed canonical query");
    };
    let [source] = query.sources() else {
        return residual("exact basic aggregation requires one direct physical source");
    };
    let relation = source.name();
    let [snapshot] = snapshots else {
        return residual("complete aggregate proof needs exactly one source snapshot");
    };
    if snapshot.relation() != relation
        || !walker.sources.contains(relation)
        || query.predicates().where_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.joins().is_empty()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || !query.diagnostics().is_empty()
    {
        return residual("aggregate is not a direct unfiltered physical source");
    }
    let Some(schema) = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == relation)
    else {
        return residual("missing authoritative physical source schema");
    };
    let grouped_key = match query.aggregation() {
        None => None,
        Some(aggregation) if aggregation.distinct() || !aggregation.distinct_on().is_empty() => {
            return residual("aggregate DISTINCT has an unproved bag law")
        }
        Some(aggregation) => match aggregation.group_by() {
            Some(GroupBy::Expressions(keys)) if keys.len() == 1 => {
                let GroupingExpression::Expression(Expression::Column(column)) = &keys[0] else {
                    return residual("group key is not a simple physical column");
                };
                if !matches_source(column, relation, source.alias()) {
                    return residual("group key source identity is unresolved");
                }
                let Some(schema_column) =
                    schema.columns().iter().find(|c| c.name() == column.name())
                else {
                    return residual("group key lacks typed physical schema");
                };
                if !signed_integer(schema_column.data_type()) {
                    return residual("group key requires exact signed integer type law");
                }
                Some(column.name())
            }
            _ => return residual("basic aggregate proof excludes complex grouping"),
        },
    };
    if grouped_key.is_some() && !query.group_rows_match_surviving_groups() {
        return residual("parser did not certify the grouped terminal row shape");
    }
    if grouped_key.is_none() && !query.proven_single_row_output() {
        return residual("parser did not certify the unshaped global singleton result");
    }

    // A zero-row GROUP BY must not make unsupported SELECT expressions appear
    // exact merely because evaluation never enters the per-group loop.
    let mut output_names = BTreeSet::new();
    let mut has_aggregate = false;
    for output in query.output().columns() {
        if !output_names.insert(output.name()) {
            return residual("duplicate output column names prevent exact terminal identity");
        }
        match output.expression() {
            Expression::Column(column)
                if grouped_key == Some(column.name())
                    && matches_source(column, relation, source.alias()) => {}
            Expression::AggregateFunction(function) => {
                has_aggregate = true;
                if let Err(reason) =
                    aggregate_value(function, &[], snapshot, schema, relation, source.alias())
                {
                    return residual(reason);
                }
            }
            _ => return residual("output needs a direct group key or basic aggregate"),
        }
    }
    if !has_aggregate {
        return residual("query contains no basic aggregate expression");
    }

    if let Some(having) = query.predicates().having_predicate() {
        let prototype = query.output().columns().iter()
            .map(|output| (output.name().to_string(), LiteralValue::Null))
            .collect::<BTreeMap<_, _>>();
        let scalar = |expr: &Expression| {
            having_scalar(expr, &prototype, None, grouped_key, &[], snapshot,
                schema, relation, source.alias())
        };
        if let Err(reason) = predicate_shape(having, &scalar) {
            return residual(reason);
        }
    }

    let mut groups = BTreeMap::<Option<i128>, Vec<usize>>::new();
    if grouped_key.is_none() {
        groups.insert(None, (0..snapshot.rows().len()).collect());
    } else {
        for (index, row) in snapshot.rows().iter().enumerate() {
            let Some(value) = row.values().get(grouped_key.unwrap_or("")) else {
                return residual("source row lacks its group key");
            };
            let key = match numeric(value) {
                Ok(key) => key,
                Err(reason) => return residual(reason),
            };
            groups.entry(key).or_default().push(index);
        }
    }
    let mut terminal = Vec::new();
    let mut identities = Vec::new();
    for (key, members) in groups {
        let mut projected = BTreeMap::new();
        let mut aggregate_row_ids = BTreeMap::new();
        for output in query.output().columns() {
            let scalar = match output.expression() {
                Expression::Column(column)
                    if grouped_key == Some(column.name())
                        && matches_source(column, relation, source.alias()) =>
                {
                    key.map(|value| LiteralValue::Number(value.to_string()))
                        .unwrap_or(LiteralValue::Null)
                }
                Expression::AggregateFunction(function) => {
                    match aggregate_value(
                        function,
                        &members,
                        snapshot,
                        schema,
                        relation,
                        source.alias(),
                    ) {
                        Ok((value, contributors)) => {
                            aggregate_row_ids.insert(output.name().to_string(), contributors);
                            value
                        }
                        Err(reason) => return residual(reason),
                    }
                }
                _ => return residual("output needs a direct group key or basic aggregate"),
            };
            if projected
                .insert(output.name().to_string(), scalar)
                .is_some()
            {
                return residual("duplicate output column names prevent exact terminal identity");
            }
        }
        let having_truth = match query.predicates().having_predicate() {
            Some(predicate) => {
                let scalar = |expr: &Expression| {
                    having_scalar(expr, &projected, key, grouped_key, &members,
                        snapshot, schema, relation, source.alias())
                };
                match predicate_truth(predicate, &scalar) {
                    Ok(truth) => Some(truth),
                    Err(reason) => return residual(reason),
                }
            }
            None => None,
        };
        identities.push(PhysicalAggregateGroup {
            key,
            source_row_ids: members
                .iter()
                .map(|index| format!("{relation}#{index}"))
                .collect(),
            aggregate_row_ids,
            having_truth,
        });
        if having_truth.is_none_or(|truth| truth == PhysicalAggregateTruth::True) {
            terminal.push(projected);
        }
    }
    let mut terminal_domains = BTreeMap::new();
    for output in query.output().columns() {
        let name = output.name();
        let mut exact_values = Vec::new();
        for row in &terminal {
            let Some(value) = row.get(name) else {
                return residual("terminal output identity missing");
            };
            if !exact_values
                .iter()
                .any(|candidate: &LiteralExpression| candidate.value() == value)
            {
                let literal_type = if matches!(value, LiteralValue::Null) {
                    LiteralType::Null
                } else {
                    LiteralType::Integer
                };
                exact_values.push(LiteralExpression::new(literal_type, value.clone()));
            }
        }
        terminal_domains.insert(
            name.to_string(),
            ValueDomain::set(SetMode::Include, exact_values),
        );
    }
    PhysicalFiniteAggregateProof::Exact(Box::new(PhysicalFiniteAggregateExact {
        nodes: walker.nodes,
        source: relation.to_string(),
        grouped: grouped_key.is_some(),
        groups: identities,
        terminal,
        terminal_domains,
    }))
}
