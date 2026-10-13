//! Relation constraints and typed contract values.

use super::*;

pub(super) fn relation_constraint_set_to_value(set: &RelationConstraintSet) -> Value {
    let constraints = set
        .constraints()
        .iter()
        .map(|constraint| {
            let evidence = constraint
                .evidence()
                .iter()
                .map(|evidence| {
                    json!({
                        "source_kind": evidence.provenance().source_kind().as_str(),
                        "source_id": evidence.provenance().source_id(),
                        "enforcement": evidence.enforcement().as_str()
                    })
                })
                .collect::<Vec<_>>();
            match constraint {
                RelationConstraint::PrimaryKey(key) => json!({
                    "kind": "primary_key",
                    "columns": key.columns(),
                    "evidence": evidence
                }),
                RelationConstraint::UniqueKey(key) => json!({
                    "kind": "unique_key",
                    "columns": key.columns(),
                    "evidence": evidence
                }),
                RelationConstraint::ForeignKey(key) => json!({
                    "kind": "foreign_key",
                    "columns": key.columns(),
                    "referenced_relation": key.referenced_relation(),
                    "referenced_columns": key.referenced_columns(),
                    "evidence": evidence
                }),
                RelationConstraint::NotNull(constraint) => json!({
                    "kind": "not_null",
                    "column": constraint.column(),
                    "evidence": evidence
                }),
                RelationConstraint::CheckComparison(constraint) => json!({
                    "kind": "check_comparison",
                    "column": constraint.column(),
                    "operator": constraint.operator().as_str(),
                    "value": constraint_value_to_value(constraint.value()),
                    "evidence": evidence
                }),
                RelationConstraint::AcceptedValues(constraint) => json!({
                    "kind": "accepted_values",
                    "column": constraint.column(),
                    "values": constraint
                        .values()
                        .iter()
                        .map(constraint_value_to_value)
                        .collect::<Vec<_>>(),
                    "quote": constraint.quote(),
                    "evidence": evidence
                }),
            }
        })
        .collect::<Vec<_>>();

    let mut value = json!({
        "relation": set.relation(),
        "constraints": constraints
    });
    if !set.computed_checks().is_empty() {
        value["computed_checks"] = json!(set
            .computed_checks()
            .iter()
            .map(|check| {
                json!({
                    "left": generated_expression_to_value(check.left()),
                    "operator": check.operator().as_str(),
                    "right": generated_expression_to_value(check.right()),
                    "evidence": check.evidence().iter().map(|evidence| json!({
                        "source_kind": evidence.provenance().source_kind().as_str(),
                        "source_id": evidence.provenance().source_id(),
                        "enforcement": evidence.enforcement().as_str(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>());
    }
    if !set.computed_boolean_checks().is_empty() {
        value["computed_boolean_checks"] = json!(set
            .computed_boolean_checks()
            .iter()
            .map(|check| {
                json!({
                    "predicate": computed_boolean_predicate_to_value(check.predicate()),
                    "evidence": check.evidence().iter().map(|evidence| json!({
                        "source_kind": evidence.provenance().source_kind().as_str(),
                        "source_id": evidence.provenance().source_id(),
                        "enforcement": evidence.enforcement().as_str(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>());
    }
    if !set.computed_keys().is_empty() {
        value["computed_keys"] = json!(set.computed_keys().iter().map(|key| {
            json!({
                "primary": key.primary(),
                "expressions": key.expressions().iter().map(generated_expression_to_value).collect::<Vec<_>>(),
                "evidence": key.evidence().iter().map(|evidence| json!({
                    "source_kind": evidence.provenance().source_kind().as_str(),
                    "source_id": evidence.provenance().source_id(),
                    "enforcement": evidence.enforcement().as_str(),
                })).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>());
    }
    if !set.column_value_sources().is_empty() {
        value["column_value_sources"] = json!(set
            .column_value_sources()
            .iter()
            .map(|source| {
                json!({
                    "column": source.column(),
                    "kind": source.kind().as_str(),
                    "value": source.value().map(constraint_value_to_value),
                    "expression": source.expression().map(generated_expression_to_value),
                    "evidence": source.evidence().iter().map(|evidence| json!({
                        "source_kind": evidence.provenance().source_kind().as_str(),
                        "source_id": evidence.provenance().source_id(),
                        "enforcement": evidence.enforcement().as_str()
                    })).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>());
    }
    if !set.diagnostics().is_empty() {
        value["diagnostics"] = json!(set
            .diagnostics()
            .iter()
            .map(constraint_diagnostic_to_value)
            .collect::<Vec<_>>());
    }
    value
}

pub(super) fn constraint_diagnostic_to_value(diagnostic: &ConstraintDiagnostic) -> Value {
    json!({
        "code": diagnostic.code(),
        "message": diagnostic.message()
    })
}

fn computed_boolean_predicate_to_value(
    predicate: &crate::computed_boolean_checks::ComputedBooleanCheckPredicate,
) -> Value {
    use crate::computed_boolean_checks::ComputedBooleanCheckPredicate as P;
    match predicate {
        P::Comparison {
            left,
            operator,
            right,
        } => json!({
            "kind":"comparison",
            "left":generated_expression_to_value(left),
            "operator":operator.as_str(),
            "right":generated_expression_to_value(right),
        }),
        P::IsNull {
            expression,
            negated,
        } => json!({
            "kind":"is_null",
            "expression":generated_expression_to_value(expression),
            "negated":negated,
        }),
        P::And(children) => json!({
            "kind":"and",
            "children":children.iter().map(computed_boolean_predicate_to_value).collect::<Vec<_>>(),
        }),
        P::Or(children) => json!({
            "kind":"or",
            "children":children.iter().map(computed_boolean_predicate_to_value).collect::<Vec<_>>(),
        }),
        P::Not(child) => json!({
            "kind":"not",
            "operand":computed_boolean_predicate_to_value(child),
        }),
    }
}

fn generated_expression_to_value(
    expression: &crate::column_sources::GeneratedValueExpression,
) -> Value {
    use crate::column_sources::GeneratedValueExpression as G;
    match expression {
        G::Column(column) => json!({"kind":"column","column":column}),
        G::Literal(value) => json!({"kind":"literal","value":constraint_value_to_value(value)}),
        G::Add(left, right) => json!({
            "kind":"add","left":generated_expression_to_value(left),"right":generated_expression_to_value(right)
        }),
        G::Subtract(left, right) => json!({
            "kind":"subtract","left":generated_expression_to_value(left),"right":generated_expression_to_value(right)
        }),
        G::Multiply(left, right) => json!({
            "kind":"multiply","left":generated_expression_to_value(left),"right":generated_expression_to_value(right)
        }),
        G::Abs(operand) => json!({
            "kind":"abs","operand":generated_expression_to_value(operand)
        }),
        G::Coalesce(arguments) => json!({
            "kind":"coalesce",
            "arguments":arguments.iter().map(generated_expression_to_value).collect::<Vec<_>>()
        }),
    }
}

pub(super) fn constraint_value_to_value(value: &ConstraintValue) -> Value {
    match value {
        ConstraintValue::Null => json!({ "type": "null", "value": null }),
        ConstraintValue::Boolean(value) => json!({ "type": "boolean", "value": value }),
        ConstraintValue::Integer(value) => json!({ "type": "integer", "value": value }),
        ConstraintValue::UnsignedInteger(value) => {
            json!({ "type": "unsigned_integer", "value": value })
        }
        ConstraintValue::Number(value) => json!({ "type": "number", "value": value }),
        ConstraintValue::String(value) => json!({ "type": "string", "value": value }),
    }
}

pub(super) fn data_type_to_value(data_type: &DataType) -> Value {
    match data_type {
        DataType::Boolean => json!({ "kind": "boolean" }),
        DataType::SignedInteger { bits } => {
            json!({ "kind": "signed_integer", "bits": bits })
        }
        DataType::UnsignedInteger { bits } => {
            json!({ "kind": "unsigned_integer", "bits": bits })
        }
        DataType::Decimal { precision, scale } => {
            json!({ "kind": "decimal", "precision": precision, "scale": scale })
        }
        DataType::FloatingPoint { bits } => {
            json!({ "kind": "floating_point", "bits": bits })
        }
        DataType::String { length, fixed } => {
            json!({ "kind": "string", "length": length, "fixed": fixed })
        }
        DataType::Binary { length, fixed } => {
            json!({ "kind": "binary", "length": length, "fixed": fixed })
        }
        DataType::Date => json!({ "kind": "date" }),
        DataType::Time { precision } => {
            json!({ "kind": "time", "precision": precision })
        }
        DataType::Timestamp { precision } => {
            json!({ "kind": "timestamp", "precision": precision })
        }
        DataType::Interval => json!({ "kind": "interval" }),
        DataType::Uuid => json!({ "kind": "uuid" }),
        DataType::Json => json!({ "kind": "json" }),
        DataType::BitString { length } => {
            json!({ "kind": "bit_string", "length": length })
        }
        DataType::Array { element, length } => json!({
            "kind": "array",
            "element": element.as_deref().map(data_type_to_value),
            "length": length
        }),
        DataType::Map { key, value } => json!({
            "kind": "map",
            "key": data_type_to_value(key),
            "value": data_type_to_value(value)
        }),
        DataType::Struct { fields } => json!({
            "kind": "struct",
            "fields": fields
                .iter()
                .map(|field| json!({
                    "name": field.name(),
                    "data_type": data_type_to_value(field.data_type())
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Union { fields } => json!({
            "kind": "union",
            "fields": fields
                .iter()
                .map(|field| json!({
                    "name": field.name(),
                    "data_type": data_type_to_value(field.data_type())
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Enum { values } => json!({
            "kind": "enum",
            "values": values
                .iter()
                .map(|value| json!({
                    "name": value.name(),
                    "value": value.value()
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Set { values } => json!({
            "kind": "set",
            "values": values
        }),
        DataType::Table { name, fields } => json!({
            "kind": "table",
            "name": name,
            "fields": fields
                .iter()
                .map(|field| json!({
                    "name": field.name(),
                    "data_type": data_type_to_value(field.data_type())
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Geometry { kind } => json!({
            "kind": "geometry",
            "geometry_kind": kind
        }),
        DataType::Regclass => json!({ "kind": "regclass" }),
        DataType::TextSearchVector => json!({ "kind": "text_search_vector" }),
        DataType::TextSearchQuery => json!({ "kind": "text_search_query" }),
        DataType::Nullable(inner) => json!({
            "kind": "nullable",
            "inner": data_type_to_value(inner)
        }),
        DataType::Any => json!({ "kind": "any" }),
        DataType::Unspecified => json!({ "kind": "unspecified" }),
        DataType::Trigger => json!({ "kind": "trigger" }),
        DataType::Custom { name, modifiers } => json!({
            "kind": "custom",
            "name": name,
            "modifiers": modifiers
        }),
    }
}
