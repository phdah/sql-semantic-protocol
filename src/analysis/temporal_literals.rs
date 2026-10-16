//! Certified calendar literals and formatting-free temporal identity casts.
//!
//! No clock, timezone database, session coercion or vendor function is executed here.

use super::*;

pub(super) fn validated_temporal_text(kind: LiteralType, text: &str) -> Option<String> {
    match kind {
        LiteralType::Date => {
            crate::temporal::canonical_date_text(text)?;
            Some(text.to_string())
        }
        LiteralType::Time => {
            let (_, has_offset) = canonical_timestamp_text(&format!("2000-01-01 {text}"))?;
            (!has_offset).then(|| text.to_string())
        }
        LiteralType::Timestamp => {
            canonical_timestamp_text(text)?;
            Some(text.to_string())
        }
        // Interval ordering/units and other temporal spellings have no
        // portable calendar law merely because the parser accepts a string.
        LiteralType::Interval
        | LiteralType::Null
        | LiteralType::Boolean
        | LiteralType::Integer
        | LiteralType::Decimal
        | LiteralType::String => None,
    }
}

pub(super) fn normalize_temporal_identity_cast(
    kind: &SqlCastKind,
    target: &sqlparser::ast::DataType,
    operand: &Expression,
) -> Option<Expression> {
    if !matches!(
        kind,
        SqlCastKind::Cast | SqlCastKind::TryCast | SqlCastKind::SafeCast | SqlCastKind::DoubleColon
    ) {
        return None;
    }
    let Expression::Literal(literal) = operand else {
        return None;
    };
    if !matches!(target, sqlparser::ast::DataType::Date)
        || literal.literal_type() != LiteralType::Date
    {
        return None;
    }
    let LiteralValue::Text(text) = literal.value() else {
        return None;
    };
    // Reuse the canonical Gregorian calendar validator; this does not turn a
    // text-to-date conversion into an identity or choose a warehouse date style.
    validated_temporal_text(LiteralType::Date, text)?;
    Some(operand.clone())
}

pub(super) fn normalize_date_literal_extract(
    field: &sqlparser::ast::DateTimeField,
    operand: &Expression,
) -> Option<Expression> {
    use sqlparser::ast::DateTimeField;
    let Expression::Literal(literal) = operand else {
        return None;
    };
    if literal.literal_type() != LiteralType::Date {
        return None;
    }
    let LiteralValue::Text(text) = literal.value() else {
        return None;
    };
    validated_temporal_text(LiteralType::Date, text)?;
    let part = match field {
        DateTimeField::Year => text.get(..4)?,
        DateTimeField::Month => text.get(5..7)?,
        DateTimeField::Day => text.get(8..10)?,
        // Week numbering, epochs, timezone fields and fractional units need
        // independent execution laws; a parsed EXTRACT does not certify them.
        _ => return None,
    };
    let value = part.parse::<u16>().ok()?;
    Some(Expression::Literal(LiteralExpression::new(
        LiteralType::Integer,
        LiteralValue::Number(value.to_string()),
    )))
}
