//! Conservative execution of attested row-local generated column arithmetic.
//!
//! This exact integer-only subset cannot represent sequence state, arbitrary
//! casts or expressions whose operands depend on other generated columns.

use super::*;
use crate::column_sources::{ColumnValueKind, ColumnValueSource, GeneratedValueExpression};

pub(super) fn scalar_literal(value: &ConstraintValue) -> LiteralValue {
    match value {
        ConstraintValue::Null => LiteralValue::Null,
        ConstraintValue::Boolean(value) => LiteralValue::Boolean(*value),
        ConstraintValue::Integer(value) => LiteralValue::Number(value.to_string()),
        ConstraintValue::UnsignedInteger(value) => LiteralValue::Number(value.to_string()),
        ConstraintValue::Number(value) => LiteralValue::Number(value.clone()),
        ConstraintValue::String(value) => LiteralValue::Text(value.clone()),
    }
}

pub(super) fn authoritative(source: &ColumnValueSource) -> bool {
    source.evidence().iter().any(|evidence| {
        evidence.enforcement() == ConstraintEnforcement::Enforced
            || evidence.provenance().source_kind()
                == crate::constraints::ConstraintSourceKind::SqlDdl
    })
}


#[derive(Clone, Copy, PartialEq, Eq)]
enum CoalesceKind {
    SignedInteger,
    UnsignedInteger,
    Boolean,
    Text,
}

/// SQL resolves COALESCE argument types before considering which branches
/// happen to be NULL in a supplied physical row. Runtime literal categories
/// alone cannot establish a safe common type.
fn coalesce_kind(
    expression: &GeneratedValueExpression,
    schema: &RelationSchema,
) -> StateResult<Option<CoalesceKind>> {
    use GeneratedValueExpression as G;
    match expression {
        G::Column(name) => {
            let column = schema.columns().iter().find(|column| column.name() == name)
                .ok_or_else(|| residual("COALESCE references a column without declared type"))?;
            let mut kind = column.data_type();
            while let DataType::Nullable(inner) = kind {
                kind = inner;
            }
            match kind {
                DataType::SignedInteger { .. } => Ok(Some(CoalesceKind::SignedInteger)),
                DataType::UnsignedInteger { .. } => Ok(Some(CoalesceKind::UnsignedInteger)),
                DataType::Boolean => Ok(Some(CoalesceKind::Boolean)),
                DataType::String { .. } => Ok(Some(CoalesceKind::Text)),
                _ => Err(residual("COALESCE column has no attested portable type law")),
            }
        }
        G::Literal(ConstraintValue::Null) => Ok(None),
        G::Literal(ConstraintValue::Boolean(_)) => Ok(Some(CoalesceKind::Boolean)),
        G::Literal(ConstraintValue::String(_)) => Ok(Some(CoalesceKind::Text)),
        G::Literal(ConstraintValue::Integer(_)) => Ok(Some(CoalesceKind::SignedInteger)),
        G::Literal(ConstraintValue::UnsignedInteger(_)) => Ok(Some(CoalesceKind::UnsignedInteger)),
        G::Literal(ConstraintValue::Number(number)) if number.parse::<i128>().is_ok() => {
            Ok(Some(CoalesceKind::SignedInteger))
        }
        G::Literal(ConstraintValue::Number(_)) => {
            Err(residual("COALESCE numeric literal has unproved coercion"))
        }
        G::Abs(operand) => {
            if coalesce_kind(operand, schema)? == Some(CoalesceKind::SignedInteger) {
                Ok(Some(CoalesceKind::SignedInteger))
            } else {
                Err(residual("COALESCE ABS operand lacks an exact integer type"))
            }
        }
        G::Add(left, right) | G::Subtract(left, right) | G::Multiply(left, right) => {
            if coalesce_kind(left, schema)? == Some(CoalesceKind::SignedInteger)
                && coalesce_kind(right, schema)? == Some(CoalesceKind::SignedInteger)
            {
                Ok(Some(CoalesceKind::SignedInteger))
            } else {
                Err(residual("COALESCE arithmetic lacks exact integer type evidence"))
            }
        }
        G::Coalesce(arguments) => {
            let mut common = None;
            for argument in arguments {
                if let Some(kind) = coalesce_kind(argument, schema)? {
                    if common.is_some_and(|existing| existing != kind) {
                        return Err(residual("COALESCE arguments have incompatible declared types"));
                    }
                    common = Some(kind);
                }
            }
            Ok(common)
        }
    }
}

