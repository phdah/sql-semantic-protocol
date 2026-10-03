use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use sql_semantic_protocol::{analyze_dbt_manifest, parse_dbt_manifest, to_bundle_json};
use sqlparser::dialect::dialect_from_str;

const PROJECT: &str = "sql_semantic_protocol_e2e";

fn project_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dbt_core_project")
}

fn manifest_path() -> PathBuf {
    project_dir().join("target/manifest.json")
}

fn run_results_path() -> PathBuf {
    project_dir().join("target/run_results.json")
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn model_id(name: &str) -> String {
    format!("model.{PROJECT}.{name}")
}

fn manifest_node<'a>(manifest: &'a Value, name: &str) -> &'a Value {
    let id = model_id(name);
    manifest["nodes"]
        .get(&id)
        .unwrap_or_else(|| panic!("missing dbt manifest node {id}"))
}

fn input_statement<'a>(protocol: &'a Value, name: &str) -> &'a Value {
    let id = model_id(name);
    protocol["inputs"]
        .as_array()
        .expect("protocol inputs should be an array")
        .iter()
        .find(|input| input["id"] == id)
        .unwrap_or_else(|| panic!("missing protocol input {id}"))["statements"]
        .as_array()
        .expect("statements should be an array")
        .first()
        .expect("dbt model should produce one statement")
}

fn layer_for_model<'a>(protocol: &'a Value, name: &str) -> &'a Value {
    let id = model_id(name);
    protocol["layers"]
        .as_array()
        .expect("protocol layers should be an array")
        .iter()
        .find(|layer| layer["statement"]["input_id"] == id)
        .unwrap_or_else(|| panic!("missing protocol layer for {id}"))
}

fn output_column<'a>(layer: &'a Value, name: &str) -> &'a Value {
    layer["composed_semantics"]["output"]["columns"]
        .as_array()
        .expect("resolved output columns should be an array")
        .iter()
        .find(|column| column["name"] == name)
        .unwrap_or_else(|| panic!("missing output column {name}"))
}

