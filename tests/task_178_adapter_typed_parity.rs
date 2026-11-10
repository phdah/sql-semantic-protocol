//! TASK-178: compare attested typed read facts across direct SQL, compiled
//! dbt, and ODCS metadata without asserting that dbt or vendors executed SQL.
//!
//! This test targets the public configured-input and composed-domain seam.

use std::collections::BTreeMap;

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, physical_ordered_lifecycle_plan, ComposedSemantics,
    ConfiguredSqlInput, LiteralValue, PhysicalOrderedLifecycleProof, PhysicalWriteRow,
    PhysicalWriteState, ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const SQL: &str = "SELECT amount FROM orders WHERE amount >= 10";
const DIALECTS: &[&str] = &[
    "ansi",
    "bigquery",
    "clickhouse",
    "databricks",
    "duckdb",
    "generic",
    "hive",
    "mssql",
    "mysql",
    "postgresql",
    "redshift",
    "snowflake",
    "sqlite",
];

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id INTEGER, amount INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

fn dbt_catalog() -> (RelationCatalog, sql_semantic_protocol::AnalysisBundle) {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task178.filtered": {
                "unique_id": "model.task178.filtered",
                "resource_type": "model", "name": "filtered",
                "language": "sql", "config": {"materialized": "view"},
                "relation_name": "filtered", "compiled_code": SQL,
                "depends_on": {"nodes": ["source.task178.orders"]}
            }
        },
        "sources": {
            "source.task178.orders": {
                "unique_id": "source.task178.orders",
                "resource_type": "source", "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            }
        }
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref()).unwrap();
    let catalog = RelationCatalog::from_schemas(bundle.source_schemas()).unwrap();
    (catalog, bundle)
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let bundle = sql_semantic_protocol::parse_odcs_yaml(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task178\nname: TASK-178\nversion: 1.0.0\nstatus: active\nschema:\n  - name: orders\n    physicalName: orders\n    properties:\n      - name: id\n        physicalType: INTEGER\n      - name: amount\n        physicalType: INTEGER\n",
        "duckdb",
        &RelationCatalog::new(&["orders"]).unwrap(),
    )
    .unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

fn analyze(catalog: &RelationCatalog, name: &str) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name(name).unwrap();
    let sql = SqlInput::inline(SQL);
    let configured = [ConfiguredSqlInput::new(
        "task178",
        &sql,
        name,
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

fn typed_domain(bundle: &sql_semantic_protocol::AnalysisBundle) -> ValueDomain {
    let layer = bundle.layers().last().expect("typed terminal query");
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("compiled SQL must have resolved source and output semantics");
    };
    semantics.output().columns()[0].domain().clone()
}

fn assert_inclusive_ten(value: &ValueDomain) {
    let ValueDomain::Ranges(ranges) = value else {
        panic!("typed output domain unexpectedly missing: {value:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("one interval expected: {value:?}");
    };
    let lower = range.lower().expect("inclusive 10");
    assert_eq!(
        lower.value().value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(lower.inclusive());
    assert!(range.upper().is_none());
}

fn source_row(id: Option<i64>, amount: Option<i64>) -> PhysicalWriteRow {
    let literal = |value: Option<i64>| {
        value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()))
    };
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), literal(id)),
        ("amount".to_string(), literal(amount)),
    ]))
    .unwrap()
}

#[test]
fn direct_sql_and_dbt_carry_identical_attested_typed_bounds_in_all_13_dialects() {
    let direct = direct_catalog();
    let (dbt, bundle) = dbt_catalog();
    let [producer] = bundle.producer_evidence() else {
        panic!("one compiled dbt model");
    };
    assert_eq!(producer.execution(), ProducerExecution::Unattested);

    for name in DIALECTS {
        let lhs = typed_domain(&analyze(&direct, name));
        let rhs = typed_domain(&analyze(&dbt, name));
        assert_inclusive_ten(&lhs);
        assert_eq!(lhs, rhs, "{name}: compiled dbt source typed facts drifted");
    }
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_metadata_is_equivalent_to_direct_and_dbt_only_for_the_same_typed_source_facts() {
    let direct = direct_catalog();
    let (dbt, _) = dbt_catalog();
    let odcs = odcs_catalog();
    for name in DIALECTS {
        let expected = typed_domain(&analyze(&direct, name));
        assert_inclusive_ten(&expected);
        assert_eq!(typed_domain(&analyze(&dbt, name)), expected);
        assert_eq!(typed_domain(&analyze(&odcs, name)), expected);
    }
}

#[test]
fn real_duckdb_three_valued_predicate_matches_closed_source_and_terminal_bags() {
    let db = duckdb::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE orders (id INTEGER, amount INTEGER); INSERT INTO orders VALUES (1,10),(1,10),(2,5),(NULL,NULL);")
        .expect("source with duplicates and UNKNOWN");
    let query = |sql: &str| {
        let mut statement = db.prepare(sql).unwrap();
        let iter = statement
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap();
        let mut values: Vec<_> = iter.map(|r| r.unwrap()).collect();
        values.sort();
        values
    };
    assert_eq!(
        query("SELECT amount FROM orders"),
        vec![None, Some(5), Some(10), Some(10)]
    );
    assert_eq!(query(SQL), vec![Some(10), Some(10)]);
    assert_eq!(
        query("SELECT amount FROM orders WHERE (amount >= 10) IS FALSE"),
        vec![Some(5)]
    );
    assert_eq!(
        query("SELECT amount FROM orders WHERE (amount >= 10) IS UNKNOWN"),
        vec![None]
    );

    let bundle = analyze(&direct_catalog(), "duckdb");
    let physical = PhysicalWriteState::new(
        "orders",
        vec![
            source_row(Some(1), Some(10)),
            source_row(Some(1), Some(10)),
            source_row(Some(2), Some(5)),
            source_row(None, None),
        ],
    )
    .unwrap();
    let PhysicalOrderedLifecycleProof::Exact(exact) =
        physical_ordered_lifecycle_plan(&bundle, std::slice::from_ref(&physical))
    else {
        panic!("same typed physical bag must admit Exact configured read proof");
    };
    assert_eq!(exact.final_states, vec![physical]);
    let [read] = exact.reads.as_slice() else {
        panic!("one complete terminal read");
    };
    assert_eq!(read.surviving_source_ordinals, vec![0, 1]);
    assert_eq!(read.rejected_source_ordinals, vec![2, 3]);
    assert_eq!(read.output.rows().len(), 2);
    assert_inclusive_ten(&typed_domain(&bundle));
}
