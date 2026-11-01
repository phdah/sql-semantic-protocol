//! TASK-155: canonical 13-dialect parser matrix and SQL/dbt/ODCS parity.
//! A parser success is never a native engine winner, conflict or FK proof.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, ProtocolStatement, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};
use sqlparser::parser::Parser;

const DIALECTS: [&str; 13] = [
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
const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";
const PARENT: &str = "warehouse.ref.parent";

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-155-acceptance-freeze.json")).unwrap()
}

fn classify(dialect: &str, sql: &str, kind: &str) -> &'static str {
    let parser = dialect_from_name(dialect).unwrap();
    if Parser::parse_sql(parser.as_ref(), sql).is_err() {
        return "parser_boundary";
    }
    let parsed = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
        .unwrap_or_else(|err| panic!("{dialect}: parse succeeded, analysis failed: {err}"));
    match parsed.inputs()[0].statements().first().unwrap() {
        ProtocolStatement::Unsupported(_) => "canonical_unsupported",
        ProtocolStatement::Query(q) => match q.write() {
            Some(write)
                if kind == "merge"
                    && write.kind() == WriteKind::ConditionalMutation
                    && write.match_condition().is_some()
                    && q.sources().len() == 1
                    && !write.merge_clauses().is_empty() =>
            {
                "parsed_canonical"
            }
            Some(write)
                if kind == "conflict-ignore"
                    && write.kind() == WriteKind::Append
                    && write.insert_conflict().is_some() =>
            {
                "parsed_canonical"
            }
            _ => "canonical_unsupported",
        },
        _ => "canonical_unsupported",
    }
}

#[test]
fn all_thirteen_dialects_have_explicit_parser_and_upsert_law_boundaries() {
    let matrix = frozen();
    let mut actual = BTreeMap::new();
    for dialect in DIALECTS {
        let cases = matrix["dialect_syntax"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| {
                let id = case["id"].as_str().unwrap();
                let kind = if id.starts_with("merge") {
                    "merge"
                } else {
                    "conflict-ignore"
                };
                classify(dialect, case["sql"].as_str().unwrap(), kind)
            })
            .collect::<Vec<_>>();
        assert_eq!(cases.len(), 4);
        assert_eq!(
            cases[0], "parsed_canonical",
            "{dialect}: shared MATCHED MERGE"
        );
        assert_eq!(
            cases[1], "parsed_canonical",
            "{dialect}: shared INSERT MERGE"
        );
        assert_ne!(
            cases[3], "parsed_canonical",
            "{dialect}: DO UPDATE must fail closed"
        );
        actual.insert(dialect.to_string(), cases);
    }
    assert_eq!(matrix["dialect_cells_required"], 52);
    assert_eq!(actual.values().map(Vec::len).sum::<usize>(), 52);
    assert_eq!(matrix["dialects"], serde_json::json!(DIALECTS));
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        matrix["dialect_expected"],
        "TASK-155 parser matrix changed: every cell must have a fixed expected classification"
    );
}

