//! TASK-127: source-backed JOIN / GROUP producers for bounded QUALIFY ranking.
//!
//! Only certified canonical shapes are admitted. The upstream bag is fully
//! materialized before window evaluation; derived rows are not physical sources.
use super::rank_bags::{
    compare_key, exact_domain, key_value, qualifies, rank_function, RankFunction, RankSort,
};
use super::window_bags::{
    qualify_comparisons, PhysicalFiniteWindowExact, PhysicalFiniteWindowProof,
};
use super::*;
use crate::protocol::{
    ComparisonOperator, Expression, JoinKind, LiteralValue, Predicate, WindowFunctionExpression,
};
use crate::relation::RelationSchema;

struct ProducerBag {
    label: String,
    sources: Vec<String>,
    physical_ids: BTreeMap<String, Vec<String>>,
    rows: Vec<PhysicalWriteRow>,
    row_ids: Vec<String>,
    rejected_join_pairs: Vec<String>,
    contributors: BTreeMap<String, Vec<String>>,
}

fn not_proven(reason: impl Into<String>) -> PhysicalFiniteWindowProof {
    PhysicalFiniteWindowProof::Residual {
        reason: reason.into(),
    }
}

fn attested_integer(schema: &RelationSchema, name: &str) -> Result<(), &'static str> {
    let column = schema.columns().iter().find(|column| column.name() == name)
        .ok_or("composed column is absent from physical schema")?;
    if !super::aggregate_bags::signed_integer(column.data_type()) {
        return Err("composed key lacks portable signed-integer and NULL laws");
    }
    Ok(())
}

fn source_schema<'a>(bundle: &'a AnalysisBundle, relation: &str)
    -> Result<&'a RelationSchema, &'static str>
{
    bundle.source_schemas().iter()
        .find(|schema| schema.relation() == relation)
        .ok_or("composed physical source lacks authoritative schema")
}

fn state<'a>(snapshots: &'a [PhysicalWriteState], relation: &str)
    -> Result<&'a PhysicalWriteState, &'static str>
{
    snapshots.iter().find(|snapshot| snapshot.relation() == relation)
        .ok_or("composed source bag is missing")
}

fn join_column<'a>(
    expression: &'a Expression,
    query: &'a QueryStatement,
    bundle: &AnalysisBundle,
) -> Result<(&'a str, &'a str), &'static str> {
    let Expression::Column(column) = expression else {
        return Err("JOIN condition must be a physical column equality");
    };
    let relation = column.relation().ok_or("JOIN requires qualified source identity")?;
    let source = query.sources().iter().find(|source|
        relation == source.name() || Some(relation) == source.alias()
    ).ok_or("JOIN column does not name a direct source")?;
    attested_integer(source_schema(bundle, source.name())?, column.name())?;
    Ok((source.name(), column.name()))
}

