//! Typed source schema validation and literal canonicalization.

use super::*;

/// Verify physical column references where typed source schema evidence exists.
pub(super) fn validate_schema_column_references(
    query: &SqlQuery,
    predicates: &Predicates,
    output: &Output,
    joins: &[ProtocolJoin],
    sources: &[SourceRelation],
    metadata: &AnalysisMetadata<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ResidualCondition> {
    let mut columns = BTreeSet::new();
    for output_column in output.columns() {
        collect_expression_column_refs(output_column.expression(), sources, &mut columns);
    }
    for predicate in [
        predicates.where_predicate(),
        predicates.having_predicate(),
        predicates.qualify_predicate(),
    ]
    .into_iter()
    .flatten()
    {
        collect_predicate_column_refs(predicate, sources, &mut columns);
    }
    for join in joins {
        if let Some(predicate) = join.condition() {
            collect_predicate_column_refs(predicate, sources, &mut columns);
        }
    }

    let mut residuals = Vec::new();
    for column in columns {
        let Some(relation) = column.relation() else {
            continue;
        };
        let Some(schema_columns) = metadata.schema_columns(relation) else {
            continue;
        };
        if schema_columns
            .iter()
            .any(|declared| declared == column.name())
        {
            continue;
        }
        let reference = format!("{relation}.{}", column.name());
        diagnostics.push(warning(
            "unknown_schema_column",
            DiagnosticArea::Source,
            &format!("column '{reference}' is absent from available typed schema evidence"),
        ));
        if output.columns().iter().any(|projected| {
            let mut references = BTreeSet::new();
            collect_expression_column_refs(projected.expression(), sources, &mut references);
            references.contains(&column)
        }) {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::UnknownSchemaColumn,
                ConditionClause::Select,
                format!("select:unknown_schema_column:{reference}"),
            ));
        }
        for (clause, identity) in
            condition_locations_for_column(&column, predicates, joins, sources)
        {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::UnknownSchemaColumn,
                clause,
                format!("{identity}:unknown_schema_column:{reference}"),
            ));
        }
    }

    // Remapping an invalid physical equality can replace the missing column
    // with Unknown before the normalized join reaches schema validation.
    // Inspect the original ON expression for reference evidence only.
    if let SetExpr::Select(select) = query.body.as_ref() {
        let mut join_index = 0usize;
        for table in &select.from {
            for join in &table.joins {
                if let (_, Some(JoinConstraint::On(expression)), _) =
                    analyze_join_operator(&join.join_operator)
                {
                    let predicate = analyze_predicate(expression, &mut Vec::new());
                    let mut raw_columns = BTreeSet::new();
                    collect_predicate_column_refs(&predicate, sources, &mut raw_columns);
                    for column in raw_columns {
                        let Some(relation) = column.relation() else {
                            continue;
                        };
                        let Some(schema_columns) = metadata.schema_columns(relation) else {
                            continue;
                        };
                        if schema_columns
                            .iter()
                            .any(|declared| declared == column.name())
                        {
                            continue;
                        }
                        let reference = format!("{relation}.{}", column.name());
                        let diagnostic = warning(
                            "unknown_schema_column",
                            DiagnosticArea::Source,
                            &format!("column '{reference}' is absent from available typed schema evidence"),
                        );
                        if !diagnostics.contains(&diagnostic) {
                            diagnostics.push(diagnostic);
                        }
                        residuals.push(ResidualCondition::new(
                            ResidualConditionReason::UnknownSchemaColumn,
                            ConditionClause::JoinOn,
                            format!("join:{join_index}:unknown_schema_column:{reference}"),
                        ));
                    }
                }
                join_index += 1;
            }
        }
    }
    ConditionExactness::from_residuals(residuals)
        .residual_conditions()
        .to_vec()
}

/// Keep the category of a rejected typed comparison separate from its human-readable explanation.
#[derive(Debug)]
pub(super) struct TypedLiteralError {
    pub(super) reason: ResidualConditionReason,
    pub(super) message: String,
}

