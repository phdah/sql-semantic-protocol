//! TASK-175: qualified source names, FK-only schemas, explicit missing-target boundaries,
//! and exact, independently executed source-to-terminal filter oracles.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, physical_ordered_lifecycle_plan, to_bundle_json, ComposedSemantics,
    ConfiguredSqlInput, ConstraintEnforcement, DbtArtifactsError, LiteralValue,
    PhysicalOrderedLifecycleProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationConstraint, RelationSchema, SchemaColumn, SchemaSourceKind, SqlInput, ValueDomain,
};

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

const FILTER: &str = "SELECT k FROM raw.orders WHERE k >= 1";
const ORDERS: &str = "raw.orders";
const PARENTS: &str = "raw.parents";

fn manifest_json() -> Value {
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "adapter_type": "duckdb",
            "dbt_version": "1.11.8"
        },
        "nodes": {
            "model.task175.filtered": {
                "unique_id": "model.task175.filtered",
                "resource_type": "model",
                "name": "filtered",
                "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "analytics.filtered",
                "compiled_code": FILTER,
                "depends_on": {"nodes": ["source.task175.raw.orders"]}
            }
        },
        "sources": {
            "source.task175.raw.orders": {
                "unique_id": "source.task175.raw.orders",
                "resource_type": "source",
                "relation_name": ORDERS,
                "columns": {"k": {"name": "k", "data_type": "INTEGER"}}
            },
            "source.task175.raw.parents": {
                "unique_id": "source.task175.raw.parents",
                "resource_type": "source",
                "relation_name": PARENTS,
                "columns": {"id": {"name": "id", "data_type": "INTEGER"}}
            }
        }
    })
}

fn relationship_test(to: &str) -> Value {
    json!({
        "unique_id": "test.task175.orders_parents",
        "resource_type": "test",
        "relation_name": null,
        "attached_node": "source.task175.raw.orders",
        "column_name": "k",
        "test_metadata": {
            "name": "relationships",
            "namespace": null,
            "kwargs": {
                "model": "{{ get_where_subquery(source('raw', 'orders')) }}",
                "column_name": "k",
                "arguments": {"to": to, "field": "id"}
            }
        },
        "depends_on": {
            "nodes": ["source.task175.raw.orders", "source.task175.raw.parents"]
        }
    })
}

fn constraints<'a>(
    manifest: &'a sql_semantic_protocol::DbtManifest,
) -> &'a sql_semantic_protocol::RelationConstraintSet {
    manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == ORDERS)
        .expect("source constraint metadata")
}

fn parsed(value: &Value) -> sql_semantic_protocol::DbtManifest {
    parse_dbt_manifest(&value.to_string()).expect("manifest must parse")
}

fn assert_lower_inclusive_one(domain: &ValueDomain) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected a typed range, got {domain:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one bounded range, got {domain:?}");
    };
    let lower = range.lower().expect("inclusive lower bound");
    assert!(lower.inclusive());
    assert_eq!(lower.value().value(), &LiteralValue::Number("1".into()));
}

fn physical_row(k: Option<i64>) -> PhysicalWriteRow {
    let value = k.map_or(LiteralValue::Null, |k| LiteralValue::Number(k.to_string()));
    PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), value)])).unwrap()
}

fn filtered_domain(catalog: &RelationCatalog, dialect_name: &str, sql: &str) -> ValueDomain {
    let dialect = dialect_from_name(dialect_name).unwrap();
    let sql = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task175",
        &sql,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
        panic!("expected resolved SQL semantics for {dialect_name}");
    };
    composed.output().columns()[0].domain().clone()
}

