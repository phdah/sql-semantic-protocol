//! Validated Arrow-like physical population sidecar serialization.
//!
//! The ordinary protocol document remains the single SQL semantic contract.
//! This deterministic witness sidecar retains exact physical value encodings,
//! not JSON-cast versions of the values. It cannot be created from unverified
//! source rows.

use super::*;
use crate::physical_realization::{
    physical_typed_closed_world_schema_plan, PhysicalTypedPopulationPlan,
    PhysicalTypedValue, PhysicalTypedWriteState,
};

/// Serialize only schema- and constraint-feasible complete typed populations.
///
/// Numbers use exact lexical scalar text and IEEE754 values use raw bit words.
/// Struct/array/map order and bag multiplicity survive without SQL casts.
/// The returned JSON is a physical *witness sidecar*, not another protocol root
/// document or an assertion that any SQL statement generated those rows.
pub fn to_physical_typed_population_json(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalTypedWriteState],
) -> Result<String, PhysicalTypedPopulationPlan> {
    let PhysicalTypedPopulationPlan::Feasible { final_states } =
        physical_typed_closed_world_schema_plan(bundle, snapshots)
    else {
        return Err(physical_typed_closed_world_schema_plan(bundle, snapshots));
    };
    let populations = final_states.iter().map(|state| {
        json!({
            "relation": state.relation(),
            "rows": state.rows().iter().map(|row| {
                json!({
                    "values": row.values().iter().map(|(column, value)| {
                        (column.clone(), typed_value_to_value(value))
                    }).collect::<BTreeMap<_, _>>()
                })
            }).collect::<Vec<_>>()
        })
    }).collect::<Vec<_>>();
    Ok(json!({
        "protocol_version": bundle.protocol_version(),
        "kind": "typed_physical_population",
        "populations": populations
    }).to_string())
}

fn typed_value_to_value(value: &PhysicalTypedValue) -> Value {
    match value {
        PhysicalTypedValue::Scalar(LiteralValue::Null) => json!({
            "kind": "scalar", "scalar_kind": "null", "value": null
        }),
        PhysicalTypedValue::Scalar(LiteralValue::Boolean(value)) => json!({
            "kind": "scalar", "scalar_kind": "boolean", "value": value
        }),
        PhysicalTypedValue::Scalar(LiteralValue::Number(value)) => json!({
            "kind": "scalar", "scalar_kind": "number", "value": value
        }),
        PhysicalTypedValue::Scalar(LiteralValue::Text(value)) => json!({
            "kind": "scalar", "scalar_kind": "text", "value": value
        }),
        PhysicalTypedValue::Bytes(bytes) => json!({"kind":"bytes","values":bytes}),
        PhysicalTypedValue::Json(document) => json!({
            "kind":"json_document","document":document
        }),
        PhysicalTypedValue::Bits(bits) => json!({"kind":"bits","values":bits}),
        PhysicalTypedValue::Union { field_index, value } => json!({
            "kind":"union","field_index":field_index,
            "value":typed_value_to_value(value)
        }),
        PhysicalTypedValue::Float32Bits(bits) => json!({
            "kind":"float32_bits","bits":bits
        }),
        PhysicalTypedValue::Float64Bits(bits) => json!({
            "kind":"float64_bits","bits":bits
        }),
        PhysicalTypedValue::Array(values) => json!({
            "kind":"array",
            "values":values.iter().map(typed_value_to_value).collect::<Vec<_>>()
        }),
        PhysicalTypedValue::Map(pairs) => json!({
            "kind":"map",
            "entries":pairs.iter().map(|(key,value)| json!({
                "key":typed_value_to_value(key),
                "value":typed_value_to_value(value),
            })).collect::<Vec<_>>()
        }),
        PhysicalTypedValue::Struct(values) => json!({
            "kind":"struct",
            "values":values.iter().map(typed_value_to_value).collect::<Vec<_>>()
        }),
    }
}
