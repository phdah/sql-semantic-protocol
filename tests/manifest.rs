use std::fs;
use std::process::{Command, Stdio};

use sql_semantic_protocol::{
    analyze_configured_inputs, select_targets, to_bundle_json, ConfiguredSqlInput, SqlInput,
};
use sqlparser::dialect::{GenericDialect, PostgreSqlDialect};

#[test]
fn manifest_execution_matches_configured_api_semantics() {
    let root = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-manifest-{}",
        std::process::id()
    ));
    let sql_dir = root.join("sql");
    fs::create_dir_all(&sql_dir).expect("manifest fixture directory should be created");

    let stage_sql = "CREATE TABLE stage.orders AS SELECT id, amount FROM raw.orders";
    let mart_sql =
        "CREATE TABLE mart.orders AS SELECT id, amount FROM stage.orders WHERE amount > 10";
    let audit_sql = "CREATE TABLE mart.audit AS SELECT id FROM raw.audit";
    fs::write(sql_dir.join("stage.sql"), stage_sql).expect("stage SQL should be written");

    let manifest_path = root.join("analysis.json");
    let manifest = serde_json::json!({
        "manifest_version": "1",
        "dialect": "generic",
        "output_scope": "targets",
        "targets": ["mart.orders"],
        "inputs": [
            {
                "id": "stage-orders",
                "file": "sql/stage.sql"
            },
            {
                "id": "mart-orders",
                "dialect": "postgresql",
                "sql": mart_sql
            },
            {
                "id": "audit",
                "sql": audit_sql
            }
        ]
    });
    fs::write(&manifest_path, manifest.to_string()).expect("manifest should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .arg("--manifest")
        .arg(&manifest_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let generic = GenericDialect {};
    let postgres = PostgreSqlDialect {};
    let inputs = [
        SqlInput::file("sql/stage.sql", stage_sql),
        SqlInput::inline(mart_sql),
        SqlInput::inline(audit_sql),
    ];
    let configured = [
        ConfiguredSqlInput::new("stage-orders", &inputs[0], "generic", &generic),
        ConfiguredSqlInput::new("mart-orders", &inputs[1], "postgresql", &postgres),
        ConfiguredSqlInput::new("audit", &inputs[2], "generic", &generic),
    ];
    let bundle =
        analyze_configured_inputs(&configured).expect("configured API analysis should succeed");
    let bundle = select_targets(&bundle, &["mart.orders".to_string()])
        .expect("configured API target selection should succeed");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(stdout.trim(), to_bundle_json(&bundle));

    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("manifest output should be JSON");
    assert_eq!(json["inputs"][0]["id"], "stage-orders");
    assert_eq!(json["inputs"][0]["source"]["path"], "sql/stage.sql");
    assert_eq!(json["inputs"][0]["dialect"], "generic");
    assert_eq!(json["inputs"][1]["id"], "mart-orders");
    assert_eq!(json["inputs"][1]["dialect"], "postgresql");
    assert_eq!(json["inputs"][2]["id"], "audit");
    assert_eq!(json["layers"].as_array().map(Vec::len), Some(2));
    assert_eq!(json["layers"][0]["produces"][0]["name"], "stage.orders");
    assert_eq!(json["layers"][1]["produces"][0]["name"], "mart.orders");

    fs::remove_dir_all(&root).expect("manifest fixture directory should be removed");
}

#[test]
fn manifest_rejects_duplicate_input_id_before_analysis() {
    let root = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-manifest-duplicate-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("manifest fixture directory should be created");
    let manifest_path = root.join("analysis.json");
    fs::write(
        &manifest_path,
        r#"{
            "manifest_version": "1",
            "inputs": [
                {"id": "same", "sql": "SELECT 1"},
                {"id": "same", "sql": "SELECT 2"}
            ]
        }"#,
    )
    .expect("manifest should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .arg("--manifest")
        .arg(&manifest_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    fs::remove_dir_all(&root).expect("manifest fixture directory should be removed");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("manifest input id 'same' is duplicated"));
}

#[test]
fn manifest_is_exclusive_with_direct_analysis_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .args([
            "--manifest",
            "analysis.json",
            "--dialect",
            "generic",
            "--sql",
            "SELECT 1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--manifest cannot be combined"));
}
