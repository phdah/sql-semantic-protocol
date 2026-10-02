use sql_semantic_protocol::{
    analyze_inputs, to_bundle_json, Error, ProtocolStatement, SqlInput, SqlInputSource,
};
use sqlparser::dialect::GenericDialect;

#[test]
fn multiple_inputs_preserve_caller_order_and_generated_identity() {
    let dialect = GenericDialect {};
    let inputs = vec![
        SqlInput::inline("SELECT a FROM alpha"),
        SqlInput::file("queries/beta.sql", "SELECT b FROM beta"),
        SqlInput::inline("SELECT c FROM gamma"),
    ];

    let bundle =
        analyze_inputs(&inputs, "generic", &dialect).expect("multiple inputs should be analyzed");

    assert_eq!(bundle.protocol_version(), "0.2.0");
    assert_eq!(bundle.inputs().len(), 3);
    assert_eq!(bundle.inputs()[0].id(), "input-0001");
    assert_eq!(bundle.inputs()[1].id(), "input-0002");
    assert_eq!(bundle.inputs()[2].id(), "input-0003");

    assert!(matches!(
        bundle.inputs()[0].source(),
        SqlInputSource::Inline
    ));
    assert!(matches!(
        bundle.inputs()[1].source(),
        SqlInputSource::File { path } if path == "queries/beta.sql"
    ));

    for input in bundle.inputs() {
        assert_eq!(input.dialect(), "generic");
        assert!(matches!(
            input.statements().first(),
            Some(ProtocolStatement::Query(_))
        ));
    }
}

#[test]
fn multi_input_json_is_one_deterministic_protocol_document() {
    let dialect = GenericDialect {};
    let inputs = vec![
        SqlInput::inline("SELECT a FROM alpha"),
        SqlInput::file("queries/beta.sql", "SELECT b FROM beta"),
    ];

    let bundle =
        analyze_inputs(&inputs, "generic", &dialect).expect("multiple inputs should be analyzed");
    let first = to_bundle_json(&bundle);
    let second = to_bundle_json(&bundle);

    assert_eq!(first, second);

    let json: serde_json::Value =
        serde_json::from_str(&first).expect("bundle output should be valid JSON");
    assert_eq!(json["protocol_version"], "0.2.0");
    assert_eq!(json["inputs"][0]["id"], "input-0001");
    assert_eq!(json["inputs"][0]["source"]["kind"], "inline");
    assert_eq!(json["inputs"][1]["id"], "input-0002");
    assert_eq!(json["inputs"][1]["source"]["kind"], "file");
    assert_eq!(json["inputs"][1]["source"]["path"], "queries/beta.sql");
    assert_eq!(json["graph"]["diagnostics"], serde_json::json!([]));
    assert_eq!(json["graph"]["edges"][0]["relation"], "alpha");
    assert_eq!(json["graph"]["edges"][0]["resolution"], "external");
    assert_eq!(json["graph"]["edges"][1]["relation"], "beta");
    assert_eq!(json["graph"]["edges"][1]["resolution"], "external");
    assert_eq!(json["graph"]["components"].as_array().map(Vec::len), Some(2));
}

#[test]
fn input_failure_identifies_the_failing_source() {
    let dialect = GenericDialect {};
    let inputs = vec![
        SqlInput::inline("SELECT 1"),
        SqlInput::file("queries/broken.sql", "SELECT ("),
    ];

    let error = analyze_inputs(&inputs, "generic", &dialect)
        .expect_err("the malformed second input should fail");

    assert_eq!(error.input_id(), "input-0002");
    assert!(matches!(
        error.input_source(),
        SqlInputSource::File { path } if path == "queries/broken.sql"
    ));
    assert!(matches!(error.error(), Error::Parse(_)));
    assert!(error.to_string().contains("queries/broken.sql"));
}

#[test]
fn analysis_failure_identifies_the_failing_input() {
    let dialect = GenericDialect {};
    let inputs = vec![SqlInput::inline("SELECT 1"), SqlInput::inline("")];

    let error = analyze_inputs(&inputs, "generic", &dialect)
        .expect_err("empty SQL should fail during semantic analysis");

    assert_eq!(error.input_id(), "input-0002");
    assert!(matches!(error.input_source(), SqlInputSource::Inline));
    assert!(matches!(error.error(), Error::Analysis(_)));
}
