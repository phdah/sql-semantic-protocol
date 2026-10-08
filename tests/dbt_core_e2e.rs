use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, to_bundle_json,
};
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

fn catalog_path() -> PathBuf {
    project_dir().join("target/catalog.json")
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

fn produced_relation(layer: &Value) -> Option<&str> {
    layer["produces"]
        .as_array()
        .expect("layer produces should be an array")
        .iter()
        .find_map(|dataset| {
            (dataset["kind"] == "relation")
                .then(|| dataset["name"].as_str())
                .flatten()
        })
}

fn final_outcome_snapshot(protocol: &Value) -> Value {
    let layers = protocol["layers"]
        .as_array()
        .expect("protocol layers should be an array");
    let mut outcomes = Vec::new();

    for component in protocol["graph"]["components"]
        .as_array()
        .expect("graph components should be an array")
    {
        for dataset in component["final_outcomes"]
            .as_array()
            .expect("component final outcomes should be an array")
        {
            assert_eq!(
                dataset["kind"], "relation",
                "dbt model final outcomes should be named relations"
            );
            let relation = dataset["name"]
                .as_str()
                .expect("final relation should be a string");
            let layer = layers
                .iter()
                .find(|layer| produced_relation(layer) == Some(relation))
                .unwrap_or_else(|| panic!("missing producer layer for final outcome {relation}"));

            let mut composed_semantics = layer["composed_semantics"].clone();
            composed_semantics
                .as_object_mut()
                .expect("composed semantics should be an object")
                .remove("join_equalities");
            outcomes.push(serde_json::json!({
                "relation": relation,
                "model_id": layer["statement"]["input_id"].clone(),
                "composed_semantics": composed_semantics
            }));
        }
    }

    outcomes.sort_by(|left, right| {
        left["model_id"]
            .as_str()
            .expect("model id should be a string")
            .cmp(
                right["model_id"]
                    .as_str()
                    .expect("model id should be a string"),
            )
    });

    Value::Array(outcomes)
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
    assert_eq!(
        result["status"], "success",
        "{id} did not execute successfully"
    );
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
    assert!(manifest_json["metadata"]["dbt_schema_version"]
        .as_str()
        .expect("dbt schema version should be a string")
        .ends_with("/manifest/v12.json"));
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
        expected_models.iter().copied().collect::<BTreeSet<_>>()
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

    let stg_orders_node = manifest_node(&manifest_json, "stg_orders");
    let explicit_constraints = stg_orders_node["constraints"]
        .as_array()
        .expect("stg_orders should expose explicit dbt constraints");
    assert!(explicit_constraints
        .iter()
        .any(|constraint| constraint["type"] == "primary_key"));
    assert!(explicit_constraints
        .iter()
        .any(|constraint| constraint["type"] == "unique"));
    assert!(explicit_constraints
        .iter()
        .any(|constraint| constraint["type"] == "foreign_key"));

    let generic_tests = manifest_json["nodes"]
        .as_object()
        .expect("dbt nodes should be an object")
        .values()
        .filter(|node| node["resource_type"] == "test")
        .collect::<Vec<_>>();
    assert!(generic_tests
        .iter()
        .any(|test| test["test_metadata"]["name"] == "unique"));
    assert!(generic_tests
        .iter()
        .any(|test| test["test_metadata"]["name"] == "relationships"));
    assert!(generic_tests
        .iter()
        .any(|test| test["test_metadata"]["name"] == "not_null"));
    assert!(generic_tests
        .iter()
        .any(|test| test["test_metadata"]["name"] == "accepted_values"));

    let run_results = read_json(&run_results_path());
    assert_successful_dbt_result(&run_results, "incremental_merge_orders");
    assert_successful_dbt_result(&run_results, "incremental_append_orders");

    let manifest_text = fs::read_to_string(&manifest_path).expect("manifest should be readable");
    let manifest = parse_dbt_manifest(&manifest_text).expect("real dbt manifest should parse");
    let catalog_path = catalog_path();
    assert!(
        catalog_path.exists(),
        "dbt catalog is missing; run make dbt-e2e"
    );
    let catalog_text = fs::read_to_string(&catalog_path).expect("catalog should be readable");
    let catalog = parse_dbt_catalog(&catalog_text).expect("real dbt catalog should parse");
    let dialect =
        dialect_from_str(manifest.adapter_type()).expect("dbt DuckDB dialect should resolve");
    let bundle = analyze_dbt_artifacts(
        &manifest,
        &catalog,
        manifest.adapter_type(),
        dialect.as_ref(),
    )
    .expect("real dbt project should analyze with warehouse schemas");
    let library_json = to_bundle_json(&bundle);
    let protocol: Value =
        serde_json::from_str(&library_json).expect("library protocol should be valid JSON");

    let stg_orders_relation = manifest_node(&manifest_json, "stg_orders")["relation_name"]
        .as_str()
        .expect("stg_orders should have a relation identity");
    let stg_order_constraints = protocol["relation_constraints"]
        .as_array()
        .expect("dbt protocol should include relation constraints")
        .iter()
        .find(|metadata| metadata["relation"] == stg_orders_relation)
        .expect("stg_orders constraints should be emitted");
    let emitted_constraints = stg_order_constraints["constraints"]
        .as_array()
        .expect("relation constraints should be an array");
    assert!(emitted_constraints
        .iter()
        .any(|constraint| constraint["kind"] == "primary_key"));
    assert!(emitted_constraints
        .iter()
        .any(|constraint| constraint["kind"] == "unique_key"));
    assert!(emitted_constraints
        .iter()
        .any(|constraint| constraint["kind"] == "foreign_key"));
    assert!(emitted_constraints
        .iter()
        .any(|constraint| constraint["kind"] == "not_null"));
    let accepted_values = emitted_constraints
        .iter()
        .find(|constraint| {
            constraint["kind"] == "accepted_values" && constraint["column"] == "status"
        })
        .expect("stg_orders status accepted-values constraint");
    assert_eq!(accepted_values["quote"], true);
    assert!(contains_string(accepted_values, "paid"));
    assert!(contains_string(accepted_values, "pending"));
    assert!(contains_string(stg_order_constraints, "dbt_constraint"));
    assert!(contains_string(stg_order_constraints, "dbt_test"));

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

    let raw_orders_source = manifest_json["sources"]
        .as_object()
        .expect("manifest sources should be an object")
        .values()
        .find(|source| source["source_name"] == "raw" && source["name"] == "orders")
        .expect("raw orders source should exist");
    let raw_orders_relation = raw_orders_source["relation_name"]
        .as_str()
        .expect("raw orders source should have a relation identity");
    let raw_order_constraints = protocol["relation_constraints"]
        .as_array()
        .expect("dbt protocol should include relation constraints")
        .iter()
        .find(|metadata| metadata["relation"] == raw_orders_relation)
        .expect("raw orders source constraints should be emitted");
    assert!(raw_order_constraints["constraints"]
        .as_array()
        .expect("raw order constraints should be an array")
        .iter()
        .any(|constraint| constraint["kind"] == "not_null"));
    assert!(raw_order_constraints["constraints"]
        .as_array()
        .expect("raw order constraints should be an array")
        .iter()
        .any(|constraint| {
            constraint["kind"] == "accepted_values" && constraint["column"] == "status"
        }));

    // dbt leaves attached_node empty for source tests; ownership comes from the test's
    // rendered model argument, including for a self-referencing relationships test.
    let raw_customers_relation = manifest_json["sources"]
        .as_object()
        .expect("manifest sources should be an object")
        .values()
        .find(|source| source["source_name"] == "raw" && source["name"] == "customers")
        .and_then(|source| source["relation_name"].as_str())
        .expect("raw customers source should have a relation identity");
    let raw_order_constraint_list = raw_order_constraints["constraints"]
        .as_array()
        .expect("raw order constraints should be an array");
    assert!(raw_order_constraint_list.iter().any(|constraint| {
        constraint["kind"] == "unique_key" && constraint["columns"] == serde_json::json!(["id"])
    }));
    for (column, referenced_relation) in [
        ("customer_id", raw_customers_relation),
        ("id", raw_orders_relation),
    ] {
        assert!(
            raw_order_constraint_list.iter().any(|constraint| {
                constraint["kind"] == "foreign_key"
                    && constraint["columns"] == serde_json::json!([column])
                    && constraint["referenced_relation"] == referenced_relation
                    && constraint["referenced_columns"] == serde_json::json!(["id"])
            }),
            "raw orders {column} source relationships test should emit a foreign key to {referenced_relation}"
        );
    }
    assert!(protocol["constraint_diagnostics"]
        .as_array()
        .is_none_or(|diagnostics| diagnostics
            .iter()
            .all(|diagnostic| { diagnostic["code"] != "unattributed_dbt_test" })));

    // These sources are only linked by a dbt relationships test. No compiled model
    // consumes either relation, so dependency-only schema discovery misses both.
    let isolated_sources = ["constraint_only_order_items", "constraint_only_orders"]
        .into_iter()
        .map(|name| {
            manifest_json["sources"]
                .as_object()
                .expect("manifest sources should be an object")
                .iter()
                .find(|(_, source)| source["source_name"] == "raw" && source["name"] == name)
                .map(|(id, source)| {
                    (
                        id.clone(),
                        source["relation_name"]
                            .as_str()
                            .expect("physical source identity")
                            .to_string(),
                    )
                })
                .unwrap_or_else(|| panic!("missing dbt Core source {name}"))
        })
        .collect::<Vec<_>>();
    for (source_id, _) in &isolated_sources {
        assert!(
            manifest_json["nodes"]
                .as_object()
                .expect("manifest nodes should be an object")
                .values()
                .filter(|node| node["resource_type"] == "model")
                .all(|node| !node["depends_on"]["nodes"]
                    .as_array()
                    .is_some_and(|deps| deps.iter().any(|dep| dep == source_id))),
            "{source_id} should not be a compiled model dependency"
        );
    }
    let (child_id, child_relation) = &isolated_sources[0];
    let (parent_id, parent_relation) = &isolated_sources[1];
    let relationship_test = manifest_json["nodes"]
        .as_object()
        .expect("manifest nodes should be an object")
        .iter()
        .find(|(_, node)| {
            node["resource_type"] == "test"
                && node["test_metadata"]["name"] == "relationships"
                && node["depends_on"]["nodes"].as_array().is_some_and(|deps| {
                    deps.contains(&Value::String(child_id.clone()))
                        && deps.contains(&Value::String(parent_id.clone()))
                })
        })
        .map(|(id, _)| id.clone())
        .expect("dbt Core should compile the constraint-only relationships test");
    let constraint_set = protocol["relation_constraints"]
        .as_array()
        .expect("relation constraints should be emitted")
        .iter()
        .find(|set| set["relation"] == *child_relation)
        .expect("constraint-only source foreign key should be emitted");
    assert!(constraint_set["constraints"]
        .as_array()
        .expect("constraints should be an array")
        .iter()
        .any(|constraint| {
            constraint["kind"] == "foreign_key"
                && constraint["columns"] == serde_json::json!(["order_id"])
                && constraint["referenced_relation"] == *parent_relation
                && constraint["referenced_columns"] == serde_json::json!(["id"])
        }));
    for (_, relation) in &isolated_sources {
        let source_schema = protocol["source_schemas"]
            .as_array()
            .expect("typed source schemas")
            .iter()
            .find(|schema| schema["relation"] == *relation)
            .expect("constraint-only source should have an introspected schema");
        assert_eq!(source_schema["source_kind"], "dbt_catalog");
    }

    // Reuse actual dbt-generated artifacts with only the constraint-only sources/test,
    // so catalog-less schema coverage does not depend on unrelated model inputs.
    let mut isolated_manifest_json = manifest_json.clone();
    isolated_manifest_json["sources"]
        .as_object_mut()
        .expect("manifest sources should be an object")
        .retain(|id, _| id == child_id || id == parent_id);
    isolated_manifest_json["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object")
        .retain(|id, _| id == &relationship_test);
    let isolated_manifest = parse_dbt_manifest(&isolated_manifest_json.to_string())
        .expect("isolated dbt Core artifacts should parse");
    let isolated_bundle = sql_semantic_protocol::analyze_dbt_manifest_with_schemas(
        &isolated_manifest,
        manifest.adapter_type(),
        dialect.as_ref(),
    )
    .expect("unconsumed dbt Core sources should use declared datatypes without catalog");
    let isolated_json: Value = serde_json::from_str(&to_bundle_json(&isolated_bundle))
        .expect("isolated protocol should serialize");
    for (_, relation) in &isolated_sources {
        let source_schema = isolated_json["source_schemas"]
            .as_array()
            .expect("typed source schemas")
            .iter()
            .find(|schema| schema["relation"] == *relation)
            .expect("constraint-only source should be emitted without catalog");
        assert_eq!(source_schema["source_kind"], "dbt_manifest");
        assert_eq!(
            source_schema["columns"][0]["data_type"]["kind"],
            "signed_integer"
        );
    }

    let source_schemas = protocol["source_schemas"]
        .as_array()
        .expect("dbt protocol should include warehouse source schemas");
    let raw_orders_schema = source_schemas
        .iter()
        .find(|schema| schema["relation"] == raw_orders_relation)
        .expect("raw orders warehouse schema should be present");
    let raw_order_columns = raw_orders_schema["columns"]
        .as_array()
        .expect("raw orders columns should be an array");
    assert_eq!(
        raw_order_columns
            .iter()
            .map(|column| column["name"]
                .as_str()
                .expect("catalog column name should be a string"))
            .collect::<Vec<_>>(),
        [
            "id",
            "customer_id",
            "amount",
            "status",
            "created_at",
            "region"
        ]
    );
    assert_eq!(raw_order_columns[0]["data_type"]["kind"], "signed_integer");
    assert_eq!(raw_order_columns[3]["data_type"]["kind"], "string");
    assert_eq!(raw_orders_schema["source_kind"], "dbt_catalog");

    let mut catalog_without_raw_orders: Value =
        serde_json::from_str(&catalog_text).expect("catalog should be valid JSON");
    for section in ["nodes", "sources"] {
        let matching_ids = catalog_without_raw_orders[section]
            .as_object()
            .expect("catalog resource section should be an object")
            .keys()
            .filter(|unique_id| {
                manifest_json[section]
                    .get(*unique_id)
                    .and_then(|resource| resource["relation_name"].as_str())
                    == Some(raw_orders_relation)
            })
            .cloned()
            .collect::<Vec<_>>();
        let resources = catalog_without_raw_orders[section]
            .as_object_mut()
            .expect("catalog resource section should be mutable");
        for unique_id in matching_ids {
            resources.remove(&unique_id);
        }
    }
    let fallback_catalog = parse_dbt_catalog(&catalog_without_raw_orders.to_string())
        .expect("catalog without raw orders should still parse");
    let fallback_bundle = analyze_dbt_artifacts(
        &manifest,
        &fallback_catalog,
        manifest.adapter_type(),
        dialect.as_ref(),
    )
    .expect("manifest-declared raw orders schema should replace missing catalog evidence");
    let fallback_protocol: Value = serde_json::from_str(&to_bundle_json(&fallback_bundle))
        .expect("fallback protocol should be valid JSON");
    let fallback_raw_orders_schema = fallback_protocol["source_schemas"]
        .as_array()
        .expect("fallback protocol should include source schemas")
        .iter()
        .find(|schema| schema["relation"] == raw_orders_relation)
        .expect("fallback raw orders schema should be emitted");
    assert_eq!(fallback_raw_orders_schema["source_kind"], "dbt_manifest");
    let fallback_columns = fallback_raw_orders_schema["columns"]
        .as_array()
        .expect("fallback raw orders columns should be an array")
        .iter()
        .map(|column| {
            column["name"]
                .as_str()
                .expect("fallback column name should be a string")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fallback_columns,
        [
            "amount",
            "created_at",
            "customer_id",
            "id",
            "region",
            "status",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
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
    assert_closed_number_range(&output_column(stage_layer, "amount")["domain"], "10", "100");
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

    let enriched_equalities =
        &layer_for_model(&protocol, "enriched_orders")["composed_semantics"]["join_equalities"];
    let enriched_equalities = enriched_equalities
        .as_array()
        .expect("composed join equalities should be an array");
    assert_eq!(enriched_equalities.len(), 2);
    let customer_equality = enriched_equalities
        .iter()
        .find(|equality| {
            equality["join_kind"] == "inner" && contains_string(equality, "customer_id")
        })
        .expect("customer join should be a composed inner equality");
    assert!(customer_equality["left"]["relation"]
        .as_str()
        .expect("left equality relation")
        .contains("\"raw\""));
    assert!(customer_equality["right"]["relation"]
        .as_str()
        .expect("right equality relation")
        .contains("\"raw\""));

    let return_equality = enriched_equalities
        .iter()
        .find(|equality| equality["join_kind"] == "left" && contains_string(equality, "order_id"))
        .expect("return join should retain its composed left equality");
    assert!(
        return_equality["left"]["relation"]
            .as_str()
            .expect("left equality relation")
            .contains("returns")
            || return_equality["right"]["relation"]
                .as_str()
                .expect("right equality relation")
                .contains("returns")
    );

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
    assert_eq!(aggregated["group_witness"]["aggregate"], "sum");
    assert_eq!(aggregated["group_witness"]["qualifying"]["status"], "residual");
    assert_eq!(aggregated["group_witness"]["rejected"]["status"], "residual");
    assert_lower_bounded_number_range(
        &output_column(layer_for_model(&protocol, "aggregated_orders"), "total_amount")["domain"],
        "20",
    );
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
    let lateral_output = output_column(
        layer_for_model(&protocol, "lateral_orders"),
        "adjusted_amount",
    );
    assert!(contains_string(&lateral_output["lineage"], "amount"));

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
    assert_closed_number_range(&output_column(final_layer, "amount")["domain"], "10", "50");

    let graph_components = protocol["graph"]["components"]
        .as_array()
        .expect("graph components should be an array");
    assert!(
        graph_components.len() >= 3,
        "expected the primary model graph plus disconnected feature models"
    );

    let expected_final_outcomes: Value = serde_json::from_str(include_str!(
        "fixtures/dbt_core_project/expected_final_outcomes.json"
    ))
    .expect("expected final-outcome fixture should be valid JSON");
    let actual_final_outcomes = final_outcome_snapshot(&protocol);
    assert_eq!(
        actual_final_outcomes,
        expected_final_outcomes,
        "complete dbt terminal-outcome semantics changed:\n{}",
        serde_json::to_string_pretty(&actual_final_outcomes)
            .expect("final-outcome snapshot should serialize")
    );

    let merge_statement = input_statement(&protocol, "incremental_merge_orders");
    assert!(
        !contains_string(merge_statement, "conditional_mutation"),
        "dbt model compiled_code should remain SELECT semantics; generated materialization MERGE is not part of manifest compiled_code"
    );
}