impl TypedLiteralError {
    fn new(reason: ResidualConditionReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

/// Find the actual membership clauses that reference a physical column.
pub(super) fn condition_locations_for_column(
    column: &ColumnRef,
    predicates: &Predicates,
    joins: &[ProtocolJoin],
    sources: &[SourceRelation],
) -> Vec<(ConditionClause, String)> {
    let mut locations = Vec::new();
    for (clause, identity, predicate) in [
        (
            ConditionClause::Where,
            "where",
            predicates.where_predicate(),
        ),
        (
            ConditionClause::Having,
            "having",
            predicates.having_predicate(),
        ),
        (
            ConditionClause::Qualify,
            "qualify",
            predicates.qualify_predicate(),
        ),
    ] {
        if let Some(predicate) = predicate {
            let mut columns = BTreeSet::new();
            collect_predicate_column_refs(predicate, sources, &mut columns);
            if columns.contains(column) {
                locations.push((clause, identity.to_string()));
            }
        }
    }
    for (index, join) in joins.iter().enumerate() {
        if let Some(predicate) = join.condition() {
            let mut columns = BTreeSet::new();
            collect_predicate_column_refs(predicate, sources, &mut columns);
            if columns.contains(column) {
                locations.push((ConditionClause::JoinOn, format!("join:{index}")));
            }
        }
    }
    locations
}

pub(super) fn type_column_domains(
    column_domains: Vec<ColumnDomain>,
    metadata: &AnalysisMetadata<'_>,
) -> (Vec<ColumnDomain>, Vec<(ColumnRef, ResidualConditionReason)>) {
    let mut failures = Vec::new();
    let domains = column_domains
        .into_iter()
        .map(|column_domain| {
            let Some(data_type) = metadata.column_data_type(column_domain.column()) else {
                return column_domain;
            };
            let domain = match type_value_domain(
                column_domain.domain(),
                data_type,
                metadata.column_timestamp_zone(column_domain.column()),
            ) {
                Ok(domain) => domain,
                Err(error) => {
                    failures.push((column_domain.column().clone(), error.reason));
                    ValueDomain::unknown(error.message)
                }
            };
            ColumnDomain::new(column_domain.column().clone(), domain)
        })
        .collect();
    (domains, failures)
}

pub(super) fn type_value_domain(
    domain: &ValueDomain,
    data_type: &DataType,
    zone: Option<TimestampZone>,
) -> Result<ValueDomain, TypedLiteralError> {
    match domain {
        ValueDomain::Ranges(ranges) => {
            let ranges = ranges
                .ranges()
                .iter()
                .map(|range| {
                    let lower = range
                        .lower()
                        .map(|bound| type_bound(bound, data_type, zone))
                        .transpose()?;
                    let upper = range
                        .upper()
                        .map(|bound| type_bound(bound, data_type, zone))
                        .transpose()?;
                    Ok(ValueRange::new(lower, upper))
                })
                .collect::<Result<Vec<_>, TypedLiteralError>>()?;
            Ok(ValueDomain::ranges(ranges))
        }
        ValueDomain::Set(set) => {
            let values = set
                .values()
                .iter()
                .map(|literal| type_literal(literal, data_type, zone))
                .collect::<Result<Vec<_>, TypedLiteralError>>()?;
            Ok(ValueDomain::set(set.mode(), values))
        }
        ValueDomain::Unbounded => Ok(ValueDomain::Unbounded),
        ValueDomain::Empty => Ok(ValueDomain::Empty),
        ValueDomain::Unknown(unknown) => Ok(ValueDomain::unknown(unknown.reason())),
    }
}

pub(super) fn type_bound(
    bound: &Bound,
    data_type: &DataType,
    zone: Option<TimestampZone>,
) -> Result<Bound, TypedLiteralError> {
    Ok(Bound::new(
        type_literal(bound.value(), data_type, zone)?,
        bound.inclusive(),
    ))
}

pub(super) fn type_literal(
    literal: &LiteralExpression,
    data_type: &DataType,
    zone: Option<TimestampZone>,
) -> Result<LiteralExpression, TypedLiteralError> {
    if literal.literal_type() == LiteralType::Null {
        return Ok(literal.clone());
    }

    let data_type = match data_type {
        DataType::Nullable(inner) => inner.as_ref(),
        other => other,
    };
    let incompatible = || {
        Err(TypedLiteralError::new(
            ResidualConditionReason::LiteralTypeMismatch,
            format!(
                "literal type '{}' cannot be compared exactly with canonical '{}' column semantics",
                literal.literal_type().as_str(),
                data_type.kind()
            ),
        ))
    };

    match data_type {
        DataType::Boolean => match literal.value() {
            LiteralValue::Boolean(value) => Ok(LiteralExpression::new(
                LiteralType::Boolean,
                LiteralValue::Boolean(*value),
            )),
            _ => incompatible(),
        },
        DataType::SignedInteger { bits } => {
            type_integer_literal(literal, *bits, false)
        }
        DataType::UnsignedInteger { bits } => {
            type_integer_literal(literal, *bits, true)
        }
        DataType::Decimal { precision, scale } => match literal.value() {
            LiteralValue::Number(value) if decimal_literal_fits(value, *precision, *scale) => {
                Ok(LiteralExpression::new(
                    LiteralType::Decimal,
                    LiteralValue::Number(value.clone()),
                ))
            }
            LiteralValue::Number(_) => Err(TypedLiteralError::new(
                ResidualConditionReason::OutOfRangeLiteral,
                "numeric literal exceeds the declared decimal precision or scale",
            )),
            _ => incompatible(),
        },
        DataType::FloatingPoint { .. } => match literal.value() {
            LiteralValue::Number(_) => Ok(literal.clone()),
            _ => incompatible(),
        },
        DataType::Date if literal.literal_type() == LiteralType::Date => Ok(literal.clone()),
        DataType::Time { .. } if literal.literal_type() == LiteralType::Time => Ok(literal.clone()),
        DataType::Timestamp { .. } if literal.literal_type() == LiteralType::Timestamp => {
            let LiteralValue::Text(text) = literal.value() else {
                return incompatible();
            };
            let Some((canonical, has_offset)) = canonical_timestamp_text(text) else {
                return Err(TypedLiteralError::new(
                    ResidualConditionReason::ComparisonSemantics,
                    "timestamp literal cannot be normalized to the protocol's canonical format",
                ));
            };
            if zone == Some(TimestampZone::WithoutTimeZone) && has_offset {
                return Err(TypedLiteralError::new(
                    ResidualConditionReason::LiteralTypeMismatch,
                    "offset-bearing timestamp literal conflicts with timestamp without time zone",
                ));
            }
            Ok(LiteralExpression::new(
                LiteralType::Timestamp,
                LiteralValue::Text(canonical),
            ))
        },
        DataType::Interval if literal.literal_type() == LiteralType::Interval => Ok(literal.clone()),
        DataType::String { .. } if literal.literal_type() == LiteralType::String => Ok(literal.clone()),
        DataType::Enum { .. } | DataType::Set { .. } => Err(TypedLiteralError::new(
            ResidualConditionReason::ComparisonSemantics,
            "enum and set predicate domains are residual because comparison semantics are dialect-dependent",
        )),
        DataType::Any | DataType::Unspecified | DataType::Custom { .. } => Err(
            TypedLiteralError::new(
                ResidualConditionReason::ComparisonSemantics,
                format!(
                    "canonical '{}' datatype does not define exact comparison semantics",
                    data_type.kind()
                ),
            ),
        ),
        DataType::Binary { .. }
        | DataType::Uuid
        | DataType::Json
        | DataType::BitString { .. }
        | DataType::Array { .. }
        | DataType::Map { .. }
        | DataType::Struct { .. }
        | DataType::Union { .. }
        | DataType::Table { .. }
        | DataType::Geometry { .. }
        | DataType::Regclass
        | DataType::TextSearchVector
        | DataType::TextSearchQuery
        | DataType::Trigger => Err(TypedLiteralError::new(
            ResidualConditionReason::ComparisonSemantics,
            format!(
                "canonical '{}' datatype has no protocol-defined ordered scalar comparison semantics",
                data_type.kind()
            ),
        )),
        DataType::Nullable(_) => unreachable!("nullable datatype was unwrapped above"),
        _ => incompatible(),
    }
}

pub(super) fn decimal_literal_fits(value: &str, precision: Option<u64>, scale: Option<u64>) -> bool {
    let mantissa = value
        .split(['e', 'E'])
        .next()
        .unwrap_or(value)
        .trim_start_matches(['+', '-']);
    let mut parts = mantissa.split('.');
    let integer = parts.next().unwrap_or_default().trim_start_matches('0');
    let fraction = parts.next().unwrap_or_default().trim_end_matches('0');
    if parts.next().is_some() {
        return false;
    }
    let used_scale = fraction.len() as u64;
    let used_precision = integer.len().max(1) as u64 + used_scale;
    scale.is_none_or(|scale| used_scale <= scale)
        && precision.is_none_or(|precision| used_precision <= precision)
}

pub(super) fn type_integer_literal(
    literal: &LiteralExpression,
    bits: Option<u16>,
    unsigned: bool,
) -> Result<LiteralExpression, TypedLiteralError> {
    if literal.literal_type() != LiteralType::Integer {
        return Err(TypedLiteralError::new(
            ResidualConditionReason::LossyCoercion,
            "lossy numeric coercion to an integer column is not exact",
        ));
    }
    let LiteralValue::Number(value) = literal.value() else {
        return Err(TypedLiteralError::new(
            ResidualConditionReason::LiteralTypeMismatch,
            "integer literal does not contain a numeric payload",
        ));
    };
    let in_range = if unsigned {
        value
            .parse::<u128>()
            .is_ok_and(|value| integer_fits_unsigned(value, bits))
    } else {
        value
            .parse::<i128>()
            .is_ok_and(|value| integer_fits_signed(value, bits))
    };
    if !in_range {
        return Err(TypedLiteralError::new(
            ResidualConditionReason::OutOfRangeLiteral,
            format!(
                "integer literal '{value}' is outside the canonical {}-bit {}integer range",
                bits.map_or_else(|| "unbounded".to_string(), |bits| bits.to_string()),
                if unsigned { "unsigned " } else { "" }
            ),
        ));
    }
    Ok(LiteralExpression::new(
        LiteralType::Integer,
        LiteralValue::Number(value.clone()),
    ))
}

pub(super) fn integer_fits_signed(value: i128, bits: Option<u16>) -> bool {
    match bits {
        None | Some(128..=u16::MAX) => true,
        Some(0) => false,
        Some(bits) => {
            let limit = 1_i128 << (bits - 1);
            value >= -limit && value < limit
        }
    }
}

pub(super) fn integer_fits_unsigned(value: u128, bits: Option<u16>) -> bool {
    match bits {
        None | Some(128..=u16::MAX) => true,
        Some(0) => false,
        Some(bits) => value < (1_u128 << bits),
    }
}

/// Canonicalize a timestamp without applying any warehouse-specific timezone conversion.
///
/// Explicit offsets are normalized to +HH:MM/-HH:MM; Z is written as +00:00.
/// The local wall-clock portion is not shifted because its interpretation belongs to
/// the physical column type and, when necessary, a declared session setting.
pub(super) fn canonical_timestamp_text(input: &str) -> Option<(String, bool)> {
    let value = input.trim();
    let bytes = value.as_bytes();
    if bytes.len() < 19
        || ![0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|index| bytes[*index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b' ' | b'T' | b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }

    let year = value.get(0..4)?.parse::<u16>().ok()?;
    let month = value.get(5..7)?.parse::<u8>().ok()?;
    let day = value.get(8..10)?.parse::<u8>().ok()?;
    let hour = value.get(11..13)?.parse::<u8>().ok()?;
    let minute = value.get(14..16)?.parse::<u8>().ok()?;
    let second = value.get(17..19)?.parse::<u8>().ok()?;
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return None,
    };
    if year == 0 || day == 0 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let mut suffix = value.get(19..)?;
    let mut fraction = "";
    if let Some(after_dot) = suffix.strip_prefix('.') {
        let count = after_dot.bytes().take_while(u8::is_ascii_digit).count();
        if count == 0 {
            return None;
        }
        fraction = after_dot.get(..count)?.trim_end_matches('0');
        suffix = after_dot.get(count..)?;
    }
    suffix = suffix.trim_start();

    let (offset, has_offset) = if suffix.is_empty() {
        (String::new(), false)
    } else if suffix.eq_ignore_ascii_case("z") {
        ("+00:00".to_string(), true)
    } else {
        let (sign, digits) = suffix.split_at(1);
        if sign != "+" && sign != "-" {
            return None;
        }
        let (hours, minutes) = match digits.len() {
            2 if digits.bytes().all(|byte| byte.is_ascii_digit()) => {
                (digits.parse::<u8>().ok()?, 0)
            }
            4 if digits.bytes().all(|byte| byte.is_ascii_digit()) => (
                digits.get(..2)?.parse::<u8>().ok()?,
                digits.get(2..)?.parse::<u8>().ok()?,
            ),
            5 if digits.as_bytes()[2] == b':'
                && digits.get(..2)?.bytes().all(|byte| byte.is_ascii_digit())
                && digits.get(3..)?.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                (
                    digits.get(..2)?.parse::<u8>().ok()?,
                    digits.get(3..)?.parse::<u8>().ok()?,
                )
            }
            _ => return None,
        };
        if hours > 14 || minutes > 59 || (hours == 14 && minutes != 0) {
            return None;
        }
        (format!("{sign}{hours:02}:{minutes:02}"), true)
    };

    let fraction = if fraction.is_empty() {
        String::new()
    } else {
        format!(".{fraction}")
    };
    Some((
        format!("{} {}{fraction}{offset}", &value[..10], &value[11..19]),
        has_offset,
    ))
}

pub(super) fn timestamp_literal_has_offset(literal: &LiteralExpression) -> bool {
    match literal.value() {
        LiteralValue::Text(text) => {
            canonical_timestamp_text(text).is_some_and(|(_, has_offset)| has_offset)
        }
        _ => false,
    }
}
