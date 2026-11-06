//! TASK-173: authoritative metadata facts retain their source provenance and
//! absent or unsupported lower-authority metadata never fabricates warehouse evidence.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_lifecycle_plan,
    to_bundle_json, analyze_dbt_manifest_with_schemas, ComposedSemantics, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintSourceKind, DataType, DbtArtifactsError, LiteralValue, PhysicalOrderedLifecycleProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationConstraint, SchemaSourceKind,
    SqlInput, ValueDomain,
};

const FILTER: &str = "SELECT k FROM raw.orders WHERE k >= 1";
const DIALECTS: &[&str] = &[
    "generic",
    "mysql",
    "postgresql",
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

fn physical_row(value: Option<i64>) -> PhysicalWriteRow {
    let value = value.map_or(LiteralValue::Null, |value| {
        LiteralValue::Number(value.to_string())
    });
    PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), value)])).unwrap()
}

fn verify_complete_physical_filter(catalog: &RelationCatalog) -> (Vec<usize>, Vec<usize>) {
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(FILTER);
    let configured = [ConfiguredSqlInput::new(
        "task173-physical",
        &sql,
        "duckdb",
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    let original = PhysicalWriteState::new(
        "raw.orders",
        vec![
            physical_row(Some(1)),
            physical_row(Some(1)),
            physical_row(None),
            physical_row(Some(0)),
        ],
    )
    .unwrap();
    let exact = match physical_ordered_lifecycle_plan(&bundle, &[original.clone()]) {
        PhysicalOrderedLifecycleProof::Exact(exact) => exact,
        other => panic!("full physical-source evidence must be exact: {other:?}"),
    };
    assert_eq!(exact.final_states, vec![original.clone()]);
    assert_eq!(exact.checkpoints.len(), 1);
    assert_eq!(exact.checkpoints[0].states, vec![original.clone()]);
    let [read] = exact.reads.as_slice() else {
        panic!("one canonical SELECT checkpoint is required");
    };
    assert_eq!(read.source, original);
    assert_eq!(
        read.output.rows(),
        &[physical_row(Some(1)), physical_row(Some(1))]
    );
    assert_eq!(read.surviving_source_ordinals, vec![0, 1]);
    assert_eq!(read.rejected_source_ordinals, vec![2, 3]);
    assert_eq!(
        read.output_schema.columns()[0].data_type(),
        read.source_schema.columns()[0].data_type()
    );
    let domains = read
        .exact_domains()
        .expect("complete typed output histogram");
    assert_eq!(domains.len(), 1);
    assert!(to_bundle_json(&bundle).contains(r#""protocol_version":"#));
    assert_inclusive_lower_one(&filtered_output_domain(catalog, "duckdb"));
    (
        read.surviving_source_ordinals.clone(),
        read.rejected_source_ordinals.clone(),
    )
}

#[test]
fn direct_dbt_catalog_and_manifest_fallback_attest_one_exact_physical_population() {
    let sql = SqlInput::inline("CREATE TABLE raw.orders (k INTEGER)");
    let dialect = dialect_from_name("duckdb").unwrap();
    let ddl = analyze_inputs(&[sql], "duckdb", dialect.as_ref()).unwrap();
    let direct_catalog = RelationCatalog::from_schemas(ddl.source_schemas()).unwrap();
    let dbt_catalog = analyzed_dbt(Some("TEXT"), Some("INTEGER")).unwrap();
    let dbt_manifest = analyzed_dbt(Some("INTEGER"), None).unwrap();
    let dbt_catalog = RelationCatalog::from_schemas(dbt_catalog.source_schemas()).unwrap();
    let dbt_manifest = RelationCatalog::from_schemas(dbt_manifest.source_schemas()).unwrap();

    let expected = verify_complete_physical_filter(&direct_catalog);
    assert_eq!(expected, (vec![0, 1], vec![2, 3]));
    assert_eq!(verify_complete_physical_filter(&dbt_catalog), expected);
    assert_eq!(verify_complete_physical_filter(&dbt_manifest), expected);
}

#[test]
fn dbt_declared_constraints_and_generic_tests_keep_separate_unknown_enforcement() {
    let mut value = manifest_json(Some("INTEGER"));
    value["sources"]["source.task173.orders"]["columns"]["k"]["constraints"] =
        json!([{"type": "unique"}]);
    value["nodes"]["test.task173.unique_orders_k"] = json!({
        "unique_id": "test.task173.unique_orders_k",
        "resource_type": "test",
        "relation_name": null,
        "attached_node": "source.task173.orders",
        "column_name": "k",
        "test_metadata": {
            "name": "unique",
            "namespace": null,
            "kwargs": {"column_name": "k"}
        },
        "depends_on": {"nodes": ["source.task173.orders"]}
    });
    let manifest = parse_dbt_manifest(&value.to_string()).unwrap();
    let constraints = manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "raw.orders")
        .expect("manifest owns declared key constraints and generic tests");
    let key = constraints
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::UniqueKey(key) if key.columns() == ["k"] => Some(key),
            _ => None,
        })
        .expect("one canonical merged unique key");
    assert_eq!(key.evidence().len(), 2);
    assert!(key.evidence().iter().any(|evidence| {
        evidence.provenance().source_kind() == ConstraintSourceKind::DbtConstraint
    }));
    assert!(key
        .evidence()
        .iter()
        .any(|evidence| { evidence.provenance().source_kind() == ConstraintSourceKind::DbtTest }));
    assert!(key
        .evidence()
        .iter()
        .all(|evidence| { evidence.enforcement() == ConstraintEnforcement::Unknown }));
    let catalog = parse_dbt_catalog(&catalog_json(Some("INTEGER")).to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    let doc = to_bundle_json(&bundle);
    assert!(doc.contains(r#""source_kind":"dbt_test""#));
    assert!(doc.contains(r#""source_kind":"dbt_constraint""#));
    assert!(doc.contains(r#""enforcement":"unknown""#));

    // A declared test is evidence of an asserted property, not a witnessed
    // warehouse constraint. Our physical fixture intentionally includes 1,1.
    let same_source = RelationCatalog::from_schemas(bundle.source_schemas()).unwrap();
    assert_eq!(
        verify_complete_physical_filter(&same_source),
        (vec![0, 1], vec![2, 3])
    );
}

#[test]
fn manifest_column_without_data_type_is_explicitly_rejected() {
    let mut manifest = manifest_json(Some("INTEGER"));
    manifest["sources"]["source.task173.orders"]["columns"]["k"]
        .as_object_mut()
        .unwrap()
        .remove("data_type");
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    assert!(matches!(
        analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()),
        Err(DbtArtifactsError::MissingDeclaredColumnTypes { relation, columns })
            if relation == "raw.orders" && columns == ["k"]
    ));
}

fn manifest_json(source_type: Option<&str>) -> Value {
    let mut source = json!({
        "unique_id": "source.task173.orders",
        "resource_type": "source",
        "name": "orders",
        "relation_name": "raw.orders"
    });
    if let Some(data_type) = source_type {
        source["columns"] = json!({"k": {"name": "k", "data_type": data_type}});
    }
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task173.filtered_orders": {
                "unique_id": "model.task173.filtered_orders",
                "resource_type": "model",
                "name": "filtered_orders",
                "relation_name": "filtered_orders",
                "language": "sql",
                "raw_code": "SELECT k FROM raw.orders WHERE k < 0",
                "compiled_code": FILTER,
                "depends_on": {"nodes": ["source.task173.orders"]}
            }
        },
        "sources": {"source.task173.orders": source}
    })
}

fn catalog_json(data_type: Option<&str>) -> Value {
    let mut sources = json!({});
    if let Some(data_type) = data_type {
        sources["source.task173.orders"] = json!({
            "unique_id": "source.task173.orders",
            "metadata": {
                "type": "BASE TABLE", "database": null, "schema": "raw", "name": "orders"
            },
            "columns": {"k": {"name": "k", "type": data_type, "index": 1}},
            "stats": {}
        });
    }
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": {},
        "sources": sources,
        "errors": null
    })
}

fn analyzed_dbt(
    declared_type: Option<&str>,
    catalog_type: Option<&str>,
) -> Result<sql_semantic_protocol::AnalysisBundle, DbtArtifactsError> {
    let manifest = parse_dbt_manifest(&manifest_json(declared_type).to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog_json(catalog_type).to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref())
}

fn filtered_output_domain(catalog: &RelationCatalog, dialect_name: &str) -> ValueDomain {
    let input = SqlInput::inline(FILTER);
    let dialect = dialect_from_name(dialect_name).unwrap();
    let configured = [ConfiguredSqlInput::new(
        "task173-filter",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("typed filter must produce resolved semantics for {dialect_name}");
    };
    semantics.output().columns()[0].domain().clone()
}

fn assert_inclusive_lower_one(domain: &ValueDomain) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected a provable output range, got {domain:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one range, got {domain:?}");
    };
    let lower = range.lower().expect(">= 1 must retain a lower bound");
    assert!(lower.inclusive());
    assert_eq!(lower.value().value(), &LiteralValue::Number("1".into()));
}

#[test]
fn dbt_compiled_sql_and_catalog_type_are_independent_authorities() {
    // Raw SQL contradicts compiled SQL and the manifest declaration contradicts
    // warehouse introspection. Neither lower-authority fact may override its owner.
    let bundle = analyzed_dbt(Some("TEXT"), Some("INTEGER")).unwrap();
    let [schema] = bundle.source_schemas() else {
        panic!("one physical schema is required");
    };
    assert_eq!(schema.relation(), "raw.orders");
    assert_eq!(schema.source_kind(), Some(SchemaSourceKind::DbtCatalog));
    assert_eq!(
        schema.columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    let [layer] = bundle.layers() else {
        panic!("compiled model must produce one transformation");
    };
    assert_eq!(layer.input_id(), "model.task173.filtered_orders");
    assert!(layer.consumes().contains(&"raw.orders".to_string()));
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("compiled SQL must resolve");
    };
    assert_inclusive_lower_one(semantics.output().columns()[0].domain());
}

#[test]
fn absent_warehouse_catalog_uses_only_explicit_manifest_type_evidence() {
    let bundle = analyzed_dbt(Some("INTEGER"), None).unwrap();
    let [schema] = bundle.source_schemas() else {
        panic!("declared complete source schema is required");
    };
    assert_eq!(schema.source_kind(), Some(SchemaSourceKind::DbtManifest));
    assert_eq!(
        schema.columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    assert_inclusive_lower_one(&filtered_output_domain(
        &RelationCatalog::from_schemas(bundle.source_schemas()).unwrap(),
        "duckdb",
    ));
}

#[test]
fn absent_catalog_and_declarations_fail_closed_without_inventing_datatypes() {
    assert!(matches!(
        analyzed_dbt(None, None),
        Err(DbtArtifactsError::MissingCatalogSchema { relation })
            if relation == "raw.orders"
    ));
}

#[test]
fn typed_sql_and_dbt_evidence_produce_equal_outcome_domains_for_all_dialects() {
    let dbt_bundle = analyzed_dbt(Some("TEXT"), Some("INTEGER")).unwrap();
    let sql = SqlInput::inline("CREATE TABLE raw.orders (k INTEGER)");
    let dialect = dialect_from_name("duckdb").unwrap();
    let ddl = analyze_inputs(&[sql], "duckdb", dialect.as_ref()).unwrap();
    let ddl_schema = ddl
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "raw.orders")
        .expect("direct DDL must retain typed column schema");
    assert_eq!(ddl_schema.source_kind(), Some(SchemaSourceKind::SqlDdl));
    assert_eq!(
        ddl_schema.columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    let from_dbt = RelationCatalog::from_schemas(dbt_bundle.source_schemas()).unwrap();
    let from_ddl = RelationCatalog::from_schemas(ddl.source_schemas()).unwrap();
    for name in DIALECTS {
        let dbt_domain = filtered_output_domain(&from_dbt, name);
        let ddl_domain = filtered_output_domain(&from_ddl, name);
        assert_eq!(dbt_domain, ddl_domain, "dialect {name}");
        assert_inclusive_lower_one(&dbt_domain);
    }
}

#[test]
fn duckdb_full_bags_prove_true_false_and_unknown_filter_partitions() {
    let connection = duckdb::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE SCHEMA raw;
             CREATE TABLE raw.orders(k INTEGER);
             INSERT INTO raw.orders VALUES (1),(1),(NULL),(0);",
        )
        .unwrap();

    let values = |query: &str| {
        let mut statement = connection.prepare(query).unwrap();
        statement
            .query_map([], |row| row.get::<_, Option<i32>>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };

    assert_eq!(
        values("SELECT k FROM raw.orders ORDER BY k NULLS LAST"),
        vec![Some(0), Some(1), Some(1), None]
    );
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE k >= 1 ORDER BY k"),
        vec![Some(1), Some(1)]
    );
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE NOT (k >= 1)"),
        vec![Some(0)]
    );
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE (k >= 1) IS NULL"),
        vec![None]
    );

    let dbt = analyzed_dbt(Some("INTEGER"), Some("INTEGER")).unwrap();
    let catalog = RelationCatalog::from_schemas(dbt.source_schemas()).unwrap();
    assert_inclusive_lower_one(&filtered_output_domain(&catalog, "duckdb"));
}

