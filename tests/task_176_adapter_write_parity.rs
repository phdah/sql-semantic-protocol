//! TASK-176: compare adapter-independent typed outcomes and closed-world
//! physical writes without treating dbt materialization as execution evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts,
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_catalog,
    parse_dbt_manifest, physical_duckdb_transaction_plan, physical_ordered_lifecycle_plan,
    to_bundle_json, ComposedSemantics, ConfiguredSqlInput, LiteralValue,
    PhysicalOrderedLifecycleProof, PhysicalScriptCatalog, PhysicalTransactionScriptProof,
    PhysicalTransactionStep, PhysicalWriteRow, PhysicalWriteState, ProducerExecution,
    ProducerMaterialization, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
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

fn row(k: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([(
        "k".to_string(),
        k.map_or(LiteralValue::Null, |value| {
            LiteralValue::Number(value.to_string())
        }),
    )]))
    .unwrap()
}

fn write_row(id: i64, v: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            v.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
    ]))
    .unwrap()
}

fn dbt_read_manifest(kind: &str, relation: Option<&str>) -> Value {
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task176.filtered": {
                "unique_id": "model.task176.filtered", "resource_type": "model",
                "name": "filtered", "language": "sql", "config": {"materialized": kind},
                "relation_name": relation, "compiled_code": FILTER,
                "depends_on": {"nodes": ["source.task176.orders"]}
            }
        },
        "sources": {
            "source.task176.orders": {
                "unique_id": "source.task176.orders", "resource_type": "source",
                "name": "orders", "relation_name": "raw.orders",
                "columns": {"k": {"name": "k", "data_type": "INTEGER"}}
            }
        }
    })
}

fn analyze_read(
    sql: &str,
    dialect_name: &str,
    catalog: &RelationCatalog,
) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name(dialect_name).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task176-read",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

fn domain(bundle: &sql_semantic_protocol::AnalysisBundle) -> ValueDomain {
    let layer = bundle.layers().last().unwrap();
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("missing resolved projection");
    };
    semantics.output().columns()[0].domain().clone()
}

fn assert_lower_bound(domain: &ValueDomain) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected bounded typed output: {domain:?}");
    };
    let [interval] = ranges.ranges() else {
        panic!("expected exactly one interval: {domain:?}");
    };
    let lower = interval.lower().unwrap();
    assert_eq!(lower.value().value(), &LiteralValue::Number("1".into()));
    assert!(lower.inclusive());
    assert!(interval.upper().is_none());
}

fn read_proof(
    catalog: &RelationCatalog,
) -> (ValueDomain, Vec<PhysicalWriteRow>, Vec<usize>, Vec<usize>) {
    let analyzed = analyze_read(FILTER, "duckdb", catalog);
    let bounded = domain(&analyzed);
    assert_lower_bound(&bounded);
    let source = PhysicalWriteState::new(
        "raw.orders",
        vec![row(Some(1)), row(Some(1)), row(None), row(Some(0))],
    )
    .unwrap();
    let PhysicalOrderedLifecycleProof::Exact(exact) =
        physical_ordered_lifecycle_plan(&analyzed, std::slice::from_ref(&source))
    else {
        panic!("complete read source must produce Exact physical proof");
    };
    assert_eq!(exact.final_states, vec![source]);
    let [read] = exact.reads.as_slice() else {
        panic!("one terminal read required");
    };
    (
        bounded,
        read.output.rows().to_vec(),
        read.surviving_source_ordinals.clone(),
        read.rejected_source_ordinals.clone(),
    )
}

fn direct_read_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").unwrap();
    let ddl = analyze_inputs(
        &[SqlInput::inline("CREATE TABLE raw.orders(k INTEGER)")],
        "duckdb",
        dialect.as_ref(),
    )
    .unwrap();
    RelationCatalog::from_schemas(ddl.source_schemas()).unwrap()
}

fn dbt_read_catalog(
    kind: &str,
    relation: Option<&str>,
) -> (RelationCatalog, sql_semantic_protocol::AnalysisBundle) {
    let manifest = parse_dbt_manifest(&dbt_read_manifest(kind, relation).to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref()).unwrap();
    let catalog = RelationCatalog::from_schemas(bundle.source_schemas()).unwrap();
    (catalog, bundle)
}

