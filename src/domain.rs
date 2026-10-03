//! Conservative scalar-domain derivation from normalized protocol predicates.
//!
//! This module operates only on parser-independent protocol values. Predicate normalization stays
//! in analysis.rs; domain algebra stays here so consumers see one deterministic representation.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use crate::protocol::{
    BetweenPredicate, Bound, ColumnDomain, ColumnExpression, ColumnRef, ComparisonOperator,
    ComparisonPredicate, Expression, InPredicate, IsNullPredicate, LiteralExpression, LiteralType,
    LiteralValue, Predicate, Predicates, SetMode, SourceRelation, ValueDomain, ValueRange,
};

type DomainMap = BTreeMap<ColumnRef, ValueDomain>;

pub(crate) fn derive_column_domains(
    predicates: &Predicates,
    sources: &[SourceRelation],
) -> Vec<ColumnDomain> {
    let mut domains = DomainMap::new();

    for predicate in [
        predicates.where_predicate(),
        predicates.having_predicate(),
        predicates.qualify_predicate(),
    ]
    .into_iter()
    .flatten()
    {
        domains = intersect_maps(domains, derive_predicate_domains(predicate, sources));
    }

    domains
        .into_iter()
        .map(|(column, domain)| ColumnDomain::new(column, domain))
        .collect()
}

fn derive_predicate_domains(predicate: &Predicate, sources: &[SourceRelation]) -> DomainMap {
    match predicate {
        Predicate::Comparison(predicate) => derive_comparison(predicate, sources),
        Predicate::And(predicate) => predicate
            .operands()
            .iter()
            .fold(DomainMap::new(), |domains, operand| {
                intersect_maps(domains, derive_predicate_domains(operand, sources))
            }),
        Predicate::Or(predicate) => {
            let mut operands = predicate.operands().iter();
            match operands.next() {
                Some(first) => operands.fold(
                    derive_predicate_domains(first, sources),
                    |domains, operand| {
                        union_maps(domains, derive_predicate_domains(operand, sources))
                    },
                ),
                None => DomainMap::new(),
            }
        }
        Predicate::Not(predicate) => unknown_for_predicate(
            predicate.operand(),
            sources,
            "logical NOT cannot always be reduced safely to independent scalar domains",
        ),
        Predicate::IsNull(predicate) => derive_is_null(predicate, sources),
        Predicate::In(predicate) => derive_in(predicate, sources),
        Predicate::Exists(_) => DomainMap::new(),
        Predicate::InSubquery(predicate) => unknown_for_expressions(
            [predicate.expression()],
            sources,
            "IN subquery cannot be reduced safely to a scalar domain",
        ),
        Predicate::Between(predicate) => derive_between(predicate, sources),
        Predicate::BooleanExpression(expression) => unknown_for_expressions(
            [expression],
            sources,
            "boolean expression cannot be reduced safely to a scalar domain",
        ),
        Predicate::Unknown(_) | Predicate::Unsupported(_) => DomainMap::new(),
    }
}

fn derive_comparison(predicate: &ComparisonPredicate, sources: &[SourceRelation]) -> DomainMap {
    match (predicate.left(), predicate.right()) {
        (Expression::Column(column), Expression::Literal(literal)) => BTreeMap::from([(
            resolve_column(column, sources),
            comparison_domain(predicate.operator(), literal),
        )]),
        (Expression::Literal(literal), Expression::Column(column)) => BTreeMap::from([(
            resolve_column(column, sources),
            comparison_domain(predicate.operator().reversed(), literal),
        )]),
        (left, right) => unknown_for_expressions(
            [left, right],
            sources,
            "comparison bound is not a scalar literal",
        ),
    }
}

