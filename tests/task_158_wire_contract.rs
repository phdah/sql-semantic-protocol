//! TASK-158 emitted ALTER representation is parser-independent and deterministic.

use serde_json::{json, Value};
use sql_semantic_protocol::{analyze_inputs, dialect_from_name, to_bundle_json, SqlInput};

fn alter(sql: &str) -> Value {
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap();
    serde_json::from_str(&to_bundle_json(&bundle)).unwrap()
}

#[test]
fn add_default_has_typed_canonical_shape_and_no_silent_missing_default() {
    let text = "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2";
    let output = alter(text);
    let step = &output["inputs"][0]["statements"][0]["write"];
    assert_eq!(step["kind"], "schema_alter");
    assert_eq!(step["definition_mode"], Value::Null);
    let item = &step["alter_columns"][0];
    assert_eq!(item["kind"], "add");
    assert_eq!(item["column"]["name"], "b");
    assert_eq!(item["has_default"], true);
    assert_eq!(item["default"], json!(2));

    let nullable = alter("ALTER TABLE t ADD COLUMN b INTEGER");
    let without_default = &nullable["inputs"][0]["statements"][0]["write"]["alter_columns"][0];
    assert_eq!(without_default["has_default"], false);
    assert!(without_default["default"].is_null());

    let explicit_null = alter("ALTER TABLE t ADD COLUMN b INTEGER DEFAULT NULL");
    let null_step = &explicit_null["inputs"][0]["statements"][0]["write"]["alter_columns"][0];
    assert_eq!(null_step["has_default"], true);
    assert!(null_step["default"].is_null());

    assert_eq!(output, alter(text), "canonical wire must be deterministic");
}

#[test]
fn schema_declares_optional_alter_columns_and_new_write_kind() {
    let contract: Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json")).unwrap();
    let write = &contract["$defs"]["writeOperation"];
    assert!(write["properties"]["kind"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("schema_alter")));
    assert_eq!(write["properties"]["alter_columns"]["type"], "array");
    assert_eq!(
        write["properties"]["alter_columns"]["items"]["additionalProperties"],
        false
    );
    assert!(!write["required"]
        .as_array()
        .unwrap()
        .contains(&json!("alter_columns")));
}
