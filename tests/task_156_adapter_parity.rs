//! TASK-156: source-independent SQL, compiled dbt and ODCS schema fact parity.
//! All three inputs must produce the same ordered canonical proof and terminal bags.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan, ConfiguredSqlInput,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";
const STAGE: &str = "warehouse.analytics.stage";
const TERMINAL: &str = "warehouse.analytics.terminal";

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-156-acceptance-freeze.json")).unwrap()
}

fn additional_schemas() -> Vec<RelationSchema> {
    [STAGE, TERMINAL]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "amount"]
                    .into_iter()
                    .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "postgresql").unwrap())
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

fn direct_catalog() -> RelationCatalog {
    let mut schemas = vec![
        RelationSchema::new(
            TARGET,
            vec![
                SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
            ],
        )
        .unwrap(),
        RelationSchema::new(
            SOURCE,
            ["id", "amount"]
                .into_iter()
                .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap(),
    ];
    schemas.extend(additional_schemas());
    RelationCatalog::from_schemas(&schemas).unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let parser = dialect_from_name("postgresql").unwrap();
    let evidence =
        analyze_dbt_artifacts(&manifest, &catalog, "postgresql", parser.as_ref()).unwrap();
    let mut schemas = evidence.source_schemas().to_vec();
    // Stage and terminal are separately attested identities; dbt owns only
    // the two artifact-backed physical relations. Never fabricate dbt nodes.
    schemas.extend(additional_schemas());
    RelationCatalog::from_schemas(&schemas).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;

    let known = RelationCatalog::new(&[TARGET, SOURCE, STAGE, TERMINAL]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task156parity
name: Task 156 MERGE physical schema parity
version: 1.0.0
status: active
schema:
  - name: target
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: BIGINT
      - name: amount
        physicalType: INTEGER
  - name: source
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: stage
    physicalName: warehouse.analytics.stage
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: terminal
    physicalName: warehouse.analytics.terminal
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

fn physical(name: &str) -> &str {
    match name {
        "target" => TARGET,
        "source" => SOURCE,
        "stage" => STAGE,
        "terminal" => TERMINAL,
        other => panic!("not a declared physical relation: {other}"),
    }
}

fn row(value: &Value) -> PhysicalWriteRow {
    PhysicalWriteRow::new(
        ["id", "score"]
            .into_iter()
            .map(|column| {
                (
                    if column == "score" { "amount" } else { column }.to_owned(),
                    value[column]
                        .as_i64()
                        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}

fn state(relation: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        physical(relation),
        rows.as_array().unwrap().iter().map(row).collect(),
    )
    .unwrap()
}

fn normalized_sql(text: &str) -> String {
    text.replace("target", TARGET)
        .replace("source", SOURCE)
        .replace("stage", STAGE)
        .replace("terminal", TERMINAL)
        .replace("score", "amount")
}

fn plan(case: &Value, catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let statements = case["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| normalized_sql(step["sql"].as_str().unwrap()))
        .collect::<Vec<_>>();
    let sql = statements.iter().map(SqlInput::inline).collect::<Vec<_>>();
    let parser = dialect_from_name("postgresql").unwrap();
    let ids = (0..sql.len())
        .map(|i| format!("task156-parity-{i}"))
        .collect::<Vec<_>>();
    let configured = sql
        .iter()
        .zip(&ids)
        .map(|(query, id)| ConfiguredSqlInput::new(id, query, "postgresql", parser.as_ref()))
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    if case["target_pk"] == true {
        let enforcement = ConstraintEvidence::new(
            ConstraintProvenance::new(
                ConstraintSourceKind::ExternalMetadata,
                "task156-parity-enforced",
            )
            .unwrap(),
            ConstraintEnforcement::Enforced,
        );
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            TARGET,
            vec![
                RelationConstraint::primary_key(vec!["id".to_owned()], vec![enforcement]).unwrap(),
            ],
        )
        .unwrap()]);
    }
    let initial = case["initial"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(relation, rows)| state(relation, rows))
        .collect::<Vec<_>>();
    physical_ordered_write_state_plan(&bundle, &initial)
}

#[test]
fn all_frozen_computed_merge_cases_have_identical_direct_dbt_odcs_proofs() {
    let f = frozen();
    assert_eq!(f["evidence_sources"].as_array().unwrap().len(), 3);
    assert_eq!(f["adapter_cases_required"], 6);
    assert_eq!(f["adapter_paths_required"], 3);
    let direct = direct_catalog();
    let dbt = dbt_catalog();
    #[cfg(feature = "odcs")]
    let odcs = odcs_catalog();
    for case in f["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let actual = plan(case, &direct);
        assert_eq!(
            actual,
            plan(case, &dbt),
            "{id}: compiled dbt equivalent schema"
        );
        #[cfg(feature = "odcs")]
        assert_eq!(actual, plan(case, &odcs), "{id}: ODCS equivalent schema");

        match case["classification"].as_str().unwrap() {
            "Exact" => {
                let PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                } = actual
                else {
                    panic!("{id}: expected exact adapter parity, got {actual:?}");
                };
                assert_eq!(transitions.len(), case["steps"].as_array().unwrap().len());
                for (transition, step) in transitions.iter().zip(case["steps"].as_array().unwrap())
                {
                    assert_eq!(
                        transition.before(),
                        &state(step["target"].as_str().unwrap(), &step["before"])
                    );
                    assert_eq!(
                        transition.after(),
                        &state(step["target"].as_str().unwrap(), &step["after"])
                    );
                }
                for (relation, expected) in case["final"].as_object().unwrap() {
                    assert_eq!(
                        final_states
                            .iter()
                            .find(|s| s.relation() == physical(relation)),
                        Some(&state(relation, expected)),
                        "{id}: final complete bag"
                    );
                }
            }
            "Impossible" => assert!(
                matches!(actual, PhysicalWriteStatePlan::Impossible { .. }),
                "{id}: preserved enforced contradiction: {actual:?}"
            ),
            "Residual" => assert!(
                matches!(actual, PhysicalWriteStatePlan::Residual { .. }),
                "{id}: preserved engine-specific ambiguity: {actual:?}"
            ),
            other => panic!("{id}: unreviewed proof class {other}"),
        }
    }
}