#[cfg(feature = "odcs")]
fn odcs_read_catalog() -> RelationCatalog {
    let bundle = sql_semantic_protocol::parse_odcs_yaml(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task176\nname: TASK-176\nversion: 1.0.0\nstatus: active\nschema:\n  - name: orders\n    physicalName: raw.orders\n    properties:\n      - name: k\n        physicalType: INTEGER\n",
        "duckdb", &RelationCatalog::new(&["raw.orders"]).unwrap(),
    ).unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

#[test]
fn frozen_matrix_identifies_all_adapter_and_dialect_obligations() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-176-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["owner"], "TASK-176");
    assert_eq!(freeze["frozen_before_implementation"], true);
    assert_eq!(freeze["fixture_ids"], json!(["87-C176-P", "87-C176-N"]));
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 8);
    assert_eq!(
        freeze["parser_cells"].as_array().unwrap().len(),
        5 * DIALECTS.len()
    );
    assert_eq!(freeze["adapter_cells"].as_array().unwrap().len(), 15);
    assert_eq!(freeze["oracle"]["source"], json!([1, 1, null, 0]));
    assert_eq!(freeze["oracle"]["terminal"], json!([1, 1]));
    assert_eq!(freeze["oracle"]["true"], json!([0, 1]));
    assert_eq!(freeze["oracle"]["false"], json!([3]));
    assert_eq!(freeze["oracle"]["unknown"], json!([2]));

    let mut ids = BTreeSet::new();
    let read_catalog = direct_read_catalog();
    let write_catalog = direct_write_catalog();
    for cell in freeze["parser_cells"].as_array().unwrap() {
        let id = cell["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate frozen parser cell: {id}");
        let name = cell["dialect"].as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        let sql = SqlInput::inline(cell["sql"].as_str().unwrap());
        let configured = [ConfiguredSqlInput::new(id, &sql, name, parser.as_ref())];
        let is_write = cell["case_id"] == "87-C176-P-write";
        let actual = analyze_configured_inputs_with_catalog(
            &configured,
            if is_write {
                &write_catalog
            } else {
                &read_catalog
            },
        );
        if !is_write {
            assert_lower_bound(&domain(&actual.unwrap()));
        } else if name == "duckdb" {
            assert!(actual.is_ok(), "native DuckDB MERGE must parse");
        } else if let Err(error) = actual {
            assert!(
                !error.to_string().is_empty(),
                "{id} must expose its parser boundary"
            );
        }
    }
    assert_eq!(ids.len(), 65);
}

