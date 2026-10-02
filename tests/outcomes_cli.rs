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
