//! TASK-147 active protocol wire contract and schema compatibility.

use sql_semantic_protocol::{analyze_inputs, dialect_from_name, to_bundle_json, SqlInput};

fn wire(sql: &str) -> serde_json::Value {
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap();
    serde_json::from_str(&to_bundle_json(&bundle)).unwrap()
}

#[test]
fn active_schema_describes_bounded_conflict_and_returning_without_requiring_them() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json")).unwrap();
    let write = &schema["$defs"]["writeOperation"];
    assert_eq!(write["properties"]["insert_conflict"]["type"], "object");
    assert_eq!(
        write["properties"]["insert_conflict"]["properties"]["action"]["const"],
        "do_nothing"
    );
    assert_eq!(write["properties"]["returning_columns"]["type"], "array");
    assert!(write["required"].as_array().unwrap().iter().all(|field| {
        field != "insert_conflict" && field != "returning_columns"
    }));
}

#[test]
fn old_append_wire_stays_identical_and_bounded_fields_have_stable_shape() {
    let ordinary = wire("INSERT INTO target(id,score) VALUES (1,10)");
    let value = &ordinary["inputs"][0]["statements"][0]["write"];
    assert!(value["insert_conflict"].is_null());
    assert!(value["returning_columns"].is_null());
    assert_eq!(value["kind"], "append");

    let sql = "INSERT INTO target(id,score) VALUES (1,10)
        ON CONFLICT(id) DO NOTHING RETURNING id,score";
    let document = wire(sql);
    let write = &document["inputs"][0]["statements"][0]["write"];
    assert_eq!(write["kind"], "append");
    assert_eq!(
        write["insert_conflict"],
        serde_json::json!({"action":"do_nothing","target_columns":["id"]})
    );
    assert_eq!(write["returning_columns"], serde_json::json!(["id", "score"]));

    let second = wire(sql);
    assert_eq!(document, second, "wire representation must be deterministic");
}

#[test]
fn single_modifier_writes_have_independent_canonical_semantics() {
    let returning = wire("INSERT INTO target(id,score) VALUES (1,10) RETURNING id");
    let returned = &returning["inputs"][0]["statements"][0]["write"];
    assert!(returned["insert_conflict"].is_null());
    assert_eq!(returned["returning_columns"], serde_json::json!(["id"]));

    let conflict = wire("INSERT INTO target(id,score) VALUES (1,10) ON CONFLICT(id) DO NOTHING");
    let operation = &conflict["inputs"][0]["statements"][0]["write"];
    assert_eq!(operation["insert_conflict"]["action"], "do_nothing");
    assert!(operation["returning_columns"].is_null());
}