#[test]
fn table_view_ephemeral_and_incremental_share_exact_read_facts_not_execution_claims() {
    let direct = read_proof(&direct_read_catalog());
    assert_eq!(direct.1, vec![row(Some(1)), row(Some(1))]);
    assert_eq!(direct.2, vec![0, 1]);
    assert_eq!(direct.3, vec![2, 3]);

    let cases = [
        (
            "table",
            Some("filtered"),
            ProducerMaterialization::PhysicalTable,
        ),
        (
            "view",
            Some("filtered"),
            ProducerMaterialization::LogicalView,
        ),
        ("ephemeral", None, ProducerMaterialization::Inline),
        (
            "incremental",
            Some("filtered"),
            ProducerMaterialization::Incremental,
        ),
    ];
    for (kind, relation, expected) in cases {
        let (catalog, bundle) = dbt_read_catalog(kind, relation);
        assert_eq!(
            read_proof(&catalog),
            direct,
            "{kind}: adapter changed read facts"
        );
        assert_lower_bound(&domain(&bundle));
        let [producer] = bundle.producer_evidence() else {
            panic!("{kind}: one producer expected");
        };
        assert_eq!(producer.input_id(), "model.task176.filtered");
        assert_eq!(producer.relation(), relation);
        assert_eq!(producer.materialization(), expected);
        assert_eq!(producer.execution(), ProducerExecution::Unattested);
        let emitted: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
        assert_eq!(emitted["producer_evidence"][0]["execution"], "unattested");
        assert_eq!(emitted["producer_evidence"][0]["relation"], json!(relation));
        assert!(emitted["graph"]["edges"].is_array());
        match kind {
            "table" => assert!(emitted["inputs"][0]["statements"][0]
                .to_string()
                .contains("create_table")),
            "view" => assert!(emitted["inputs"][0]["statements"][0]
                .to_string()
                .contains("create_view")),
            "incremental" => {
                assert!(!emitted.to_string().contains("\"conditional_mutation\""));
                assert!(!emitted.to_string().contains("\"append\""));
            }
            "ephemeral" => assert!(producer.relation().is_none()),
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn direct_dbt_odcs_prove_identical_complete_read_bag_and_bounds() {
    let direct = read_proof(&direct_read_catalog());
    let (dbt, _) = dbt_read_catalog("view", Some("filtered"));
    assert_eq!(read_proof(&dbt), direct);
    assert_eq!(read_proof(&odcs_read_catalog()), direct);

    let connection = duckdb::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE SCHEMA raw; CREATE TABLE raw.orders(k INTEGER);
         INSERT INTO raw.orders VALUES (1), (1), (NULL), (0);",
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
    assert_eq!(
        values(&format!("{FILTER} ORDER BY k")),
        vec![Some(1), Some(1)]
    );
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE NOT(k >= 1)"),
        vec![Some(0)]
    );
    assert_eq!(
        values("SELECT k FROM raw.orders WHERE (k >= 1) IS NULL"),
        vec![None]
    );
}

#[test]
fn native_duckdb_materialized_and_inline_outputs_match_the_complete_read_bag() {
    let connection = duckdb::Connection::open_in_memory().unwrap();
    connection.execute_batch(
        "CREATE SCHEMA raw; CREATE TABLE raw.orders(k INTEGER);
         INSERT INTO raw.orders VALUES (1), (1), (NULL), (0);
         CREATE TABLE task176_table AS SELECT k FROM raw.orders WHERE k >= 1;
         CREATE VIEW task176_view AS SELECT k FROM raw.orders WHERE k >= 1;",
    ).unwrap();
    let values = |sql: &str| {
        let mut statement = connection.prepare(sql).unwrap();
        statement.query_map([], |row| row.get::<_, Option<i32>>(0))
            .unwrap().collect::<Result<Vec<_>, _>>().unwrap()
    };
    for sql in [
        "SELECT k FROM task176_table ORDER BY k",
        "SELECT k FROM task176_view ORDER BY k",
        "WITH inline_model AS (SELECT k FROM raw.orders WHERE k >= 1) SELECT k FROM inline_model ORDER BY k",
        "SELECT k FROM raw.orders WHERE k >= 1 ORDER BY k",
    ] {
        assert_eq!(values(sql), vec![Some(1), Some(1)], "{sql}");
    }
}

#[cfg(feature = "odcs")]
#[test]
fn canonical_source_constraints_match_without_inventing_adapter_enforcement() {
    use sql_semantic_protocol::{
        ConstraintEnforcement, ConstraintSourceKind, RelationConstraint, RelationConstraintSet,
    };

    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "target",
        vec![
            SchemaColumn::from_sql_type("id", "INTEGER", "duckdb").unwrap(),
            SchemaColumn::from_sql_type("score", "INTEGER", "duckdb").unwrap(),
        ],
    ).unwrap()]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let ddl = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER NOT NULL CHECK(score IN (7,9)))",
        )],
        "duckdb", dialect.as_ref(),
    ).unwrap();
    let odcs = sql_semantic_protocol::parse_odcs_yaml(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task176-constraints\nname: TASK-176 constraints\nversion: 1.0.0\nstatus: active\nschema:\n  - name: target\n    properties:\n      - name: id\n        physicalType: INTEGER\n        primaryKey: true\n      - name: score\n        physicalType: INTEGER\n        required: true\n        enum: [7, 9]\n",
        "duckdb", &catalog,
    ).unwrap();
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["sources"]["source.demo.orders"]["relation_name"] = json!("target");
    manifest["sources"]["source.demo.orders"]["columns"] = json!({
        "id": {"name":"id", "data_type":"INTEGER"},
        "score": {"name":"score", "data_type":"INTEGER"}
    });
    manifest["sources"]["source.demo.orders"]["constraints"] = json!([
        {"type":"primary_key", "columns":["id"]},
        {"type":"not_null", "columns":["score"]},
        {"type":"check", "expression":"score IN (7,9)"}
    ]);
    let dbt = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let find = |sets: &[RelationConstraintSet]| {
        sets.iter().find(|set| set.relation() == "target").unwrap()
    };
    let sql_set = find(ddl.relation_constraints());
    let odcs_set = find(odcs.relation_constraints());
    let dbt_set = find(dbt.relation_constraints());
    let facts = |set: &RelationConstraintSet| {
        set.constraints().iter().map(|constraint| match constraint {
            RelationConstraint::PrimaryKey(key) => format!("pk:{}", key.columns().join(",")),
            RelationConstraint::NotNull(rule) => format!("required:{}", rule.column()),
            RelationConstraint::AcceptedValues(rule) => {
                format!("accepted:{}:{:?}", rule.column(), rule.values())
            }
            other => format!("unexpected:{other:?}"),
        }).collect::<BTreeSet<_>>()
    };
    let canonical = facts(sql_set);
    assert_eq!(canonical.len(), 3, "all typed constraints should remain canonical");
    assert_eq!(facts(dbt_set), canonical);
    assert_eq!(facts(odcs_set), canonical);
    for (set, source) in [
        (dbt_set, ConstraintSourceKind::DbtConstraint),
        (odcs_set, ConstraintSourceKind::ExternalMetadata),
    ] {
        for constraint in set.constraints() {
            assert!(constraint.evidence().iter().any(|evidence| {
                evidence.enforcement() == ConstraintEnforcement::Unknown
                    && evidence.provenance().source_kind() == source
            }));
        }
    }
}