fn make_join(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    snapshots: &[PhysicalWriteState],
) -> Result<ProducerBag, &'static str> {
    let [left, right] = query.sources() else {
        return Err("QUALIFY JOIN requires exactly two direct sources");
    };
    let [join] = query.joins() else {
        return Err("QUALIFY JOIN needs exactly one certified INNER JOIN");
    };
    if join.kind() != JoinKind::Inner || join.execution().is_some()
        || left.name() == right.name()
        || join.left().relation() != left.name()
        || join.right().relation() != right.name()
    {
        return Err("QUALIFY JOIN shape lacks a portable two-source INNER JOIN law");
    }
    let Some(Predicate::Comparison(condition)) = join.condition() else {
        return Err("QUALIFY JOIN lacks canonical equality condition");
    };
    if condition.operator() != ComparisonOperator::Eq {
        return Err("QUALIFY JOIN needs an exact scalar equality");
    }
    let (a_source, a_column) = join_column(condition.left(), query, bundle)?;
    let (b_source, b_column) = join_column(condition.right(), query, bundle)?;
    if !((a_source == left.name() && b_source == right.name())
        || (a_source == right.name() && b_source == left.name()))
    {
        return Err("QUALIFY JOIN equality must connect both physical sources");
    }
    let left_state = state(snapshots, left.name())?;
    let right_state = state(snapshots, right.name())?;
    let left_schema = source_schema(bundle, left.name())?;
    let right_schema = source_schema(bundle, right.name())?;
    let pairs = left_state.rows().len().checked_mul(right_state.rows().len())
        .ok_or("QUALIFY JOIN Cartesian bag exceeds finite width")?;
    if pairs > 1_000_000 {
        return Err("QUALIFY JOIN Cartesian bag exceeds bounded physical proof budget");
    }
    let mut rows = Vec::new();
    let mut row_ids = Vec::new();
    let mut rejected_join_pairs = Vec::new();
    let mut contributors = BTreeMap::new();
    for (li, lrow) in left_state.rows().iter().enumerate() {
        for (ri, rrow) in right_state.rows().iter().enumerate() {
            let identity = format!("{}#{li}|{}#{ri}",left.name(),right.name());
            let a = if a_source == left.name() { lrow } else { rrow };
            let b = if b_source == left.name() { lrow } else { rrow };
            let av = super::aggregate_bags::numeric(a.values().get(a_column)
                .ok_or("JOIN equality source column missing")?)?;
            let bv = super::aggregate_bags::numeric(b.values().get(b_column)
                .ok_or("JOIN equality source column missing")?)?;
            if av.is_none() || av != bv {
                rejected_join_pairs.push(identity);
                continue;
            }
            let mut values = BTreeMap::new();
            for (relation, schema, row) in [
                (left.name(),left_schema,lrow), (right.name(),right_schema,rrow)
            ] {
                for column in schema.columns() {
                    attested_integer(schema,column.name())?;
                    let value = row.values().get(column.name())
                        .ok_or("complete joined physical row lacks declared column")?;
                    values.insert(format!("{relation}.{}",column.name()),value.clone());
                }
            }
            rows.push(PhysicalWriteRow::new(values)
                .ok_or("joined producer could not construct a physical row")?);
            contributors.insert(identity.clone(),
                vec![format!("{}#{li}",left.name()),format!("{}#{ri}",right.name())]);
            row_ids.push(identity);
        }
    }
    let sources = vec![left.name().to_string(),right.name().to_string()];
    let physical_ids = sources.iter().zip([left_state,right_state]).map(|(name,snapshot)|
        (name.clone(),(0..snapshot.rows().len()).map(|index|format!("{name}#{index}")).collect())
    ).collect();
    Ok(ProducerBag {
        label: format!("join({},{})",left.name(),right.name()), sources,
        physical_ids, rows, row_ids, rejected_join_pairs, contributors,
    })
}

fn make_group(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    snapshots: &[PhysicalWriteState],
) -> Result<ProducerBag, &'static str> {
    let producer = query.window_grouped_count_source()
        .ok_or("QUALIFY grouped producer lacks canonical grouping law")?;
    let [derived] = query.sources() else {
        return Err("QUALIFY grouped producer must have one derived source");
    };
    if !query.joins().is_empty() || derived.name() == producer.source() {
        return Err("QUALIFY grouping has unexpected physical or joined source");
    }
    let schema = source_schema(bundle,producer.source())?;
    attested_integer(schema,producer.group_column())?;
    let source = state(snapshots,producer.source())?;
    let mut groups = BTreeMap::<Option<i128>,Vec<usize>>::new();
    for (index,row) in source.rows().iter().enumerate() {
        let key = super::aggregate_bags::numeric(row.values().get(producer.group_column())
            .ok_or("GROUP BY source key missing")?)?;
        groups.entry(key).or_default().push(index);
    }
    let mut rows = Vec::new();
    let mut row_ids = Vec::new();
    let mut contributors = BTreeMap::new();
    for (key,members) in groups {
        let count = i64::try_from(members.len())
            .map_err(|_| "COUNT(*) exceeds portable BIGINT domain")?;
        let identity = format!("group({}={})",producer.group_column(),
            key.map_or_else(||"NULL".to_string(),|v|v.to_string()));
        let values = BTreeMap::from([
            (producer.group_column().to_string(),
                key.map_or(LiteralValue::Null,|v|LiteralValue::Number(v.to_string()))),
            (producer.count_alias().to_string(),LiteralValue::Number(count.to_string())),
        ]);
        rows.push(PhysicalWriteRow::new(values)
            .ok_or("grouped producer could not construct a row")?);
        contributors.insert(identity.clone(), members.into_iter()
            .map(|index|format!("{}#{index}",producer.source())).collect());
        row_ids.push(identity);
    }
    let relation = producer.source().to_string();
    let physical_ids = BTreeMap::from([(relation.clone(),
        (0..source.rows().len()).map(|index|format!("{relation}#{index}")).collect())]);
    Ok(ProducerBag {
        label: format!("group({relation})"), sources: vec![relation],
        physical_ids, rows, row_ids, rejected_join_pairs: Vec::new(), contributors,
    })
}

