mod common;

use common::DIALECTS;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, to_bundle_json, AnalysisBundle, ComposedSemantics, ResolvedComposedSemantics,
    SqlInput, TransformationLayer,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

const STAGE_ORDERS_SQL: &str = include_str!("fixtures/advanced_bundle/stage_orders.sql");
const MART_CUSTOMER_SUMMARY_SQL: &str =
    include_str!("fixtures/advanced_bundle/mart_customer_summary.sql");

fn representative_inputs() -> Vec<SqlInput> {
    vec![
        SqlInput::file(
            "tests/fixtures/advanced_bundle/stage_orders.sql",
            STAGE_ORDERS_SQL,
        ),
        SqlInput::inline(
            "CREATE TABLE core.ranked_orders AS
             SELECT
                 order_id,
                 customer_id,
                 amount,
                 high_value,
                 ROW_NUMBER() OVER (
                     PARTITION BY customer_id
                     ORDER BY created_at
                 ) AS rn
             FROM stage.orders
             QUALIFY rn <= 10",
        ),
        SqlInput::file(
            "tests/fixtures/advanced_bundle/mart_customer_summary.sql",
            MART_CUSTOMER_SUMMARY_SQL,
        ),
        SqlInput::inline(
            "CREATE TABLE mart.active_ids AS
             SELECT id FROM raw.active_accounts
             UNION ALL
             SELECT id FROM raw.legacy_accounts",
        ),
    ]
}

fn layer_for_relation<'a>(bundle: &'a AnalysisBundle, relation: &str) -> &'a TransformationLayer {
    bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|dataset| dataset.relation_name() == Some(relation))
        })
        .expect("expected named transformation layer")
}

fn resolved(layer: &TransformationLayer) -> &ResolvedComposedSemantics {
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics,
        other => panic!("expected resolved semantics, got {other:?}"),
    }
}

fn final_relation_names(bundle: &AnalysisBundle) -> Vec<&str> {
    let mut names = bundle
        .graph()
        .components()
        .iter()
        .flat_map(|component| component.final_outcomes())
        .filter_map(|dataset| dataset.relation_name())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
}

fn contains_key_value(value: &Value, key: &str, expected: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.get(key) == Some(expected)
                || object
                    .values()
                    .any(|nested| contains_key_value(nested, key, expected))
        }
        Value::Array(values) => values
            .iter()
            .any(|nested| contains_key_value(nested, key, expected)),
        _ => false,
    }
}

fn relation_layer_json<'a>(document: &'a Value, relation: &str) -> &'a Value {
    document["layers"]
        .as_array()
        .expect("layers should be an array")
        .iter()
        .find(|layer| {
            layer["produces"]
                .as_array()
                .expect("produces should be an array")
                .iter()
                .any(|dataset| dataset["name"] == relation)
        })
        .expect("expected serialized named layer")
}

fn output_column_json<'a>(layer: &'a Value, name: &str) -> &'a Value {
    layer["composed_semantics"]["output"]["columns"]
        .as_array()
        .expect("resolved composed output should contain columns")
        .iter()
        .find(|column| column["name"] == name)
        .expect("expected output column")
}

struct Schemas {
    active: Value,
    legacy: Value,
}

impl Schemas {
    fn load() -> Self {
        Self {
            active: serde_json::from_str(include_str!("../schema/protocol-v0.2.schema.json"))
                .expect("active schema should be valid JSON"),
            legacy: serde_json::from_str(include_str!("../schema/protocol-v0.schema.json"))
                .expect("legacy referenced schema should be valid JSON"),
        }
    }

    fn validate(&self, instance: &Value) -> Result<(), String> {
        validate_schema(self, &self.active, instance, &self.active, "$")
    }

    fn resolve_ref<'a>(
        &'a self,
        current_root: &'a Value,
        reference: &str,
    ) -> Result<(&'a Value, &'a Value), String> {
        if let Some(pointer) = reference.strip_prefix('#') {
            let schema = current_root
                .pointer(pointer)
                .ok_or_else(|| format!("unresolved local schema reference {reference}"))?;
            return Ok((current_root, schema));
        }

        let Some((document, pointer)) = reference.split_once('#') else {
            return Err(format!("unsupported schema reference {reference}"));
        };
        if !document.ends_with("/schema/protocol-v0.schema.json") {
            return Err(format!("unsupported external schema reference {reference}"));
        }
        let schema = self
            .legacy
            .pointer(pointer)
            .ok_or_else(|| format!("unresolved external schema reference {reference}"))?;
        Ok((&self.legacy, schema))
    }
}