fn contains_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(actual) => actual == expected,
        Value::Array(values) => values.iter().any(|value| contains_string(value, expected)),
        Value::Object(values) => values
            .values()
            .any(|value| contains_string(value, expected)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn contains_number(value: &Value, expected: i64) -> bool {
    match value {
        Value::Number(number) => number.as_i64() == Some(expected),
        Value::Array(values) => values.iter().any(|value| contains_number(value, expected)),
        Value::Object(values) => values
            .values()
            .any(|value| contains_number(value, expected)),
        Value::Null | Value::Bool(_) | Value::String(_) => false,
    }
}

fn literal_text(bound: &Value) -> String {
    match &bound["value"]["value"] {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        other => panic!("expected scalar literal bound, got {other:?}"),
    }
}

fn assert_closed_number_range(domain: &Value, lower: &str, upper: &str) {
    assert_eq!(domain["kind"], "ranges");
    let ranges = domain["ranges"]
        .as_array()
        .expect("ranges domain should contain ranges");
    assert_eq!(ranges.len(), 1);

    let range = &ranges[0];
    assert_eq!(literal_text(&range["lower"]), lower);
    assert_eq!(range["lower"]["inclusive"], true);
    assert_eq!(literal_text(&range["upper"]), upper);
    assert_eq!(range["upper"]["inclusive"], true);
}

fn assert_lower_bounded_number_range(domain: &Value, lower: &str) {
    assert_eq!(domain["kind"], "ranges");
    let ranges = domain["ranges"]
        .as_array()
        .expect("ranges domain should contain ranges");
    assert_eq!(ranges.len(), 1);

    let range = &ranges[0];
    assert_eq!(literal_text(&range["lower"]), lower);
    assert_eq!(range["lower"]["inclusive"], true);
    assert!(range["upper"].is_null());
}

fn assert_successful_dbt_result(run_results: &Value, name: &str) {
    let id = model_id(name);
    let result = run_results["results"]
        .as_array()
        .expect("dbt run results should be an array")
        .iter()
        .find(|result| result["unique_id"] == id)
        .unwrap_or_else(|| panic!("missing dbt run result for {id}"));
    assert_eq!(result["status"], "success", "{id} did not execute successfully");
}

#[test]
#[ignore = "requires a dbt-generated manifest; run make dbt-e2e"]
fn dbt_core_project_covers_supported_model_semantics_end_to_end() {
    let manifest_path = manifest_path();
    assert!(
        manifest_path.exists(),
        "dbt manifest is missing; run make dbt-e2e"
    );

    let manifest_json = read_json(&manifest_path);
    assert!(
        manifest_json["metadata"]["dbt_schema_version"]
            .as_str()
            .expect("dbt schema version should be a string")
            .ends_with("/manifest/v12.json")
    );
    assert_eq!(manifest_json["metadata"]["adapter_type"], "duckdb");

    let expected_models = [
        "aggregated_orders",
        "constant_domains",
        "derived_orders",
        "distinct_regions",
        "enriched_orders",
        "excluded_order_ids",
        "final_orders",
        "incremental_append_orders",
        "incremental_merge_orders",
        "independent_return_summary",
        "intersected_order_ids",
        "lateral_orders",
        "named_window_orders",
        "ordered_limited_union",
        "ranked_orders",
        "rollup_orders",
        "stg_customers",
        "stg_orders",
        "subquery_orders",
        "unioned_orders",
    ];
    let actual_models = manifest_json["nodes"]
        .as_object()
        .expect("dbt nodes should be an object")
        .iter()
        .filter_map(|(id, node)| {
            (node["resource_type"] == "model" && id.starts_with(&format!("model.{PROJECT}.")))
                .then_some(
                    id.rsplit_once('.')
                        .expect("model unique id should contain a name")
                        .1,
                )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_models,
        expected_models.into_iter().collect::<BTreeSet<_>>()
    );

    for model in &expected_models {
        let node = manifest_node(&manifest_json, model);
        let compiled = node["compiled_code"]
            .as_str()
            .unwrap_or_else(|| panic!("{model} should have compiled SQL"));
        assert!(!compiled.contains("{{"), "{model} retained Jinja");
        assert!(!compiled.contains("{%"), "{model} retained Jinja");
        assert!(
            node["relation_name"].is_string(),
            "{model} should have a dbt relation identity"
        );
    }

    let merge_node = manifest_node(&manifest_json, "incremental_merge_orders");
    assert_eq!(merge_node["config"]["materialized"], "incremental");
    assert_eq!(merge_node["config"]["incremental_strategy"], "merge");
    assert_eq!(merge_node["config"]["unique_key"], "order_id");

    let append_node = manifest_node(&manifest_json, "incremental_append_orders");
    assert_eq!(append_node["config"]["materialized"], "incremental");
    assert_eq!(append_node["config"]["incremental_strategy"], "append");

    let run_results = read_json(&run_results_path());
    assert_successful_dbt_result(&run_results, "incremental_merge_orders");
    assert_successful_dbt_result(&run_results, "incremental_append_orders");

    let manifest_text = fs::read_to_string(&manifest_path).expect("manifest should be readable");
    let manifest = parse_dbt_manifest(&manifest_text).expect("real dbt manifest should parse");
    let dialect =
        dialect_from_str(manifest.adapter_type()).expect("dbt DuckDB dialect should resolve");
    let bundle = analyze_dbt_manifest(&manifest, manifest.adapter_type(), dialect.as_ref())
        .expect("real dbt project should analyze");
    let library_json = to_bundle_json(&bundle);
    let protocol: Value =
        serde_json::from_str(&library_json).expect("library protocol should be valid JSON");

    let cli = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .args([
            "--dbt-manifest",
            manifest_path
                .to_str()
                .expect("fixture manifest path should be UTF-8"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    assert!(cli.stderr.is_empty());
    assert_eq!(
        String::from_utf8(cli.stdout)
            .expect("CLI stdout should be UTF-8")
            .trim(),
        library_json
    );

    let protocol_input_ids = protocol["inputs"]
        .as_array()
        .expect("protocol inputs should be an array")
        .iter()
        .map(|input| {
            input["id"]
                .as_str()
                .expect("input id should be a string")
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        protocol_input_ids,
        expected_models
            .iter()
            .map(|name| model_id(name))
            .collect::<BTreeSet<_>>()
    );

    let stage = input_statement(&protocol, "stg_orders");
    assert!(contains_string(stage, "between"));
    assert!(contains_string(stage, "in"));
    assert!(contains_string(stage, "case"));
    assert!(contains_string(stage, "boolean_predicate"));
    assert!(contains_string(stage, "binary"));
    let stage_layer = layer_for_model(&protocol, "stg_orders");
    assert_closed_number_range(
        &output_column(stage_layer, "amount")["domain"],
        "10",
        "100",
    );
    let bucket_domain = &output_column(stage_layer, "amount_bucket")["domain"];
    assert_eq!(bucket_domain["kind"], "set");
    for expected in ["high", "medium", "standard"] {
        assert!(contains_string(bucket_domain, expected));
    }

    let enriched = input_statement(&protocol, "enriched_orders");
    assert_eq!(
        enriched["joins"]
            .as_array()
            .expect("joins should be an array")
            .len(),
        2
    );
    assert!(contains_string(enriched, "binary"));

    let ranked = input_statement(&protocol, "ranked_orders");
    assert!(contains_string(ranked, "window_function"));
    assert!(contains_string(ranked, "rows"));
    assert!(!ranked["predicates"]["qualify"].is_null());
    assert_closed_number_range(
        &output_column(layer_for_model(&protocol, "ranked_orders"), "rn")["domain"],
        "1",
        "2",
    );

    let named_window = input_statement(&protocol, "named_window_orders");
    assert!(contains_string(named_window, "window_function"));
    assert!(contains_string(named_window, "customer_window"));
    assert!(contains_string(named_window, "rows"));

    let unioned = input_statement(&protocol, "unioned_orders");
    assert_eq!(unioned["set_operation"]["operator"], "union");
    assert_eq!(unioned["set_operation"]["quantifier"], "all");

    let intersected = input_statement(&protocol, "intersected_order_ids");
    assert_eq!(intersected["set_operation"]["operator"], "intersect");

    let excluded = input_statement(&protocol, "excluded_order_ids");
    assert_eq!(excluded["set_operation"]["operator"], "except");

    let aggregated = input_statement(&protocol, "aggregated_orders");
    assert!(!aggregated["aggregation"].is_null());
    assert!(contains_string(aggregated, "aggregate_function"));
    assert!(!aggregated["predicates"]["having"].is_null());
    assert!(contains_string(aggregated, "paid"));
    assert_lower_bounded_number_range(
        &output_column(
            layer_for_model(&protocol, "aggregated_orders"),
            "order_count",
        )["domain"],
        "0",
    );

    let distinct = input_statement(&protocol, "distinct_regions");
    assert_eq!(distinct["aggregation"]["distinct"], true);

    let rollup = input_statement(&protocol, "rollup_orders");
    assert!(contains_string(rollup, "rollup"));

    let subqueries = input_statement(&protocol, "subquery_orders");
    assert!(contains_string(subqueries, "scalar_subquery"));
    assert!(contains_string(subqueries, "exists"));
    assert!(contains_string(subqueries, "in_subquery"));

    let derived = input_statement(&protocol, "derived_orders");
    assert!(derived["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .is_empty());

    let lateral = input_statement(&protocol, "lateral_orders");
    assert!(lateral["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .is_empty());
    assert!(contains_string(lateral, "binary"));

    let constants = layer_for_model(&protocol, "constant_domains");
    let answer_domain = &output_column(constants, "answer")["domain"];
    assert_eq!(answer_domain["kind"], "set");
    assert!(contains_number(answer_domain, 42));
    let enabled_domain = &output_column(constants, "enabled")["domain"];
    assert_eq!(enabled_domain["kind"], "set");
    assert!(enabled_domain.to_string().contains("true"));

    let ordered_union = input_statement(&protocol, "ordered_limited_union");
    assert!(contains_string(ordered_union, "unsupported_order_by"));
    assert!(contains_string(ordered_union, "unsupported_limit"));

    let final_layer = layer_for_model(&protocol, "final_orders");
    assert_closed_number_range(
        &output_column(final_layer, "amount")["domain"],
        "10",
        "50",
    );

    let graph_components = protocol["graph"]["components"]
        .as_array()
        .expect("graph components should be an array");
    assert!(
        graph_components.len() >= 3,
        "expected the primary model graph plus disconnected feature models"
    );

    let merge_statement = input_statement(&protocol, "incremental_merge_orders");
    assert!(
        !contains_string(merge_statement, "conditional_mutation"),
        "dbt model compiled_code should remain SELECT semantics; generated materialization MERGE is not part of manifest compiled_code"
    );
}
