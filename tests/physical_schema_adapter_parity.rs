#![cfg(feature = "odcs")]
//! Adapter parity means equal canonical facts, not fabricated enforcement.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
    physical_ordered_write_state_plan, ColumnValueKind, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintSourceKind, ConstraintValue, DataType, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

const ODCS: &str = r#"
apiVersion: v3.2.0
kind: DataContract
id: schema-parity
name: Schema Parity
version: 1.0.0
status: active
schema:
  - name: target
    properties:
      - name: id
        physicalType: INTEGER
        logicalType: integer
      - name: score
        physicalType: INTEGER
        logicalType: integer
        default: 7
"#;

fn schemas() -> Vec<RelationSchema> {
    ["target", "source"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("id", "INTEGER", "duckdb").unwrap(),
                    SchemaColumn::from_sql_type("score", "INTEGER", "duckdb").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect()
}

fn analyzed(sql: &str, catalog: &RelationCatalog) -> sql_semantic_protocol::AnalysisBundle {
    let input_sql = SqlInput::inline(sql);
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = ConfiguredSqlInput::new("source-parity", &input_sql, "duckdb", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[input], catalog).unwrap()
}

fn row(id: i64, score: i64) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".into(), LiteralValue::Number(id.to_string())),
        ("score".into(), LiteralValue::Number(score.to_string())),
    ]))
    .unwrap()
}

#[test]
fn ddl_odcs_named_schemas_share_typed_default_facts_but_not_execution_authority() {
    let catalog = RelationCatalog::from_schemas(&schemas()).unwrap();
    let ddl = analyzed(
        "CREATE TABLE target(id INTEGER,score INTEGER DEFAULT 7)",
        &catalog,
    );
    let odcs = parse_odcs_yaml(ODCS, "duckdb", &catalog).unwrap();
    let from_sql = ddl
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "target")
        .unwrap()
        .column_value_sources()
        .iter()
        .find(|source| source.column() == "score")
        .unwrap();
    let from_odcs = odcs
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "target")
        .unwrap()
        .column_value_sources()
        .iter()
        .find(|source| source.column() == "score")
        .unwrap();
    assert_eq!(from_sql.kind(), ColumnValueKind::Default);
    assert_eq!(from_odcs.kind(), from_sql.kind());
    assert_eq!(from_sql.value(), Some(&ConstraintValue::Integer(7)));
    assert_eq!(from_odcs.value(), from_sql.value());
    assert!(from_odcs
        .evidence()
        .iter()
        .all(|evidence| { evidence.enforcement() == ConstraintEnforcement::Unknown }));
    assert!(odcs.source_schemas()[0]
        .columns()
        .iter()
        .all(|column| { column.data_type() == &DataType::SignedInteger { bits: Some(32) } }));

    let statement = "INSERT INTO target (id) SELECT id FROM source";
    let mut with_ddl = analyzed(statement, &catalog);
    with_ddl.enrich_relation_constraints(ddl.relation_constraints());
    let mut with_odcs = analyzed(statement, &catalog);
    with_odcs.enrich_relation_constraints(odcs.relation_constraints());
    let initial = [
        PhysicalWriteState::new("target", vec![]).unwrap(),
        PhysicalWriteState::new("source", vec![row(1, 9)]).unwrap(),
    ];
    let sql_plan = physical_ordered_write_state_plan(&with_ddl, &initial);
    assert!(matches!(sql_plan, PhysicalWriteStatePlan::Feasible { .. }));
    assert!(matches!(
        physical_ordered_write_state_plan(&with_odcs, &initial),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}

const CONSTRAINT_ODCS: &str = r#"
apiVersion: v3.2.0
kind: DataContract
id: constraint-parity
name: Constraint Parity
version: 1.0.0
status: active
schema:
  - name: target
    properties:
      - name: id
        physicalType: INTEGER
        logicalType: integer
        primaryKey: true
      - name: score
        physicalType: INTEGER
        logicalType: integer
        required: true
        enum: [7, 9]
"#;

fn semantic_constraints(set: &RelationConstraintSet) -> BTreeSet<String> {
    set.constraints()
        .iter()
        .map(|constraint| match constraint {
            RelationConstraint::PrimaryKey(key) => format!("pk:{}", key.columns().join(",")),
            RelationConstraint::UniqueKey(key) => format!("unique:{}", key.columns().join(",")),
            RelationConstraint::NotNull(rule) => format!("required:{}", rule.column()),
            RelationConstraint::AcceptedValues(rule) => {
                format!("accepted:{}:{:?}", rule.column(), rule.values())
            }
            RelationConstraint::ForeignKey(key) => format!(
                "fk:{}:{}:{}",
                key.columns().join(","),
                key.referenced_relation(),
                key.referenced_columns().join(","),
            ),
            RelationConstraint::CheckComparison(rule) => {
                format!(
                    "check:{}:{:?}:{:?}",
                    rule.column(),
                    rule.operator(),
                    rule.value()
                )
            }
        })
        .collect()
}

#[test]
fn sql_dbt_odcs_and_named_schema_constraints_preserve_canonical_meaning() {
    let catalog = RelationCatalog::from_schemas(&schemas()).unwrap();
    let ddl = analyzed(
        "CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER NOT NULL CHECK(score IN (7,9)))",
        &catalog,
    );
    let odcs = parse_odcs_yaml(CONSTRAINT_ODCS, "duckdb", &catalog).unwrap();

    // dbt manifest owns declarations; catalog.json owns introspected types.
    // Neither artifact licenses inferring that a dbt constraint is enforced.
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

    let sql_set = ddl
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "target")
        .unwrap();
    let odcs_set = odcs
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "target")
        .unwrap();
    let dbt_set = dbt
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "target")
        .unwrap();
    let expected = semantic_constraints(sql_set);
    assert_eq!(semantic_constraints(odcs_set), expected);
    assert_eq!(semantic_constraints(dbt_set), expected);
    assert_eq!(expected.len(), 3);
    for (set, provenance) in [
        (odcs_set, ConstraintSourceKind::ExternalMetadata),
        (dbt_set, ConstraintSourceKind::DbtConstraint),
    ] {
        for constraint in set.constraints() {
            assert!(constraint.evidence().iter().any(|proof| {
                proof.enforcement() == ConstraintEnforcement::Unknown
                    && proof.provenance().source_kind() == provenance
            }));
        }
    }

    // Named schemas carry type facts, not invented PK/CHECK evidence.
    assert_eq!(
        catalog.schemas()[0].columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    assert!(analyzed("SELECT id FROM target", &catalog)
        .relation_constraints()
        .iter()
        .all(|set| set.constraints().is_empty()));
}
