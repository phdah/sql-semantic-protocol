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
fn cli_selects_final_or_all_layer_protocol_output() {
    let arguments = [
        "--sql",
        "CREATE TABLE stage.orders AS SELECT id FROM raw.orders",
        "--sql",
        "CREATE TABLE mart.orders AS SELECT id FROM stage.orders",
    ];

    let mut final_arguments = vec!["--scope", "final"];
    final_arguments.extend(arguments);
    let final_output = run(&final_arguments);
    assert!(
        final_output.status.success(),
        "{}",
        String::from_utf8_lossy(&final_output.stderr)
    );

    let mut all_arguments = vec!["--scope", "all"];
    all_arguments.extend(arguments);
    let all_output = run(&all_arguments);
    assert!(
        all_output.status.success(),
        "{}",
        String::from_utf8_lossy(&all_output.stderr)
    );

    let final_json: serde_json::Value =
        serde_json::from_slice(&final_output.stdout).expect("final scope should emit JSON");
    let all_json: serde_json::Value =
        serde_json::from_slice(&all_output.stdout).expect("all scope should emit JSON");

    assert_eq!(final_json["layers"].as_array().map(Vec::len), Some(1));
    assert_eq!(all_json["layers"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        final_json["layers"][0]["produces"][0]["name"],
        "mart.orders"
    );
    assert_eq!(final_json["graph"], all_json["graph"]);
}

#[test]
fn cli_rejects_unknown_output_scope() {
    let output = run(&["--scope", "intermediate", "--sql", "SELECT 1"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("unsupported output scope 'intermediate'; expected final or all"));
}

#[test]
fn cli_does_not_silently_ignore_scope_for_openlineage() {
    let output = run(&[
        "--format",
        "openlineage",
        "--namespace",
        "postgresql://warehouse",
        "--scope",
        "final",
        "--sql",
        "SELECT id FROM raw.orders",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("--scope is only supported with --format protocol"));
}
