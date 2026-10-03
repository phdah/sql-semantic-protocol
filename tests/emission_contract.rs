use sql_semantic_protocol::{analyze_inputs, analyze_sql, to_bundle_json, to_json, SqlInput};
use sqlparser::dialect::GenericDialect;

#[test]
fn repeated_analysis_emits_byte_identical_json() {
    let dialect = GenericDialect {};
    let sql = "SELECT z.b + z.a AS total FROM z, a WHERE z.x IN (3, 1, 2)";

    let first = analyze_sql(sql, "generic", &dialect).expect("first analysis should succeed");
    let second = analyze_sql(sql, "generic", &dialect).expect("second analysis should succeed");

    assert_eq!(to_json(&first), to_json(&second));
}

#[test]
fn serialized_collections_follow_protocol_ordering_rules() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT z.b + z.a AS total FROM z, a WHERE z.x IN (3, 1, 2)",
        "generic",
        &dialect,
    )
    .expect("query should analyze");

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    let statement = &json["inputs"][0]["statements"][0];

    assert_eq!(statement["sources"][0]["name"], "z");
    assert_eq!(statement["sources"][1]["name"], "a");
    assert_eq!(statement["dependencies"], serde_json::json!(["a", "z"]));
    assert_eq!(
        statement["column_domains"][0]["domain"]["values"],
        serde_json::json!([
            {"kind": "literal", "type": "integer", "value": 1},
            {"kind": "literal", "type": "integer", "value": 2},
            {"kind": "literal", "type": "integer", "value": 3}
        ])
    );
    assert_eq!(
        statement["output"]["columns"][0]["lineage"],
        serde_json::json!([
            {"relation": "z", "column": "a"},
            {"relation": "z", "column": "b"}
        ])
    );
}

#[test]
fn simple_query_emission_matches_active_protocol_fixture() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql("SELECT t.b FROM t WHERE t.a > 10", "generic", &dialect)
        .expect("fixture query should analyze");

    let actual: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("emitted protocol should be JSON");
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../examples/protocol-simple.json"))
            .expect("active protocol fixture should be JSON");

    assert_eq!(actual, expected);
}

#[test]
fn single_and_collection_emission_use_the_same_active_contract() {
    let dialect = GenericDialect {};
    let sql = "SELECT a FROM t WHERE a > 10";

    let single =
        analyze_sql(sql, "generic", &dialect).expect("single input analysis should succeed");
    let collection = analyze_inputs(&[SqlInput::inline(sql)], "generic", &dialect)
        .expect("one-element collection analysis should succeed");

    let single_json: serde_json::Value =
        serde_json::from_str(&to_json(&single)).expect("single input should emit JSON");
    let collection_json: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&collection)).expect("collection should emit JSON");

    assert_eq!(single_json["protocol_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(single_json, collection_json);
}