fn resolved_key(
    expression: &Expression,
    bag: &ProducerBag,
    query: &QueryStatement,
    bundle: &AnalysisBundle,
) -> Result<String, &'static str> {
    let Expression::Column(column) = expression else {
        return Err("composed window keys and projections must be direct columns");
    };
    if let Some(group) = query.window_grouped_count_source() {
        if !matches!(column.name(), n if n == group.group_column() || n == group.count_alias()) {
            return Err("window grouped producer has no such projected column");
        }
        let [derived] = query.sources() else { return Err("missing grouped derived scope"); };
        if column.relation().is_some_and(|relation|
            relation != derived.name() && Some(relation) != derived.alias()) {
            return Err("window grouped column crosses an unproved scope");
        }
        return Ok(column.name().to_string());
    }
    let schema_matches = query.sources().iter().filter(|source|
        column.relation().is_none_or(|qualifier|
            qualifier == source.name() || Some(qualifier) == source.alias())
        && source_schema(bundle,source.name()).is_ok_and(|schema|
            schema.columns().iter().any(|item| item.name() == column.name()))
    ).collect::<Vec<_>>();
    let [source] = schema_matches.as_slice() else {
        return Err("window joined key or projection is unqualified, ambiguous or untyped");
    };
    let schema = source_schema(bundle,source.name())?;
    attested_integer(schema,column.name())?;
    let key = format!("{}.{}",source.name(),column.name());
    if bag.rows.iter().any(|row| !row.values().contains_key(&key)) {
        return Err("joined canonical source column is absent from a producer row");
    }
    Ok(key)
}

fn rank_keys(
    window: &WindowFunctionExpression,
    bag: &ProducerBag,
    query: &QueryStatement,
    bundle: &AnalysisBundle,
) -> Result<(Vec<String>,Vec<RankSort>), &'static str> {
    if window.window().frame().is_some() || window.window().order_by().is_empty() {
        return Err("composed rank needs explicit ORDER BY and no window frame");
    }
    let partition = window.window().partition_by().iter()
        .map(|expr|resolved_key(expr,bag,query,bundle))
        .collect::<Result<Vec<_>,_>>()?;
    let mut sort = Vec::new();
    for key in window.window().order_by() {
        let nulls_first = key.nulls_first()
            .ok_or("QUALIFY composed window requires explicit NULL ordering")?;
        sort.push(RankSort {
            column: resolved_key(key.expression(),bag,query,bundle)?,
            ascending: key.ascending().unwrap_or(true),
            nulls_first,
        });
    }
    Ok((partition,sort))
}

fn sorted_ranks(
    bag: &ProducerBag,
    partition_keys: &[String],
    sort: &[RankSort],
    rank_kind: RankFunction,
) -> Result<Vec<i128>, &'static str> {
    let mut partitions = BTreeMap::<Vec<Option<i128>>,Vec<(usize,Vec<Option<i128>>)>>::new();
    for (index,row) in bag.rows.iter().enumerate() {
        let partition = partition_keys.iter().map(|key|key_value(row,key))
            .collect::<Result<Vec<_>,_>>()?;
        let order = sort.iter().map(|key|key_value(row,&key.column))
            .collect::<Result<Vec<_>,_>>()?;
        partitions.entry(partition).or_default().push((index,order));
    }
    let mut ranks = vec![0i128;bag.rows.len()];
    for partition in partitions.values_mut() {
        partition.sort_by(|(_,a),(_,b)|a.iter().zip(b).zip(sort)
            .map(|((a,b),key)|compare_key(*a,*b,key))
            .find(|result|!result.is_eq()).unwrap_or(std::cmp::Ordering::Equal));
        if matches!(rank_kind,RankFunction::RowNumber)
            && partition.windows(2).any(|window|window[0].1==window[1].1) {
            return Err("composed ROW_NUMBER peer identities are underordered");
        }
        let mut dense_rank = 0usize;
        let mut peer_rank = 0usize;
        for (position,(index,order)) in partition.iter().enumerate() {
            if position == 0 || partition[position-1].1 != *order {
                dense_rank+=1;
                peer_rank=position+1;
            }
            ranks[*index] = i128::try_from(match rank_kind {
                RankFunction::RowNumber => position+1,
                RankFunction::Rank => peer_rank,
                RankFunction::DenseRank => dense_rank,
            }).map_err(|_|"rank exceeds supported signed width")?;
        }
    }
    Ok(ranks)
}