fn target_schema() -> RelationSchema {
    RelationSchema::new(
        TARGET,
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()
}

fn source_schema() -> RelationSchema {
    RelationSchema::new(
        SOURCE,
        ["id", "amount"]
            .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
            .to_vec(),
    )
    .unwrap()
}

fn parent_schema() -> RelationSchema {
    RelationSchema::new(
        PARENT,
        vec![SchemaColumn::from_sql_type("id", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()
}

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[target_schema(), source_schema(), parent_schema()]).unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let warehouse = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let evidence =
        analyze_dbt_artifacts(&manifest, &warehouse, "postgresql", dialect.as_ref()).unwrap();
    let mut schemas = evidence.source_schemas().to_vec();
    schemas.push(parent_schema());
    RelationCatalog::from_schemas(&schemas).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let known = RelationCatalog::new(&[TARGET, SOURCE, PARENT]).unwrap();
    let metadata = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task155parity
name: Task 155 equivalent write schemas
version: 1.0.0
status: active
schema:
  - name: stg_orders
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: BIGINT
      - name: amount
        physicalType: INTEGER
  - name: orders
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: parent
    physicalName: warehouse.ref.parent
    properties:
      - name: id
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    RelationCatalog::from_schemas(metadata.source_schemas()).unwrap()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task155-parity")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn state(relation: &str, value: &Value) -> PhysicalWriteState {
    let columns = if relation == PARENT {
        &["id"][..]
    } else {
        &["id", "amount"][..]
    };
    let rows = value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            let values = columns
                .iter()
                .map(|column| {
                    let source_name = if *column == "amount" {
                        "score"
                    } else {
                        *column
                    };
                    (
                        (*column).to_string(),
                        item[source_name]
                            .as_i64()
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            PhysicalWriteRow::new(values).unwrap()
        })
        .collect();
    PhysicalWriteState::new(relation, rows).unwrap()
}

fn plan(case: &Value, catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let text = case["sql"]
        .as_str()
        .unwrap()
        .replace("target", TARGET)
        .replace("source", SOURCE)
        .replace("score", "amount");
    let sql = SqlInput::inline(&text);
    let dialect = dialect_from_name("postgresql").unwrap();
    let input = ConfiguredSqlInput::new("task155-parity", &sql, "postgresql", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[input], catalog).unwrap();
    let mut constraints = Vec::new();
    let mut target = Vec::new();
    if case["target_pk"] == true {
        target.push(RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap());
    }
    if case["target_unique"] == true {
        target.push(RelationConstraint::unique_key(vec!["id".into()], evidence()).unwrap());
    }
    if case["score_fk"] == true {
        target.push(
            RelationConstraint::foreign_key(
                vec!["amount".into()],
                PARENT,
                vec!["id".into()],
                evidence(),
            )
            .unwrap(),
        );
    }
    if !target.is_empty() {
        constraints.push(RelationConstraintSet::new(TARGET, target).unwrap());
    }
    if case["source_pk"] == true {
        constraints.push(
            RelationConstraintSet::new(
                SOURCE,
                vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
            )
            .unwrap(),
        );
    }
    if case["parent_pk"] == true {
        constraints.push(
            RelationConstraintSet::new(
                PARENT,
                vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
            )
            .unwrap(),
        );
    }
    bundle.enrich_relation_constraints(&constraints);
    let mut states = vec![state(TARGET, &case["target_before"])];
    if text.starts_with("MERGE") {
        states.push(state(SOURCE, &case["source"]));
    }
    if case["parent"].is_array() {
        states.push(state(PARENT, &case["parent"]));
    }
    physical_ordered_write_state_plan(&bundle, &states)
}

#[test]
fn every_frozen_case_has_identical_direct_dbt_and_odcs_proof_strength() {
    let matrix = frozen();
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(matrix["adapter_cases_required"], 12);
    assert_eq!(matrix["adapter_paths_required"], 3);
    assert_eq!(cases.len(), 12);
    for case in cases {
        let direct = plan(case, &direct_catalog());
        let dbt = plan(case, &dbt_catalog());
        assert_eq!(direct, dbt, "{}: direct/dbt schema fact parity", case["id"]);
        #[cfg(feature = "odcs")]
        assert_eq!(
            direct,
            plan(case, &odcs_catalog()),
            "{}: direct/ODCS schema fact parity",
            case["id"]
        );
        match case["classification"].as_str().unwrap() {
            "Exact" => {
                let PhysicalWriteStatePlan::Feasible { transitions, .. } = direct else {
                    panic!("{}: exact result not preserved by adapters", case["id"]);
                };
                assert_eq!(transitions.len(), 1);
                assert_eq!(
                    transitions[0].after(),
                    &state(TARGET, &case["target_after"])
                );
            }
            "Impossible" => assert!(
                matches!(direct, PhysicalWriteStatePlan::Impossible { .. }),
                "{}: expected enforced contradiction, actual {direct:?}",
                case["id"]
            ),
            "Residual" => assert!(
                matches!(direct, PhysicalWriteStatePlan::Residual { .. }),
                "{}: expected explicit conditional boundary",
                case["id"]
            ),
            other => panic!("unreviewed adapter classification: {other}"),
        }
    }
}

fn replay_plan(replay: &Value, catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let statements = replay["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| {
            step["sql"]
                .as_str()
                .unwrap()
                .replace("target", TARGET)
                .replace("source", SOURCE)
                .replace("score", "amount")
        })
        .collect::<Vec<_>>();
    let inputs = statements.iter().map(SqlInput::inline).collect::<Vec<_>>();
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = inputs
        .iter()
        .enumerate()
        .map(|(i, sql)| {
            ConfiguredSqlInput::new(
                if i == 0 { "task155-replay0" } else { "task155-replay1" },
                sql,
                "postgresql",
                dialect.as_ref(),
            )
        })
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        TARGET,
        vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
    )
    .unwrap()]);
    physical_ordered_write_state_plan(
        &bundle,
        &[
            state(TARGET, &replay["target_initial"]),
            state(SOURCE, &replay["source"]),
        ],
    )
}

#[test]
fn ordered_conflict_then_merge_preserves_source_independent_adapter_evidence() {
    let f = frozen();
    let replay = &f["ordered_replay"];
    assert_eq!(f["version"], 4);
    assert_eq!(f["ordered_replay_cases_required"], 1);
    assert_eq!(f["adapter_replay_paths_required"], 3);
    let direct = replay_plan(replay, &direct_catalog());
    assert_eq!(direct, replay_plan(replay, &dbt_catalog()));
    #[cfg(feature = "odcs")]
    assert_eq!(direct, replay_plan(replay, &odcs_catalog()));
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = direct
    else {
        panic!("ordered source-independent parity must be Exact");
    };
    let steps = replay["steps"].as_array().unwrap();
    assert_eq!(transitions.len(), steps.len());
    for (transition, step) in transitions.iter().zip(steps) {
        assert_eq!(transition.before(), &state(TARGET, &step["before"]));
        assert_eq!(transition.after(), &state(TARGET, &step["after"]));
        assert_eq!(
            transition.affected().inserted(),
            step["affected"]["inserted"].as_u64().unwrap()
        );
        assert_eq!(
            transition.affected().updated(),
            step["affected"]["updated"].as_u64().unwrap()
        );
        assert_eq!(
            transition.affected().deleted(),
            step["affected"]["deleted"].as_u64().unwrap()
        );
        assert!(transition.intermediate_states().is_empty());
    }
    assert_eq!(transitions[0].after(), transitions[1].before());
    assert_eq!(
        final_states.iter().find(|item| item.relation() == TARGET),
        Some(&state(TARGET, &replay["terminal"]))
    );
    assert_eq!(
        final_states.iter().find(|item| item.relation() == SOURCE),
        Some(&state(SOURCE, &replay["source"]))
    );
}
