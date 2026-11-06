//! TASK-174: dbt materialization intent and execution evidence are independent
//! of source-backed SELECT outcomes and never certify an unexecuted target.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, physical_ordered_lifecycle_plan, select_targets,
    to_bundle_json, ComposedSemantics, ConfiguredSqlInput, DbtManifestError,
    InlineDependencyEvidence, LiteralValue, PhysicalOrderedLifecycleProof, PhysicalWriteRow,
    PhysicalWriteState, ProducerExecution, ProducerMaterialization, RelationCatalog, SqlInput,
    ValueDomain,
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

fn manifest(materialization: &str, relation_name: Option<&str>) -> Value {
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "adapter_type": "duckdb",
            "dbt_version": "1.11.8"
        },
        "nodes": {
            "model.task174.filtered": {
                "unique_id": "model.task174.filtered",
                "resource_type": "model",
                "name": "filtered",
                "language": "sql",
                "config": {"materialized": materialization},
                "relation_name": relation_name,
                "compiled_code": FILTER,
                "raw_code": "SELECT k FROM {{ ref('missing') }}",
                "depends_on": {"nodes": ["source.task174.orders"]}
            }
        },
        "sources": {
            "source.task174.orders": {
                "unique_id": "source.task174.orders",
                "resource_type": "source",
                "name": "orders",
                "relation_name": "raw.orders",
                "columns": {"k": {"name": "k", "data_type": "INTEGER"}}
            }
        }
    })
}

fn analyze(value: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let manifest = parse_dbt_manifest(&value.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()).unwrap()
}

fn row(value: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([(
        "k".to_string(),
        value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
    )]))
    .unwrap()
}

fn source_bag() -> PhysicalWriteState {
    PhysicalWriteState::new(
        "raw.orders",
        vec![row(Some(1)), row(Some(1)), row(None), row(Some(0))],
    )
    .unwrap()
}

fn assert_positive_domain(domain: &ValueDomain) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("outcome must retain a typed range, got {domain:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one range, got {domain:?}");
    };
    let lower = range.lower().expect("filter provides a lower bound");
    assert!(lower.inclusive());
    assert_eq!(lower.value().value(), &LiteralValue::Number("1".into()));
}

fn canonical_filter_domain(bundle: &sql_semantic_protocol::AnalysisBundle) -> ValueDomain {
    let layer = bundle.layers().last().expect("one produced SELECT");
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("selected model must retain resolved semantics");
    };
    semantics.output().columns()[0].domain().clone()
}

#[test]
fn dbt_materialization_declares_storage_without_attesting_an_executed_write() {
    let cases = [
        (
            "view",
            Some("filtered"),
            ProducerMaterialization::LogicalView,
            "create_view",
        ),
        (
            "table",
            Some("filtered"),
            ProducerMaterialization::PhysicalTable,
            "create_table",
        ),
        ("ephemeral", None, ProducerMaterialization::Inline, ""),
        (
            "incremental",
            Some("filtered"),
            ProducerMaterialization::Incremental,
            "create_view",
        ),
    ];
    let mut canonical = None;
    for (tag, relation, kind, expected_definition) in cases {
        let bundle = analyze(&manifest(tag, relation));
        assert_eq!(bundle.producer_evidence().len(), 1);
        let evidence = &bundle.producer_evidence()[0];
        assert_eq!(evidence.input_id(), "model.task174.filtered");
        assert_eq!(evidence.relation(), relation);
        assert_eq!(evidence.materialization(), kind);
        assert_eq!(evidence.execution(), ProducerExecution::Unattested);
        assert_eq!(
            evidence.inline_dependencies(),
            InlineDependencyEvidence::NotApplicable
        );

        let domain = canonical_filter_domain(&bundle);
        assert_positive_domain(&domain);
        if let Some(previous) = &canonical {
            assert_eq!(&domain, previous, "{tag} changes the same SELECT outcome");
        } else {
            canonical = Some(domain);
        }

        let emission: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
        assert_eq!(emission["producer_evidence"][0]["execution"], "unattested");
        assert_eq!(
            emission["producer_evidence"][0]["input_id"],
            "model.task174.filtered"
        );
        assert_eq!(
            emission["producer_evidence"][0]["relation"],
            json!(relation)
        );
        assert!(emission["producer_evidence"][0]["source_kind"] == "dbt_manifest");
        assert!(emission["graph"]["edges"].is_array());
        if !expected_definition.is_empty() {
            let definition = &emission["inputs"][0]["statements"][0];
            let serialized = definition.to_string();
            assert!(
                serialized.contains(expected_definition),
                "{tag} should use {expected_definition}: {serialized}"
            );
        }
        if tag == "incremental" {
            assert!(!emission.to_string().contains("\"append\""));
            assert!(!emission.to_string().contains("\"conditional_mutation\""));
        }
        if let Some(relation) = relation {
            let selected = select_targets(&bundle, &[relation.to_string()]).unwrap();
            assert_eq!(selected.producer_evidence(), bundle.producer_evidence());
        }
    }
}