fn comparison_domain(operator: ComparisonOperator, literal: &LiteralExpression) -> ValueDomain {
    let literal = literal.clone();

    if matches!(literal.value(), LiteralValue::Null) {
        return match operator {
            ComparisonOperator::IsDistinctFrom => {
                ValueDomain::set(SetMode::Exclude, vec![null_literal()])
            }
            ComparisonOperator::IsNotDistinctFrom => {
                ValueDomain::set(SetMode::Include, vec![null_literal()])
            }
            ComparisonOperator::Eq
            | ComparisonOperator::Neq
            | ComparisonOperator::Lt
            | ComparisonOperator::Lte
            | ComparisonOperator::Gt
            | ComparisonOperator::Gte => ValueDomain::Empty,
        };
    }

    match operator {
        ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => {
            ValueDomain::set(SetMode::Include, vec![literal])
        }
        ComparisonOperator::Neq => {
            ValueDomain::set(SetMode::Exclude, vec![literal, null_literal()])
        }
        ComparisonOperator::IsDistinctFrom => ValueDomain::set(SetMode::Exclude, vec![literal]),
        ComparisonOperator::Lt => ValueDomain::ranges(vec![ValueRange::new(
            None,
            Some(Bound::new(literal, false)),
        )]),
        ComparisonOperator::Lte => {
            ValueDomain::ranges(vec![ValueRange::new(None, Some(Bound::new(literal, true)))])
        }
        ComparisonOperator::Gt => ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(literal, false)),
            None,
        )]),
        ComparisonOperator::Gte => {
            ValueDomain::ranges(vec![ValueRange::new(Some(Bound::new(literal, true)), None)])
        }
    }
}

fn derive_is_null(predicate: &IsNullPredicate, sources: &[SourceRelation]) -> DomainMap {
    match predicate.expression() {
        Expression::Column(column) => {
            let mode = if predicate.negated() {
                SetMode::Exclude
            } else {
                SetMode::Include
            };
            BTreeMap::from([(
                resolve_column(column, sources),
                ValueDomain::set(mode, vec![null_literal()]),
            )])
        }
        expression => unknown_for_expressions(
            [expression],
            sources,
            "null predicate targets a non-column expression",
        ),
    }
}

fn derive_in(predicate: &InPredicate, sources: &[SourceRelation]) -> DomainMap {
    let column = match predicate.expression() {
        Expression::Column(column) => column,
        expression => {
            return unknown_for_expressions(
                std::iter::once(expression).chain(predicate.values().iter()),
                sources,
                "IN predicate targets a non-column expression",
            );
        }
    };

    let mut literals = Vec::with_capacity(predicate.values().len());
    for value in predicate.values() {
        match value {
            Expression::Literal(literal) => literals.push(literal.clone()),
            _ => {
                return unknown_for_expressions(
                    std::iter::once(predicate.expression()).chain(predicate.values().iter()),
                    sources,
                    "IN list contains a non-literal expression",
                );
            }
        }
    }

    let domain = if predicate.negated() {
        if literals
            .iter()
            .any(|literal| matches!(literal.value(), LiteralValue::Null))
        {
            ValueDomain::Empty
        } else {
            literals.push(null_literal());
            ValueDomain::set(SetMode::Exclude, literals)
        }
    } else {
        literals.retain(|literal| !matches!(literal.value(), LiteralValue::Null));
        ValueDomain::set(SetMode::Include, literals)
    };

    BTreeMap::from([(resolve_column(column, sources), domain)])
}

