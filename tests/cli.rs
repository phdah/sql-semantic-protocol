use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_with_stdin(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI should start");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("test SQL should be written");

    child.wait_with_output().expect("CLI should exit")
}

#[test]
fn stdin_input_and_selected_dialect_emit_only_protocol_json() {
    let output = run_with_stdin(&["--dialect", "snowflake"], "SELECT a FROM t WHERE a > 10");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout should contain protocol JSON only");

    assert_eq!(json["protocol_version"], "0.1.0");
    assert_eq!(json["source"]["dialect"], "snowflake");
    assert_eq!(json["statements"][0]["kind"], "query");
    assert!(!stdout.contains("sqlparser"));
}

#[test]
fn all_sqlparser_recognized_dialect_names_are_supported() {
    const DIALECTS: &[&str] = &[
        "generic",
        "mysql",
        "postgresql",
        "postgres",
        "hive",
        "sqlite",
        "snowflake",
        "redshift",
        "mssql",
        "clickhouse",
        "bigquery",
        "ansi",
        "duckdb",
        "databricks",
    ];

    for dialect in DIALECTS {
        let output = run_with_stdin(&["--dialect", *dialect], "SELECT a FROM t WHERE a > 10");

        assert!(
            output.status.success(),
            "dialect {dialect} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("dialect should emit protocol JSON");
        assert_eq!(json["source"]["dialect"].as_str(), Some(*dialect));
    }
}

#[test]
fn unknown_dialect_is_an_input_error() {
    let output = run_with_stdin(&["--dialect", "not-a-real-dialect"], "SELECT a FROM t");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("use a built-in dialect recognized by sqlparser"));
}

#[test]
fn file_input_is_supported() {
    let path = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-cli-{}.sql",
        std::process::id()
    ));
    fs::write(&path, "SELECT b FROM t WHERE a > 10").expect("test SQL file should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .arg("--file")
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    fs::remove_file(&path).expect("test SQL file should be removed");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("file input should emit protocol JSON");
    assert_eq!(json["source"]["dialect"], "generic");
}

#[test]
fn parse_errors_are_fatal_and_distinct_from_protocol_diagnostics() {
    let output = run_with_stdin(&[], "SELECT (");

    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("SQL parse error:"));
}

#[test]
fn input_errors_use_a_distinct_exit_code() {
    let output = run_with_stdin(&[], "");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert_eq!(stderr.trim(), "input error: SQL input is empty");
}

#[test]
fn unsupported_semantics_remain_successful_protocol_output() {
    let output = run_with_stdin(&[], "CREATE TABLE t (a INT)");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("unsupported SQL should still emit JSON");
    assert_eq!(json["statements"][0]["kind"], "unsupported");
    assert_eq!(
        json["statements"][0]["diagnostics"][0]["code"],
        "unsupported_statement"
    );
}


#[test]
fn repeated_sql_inputs_emit_one_ordered_bundle() {
    let output = run_with_stdin(
        &[
            "--sql",
            "SELECT a FROM alpha",
            "--sql",
            "SELECT b FROM beta",
        ],
        "",
    );

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("multiple SQL inputs should emit JSON");
    assert_eq!(json["protocol_version"], "0.2.0");
    assert_eq!(json["inputs"].as_array().map(Vec::len), Some(2));
    assert_eq!(json["inputs"][0]["id"], "input-0001");
    assert_eq!(json["inputs"][1]["id"], "input-0002");
    assert_eq!(json["inputs"][0]["source"]["kind"], "inline");
    assert_eq!(json["inputs"][1]["source"]["kind"], "inline");
    assert_eq!(
        json["inputs"][0]["statements"][0]["dependencies"][0],
        "alpha"
    );
    assert_eq!(
        json["inputs"][1]["statements"][0]["dependencies"][0],
        "beta"
    );
}

#[test]
fn repeated_file_inputs_are_supported() {
    let base = std::env::temp_dir();
    let first = base.join(format!(
        "sql-semantic-protocol-cli-{}-first.sql",
        std::process::id()
    ));
    let second = base.join(format!(
        "sql-semantic-protocol-cli-{}-second.sql",
        std::process::id()
    ));
    fs::write(&first, "SELECT a FROM alpha").expect("first SQL file should be written");
    fs::write(&second, "SELECT b FROM beta").expect("second SQL file should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .arg("--file")
        .arg(&first)
        .arg("--file")
        .arg(&second)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    fs::remove_file(&first).expect("first SQL file should be removed");
    fs::remove_file(&second).expect("second SQL file should be removed");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("multiple files should emit JSON");
    assert_eq!(json["inputs"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        json["inputs"][0]["source"]["path"],
        first.display().to_string()
    );
    assert_eq!(
        json["inputs"][1]["source"]["path"],
        second.display().to_string()
    );
}

#[test]
fn mixed_sql_and_file_inputs_preserve_command_line_order() {
    let path = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-cli-{}-mixed.sql",
        std::process::id()
    ));
    fs::write(&path, "SELECT b FROM beta").expect("test SQL file should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .arg("--sql")
        .arg("SELECT a FROM alpha")
        .arg("--file")
        .arg(&path)
        .arg("--sql")
        .arg("SELECT c FROM gamma")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    fs::remove_file(&path).expect("test SQL file should be removed");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("mixed inputs should emit JSON");
    assert_eq!(json["inputs"].as_array().map(Vec::len), Some(3));
    assert_eq!(json["inputs"][0]["source"]["kind"], "inline");
    assert_eq!(json["inputs"][1]["source"]["kind"], "file");
    assert_eq!(json["inputs"][2]["source"]["kind"], "inline");
}

#[test]
fn multi_input_parse_error_identifies_the_failing_input() {
    let output = run_with_stdin(&["--sql", "SELECT 1", "--sql", "SELECT ("], "");

    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("input-0002"));
    assert!(stderr.contains("(inline)"));
    assert!(stderr.contains("SQL parse error"));
}

#[test]
fn file_read_error_identifies_the_input_position_and_path() {
    let missing = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-cli-{}-missing.sql",
        std::process::id()
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .arg("--sql")
        .arg("SELECT 1")
        .arg("--file")
        .arg(&missing)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("input 2"));
    assert!(stderr.contains(&missing.display().to_string()));
}
