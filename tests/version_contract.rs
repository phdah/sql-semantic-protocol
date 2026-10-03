use sql_semantic_protocol::PROTOCOL_VERSION;

#[test]
fn active_release_artifacts_match_package_version() {
    assert_eq!(PROTOCOL_VERSION, env!("CARGO_PKG_VERSION"));

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json"))
            .expect("active protocol schema should be valid JSON");
    assert_eq!(
        schema["properties"]["protocol_version"]["const"],
        PROTOCOL_VERSION
    );

    for (name, raw) in [
        ("full example", include_str!("../examples/protocol.json")),
        (
            "simple example",
            include_str!("../examples/protocol-simple.json"),
        ),
    ] {
        let example: serde_json::Value = serde_json::from_str(raw)
            .unwrap_or_else(|error| panic!("{name} should be valid JSON: {error}"));
        assert_eq!(
            example["protocol_version"], PROTOCOL_VERSION,
            "{name} protocol version must match the Cargo package version"
        );
    }

    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../.release-please-manifest.json"))
            .expect("release-please manifest should be valid JSON");
    assert_eq!(
        manifest["."], PROTOCOL_VERSION,
        "release-please current version must match the Cargo package version"
    );
}