fn derive_between(predicate: &BetweenPredicate, sources: &[SourceRelation]) -> DomainMap {
    let column = match predicate.expression() {
        Expression::Column(column) => column,
        expression => {
            return unknown_for_expressions(
                [expression, predicate.lower(), predicate.upper()],
                sources,
                "BETWEEN predicate targets a non-column expression",
            );
        }
    };

    let (lower, upper) = match (predicate.lower(), predicate.upper()) {
        (Expression::Literal(lower), Expression::Literal(upper)) => (lower.clone(), upper.clone()),
        _ => {
            return unknown_for_expressions(
                [predicate.expression(), predicate.lower(), predicate.upper()],
                sources,
                "BETWEEN bound is not a scalar literal",
            );
        }
    };

    let domain = if matches!(lower.value(), LiteralValue::Null)
        || matches!(upper.value(), LiteralValue::Null)
    {
        ValueDomain::Empty
    } else if predicate.negated() {
        match compare_literals(&lower, &upper) {
            Some(Ordering::Greater) => ValueDomain::set(SetMode::Exclude, vec![null_literal()]),
            Some(Ordering::Equal) => {
                ValueDomain::set(SetMode::Exclude, vec![lower, null_literal()])
            }
            Some(Ordering::Less) | None => ValueDomain::ranges(vec![
                ValueRange::new(None, Some(Bound::new(lower, false))),
                ValueRange::new(Some(Bound::new(upper, false)), None),
            ]),
        }
    } else {
        match compare_literals(&lower, &upper) {
            Some(Ordering::Greater) => ValueDomain::Empty,
            Some(Ordering::Equal) => ValueDomain::set(SetMode::Include, vec![lower]),
            Some(Ordering::Less) | None => ValueDomain::ranges(vec![ValueRange::new(
                Some(Bound::new(lower, true)),
                Some(Bound::new(upper, true)),
            )]),
        }
    };

    BTreeMap::from([(resolve_column(column, sources), domain)])
}

fn unknown_for_predicate(
    predicate: &Predicate,
    sources: &[SourceRelation],
    reason: &str,
) -> DomainMap {
    let mut columns = BTreeSet::new();
    collect_predicate_columns(predicate, sources, &mut columns);
    columns
        .into_iter()
        .map(|column| (column, ValueDomain::unknown(reason)))
        .collect()
}

fn unknown_for_expressions<'a>(
    expressions: impl IntoIterator<Item = &'a Expression>,
    sources: &[SourceRelation],
    reason: &str,
) -> DomainMap {
    let mut columns = BTreeSet::new();
    for expression in expressions {
        collect_expression_columns(expression, sources, &mut columns);
    }
    columns
        .into_iter()
        .map(|column| (column, ValueDomain::unknown(reason)))
        .collect()
}

fn collect_predicate_columns(
    predicate: &Predicate,
    sources: &[SourceRelation],
    columns: &mut BTreeSet<ColumnRef>,
) {
    match predicate {
        Predicate::Comparison(predicate) => {
            collect_expression_columns(predicate.left(), sources, columns);
            collect_expression_columns(predicate.right(), sources, columns);
        }
        Predicate::And(predicate) | Predicate::Or(predicate) => {
            for operand in predicate.operands() {
                collect_predicate_columns(operand, sources, columns);
            }
        }
        Predicate::Not(predicate) => {
            collect_predicate_columns(predicate.operand(), sources, columns)
        }
        Predicate::IsNull(predicate) => {
            collect_expression_columns(predicate.expression(), sources, columns);
        }
        Predicate::In(predicate) => {
            collect_expression_columns(predicate.expression(), sources, columns);
            for value in predicate.values() {
                collect_expression_columns(value, sources, columns);
            }
        }
        Predicate::Exists(_) => {}
        Predicate::InSubquery(predicate) => {
            collect_expression_columns(predicate.expression(), sources, columns);
        }
        Predicate::Between(predicate) => {
            collect_expression_columns(predicate.expression(), sources, columns);
            collect_expression_columns(predicate.lower(), sources, columns);
            collect_expression_columns(predicate.upper(), sources, columns);
        }
        Predicate::BooleanExpression(expression) => {
            collect_expression_columns(expression, sources, columns);
        }
        Predicate::Unknown(_) | Predicate::Unsupported(_) => {}
    }
}