#[test]
fn task_175_freeze_is_versioned_and_complete() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-175-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["owner"], "TASK-175");
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["frozen_before_semantic_changes"], true);
    assert_eq!(freeze["fixture_ids"], json!(["87-C175-P", "87-C175-N"]));
    assert_eq!(freeze["oracle"]["source"], json!([1, 1, null, 0]));
    assert_eq!(freeze["oracle"]["terminal"], json!([1, 1]));
    assert_eq!(freeze["oracle"]["positive"], json!([0, 1]));
    assert_eq!(freeze["oracle"]["rejected_false"], json!([3]));
    assert_eq!(freeze["oracle"]["rejected_unknown"], json!([2]));
    assert_eq!(freeze["oracle"]["domain"]["lower_inclusive"], true);
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        assert!(matches!(
            case["fixture"].as_str(),
            Some("87-C175-P" | "87-C175-N")
        ));
        assert_eq!(case["oracle"]["source"], freeze["oracle"]["source"]);
        assert_eq!(case["oracle"]["terminal"], freeze["oracle"]["terminal"]);
        assert!(case["id"].as_str().is_some());
        assert!(case["adapters"].as_array().is_some());
        assert!(case["proof"].as_str().is_some());
    }
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), 13);
}

#[test]
fn unknown_relationship_to_does_not_become_fk_from_dependency_alone() {
    let mut value = manifest_json();
    value["nodes"]["test.task175.orders_parents"] = relationship_test("source('raw', 'missing')");
    let parsed = parsed(&value);
    let set = constraints(&parsed);
    assert!(set
        .constraints()
        .iter()
        .all(|constraint| !matches!(constraint, RelationConstraint::ForeignKey(_))));
    assert!(set
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unresolved_relationship_target"));
}

#[test]
fn declared_fk_with_unknown_target_is_diagnostic_without_losing_other_constraints() {
    let mut value = manifest_json();
    value["sources"]["source.task175.raw.orders"]["constraints"] = json!([
        {"type": "not_null", "columns": ["k"]},
        {"type": "foreign_key", "columns": ["k"], "to": "source('raw', 'missing')", "to_columns": ["id"]}
    ]);
    let parsed = parsed(&value);
    let set = constraints(&parsed);
    assert!(set
        .constraints()
        .iter()
        .any(|constraint| matches!(constraint, RelationConstraint::NotNull(_))));
    assert!(set
        .constraints()
        .iter()
        .all(|constraint| !matches!(constraint, RelationConstraint::ForeignKey(_))));
    assert!(set
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unresolved_foreign_key_target"));
}

#[test]
fn resolved_relationship_keeps_unknown_enforcement_and_fk_only_target_schema() {
    let mut value = manifest_json();
    value["nodes"]["test.task175.orders_parents"] = relationship_test("source('raw', 'parents')");
    let manifest = parsed(&value);
    let set = constraints(&manifest);
    let foreign_key = set
        .constraints()
        .iter()
        .find_map(|rule| match rule {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("resolved relationships test must contribute an FK");
    assert_eq!(foreign_key.referenced_relation(), PARENTS);
    assert_eq!(
        foreign_key.evidence()[0].enforcement(),
        ConstraintEnforcement::Unknown
    );

    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()).unwrap();
    let parent = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == PARENTS)
        .expect("FK-only relation must receive a typed source schema");
    assert_eq!(parent.source_kind(), Some(SchemaSourceKind::DbtManifest));
    assert_eq!(parent.columns()[0].name(), "id");
    assert!(to_bundle_json(&bundle).contains(PARENTS));
}

#[test]
fn missing_fk_only_warehouse_types_are_explicitly_rejected() {
    let mut value = manifest_json();
    value["nodes"]["test.task175.orders_parents"] = relationship_test("source('raw', 'parents')");
    value["sources"]["source.task175.raw.parents"]["columns"] = json!({});
    let manifest = parsed(&value);
    let dialect = dialect_from_name("duckdb").unwrap();
    assert!(matches!(
        analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()),
        Err(DbtArtifactsError::MissingConstraintSchema { relation, columns })
            if relation == PARENTS && columns == ["id"]
    ));
}

#[test]
fn quoted_schema_qualified_names_do_not_collapse_onto_unquoted_aliases() {
    let quoted = "\"Raw\".\"Orders\"";
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            quoted,
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap(),
        RelationSchema::new(
            "raw.orders",
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap(),
    ])
    .unwrap();
    assert_eq!(catalog.resolve(quoted, "postgresql", None).unwrap(), quoted);
    assert_eq!(
        catalog.resolve("raw.orders", "postgresql", None).unwrap(),
        "raw.orders"
    );
    assert_lower_inclusive_one(&filtered_domain(
        &catalog,
        "postgresql",
        "SELECT k FROM \"Raw\".\"Orders\" WHERE k >= 1",
    ));
    assert_lower_inclusive_one(&filtered_domain(
        &catalog,
        "postgresql",
        "SELECT k FROM raw.orders WHERE k >= 1",
    ));
    assert!(catalog.resolve("orders", "postgresql", None).is_ok());
}