pub(super) fn evaluate(
    expression: &GeneratedValueExpression,
    values: &BTreeMap<String, LiteralValue>,
    schema: &RelationSchema,
) -> StateResult<LiteralValue> {
    use GeneratedValueExpression as G;
    match expression {
        G::Column(name) => values.get(name).cloned().ok_or_else(|| {
            residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                format!("generated expression depends on an unmaterialized column {name}"),
            )
        }),
        G::Literal(value) => Ok(scalar_literal(value)),
        G::Abs(operand) => {
            let value = evaluate(operand, values, schema)?;
            match value {
                LiteralValue::Null => Ok(LiteralValue::Null),
                LiteralValue::Number(value) => {
                    let integer = value
                        .parse::<i128>()
                        .map_err(|_| residual("ABS requires exact integer semantics"))?;
                    let positive = integer
                        .checked_abs()
                        .ok_or_else(|| residual("ABS overflows signed integer storage"))?;
                    Ok(LiteralValue::Number(positive.to_string()))
                }
                _ => Err(residual("ABS requires an exact integer operand")),
            }
        }
        G::Coalesce(arguments) => {
            // A NULL argument still has a declared type; do not infer SQL
            // coercion from only those branches that produced non-NULL rows.
            coalesce_kind(expression, schema)?;
            let values = arguments
                .iter()
                .map(|argument| evaluate(argument, values, schema))
                .collect::<StateResult<Vec<_>>>()?;
            // SQL resolves COALESCE argument types across every branch, even
            // branches not selected for this row. Never certify a mixed-type
            // expression on the strength of a coincidental chosen value.
            let mut chosen = LiteralValue::Null;
            let mut category = None;
            for value in values {
                if value == LiteralValue::Null {
                    continue;
                }
                if matches!(&value, LiteralValue::Number(number) if number.parse::<i128>().is_err())
                {
                    return Err(residual("COALESCE numeric coercion is not proved"));
                }
                let kind = std::mem::discriminant(&value);
                if category.is_some_and(|prior| prior != kind) {
                    return Err(residual(
                        "COALESCE argument types need a common attested law",
                    ));
                }
                category = Some(kind);
                if chosen == LiteralValue::Null {
                    chosen = value;
                }
            }
            Ok(chosen)
        }
        G::Add(left, right) | G::Subtract(left, right) | G::Multiply(left, right) => {
            let left = evaluate(left, values, schema)?;
            let right = evaluate(right, values, schema)?;
            if left == LiteralValue::Null || right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            let (LiteralValue::Number(left), LiteralValue::Number(right)) = (left, right) else {
                return Err(residual(
                    "generated arithmetic requires exact integer storage",
                ));
            };
            let left = left
                .parse::<i128>()
                .map_err(|_| residual("unproved generated numeric coercion"))?;
            let right = right
                .parse::<i128>()
                .map_err(|_| residual("unproved generated numeric coercion"))?;
            let result = if matches!(expression, G::Add(_, _)) {
                left.checked_add(right)
            } else if matches!(expression, G::Subtract(_, _)) {
                left.checked_sub(right)
            } else {
                left.checked_mul(right)
            }
            .ok_or_else(|| {
                residual("generated integer arithmetic exceeds exact representable range")
            })?;
            Ok(LiteralValue::Number(result.to_string()))
        }
    }
}