/// Invoke only for a parser-certified composed shape. The caller must have
/// independently validated every complete physical source against its schema.
pub(super) fn composed_window_plan(
    bundle: &AnalysisBundle,
    query: &QueryStatement,
    snapshots: &[PhysicalWriteState],
    nodes: Vec<PhysicalPlanNode>,
) -> PhysicalFiniteWindowProof {
    if !query.ranked_composed_goal_shape() {
        return not_proven("QUALIFY source composition has unproved SQL row shaping");
    }
    if query.set_operation().is_some() || query.aggregation().is_some()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || !query.subquery_witnesses().is_empty() || !query.diagnostics().is_empty()
    {
        return not_proven("QUALIFY producer carries unsupported row shaping");
    }
    let bag = if query.window_grouped_count_source().is_some() {
        make_group(bundle,query,snapshots)
    } else {
        make_join(bundle,query,snapshots)
    };
    let bag = match bag {
        Ok(bag) => bag,
        Err(reason) => return not_proven(reason),
    };
    if bag.sources.len() != snapshots.len() {
        return not_proven("uncontrolled extra physical source supplied for QUALIFY");
    }
    let comparisons = match qualify_comparisons(query) {
        Ok(comp) if !comp.is_empty() => comp,
        Ok(_) => return not_proven("QUALIFY is missing rank comparison"),
        Err(reason) => return not_proven(reason),
    };
    let mut qualifies_all = vec![true;bag.rows.len()];
    for (window,operator,threshold) in comparisons {
        let Some(kind) = rank_function(window) else {
            return not_proven("composed QUALIFY only certifies ROW_NUMBER/RANK/DENSE_RANK");
        };
        let (partition,sort) = match rank_keys(window,&bag,query,bundle) {
            Ok(keys) => keys,
            Err(reason) => return not_proven(reason),
        };
        let ranks = match sorted_ranks(&bag,&partition,&sort,kind) {
            Ok(ranks) => ranks,
            Err(reason) => return not_proven(reason),
        };
        for (survives,rank) in qualifies_all.iter_mut().zip(ranks) {
            *survives &= qualifies(rank,operator,threshold);
        }
    }
    let mut output_keys = Vec::new();
    let mut output_names = BTreeSet::new();
    for column in query.output().columns() {
        if !output_names.insert(column.name().to_string()) {
            return not_proven("duplicate composed output names");
        }
        let resolved = match resolved_key(column.expression(),&bag,query,bundle) {
            Ok(name) => name,
            Err(reason) => return not_proven(reason),
        };
        output_keys.push((column.name().to_string(),resolved));
    }
    let mut terminal = Vec::new();
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    for ((row,id),survives) in bag.rows.iter().zip(&bag.row_ids).zip(qualifies_all) {
        if !survives {
            rejected.push(id.clone());
            continue;
        }
        let mut projected = BTreeMap::new();
        for (name,key) in &output_keys {
            let Some(value) = row.values().get(key) else {
                return not_proven("composed terminal projection is missing its source value");
            };
            projected.insert(name.clone(),value.clone());
        }
        terminal.push(projected);
        selected.push(id.clone());
    }
    let terminal_domains = output_keys.iter().map(|(name,_)|
        (name.clone(),exact_domain(terminal.iter().filter_map(|row|row.get(name).cloned())))
    ).collect();
    PhysicalFiniteWindowProof::Exact(Box::new(PhysicalFiniteWindowExact {
        nodes, source: bag.label, source_row_ids: bag.row_ids,
        terminal_row_ids: selected, rejected_row_ids: rejected,
        physical_sources: bag.sources, physical_source_row_ids: bag.physical_ids,
        rejected_upstream_row_ids: bag.rejected_join_pairs,
        producer_contributors: bag.contributors,
        terminal, terminal_domains,
    }))
}