fn validate_schema(
    schemas: &Schemas,
    root: &Value,
    instance: &Value,
    schema: &Value,
    path: &str,
) -> Result<(), String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let (resolved_root, resolved_schema) = schemas.resolve_ref(root, reference)?;
        return validate_schema(schemas, resolved_root, instance, resolved_schema, path);
    }

    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = options
            .iter()
            .filter(|option| validate_schema(schemas, root, instance, option, path).is_ok())
            .count();
        if matches != 1 {
            return Err(format!(
                "{path}: expected exactly one oneOf branch, matched {matches}"
            ));
        }
    }

    if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
        if !options
            .iter()
            .any(|option| validate_schema(schemas, root, instance, option, path).is_ok())
        {
            return Err(format!("{path}: did not match any anyOf branch"));
        }
    }

    if let Some(expected) = schema.get("const") {
        if instance != expected {
            return Err(format!("{path}: expected const {expected}, got {instance}"));
        }
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|expected| expected == instance) {
            return Err(format!(
                "{path}: value {instance} is outside enum {values:?}"
            ));
        }
    }

    if let Some(schema_type) = schema.get("type") {
        let valid = match schema_type {
            Value::String(kind) => instance_has_type(instance, kind),
            Value::Array(kinds) => kinds
                .iter()
                .filter_map(Value::as_str)
                .any(|kind| instance_has_type(instance, kind)),
            other => {
                return Err(format!(
                    "{path}: unsupported schema type declaration {other}"
                ))
            }
        };
        if !valid {
            return Err(format!("{path}: instance {instance} has unexpected type"));
        }
    }

    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        let value = instance
            .as_f64()
            .ok_or_else(|| format!("{path}: minimum applies to a non-number"))?;
        if value < minimum {
            return Err(format!("{path}: {value} is below minimum {minimum}"));
        }
    }

    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64) {
        let value = instance
            .as_str()
            .ok_or_else(|| format!("{path}: minLength applies to a non-string"))?;
        if value.chars().count() < min_length as usize {
            return Err(format!("{path}: string is shorter than {min_length}"));
        }
    }

    if let Some(object) = instance.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("{path}: missing required property {key}"));
                }
            }
        }

        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                for key in object.keys() {
                    if !properties.contains_key(key) {
                        return Err(format!("{path}: unexpected property {key}"));
                    }
                }
            }

            for (key, property_schema) in properties {
                if let Some(value) = object.get(key) {
                    validate_schema(
                        schemas,
                        root,
                        value,
                        property_schema,
                        &format!("{path}.{key}"),
                    )?;
                }
            }
        }
    }

    if let Some(array) = instance.as_array() {
        if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64) {
            if array.len() < min_items as usize {
                return Err(format!("{path}: array has fewer than {min_items} items"));
            }
        }
        if let Some(max_items) = schema.get("maxItems").and_then(Value::as_u64) {
            if array.len() > max_items as usize {
                return Err(format!("{path}: array has more than {max_items} items"));
            }
        }
        if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
            for (index, value) in array.iter().enumerate() {
                if array[..index].contains(value) {
                    return Err(format!("{path}: duplicate array item at index {index}"));
                }
            }
        }
        if let Some(item_schema) = schema.get("items") {
            for (index, value) in array.iter().enumerate() {
                validate_schema(
                    schemas,
                    root,
                    value,
                    item_schema,
                    &format!("{path}[{index}]"),
                )?;
            }
        }
    }

    Ok(())
}