/// Topological order for row-local generated values. Dependency cycles
/// cannot be proved by a self-consistent supplied row: each value would be
/// justified only by another value the caller also supplied.
fn ordered_generated_sources(
    sources: &[ColumnValueSource],
) -> StateResult<Vec<&ColumnValueSource>> {
    fn visit<'a>(
        source: &'a ColumnValueSource,
        sources: &'a [ColumnValueSource],
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        ordered: &mut Vec<&'a ColumnValueSource>,
    ) -> StateResult<()> {
        if visited.contains(source.column()) {
            return Ok(());
        }
        if !visiting.insert(source.column().to_string()) {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "cyclic generated expression dependencies have no execution order",
            ));
        }
        if !authoritative(source) {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "generated expression lacks authoritative execution evidence",
            ));
        }
        let expression = source
            .expression()
            .ok_or_else(|| residual("generated column has no executable expression law"))?;
        for referenced in expression.referenced_columns() {
            let dependency = sources.iter().find(|other| {
                other.column() == referenced && other.kind() == ColumnValueKind::Generated
            });
            if let Some(dependency) = dependency {
                // Some dialects reject one generated column referring to
                // another. Explicit enforced metadata is required to attest
                // that this dependency is legal for the physical relation.
                if !source
                    .evidence()
                    .iter()
                    .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
                {
                    return Err(residual_gap(
                        PhysicalWriteStateGap::UnprovedConstraint,
                        "generated-to-generated dependency needs authoritative engine evidence",
                    ));
                }
                visit(dependency, sources, visiting, visited, ordered)?;
            }
        }
        visiting.remove(source.column());
        visited.insert(source.column().to_string());
        ordered.push(source);
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    for source in sources {
        if source.kind() == ColumnValueKind::Generated {
            visit(source, sources, &mut visiting, &mut visited, &mut ordered)?;
        }
    }
    Ok(ordered)
}

pub(super) fn check_declared_sources(
    sources: &[ColumnValueSource],
    row: &PhysicalWriteRow,
    controlled_identity_columns: &BTreeSet<String>,
    schema: &RelationSchema,
) -> StateResult<()> {
    for source in sources {
        if source.kind() == ColumnValueKind::Identity
            && !controlled_identity_columns.contains(source.column())
        {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "identity value needs an attested sequence and allocation-state law",
            ));
        }
    }
    for source in ordered_generated_sources(sources)? {
        let expression = source
            .expression()
            .ok_or_else(|| residual("generated expression has no executable semantics"))?;
        let actual = row
            .values
            .get(source.column())
            .ok_or_else(|| impossible("physical row is missing its declared generated column"))?;
        let expected = evaluate(expression, &row.values, schema)?;
        if !matches!(
            (&expected, actual),
            (LiteralValue::Number(_), LiteralValue::Number(_))
                | (LiteralValue::Null, LiteralValue::Null)
                | (LiteralValue::Boolean(_), LiteralValue::Boolean(_))
                | (LiteralValue::Text(_), LiteralValue::Text(_))
        ) {
            return Err(residual(
                "generated value comparison lacks exact numeric or boolean law",
            ));
        }
        if !super::evaluator::scalar_equality(&expected, actual)? {
            return Err(impossible(
                "generated column contradicts its attested expression",
            ));
        }
    }
    Ok(())
}

pub(super) fn complete_generated(
    sources: &[ColumnValueSource],
    values: &mut BTreeMap<String, LiteralValue>,
    schema: &RelationSchema,
) -> StateResult<()> {
    let ordered = ordered_generated_sources(sources)?;
    // Do not overwrite an explicitly supplied generated column, even when
    // the value happens to match its declared expression.
    if ordered
        .iter()
        .any(|source| values.contains_key(source.column()))
    {
        return Err(residual(
            "direct INSERT assignment to generated column is unproved",
        ));
    }
    for source in ordered {
        let expression = source
            .expression()
            .ok_or_else(|| residual("generated column has no executable expression law"))?;
        let value = evaluate(expression, values, schema)?;
        values.insert(source.column().to_string(), value);
    }
    Ok(())
}