#[test]
fn relationless_ephemeral_dependency_is_accepted_only_when_downstream_is_compiled() {
    let mut value = manifest("ephemeral", None);
    value["nodes"]["model.task174.consumer"] = json!({
        "unique_id": "model.task174.consumer",
        "resource_type": "model",
        "name": "consumer",
        "config": {"materialized": "view"},
        "relation_name": "consumer",
        "language": "sql",
        "raw_code": "SELECT k FROM {{ ref('filtered') }}",
        "compiled_code": "WITH __dbt__cte__filtered AS (SELECT k FROM raw.orders WHERE k >= 1) SELECT k FROM __dbt__cte__filtered",
        "depends_on": {"nodes": ["model.task174.filtered"]}
    });
    let bundle = analyze(&value);
    assert_eq!(bundle.producer_evidence().len(), 2);
    let consumer = bundle
        .producer_evidence()
        .iter()
        .find(|e| e.input_id() == "model.task174.consumer")
        .unwrap();
    assert_eq!(
        consumer.inline_dependencies(),
        InlineDependencyEvidence::Unattested
    );
    assert_eq!(consumer.execution(), ProducerExecution::Unattested);
    let json = to_bundle_json(&bundle);
    assert!(!json.contains("create_view__dbt__cte__filtered"));
    assert!(json.contains("raw.orders"));

    value["nodes"]["model.task174.consumer"]["compiled_code"] = Value::Null;
    let error = parse_dbt_manifest(&value.to_string()).unwrap_err();
    assert!(matches!(
        error,
        DbtManifestError::UnsupportedDependency { .. }
    ));
}

#[test]
fn unsupported_materializations_and_missing_relations_fail_closed() {
    let mut unknown = manifest("vendor_custom", Some("filtered"));
    assert!(matches!(
        parse_dbt_manifest(&unknown.to_string()),
        Err(DbtManifestError::UnsupportedModel { .. })
    ));

    unknown = manifest("table", None);
    assert!(matches!(
        parse_dbt_manifest(&unknown.to_string()),
        Err(DbtManifestError::UnsupportedModel { .. })
    ));

    unknown = manifest("ephemeral", Some("fake_relation"));
    assert!(matches!(
        parse_dbt_manifest(&unknown.to_string()),
        Err(DbtManifestError::UnsupportedModel { .. })
    ));
}

#[test]
fn task_174_frozen_dialect_and_adapter_matrix_is_complete_and_parser_safe() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-174-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(freeze["variants"].as_array().unwrap().len(), 7);
    assert_eq!(
        freeze["parser_equivalence_cells"].as_array().unwrap().len(),
        65
    );
    assert_eq!(freeze["adapter_cells"].as_array().unwrap().len(), 20);

    let declared = analyze(&manifest("view", Some("filtered")));
    let catalog = RelationCatalog::from_schemas(declared.source_schemas()).unwrap();
    let mut checked = BTreeSet::new();
    for dialect_name in DIALECTS {
        let dialect = dialect_from_name(dialect_name).expect("public dialect");
        let input = SqlInput::inline(FILTER);
        let configured = [ConfiguredSqlInput::new(
            "task174-sql",
            &input,
            dialect_name,
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
        assert_positive_domain(&canonical_filter_domain(&bundle));
        checked.insert((*dialect_name).to_string());
    }
    assert_eq!(checked.len(), 13);

    for case in freeze["variants"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert!(id.starts_with("87-C174-P-") || id.starts_with("87-C174-N-"));
        assert_eq!(case["oracle"]["source"], json!([1, 1, null, 0]));
        assert_eq!(case["oracle"]["terminal"], json!([1, 1]));
    }
}

#[test]
fn complete_source_oracle_proves_positive_false_unknown_and_duplicate_rows() {
    let dialect = dialect_from_name("duckdb").unwrap();
    let ddl = analyze_inputs(
        &[SqlInput::inline("CREATE TABLE raw.orders(k INTEGER)")],
        "duckdb",
        dialect.as_ref(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(ddl.source_schemas()).unwrap();
    let input = SqlInput::inline(FILTER);
    let configured = [ConfiguredSqlInput::new(
        "filter",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    let direct = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    assert_positive_domain(&canonical_filter_domain(&direct));
    let source = source_bag();
    let proof = match physical_ordered_lifecycle_plan(&direct, std::slice::from_ref(&source)) {
        PhysicalOrderedLifecycleProof::Exact(exact) => exact,
        other => panic!("complete bag must prove exact filtering, got {other:?}"),
    };
    assert_eq!(proof.final_states, vec![source]);
    let [read] = proof.reads.as_slice() else {
        panic!("one terminal read expected");
    };
    assert_eq!(read.output.rows(), &[row(Some(1)), row(Some(1))]);
    assert_eq!(read.surviving_source_ordinals, vec![0, 1]);
    assert_eq!(read.rejected_source_ordinals, vec![2, 3]);

    let connection = duckdb::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE SCHEMA raw;
         CREATE TABLE raw.orders(k INTEGER);
         INSERT INTO raw.orders VALUES (1), (1), (NULL), (0);
         CREATE VIEW filtered_view AS SELECT k FROM raw.orders WHERE k >= 1;
         CREATE TABLE filtered_table AS SELECT k FROM raw.orders WHERE k >= 1;",
        )
        .unwrap();
    let values = |sql: &str| {
        let mut statement = connection.prepare(sql).unwrap();
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
    for sql in [
        "SELECT k FROM filtered_view ORDER BY k",
        "SELECT k FROM filtered_table ORDER BY k",
        FILTER,
        "WITH inline_model AS (SELECT k FROM raw.orders WHERE k >= 1) SELECT k FROM inline_model",
    ] {
        assert_eq!(values(sql), vec![Some(1), Some(1)], "{sql}");
    }
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE NOT (k >= 1)"),
        vec![Some(0)]
    );
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE (k >= 1) IS NULL"),
        vec![None]
    );
}