#[test]
fn all_exposed_dialects_preserve_portable_typed_filter_bound() {
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        ORDERS,
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap()])
    .unwrap();
    for dialect in DIALECTS {
        assert_lower_inclusive_one(&filtered_domain(&catalog, dialect, FILTER));
    }
}

#[test]
fn full_duckdb_source_and_terminal_bags_match_canonical_positive_and_not_true() {
    let conn = duckdb::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE SCHEMA raw; CREATE TABLE raw.orders(k INTEGER); INSERT INTO raw.orders VALUES (1),(1),(NULL),(0);").unwrap();
    let values = |sql: &str| {
        let mut stmt = conn.prepare(sql).unwrap();
        stmt.query_map([], |row| row.get::<_, Option<i32>>(0))
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

    let manifest = parsed(&manifest_json());
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()).unwrap();
    let catalog = RelationCatalog::from_schemas(bundle.source_schemas()).unwrap();
    let sql = SqlInput::inline(FILTER);
    let inputs = [ConfiguredSqlInput::new(
        "task175-physical",
        &sql,
        "duckdb",
        dialect.as_ref(),
    )];
    let read = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let source = PhysicalWriteState::new(
        ORDERS,
        vec![
            physical_row(Some(1)),
            physical_row(Some(1)),
            physical_row(None),
            physical_row(Some(0)),
        ],
    )
    .unwrap();
    let PhysicalOrderedLifecycleProof::Exact(exact) =
        physical_ordered_lifecycle_plan(&read, std::slice::from_ref(&source))
    else {
        panic!("complete typed source must prove exact filter output");
    };
    let [observation] = exact.reads.as_slice() else {
        panic!("one exact read");
    };
    assert_eq!(
        observation.output.rows(),
        &[physical_row(Some(1)), physical_row(Some(1))]
    );
    assert_eq!(observation.surviving_source_ordinals, vec![0, 1]);
    assert_eq!(observation.rejected_source_ordinals, vec![2, 3]);
    assert_lower_inclusive_one(&filtered_domain(&catalog, "duckdb", FILTER));
}

#[cfg(feature = "odcs")]
#[test]
fn direct_dbt_and_odcs_metadata_produce_one_canonical_typed_filter_domain() {
    use sql_semantic_protocol::{parse_odcs_yaml, DataType};

    let manifest = parsed(&manifest_json());
    let dialect = dialect_from_name("duckdb").unwrap();
    let dbt = analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()).unwrap();
    let dbt_catalog = RelationCatalog::from_schemas(dbt.source_schemas()).unwrap();

    let odcs = parse_odcs_yaml(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task175\nname: TASK-175\nversion: 1.0.0\nstatus: active\nschema:\n  - name: orders\n    physicalName: raw.orders\n    properties:\n      - name: k\n        physicalType: INTEGER\n",
        "duckdb",
        &RelationCatalog::new(&[ORDERS]).unwrap()
    ).unwrap();
    assert_eq!(
        odcs.source_schemas()[0].source_kind(),
        Some(SchemaSourceKind::ExternalMetadata)
    );
    assert_eq!(
        odcs.source_schemas()[0].columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();

    let ddl = sql_semantic_protocol::analyze_inputs(
        &[SqlInput::inline("CREATE TABLE raw.orders (k INTEGER)")],
        "duckdb",
        dialect.as_ref(),
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(ddl.source_schemas()).unwrap();
    let expected = filtered_domain(&direct_catalog, "duckdb", FILTER);
    assert_eq!(filtered_domain(&dbt_catalog, "duckdb", FILTER), expected);
    assert_eq!(filtered_domain(&odcs_catalog, "duckdb", FILTER), expected);
}