fn direct_write_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["t", "s"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["id", "v"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn dbt_write_catalog() -> RelationCatalog {
    let sources = ["t", "s"]
        .into_iter()
        .map(|relation| {
            let id = format!("source.task176.{relation}");
            (
                id.clone(),
                json!({
                    "unique_id": id, "resource_type": "source", "name": relation,
                    "relation_name": relation,
                    "columns": {
                        "id": {"name": "id", "data_type": "INTEGER"},
                        "v": {"name": "v", "data_type": "INTEGER"}
                    }
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let catalog_sources = ["t", "s"]
        .into_iter()
        .map(|relation| {
            let id = format!("source.task176.{relation}");
            (
                id.clone(),
                json!({
                    "unique_id": id, "metadata": {
                        "type": "BASE TABLE", "database": "main", "schema": "main", "name": relation
                    },
                    "columns": {
                        "id": {"name": "id", "type": "INTEGER", "index": 1},
                        "v": {"name": "v", "type": "INTEGER", "index": 2}
                    },
                    "stats": {}
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task176.probe": {
                "unique_id": "model.task176.probe", "resource_type": "model",
                "name": "probe", "relation_name": "task176_probe",
                "original_file_path": "models/probe.sql", "language": "sql",
                "config": {"materialized": "table"},
                "compiled_code": "SELECT t.id, t.v FROM t JOIN s ON t.id = s.id",
                "depends_on": {
                    "nodes": ["source.task176.t", "source.task176.s"]
                }
            }
        },
        "sources": sources
    });
    let catalog = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": {}, "sources": catalog_sources, "errors": null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    let [producer] = bundle.producer_evidence() else {
        panic!("one dbt write-source model expected");
    };
    assert_eq!(producer.execution(), ProducerExecution::Unattested);
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_write_catalog() -> RelationCatalog {
    let body = "apiVersion: v3.2.0\nkind: DataContract\nid: task176-write\nname: TASK-176 source parity\nversion: 1.0.0\nstatus: active\nschema:\n  - name: t\n    properties:\n      - name: id\n        physicalType: INTEGER\n      - name: v\n        physicalType: INTEGER\n  - name: s\n    properties:\n      - name: id\n        physicalType: INTEGER\n      - name: v\n        physicalType: INTEGER\n";
    let bundle = sql_semantic_protocol::parse_odcs_yaml(
        body,
        "duckdb",
        &RelationCatalog::new(&["t", "s"]).unwrap(),
    )
    .unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

fn write_proof(
    catalog: &RelationCatalog,
) -> (Vec<PhysicalWriteRow>, Vec<PhysicalWriteRow>, usize, String) {
    let dialect = dialect_from_name("duckdb").unwrap();
    let statements = [
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET v = s.v",
        "DELETE FROM t WHERE id = 2",
        "SELECT id, v FROM t WHERE v >= 20",
    ];
    let sql = statements
        .iter()
        .map(|statement| SqlInput::inline(*statement))
        .collect::<Vec<_>>();
    let ids = ["task176-merge", "task176-delete", "task176-read"];
    let bundles = sql
        .iter()
        .zip(ids)
        .map(|(input, id)| {
            analyze_configured_inputs_with_catalog(
                &[ConfiguredSqlInput::new(
                    id,
                    input,
                    "duckdb",
                    dialect.as_ref(),
                )],
                catalog,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let steps = bundles
        .iter()
        .map(PhysicalTransactionStep::Sql)
        .collect::<Vec<_>>();
    let schemas = catalog
        .schemas()
        .iter()
        .filter(|schema| matches!(schema.relation(), "t" | "s"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(schemas.len(), 2);
    let states = schemas
        .iter()
        .map(|schema| {
            let rows = match schema.relation() {
                "t" => vec![
                    write_row(1, Some(10)),
                    write_row(2, None),
                    write_row(3, Some(99)),
                    write_row(4, Some(10)),
                    write_row(5, None),
                ],
                "s" => vec![write_row(1, Some(30))],
                other => panic!("unexpected source {other}"),
            };
            PhysicalWriteState::new(schema.relation(), rows).unwrap()
        })
        .collect();
    let proof =
        physical_duckdb_transaction_plan(&steps, &PhysicalScriptCatalog { states, schemas });
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("same complete typed sources should prove Exact write: {proof:?}");
    };
    assert_eq!(exact.checkpoints.len(), 3);
    assert_eq!(
        exact.checkpoints[0]
            .statement
            .as_ref()
            .unwrap()
            .write
            .as_ref()
            .unwrap()
            .affected()
            .updated(),
        1,
    );
    assert_eq!(
        exact.checkpoints[1]
            .statement
            .as_ref()
            .unwrap()
            .write
            .as_ref()
            .unwrap()
            .affected()
            .deleted(),
        1,
    );
    let read = exact.checkpoints[2]
        .statement
        .as_ref()
        .unwrap()
        .read
        .as_ref()
        .unwrap();
    assert_eq!(
        read.output.rows(),
        &[write_row(1, Some(30)), write_row(3, Some(99))]
    );
    let final_t = exact
        .final_catalog
        .states
        .iter()
        .find(|state| state.relation() == "t")
        .unwrap();
    let domains = exact.final_catalog.exact_domains().unwrap();
    assert!(domains
        .iter()
        .find(|(name, _)| name == "t")
        .unwrap()
        .1
        .iter()
        .all(|domain| !domain.values().is_empty()));
    (
        final_t.rows().to_vec(),
        read.output.rows().to_vec(),
        read.rejected_source_ordinals.len(),
        format!("{domains:?}"),
    )
}

#[test]
fn compiled_dbt_and_direct_sql_preserve_exact_merges_deletes_and_rejections() {
    let direct = write_proof(&direct_write_catalog());
    assert_eq!(write_proof(&dbt_write_catalog()), direct);
    assert_eq!(
        direct.0,
        vec![
            write_row(1, Some(30)),
            write_row(3, Some(99)),
            write_row(4, Some(10)),
            write_row(5, None),
        ]
    );
    assert_eq!(
        direct.1,
        vec![write_row(1, Some(30)), write_row(3, Some(99))]
    );
    assert_eq!(
        direct.2, 2,
        "false and unknown are separately rejected, not dropped"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_and_dbt_catalogs_preserve_the_same_write_and_terminal_proof() {
    let direct = write_proof(&direct_write_catalog());
    assert_eq!(write_proof(&odcs_write_catalog()), direct);
    assert_eq!(write_proof(&dbt_write_catalog()), direct);

    let connection = duckdb::Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE t(id INTEGER, v INTEGER); CREATE TABLE s(id INTEGER, v INTEGER);
         INSERT INTO t VALUES (1,10),(2,NULL),(3,99),(4,10),(5,NULL);
         INSERT INTO s VALUES (1,30);
         MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET v = s.v;
         DELETE FROM t WHERE id = 2;",
        )
        .unwrap();
    let pairs = |statement: &str| {
        let mut query = connection.prepare(statement).unwrap();
        query
            .query_map([], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, Option<i32>>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(
        pairs("SELECT id,v FROM t ORDER BY id"),
        vec![(1, Some(30)), (3, Some(99)), (4, Some(10)), (5, None)],
    );
    assert_eq!(
        pairs("SELECT id,v FROM t WHERE v>=20 ORDER BY id"),
        vec![(1, Some(30)), (3, Some(99))],
    );
    assert_eq!(
        pairs("SELECT id,v FROM t WHERE NOT(v>=20) ORDER BY id"),
        vec![(4, Some(10))],
    );
    assert_eq!(
        pairs("SELECT id,v FROM t WHERE (v>=20) IS NULL ORDER BY id"),
        vec![(5, None)],
    );
}
