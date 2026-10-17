//! Output projections, lineage, source relations and value domains.

use super::*;

pub(super) fn output_to_value(output: &Output) -> Value {
    json!({
        "columns": output
            .columns()
            .iter()
            .map(output_column_to_value)
            .collect::<Vec<_>>()
    })
}

pub(super) fn output_column_to_value(column: &OutputColumn) -> Value {
    json!({
        "name": column.name(),
        "expression": expression_to_value(column.expression()),
        "domain": value_domain_to_value(column.domain()),
        "lineage": column
            .lineage()
            .iter()
            .map(lineage_source_to_value)
            .collect::<Vec<_>>()
    })
}

pub(super) fn lineage_source_to_value(source: &LineageSource) -> Value {
    json!({
        "relation": source.relation(),
        "column": source.column()
    })
}

pub(super) fn column_domain_to_value(column_domain: &ColumnDomain) -> Value {
    json!({
        "column": column_ref_to_value(column_domain.column()),
        "domain": value_domain_to_value(column_domain.domain())
    })
}

pub(super) fn column_ref_to_value(column: &ColumnRef) -> Value {
    json!({
        "relation": column.relation(),
        "name": column.name()
    })
}

pub(super) fn value_domain_to_value(domain: &ValueDomain) -> Value {
    match domain {
        ValueDomain::Unbounded => json!({ "kind": "unbounded" }),
        ValueDomain::Ranges(domain) => json!({
            "kind": "ranges",
            "ranges": domain
                .ranges()
                .iter()
                .map(value_range_to_value)
                .collect::<Vec<_>>()
        }),
        ValueDomain::Set(domain) => json!({
            "kind": "set",
            "mode": domain.mode().as_str(),
            "values": domain
                .values()
                .iter()
                .map(literal_expression_to_value)
                .collect::<Vec<_>>()
        }),
        ValueDomain::Empty => json!({ "kind": "empty" }),
        ValueDomain::Unknown(domain) => json!({
            "kind": "unknown",
            "reason": domain.reason()
        }),
    }
}

pub(super) fn value_range_to_value(range: &ValueRange) -> Value {
    json!({
        "lower": range.lower().map_or(Value::Null, bound_to_value),
        "upper": range.upper().map_or(Value::Null, bound_to_value)
    })
}

pub(super) fn bound_to_value(bound: &Bound) -> Value {
    json!({
        "value": literal_expression_to_value(bound.value()),
        "inclusive": bound.inclusive()
    })
}

pub(super) fn source_relation_to_value(source: &SourceRelation) -> Value {
    json!({
        "kind": "relation",
        "name": source.name(),
        "alias": source.alias()
    })
}

pub(super) fn join_to_value(join: &Join) -> Value {
    let mut value = json!({
        "kind": join.kind().as_str(),
        "left": relation_ref_to_value(join.left()),
        "right": relation_ref_to_value(join.right()),
        "condition": join
            .condition()
            .map_or(Value::Null, predicate_to_value)
    });
    if let Some(execution) = join.execution() {
        value["evaluation"] = json!(execution.as_str());
        if let Some(source) = join.right_physical_source() {
            value["right_physical_source"] = json!(source);
        }
    }
    value
}

pub(super) fn relation_ref_to_value(relation: &RelationRef) -> Value {
    json!({
        "relation": relation.relation(),
        "alias": relation.alias()
    })
}