fn instance_has_type(instance: &Value, kind: &str) -> bool {
    match kind {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "integer" => instance
            .as_number()
            .is_some_and(|number| number.is_i64() || number.is_u64()),
        "number" => instance.is_number(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        _ => false,
    }
}

#[test]
fn representative_bundle_validates_the_complete_advanced_target() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(&representative_inputs(), "generic", &dialect)
        .expect("representative bundle should analyze");

    assert_eq!(bundle.inputs().len(), 4);
    assert_eq!(bundle.layers().len(), 4);
    assert_eq!(
        final_relation_names(&bundle),
        vec!["mart.active_ids", "mart.customer_summary"]
    );

    let summary = resolved(layer_for_relation(&bundle, "mart.customer_summary"));
    assert_eq!(
        summary.dependencies(),
        &[
            "raw.allowed_customers".to_string(),
            "raw.orders".to_string()
        ]
    );

    let document: Value = serde_json::from_str(&to_bundle_json(&bundle))
        .expect("representative bundle should emit JSON");
    assert!(contains_key_value(
        &document,
        "kind",
        &Value::String("case".to_string())
    ));
    assert!(contains_key_value(
        &document,
        "kind",
        &Value::String("window_function".to_string())
    ));
    assert!(contains_key_value(
        &document,
        "kind",
        &Value::String("aggregate_function".to_string())
    ));
    assert!(contains_key_value(
        &document,
        "kind",
        &Value::String("exists".to_string())
    ));
    assert!(contains_key_value(
        &document,
        "operator",
        &Value::String("union".to_string())
    ));

    let stage = relation_layer_json(&document, "stage.orders");
    let high_value = output_column_json(stage, "high_value");
    assert_eq!(high_value["domain"]["kind"], "set");

    let ranked = relation_layer_json(&document, "core.ranked_orders");
    let rn = output_column_json(ranked, "rn");
    assert_eq!(rn["domain"]["kind"], "ranges");
    assert_eq!(rn["domain"]["ranges"][0]["lower"]["value"]["value"], 1);
    assert_eq!(rn["domain"]["ranges"][0]["upper"]["value"]["value"], 10);

    Schemas::load()
        .validate(&document)
        .unwrap_or_else(|error| panic!("representative document violates active schema: {error}"));
}

#[test]
fn representative_bundle_is_byte_deterministic() {
    let dialect = GenericDialect {};
    let inputs = representative_inputs();

    let first = analyze_inputs(&inputs, "generic", &dialect).expect("first analysis");
    let second = analyze_inputs(&inputs, "generic", &dialect).expect("second analysis");

    assert_eq!(to_bundle_json(&first), to_bundle_json(&second));
}

#[test]
fn high_input_count_has_no_artificial_collection_limit() {
    let dialect = GenericDialect {};
    let inputs = (0..300)
        .map(|index| SqlInput::inline(format!("SELECT {index} AS value")))
        .collect::<Vec<_>>();

    let bundle = analyze_inputs(&inputs, "generic", &dialect)
        .expect("high input count should not hit a fixed limit");

    assert_eq!(bundle.inputs().len(), 300);
    assert_eq!(bundle.layers().len(), 300);
    assert_eq!(bundle.graph().components().len(), 300);
}

#[test]
fn representative_shared_semantics_use_sqlparser_dialect_delegation() {
    let sql = "CREATE TABLE mart.result AS
               SELECT id, CASE WHEN amount > 10 THEN TRUE ELSE FALSE END AS expensive
               FROM raw.orders";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should resolve in sqlparser");
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| {
                panic!("dialect {dialect_name} should analyze shared semantics: {error}")
            });

        assert_eq!(bundle.inputs()[0].dialect(), *dialect_name);
        assert_eq!(
            final_relation_names(&bundle),
            vec!["mart.result"],
            "dialect {dialect_name} should expose the same terminal relation"
        );
    }
}

#[test]
fn complete_document_exposes_all_layers_and_terminal_outcomes_together() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(&representative_inputs(), "generic", &dialect)
        .expect("representative bundle should analyze");

    let produced = bundle
        .layers()
        .iter()
        .flat_map(|layer| layer.produces())
        .filter_map(|dataset| dataset.relation_name())
        .collect::<Vec<_>>();

    assert_eq!(
        produced,
        vec![
            "stage.orders",
            "core.ranked_orders",
            "mart.customer_summary",
            "mart.active_ids"
        ]
    );
    assert_eq!(
        final_relation_names(&bundle),
        vec!["mart.active_ids", "mart.customer_summary"]
    );
}