fn collect_expression_columns(
    expression: &Expression,
    sources: &[SourceRelation],
    columns: &mut BTreeSet<ColumnRef>,
) {
    match expression {
        Expression::Column(column) => {
            columns.insert(resolve_column(column, sources));
        }
        Expression::Function(function) => {
            for argument in function.arguments() {
                collect_expression_columns(argument, sources, columns);
            }
        }
        Expression::AggregateFunction(_) | Expression::WindowFunction(_) => {}
        Expression::Unary(expression) => {
            collect_expression_columns(expression.operand(), sources, columns);
        }
        Expression::Binary(expression) => {
            collect_expression_columns(expression.left(), sources, columns);
            collect_expression_columns(expression.right(), sources, columns);
        }
        Expression::Literal(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => {}
    }
}

fn resolve_column(column: &ColumnExpression, sources: &[SourceRelation]) -> ColumnRef {
    let relation = match column.relation() {
        Some(qualifier) => {
            let candidates = sources
                .iter()
                .filter(|source| relation_matches(source, qualifier))
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [source] => Some(source.name().to_string()),
                _ => Some(qualifier.to_string()),
            }
        }
        None => match sources {
            [source] => Some(source.name().to_string()),
            _ => None,
        },
    };

    ColumnRef::new(relation, column.name().to_string())
}

fn relation_matches(source: &SourceRelation, qualifier: &str) -> bool {
    source.alias() == Some(qualifier)
        || source.name() == qualifier
        || (source.alias().is_none() && source.name().rsplit('.').next() == Some(qualifier))
}

fn intersect_maps(mut left: DomainMap, right: DomainMap) -> DomainMap {
    for (column, right_domain) in right {
        match left.remove(&column) {
            Some(left_domain) => {
                left.insert(column, intersect_domains(&left_domain, &right_domain));
            }
            None => {
                left.insert(column, right_domain);
            }
        }
    }
    left
}

