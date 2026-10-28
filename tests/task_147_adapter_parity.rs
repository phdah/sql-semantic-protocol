//! TASK-147: equivalent SQL, dbt catalog and ODCS source evidence must
//! drive the same canonical conflict/RETURNING state transition.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan, ConfiguredSqlInput,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";
const SQL: &str = "INSERT INTO warehouse.analytics.stg_orders(id,amount)
    SELECT id,amount FROM warehouse.raw.orders WHERE TRUE
    ON CONFLICT(id) DO NOTHING RETURNING id";

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "amount"]
                .into_iter()
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    }))
    .unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let result =
        analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(result.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let known = RelationCatalog::new(&[TARGET, SOURCE]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task147parity
name: Equivalent source target evidence
version: 1.0.0
status: active
schema:
  - name: stg_orders
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: orders
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    RelationCatalog::from_schemas(contract.source_schemas()).unwrap()
}

fn row(id: i64, amount: i64) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".into(), LiteralValue::Number(id.to_string())),
        ("amount".into(), LiteralValue::Number(amount.to_string())),
    ]))
    .unwrap()
}

fn state(relation: &str, rows: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter().map(|(id, amount)| row(*id, *amount)).collect(),
    )
    .unwrap()
}

fn execute(catalog: &RelationCatalog, enforced_key: bool) -> PhysicalWriteStatePlan {
    let sql = SqlInput::inline(SQL);
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured =
        ConfiguredSqlInput::new("task-147-parity", &sql, "postgresql", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task147").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    if enforced_key {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            TARGET,
            vec![RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()],
        )
        .unwrap()]);
    }
    physical_ordered_write_state_plan(
        &bundle,
        &[
            state(SOURCE, &[(1, 20), (2, 30)]),
            state(TARGET, &[(1, 10)]),
        ],
    )
}

fn assert_full_bag(plan: &PhysicalWriteStatePlan) {
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("equivalent metadata must preserve exact physical proof: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    let transition = &transitions[0];
    assert_eq!(transition.before().rows(), state(TARGET, &[(1, 10)]).rows());
    assert_eq!(
        transition.after().rows(),
        state(TARGET, &[(1, 10), (2, 30)]).rows()
    );
    assert_eq!(transition.affected().inserted(), 1);
    assert_eq!(
        transition.returning().unwrap().rows()[0].values(),
        &BTreeMap::from([("id".into(), LiteralValue::Number("2".into()))])
    );
    assert_eq!(
        final_states
            .iter()
            .find(|state| state.relation() == SOURCE)
            .unwrap()
            .rows(),
        state(SOURCE, &[(1, 20), (2, 30)]).rows()
    );
}

#[test]
fn direct_and_compiled_dbt_catalog_evidence_produce_identical_results() {
    let direct = execute(&direct_catalog(), true);
    let dbt = execute(&dbt_catalog(), true);
    assert_full_bag(&direct);
    assert_eq!(direct, dbt);
}

#[cfg(feature = "odcs")]
#[test]
fn direct_and_odcs_catalog_evidence_produce_identical_results() {
    let direct = execute(&direct_catalog(), true);
    let odcs = execute(&odcs_catalog(), true);
    assert_full_bag(&direct);
    assert_eq!(direct, odcs);
}

#[test]
fn absent_enforced_key_is_equally_residual_for_direct_and_dbt_metadata() {
    let direct = execute(&direct_catalog(), false);
    let dbt = execute(&dbt_catalog(), false);
    assert!(matches!(direct, PhysicalWriteStatePlan::Residual { .. }), "{direct:?}");
    assert_eq!(direct, dbt);
}

#[cfg(feature = "odcs")]
#[test]
fn absent_enforced_key_is_equally_residual_for_direct_and_odcs_metadata() {
    let direct = execute(&direct_catalog(), false);
    let odcs = execute(&odcs_catalog(), false);
    assert!(matches!(direct, PhysicalWriteStatePlan::Residual { .. }), "{direct:?}");
    assert_eq!(direct, odcs);
}
