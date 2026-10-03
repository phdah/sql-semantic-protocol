use std::process::{Command, Output, Stdio};

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run")
}

#[test]
fn cli_always_emits_complete_protocol_with_terminal_outcomes() {
    let output = run(&[
        "--sql",
        "CREATE TABLE stage.orders AS SELECT id FROM raw.orders",
        "--sql",
        "CREATE TABLE mart.orders AS SELECT id FROM stage.orders",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("protocol output should be JSON");

    assert_eq!(json["layers"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        json["graph"]["components"][0]["final_outcomes"][0]["name"],
        "mart.orders"
    );
}

#[test]
fn cli_has_no_producer_side_output_scope() {
    let output = run(&["--scope", "final", "--sql", "SELECT 1"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown option: --scope"));
}


fn produced_relation_names(json: &serde_json::Value) -> Vec<&str> {
    json["layers"]
        .as_array()
        .expect("layers should be an array")
        .iter()
        .filter_map(|layer| {
            layer["produces"]
                .as_array()
                .and_then(|datasets| datasets.iter().find_map(|dataset| dataset["name"].as_str()))
        })
        .collect()
}

#[test]
fn cli_target_keeps_target_and_required_ancestors() {
    let output = run(&[
        "--target",
        "mart.orders",
        "--sql",
        "CREATE TABLE stage.orders AS SELECT id FROM raw.orders",
        "--sql",
        "CREATE TABLE mart.orders AS SELECT id FROM stage.orders",
        "--sql",
        "CREATE TABLE mart.audit AS SELECT id FROM raw.audit",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("protocol output should be JSON");

    assert_eq!(
        produced_relation_names(&json),
        vec!["stage.orders", "mart.orders"]
    );
    assert_eq!(json["inputs"].as_array().map(Vec::len), Some(3));
    assert_eq!(json["graph"]["components"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["graph"]["components"][0]["final_outcomes"][0]["name"],
        "mart.orders"
    );
}

#[test]
fn cli_accepts_multiple_targets() {
    let output = run(&[
        "--target",
        "mart.orders",
        "--target",
        "mart.audit",
        "--sql",
        "CREATE TABLE stage.orders AS SELECT id FROM raw.orders",
        "--sql",
        "CREATE TABLE mart.orders AS SELECT id FROM stage.orders",
        "--sql",
        "CREATE TABLE mart.audit AS SELECT id FROM raw.audit",
        "--sql",
        "CREATE TABLE mart.customers AS SELECT id FROM raw.customers",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("protocol output should be JSON");

    assert_eq!(
        produced_relation_names(&json),
        vec!["stage.orders", "mart.orders", "mart.audit"]
    );
    assert_eq!(json["graph"]["components"].as_array().map(Vec::len), Some(2));
}

#[test]
fn cli_unknown_target_is_an_input_error() {
    let output = run(&[
        "--target",
        "mart.missing",
        "--sql",
        "CREATE TABLE mart.orders AS SELECT id FROM raw.orders",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        "input error: target relation 'mart.missing' is not produced by any supplied transformation"
    ));
}