fn union_maps(left: DomainMap, right: DomainMap) -> DomainMap {
    let keys = left
        .keys()
        .chain(right.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    keys.into_iter()
        .map(|column| {
            let domain = match (left.get(&column), right.get(&column)) {
                (Some(left), Some(right)) => union_domains(left, right),
                (Some(_), None) | (None, Some(_)) => ValueDomain::Unbounded,
                (None, None) => ValueDomain::Unbounded,
            };
            (column, domain)
        })
        .collect()
}

pub(crate) fn intersect_domains(left: &ValueDomain, right: &ValueDomain) -> ValueDomain {
    match (left, right) {
        (ValueDomain::Empty, _) | (_, ValueDomain::Empty) => ValueDomain::Empty,
        (ValueDomain::Unbounded, domain) | (domain, ValueDomain::Unbounded) => domain.clone(),
        (ValueDomain::Unknown(_), ValueDomain::Unknown(_)) => left.clone(),
        (ValueDomain::Unknown(_), domain) | (domain, ValueDomain::Unknown(_)) => domain.clone(),
        (ValueDomain::Ranges(left), ValueDomain::Ranges(right)) => {
            intersect_range_domains(left.ranges(), right.ranges())
        }
        (ValueDomain::Set(left), ValueDomain::Set(right)) => {
            intersect_set_domains(left.mode(), left.values(), right.mode(), right.values())
        }
        (ValueDomain::Ranges(ranges), ValueDomain::Set(set))
        | (ValueDomain::Set(set), ValueDomain::Ranges(ranges)) => {
            intersect_ranges_and_set(ranges.ranges(), set.mode(), set.values())
        }
    }
}

fn union_domains(left: &ValueDomain, right: &ValueDomain) -> ValueDomain {
    match (left, right) {
        (ValueDomain::Unbounded, _) | (_, ValueDomain::Unbounded) => ValueDomain::Unbounded,
        (ValueDomain::Empty, domain) | (domain, ValueDomain::Empty) => domain.clone(),
        (ValueDomain::Unknown(_), _) => left.clone(),
        (_, ValueDomain::Unknown(_)) => right.clone(),
        (ValueDomain::Ranges(left), ValueDomain::Ranges(right)) => {
            let mut ranges = left.ranges().to_vec();
            ranges.extend_from_slice(right.ranges());
            ValueDomain::ranges(ranges)
        }
        (ValueDomain::Set(left), ValueDomain::Set(right)) => {
            union_set_domains(left.mode(), left.values(), right.mode(), right.values())
        }
        (ValueDomain::Ranges(ranges), ValueDomain::Set(set))
        | (ValueDomain::Set(set), ValueDomain::Ranges(ranges))
            if set.mode() == SetMode::Include =>
        {
            let mut combined = ranges.ranges().to_vec();
            for value in set.values() {
                if matches!(value.value(), LiteralValue::Null) {
                    return ValueDomain::unknown(
                        "range union with NULL cannot be represented precisely by protocol v0",
                    );
                }
                combined.push(ValueRange::new(
                    Some(Bound::new(value.clone(), true)),
                    Some(Bound::new(value.clone(), true)),
                ));
            }
            ValueDomain::ranges(combined)
        }
        (ValueDomain::Ranges(_), ValueDomain::Set(_))
        | (ValueDomain::Set(_), ValueDomain::Ranges(_)) => ValueDomain::unknown(
            "range union with an exclusion set cannot be represented precisely by protocol v0",
        ),
    }
}

fn intersect_set_domains(
    left_mode: SetMode,
    left: &[LiteralExpression],
    right_mode: SetMode,
    right: &[LiteralExpression],
) -> ValueDomain {
    match (left_mode, right_mode) {
        (SetMode::Include, SetMode::Include) => ValueDomain::set(
            SetMode::Include,
            left.iter()
                .filter(|value| right.contains(value))
                .cloned()
                .collect(),
        ),
        (SetMode::Exclude, SetMode::Exclude) => {
            let mut values = left.to_vec();
            values.extend_from_slice(right);
            ValueDomain::set(SetMode::Exclude, values)
        }
        (SetMode::Include, SetMode::Exclude) => ValueDomain::set(
            SetMode::Include,
            left.iter()
                .filter(|value| !right.contains(value))
                .cloned()
                .collect(),
        ),
        (SetMode::Exclude, SetMode::Include) => ValueDomain::set(
            SetMode::Include,
            right
                .iter()
                .filter(|value| !left.contains(value))
                .cloned()
                .collect(),
        ),
    }
}

fn union_set_domains(
    left_mode: SetMode,
    left: &[LiteralExpression],
    right_mode: SetMode,
    right: &[LiteralExpression],
) -> ValueDomain {
    match (left_mode, right_mode) {
        (SetMode::Include, SetMode::Include) => {
            let mut values = left.to_vec();
            values.extend_from_slice(right);
            ValueDomain::set(SetMode::Include, values)
        }
        (SetMode::Exclude, SetMode::Exclude) => ValueDomain::set(
            SetMode::Exclude,
            left.iter()
                .filter(|value| right.contains(value))
                .cloned()
                .collect(),
        ),
        (SetMode::Include, SetMode::Exclude) => ValueDomain::set(
            SetMode::Exclude,
            right
                .iter()
                .filter(|value| !left.contains(value))
                .cloned()
                .collect(),
        ),
        (SetMode::Exclude, SetMode::Include) => ValueDomain::set(
            SetMode::Exclude,
            left.iter()
                .filter(|value| !right.contains(value))
                .cloned()
                .collect(),
        ),
    }
}

fn intersect_ranges_and_set(
    ranges: &[ValueRange],
    mode: SetMode,
    values: &[LiteralExpression],
) -> ValueDomain {
    match mode {
        SetMode::Include => ValueDomain::set(
            SetMode::Include,
            values
                .iter()
                .filter(|value| {
                    ranges
                        .iter()
                        .any(|range| !matches!(literal_in_range(value, range), Some(false)))
                })
                .cloned()
                .collect(),
        ),
        SetMode::Exclude => subtract_excluded_values(ranges, values),
    }
}

fn subtract_excluded_values(ranges: &[ValueRange], values: &[LiteralExpression]) -> ValueDomain {
    let mut result = ranges.to_vec();

    for value in values {
        let mut next = Vec::new();
        for range in result {
            match literal_in_range(value, &range) {
                Some(true) => {
                    let left = ValueRange::new(
                        range.lower().cloned(),
                        Some(Bound::new(value.clone(), false)),
                    );
                    let right = ValueRange::new(
                        Some(Bound::new(value.clone(), false)),
                        range.upper().cloned(),
                    );
                    if !range_is_empty(&left) {
                        next.push(left);
                    }
                    if !range_is_empty(&right) {
                        next.push(right);
                    }
                }
                Some(false) | None => next.push(range),
            }
        }
        result = next;
    }

    ValueDomain::ranges(result)
}

fn intersect_range_domains(left: &[ValueRange], right: &[ValueRange]) -> ValueDomain {
    let mut ranges = Vec::new();

    for left_range in left {
        for right_range in right {
            if let Some(range) = intersect_range(left_range, right_range) {
                ranges.push(range);
            }
        }
    }

    ValueDomain::ranges(ranges)
}

fn intersect_range(left: &ValueRange, right: &ValueRange) -> Option<ValueRange> {
    let lower = tighter_lower(left.lower(), right.lower());
    let upper = tighter_upper(left.upper(), right.upper());
    let range = ValueRange::new(lower, upper);
    (!range_is_empty(&range)).then_some(range)
}

fn tighter_lower(left: Option<&Bound>, right: Option<&Bound>) -> Option<Bound> {
    match (left, right) {
        (None, None) => None,
        (Some(bound), None) | (None, Some(bound)) => Some(bound.clone()),
        (Some(left), Some(right)) => match compare_literals(left.value(), right.value()) {
            Some(Ordering::Less) => Some(right.clone()),
            Some(Ordering::Greater) => Some(left.clone()),
            Some(Ordering::Equal) => Some(Bound::new(
                left.value().clone(),
                left.inclusive() && right.inclusive(),
            )),
            None => Some(left.clone()),
        },
    }
}

fn tighter_upper(left: Option<&Bound>, right: Option<&Bound>) -> Option<Bound> {
    match (left, right) {
        (None, None) => None,
        (Some(bound), None) | (None, Some(bound)) => Some(bound.clone()),
        (Some(left), Some(right)) => match compare_literals(left.value(), right.value()) {
            Some(Ordering::Less) => Some(left.clone()),
            Some(Ordering::Greater) => Some(right.clone()),
            Some(Ordering::Equal) => Some(Bound::new(
                left.value().clone(),
                left.inclusive() && right.inclusive(),
            )),
            None => Some(left.clone()),
        },
    }
}

fn range_is_empty(range: &ValueRange) -> bool {
    match (range.lower(), range.upper()) {
        (Some(lower), Some(upper)) => match compare_literals(lower.value(), upper.value()) {
            Some(Ordering::Greater) => true,
            Some(Ordering::Equal) => !lower.inclusive() || !upper.inclusive(),
            Some(Ordering::Less) | None => false,
        },
        _ => false,
    }
}

fn literal_in_range(literal: &LiteralExpression, range: &ValueRange) -> Option<bool> {
    if matches!(literal.value(), LiteralValue::Null) {
        return Some(false);
    }

    if let Some(lower) = range.lower() {
        match compare_literals(literal, lower.value()) {
            Some(Ordering::Less) => return Some(false),
            Some(Ordering::Equal) if !lower.inclusive() => return Some(false),
            None => return None,
            Some(Ordering::Equal | Ordering::Greater) => {}
        }
    }

    if let Some(upper) = range.upper() {
        match compare_literals(literal, upper.value()) {
            Some(Ordering::Greater) => return Some(false),
            Some(Ordering::Equal) if !upper.inclusive() => return Some(false),
            None => return None,
            Some(Ordering::Less | Ordering::Equal) => {}
        }
    }

    Some(true)
}

fn compare_literals(left: &LiteralExpression, right: &LiteralExpression) -> Option<Ordering> {
    if left == right {
        return Some(Ordering::Equal);
    }

    match (numeric_text(left), numeric_text(right)) {
        (Some(left), Some(right)) => compare_numeric_text(left, right),
        _ => None,
    }
}

fn numeric_text(literal: &LiteralExpression) -> Option<&str> {
    match (literal.literal_type(), literal.value()) {
        (LiteralType::Integer | LiteralType::Decimal, LiteralValue::Number(value)) => Some(value),
        _ => None,
    }
}

#[derive(Debug)]
struct DecimalNumber {
    negative: bool,
    digits: String,
    scale: i64,
}

fn compare_numeric_text(left: &str, right: &str) -> Option<Ordering> {
    let left = parse_decimal_number(left)?;
    let right = parse_decimal_number(right)?;

    if left.digits == "0" && right.digits == "0" {
        return Some(Ordering::Equal);
    }

    if left.negative != right.negative {
        return Some(if left.negative {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }

    let ordering = compare_positive_decimal(&left, &right)?;
    Some(if left.negative {
        ordering.reverse()
    } else {
        ordering
    })
}

fn compare_positive_decimal(left: &DecimalNumber, right: &DecimalNumber) -> Option<Ordering> {
    let left_len = i64::try_from(left.digits.len()).ok()?;
    let right_len = i64::try_from(right.digits.len()).ok()?;
    let left_magnitude = left_len.checked_add(left.scale)?;
    let right_magnitude = right_len.checked_add(right.scale)?;

    match left_magnitude.cmp(&right_magnitude) {
        Ordering::Equal => {}
        ordering => return Some(ordering),
    }

    let length = left.digits.len().max(right.digits.len());
    let left_bytes = left.digits.as_bytes();
    let right_bytes = right.digits.as_bytes();

    for index in 0..length {
        let left_digit = left_bytes.get(index).copied().unwrap_or(b'0');
        let right_digit = right_bytes.get(index).copied().unwrap_or(b'0');
        match left_digit.cmp(&right_digit) {
            Ordering::Equal => {}
            ordering => return Some(ordering),
        }
    }

    Some(Ordering::Equal)
}

fn parse_decimal_number(value: &str) -> Option<DecimalNumber> {
    let (negative, unsigned) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else if let Some(value) = value.strip_prefix('+') {
        (false, value)
    } else {
        (false, value)
    };

    let exponent_index = unsigned.find(['e', 'E']);
    let (mantissa, exponent) = match exponent_index {
        Some(index) => {
            let (mantissa, exponent_with_marker) = unsigned.split_at(index);
            let exponent = exponent_with_marker.get(1..)?.parse::<i64>().ok()?;
            (mantissa, exponent)
        }
        None => (unsigned, 0),
    };

    let (integer, fraction) = match mantissa.split_once('.') {
        Some((integer, fraction)) if !fraction.contains('.') => (integer, fraction),
        Some(_) => return None,
        None => (mantissa, ""),
    };

    if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let mut digits = format!("{integer}{fraction}")
        .trim_start_matches('0')
        .to_string();

    if digits.is_empty() {
        return Some(DecimalNumber {
            negative: false,
            digits: "0".to_string(),
            scale: 0,
        });
    }

    let fraction_len = i64::try_from(fraction.len()).ok()?;
    let mut scale = exponent.checked_sub(fraction_len)?;

    while digits.len() > 1 && digits.ends_with('0') {
        let _ = digits.pop();
        scale = scale.checked_add(1)?;
    }

    Some(DecimalNumber {
        negative,
        digits,
        scale,
    })
}

fn null_literal() -> LiteralExpression {
    LiteralExpression::new(LiteralType::Null, LiteralValue::Null)
}