#[cfg(feature = "odcs")]
mod odcs_authority {
    use super::*;
    use sql_semantic_protocol::{
        parse_odcs_yaml, ConstraintEnforcement, ConstraintSourceKind, OdcsError, RelationConstraint,
    };

    fn contract(property: &str) -> String {
        format!(
            "apiVersion: v3.2.0\nkind: DataContract\nid: task173\nname: TASK-173\nversion: 1.0.0\nstatus: active\nschema:\n  - name: orders\n    physicalName: raw.orders\n    properties:\n      - name: k\n{property}\n"
        )
    }

    fn parse(property: &str) -> Result<sql_semantic_protocol::OdcsMetadata, OdcsError> {
        parse_odcs_yaml(
            &contract(property),
            "duckdb",
            &RelationCatalog::new(&["raw.orders"]).unwrap(),
        )
    }

    #[test]
    fn valid_physical_type_wins_over_unsupported_logical_hint_with_diagnostic() {
        let metadata = parse("        physicalType: INTEGER\n        logicalType: vendorMystery\n        required: true").unwrap();
        let [schema] = metadata.source_schemas() else {
            panic!("valid physical datatype must produce a typed schema");
        };
        assert_eq!(
            schema.source_kind(),
            Some(SchemaSourceKind::ExternalMetadata)
        );
        assert_eq!(
            schema.columns()[0].data_type(),
            &DataType::SignedInteger { bits: Some(32) }
        );
        assert!(metadata.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "odcs_unsupported_logical_type"
                && diagnostic
                    .message()
                    .contains("valid physicalType is retained")
        }));
        let constraint = metadata.relation_constraints()[0]
            .constraints()
            .iter()
            .find_map(|constraint| match constraint {
                RelationConstraint::NotNull(rule) => Some(rule),
                _ => None,
            })
            .expect("required ODCS column");
        assert_eq!(
            constraint.evidence()[0].enforcement(),
            ConstraintEnforcement::Unknown
        );
        assert_eq!(
            constraint.evidence()[0].provenance().source_kind(),
            ConstraintSourceKind::ExternalMetadata
        );

        let from_odcs = RelationCatalog::from_schemas(metadata.source_schemas()).unwrap();
        let from_dbt = RelationCatalog::from_schemas(
            analyzed_dbt(Some("TEXT"), Some("INTEGER"))
                .unwrap()
                .source_schemas(),
        )
        .unwrap();
        assert_eq!(
            filtered_output_domain(&from_odcs, "duckdb"),
            filtered_output_domain(&from_dbt, "duckdb")
        );
    }

    #[test]
    fn odcs_physical_evidence_reuses_complete_source_and_terminal_oracle() {
        let metadata = parse("        physicalType: INTEGER").unwrap();
        let catalog = RelationCatalog::from_schemas(metadata.source_schemas()).unwrap();
        assert_eq!(
            verify_complete_physical_filter(&catalog),
            (vec![0, 1], vec![2, 3])
        );
    }

    #[test]
    fn unsupported_logical_only_type_is_an_error_not_a_guessed_schema() {
        assert!(matches!(
            parse("        logicalType: vendorMystery"),
            Err(OdcsError::InvalidDocument { .. })
        ));
    }

    #[test]
    fn absent_types_report_both_column_and_relation_absence() {
        let metadata = parse("").unwrap();
        assert!(metadata.source_schemas().is_empty());
        let codes = metadata
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"odcs_unresolved_data_type"));
        assert!(codes.contains(&"odcs_incomplete_source_schema"));
    }
}
