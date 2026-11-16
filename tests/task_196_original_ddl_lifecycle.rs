//! TASK-196 canonical physical lifecycle proofs against original DuckDB DDL.
//!
//! Original SQL and complete native bags remain frozen separately. This test
//! uses public lifecycle APIs, preserving schema, NULL and duplicate identity.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn freeze() -> Value {
    serde_json::from_str(FREEZE).expect("immutable 38 original SQL cases")
}

fn original<'a>(freeze: &'a Value, suffix: &str) -> &'a Value {
    let id = format!("88-C177-{suffix}");
    freeze["cases"]
        .as_array()
        .expect("original task cases")
        .iter()
        .find(|case| case["id"] == id)
        .expect("original SQL variant")
}

fn schema(name: &str, columns: &[&str]) -> RelationSchema {
    RelationSchema::new(
        name,
        columns
            .iter()
            .map(|column| {
                SchemaColumn::from_sql_type(*column, "INTEGER", "duckdb")
                    .expect("independently typed native integer column")
            })
            .collect(),
    )
    .expect("complete named schema")
}

fn state(relation: &str, columns: &[&str], rows: &[Vec<Option<i64>>]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|row| {
                assert_eq!(row.len(), columns.len());
                PhysicalWriteRow::new(
                    columns
                        .iter()
                        .zip(row)
                        .map(|(name, value)| {
                            (
                                (*name).to_string(),
                                value.map_or(LiteralValue::Null, |number| {
                                    LiteralValue::Number(number.to_string())
                                }),
                            )
                        })
                        .collect::<BTreeMap<_, _>>(),
                )
                .expect("closed physical occurrence")
            })
            .collect(),
    )
    .expect("named state")
}

fn literal_state(relation: &str, columns: &[&str], rows: &Value) -> PhysicalWriteState {
    let parsed = rows
        .as_array()
        .expect("pinned terminal or source bag")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("tuple")
                .iter()
                .map(Value::as_i64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    state(relation, columns, &parsed)
}

fn analyze(sql: &str, schemas: &[RelationSchema]) -> sql_semantic_protocol::AnalysisBundle {
    let catalog = RelationCatalog::from_schemas(schemas).expect("authoritative original source");
    let dialect = dialect_from_name("duckdb").expect("public dialect");
    let source = SqlInput::inline(sql);
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "task196-original-ddl",
            &source,
            "duckdb",
            dialect.as_ref(),
        )],
        &catalog,
    )
    .expect("original SQL public analysis")
}

#[test]
fn original_ctas_keeps_its_four_physical_source_occurrences_and_typed_null_domain() {
    let frozen = freeze();
    let case = original(&frozen, "S212");
    let native = &case["native_oracle"];
    let source = literal_state("customers", &["id"], &native["before_bag"]);
    let bundle = analyze(
        case["sql"].as_str().expect("original SQL"),
        &[schema("customers", &["id"]), schema("orders", &["id"])],
    );
    let result = physical_ordered_write_state_plan(&bundle, std::slice::from_ref(&source));
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = result
    else {
        panic!("S212: full original CTAS should be exact: {result:?}");
    };
    assert_eq!(transitions.len(), 1);
    let created = &transitions[0];
    assert_eq!(created.kind(), WriteKind::Definition);
    assert_eq!(created.affected().inserted(), 4);
    assert_eq!(created.affected().deleted(), 0);
    assert_eq!(
        created.definition_source_row_ids(),
        [
            vec!["customers#0".to_owned()],
            vec!["customers#1".to_owned()],
            vec!["customers#2".to_owned()],
            vec!["customers#3".to_owned()],
        ],
    );
    let orders = final_states
        .iter()
        .find(|item| item.relation() == "orders")
        .expect("created physical target");
    assert_eq!(
        orders.rows(),
        literal_state("orders", &["id"], &native["terminal_bag"]).rows()
    );
    assert_eq!(
        final_states
            .iter()
            .find(|item| item.relation() == "customers"),
        Some(&source)
    );
    let schema = created.after_schema().expect("typed produced relation");
    let domains = orders.exact_domains(schema).expect("schema complete");
    assert_eq!(
        domains[0].values(),
        &[
            (LiteralValue::Number("1".to_owned()), 2),
            (LiteralValue::Number("2".to_owned()), 1),
            (LiteralValue::Null, 1),
        ]
    );
}

#[test]
fn original_alter_drop_and_truncate_keep_complete_schema_and_row_laws() {
    let frozen = freeze();
    for (suffix, kind) in [
        ("S216", WriteKind::SchemaAlter),
        ("S217", WriteKind::Lifecycle),
        ("S219", WriteKind::Lifecycle),
    ] {
        let case = original(&frozen, suffix);
        let native = &case["native_oracle"];
        let source = literal_state("orders", &["id"], &native["before_bag"]);
        let bundle = analyze(
            case["sql"].as_str().expect("original DDL"),
            &[schema("orders", &["id"])],
        );
        let result = physical_ordered_write_state_plan(&bundle, std::slice::from_ref(&source));
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = result
        else {
            panic!("{suffix}: closed source lifecycle must be exact: {result:?}");
        };
        assert_eq!(transitions.len(), 1, "{suffix}: one DDL transition");
        let step = &transitions[0];
        assert_eq!(step.kind(), kind, "{suffix}");
        assert_eq!(
            step.before().rows(),
            source.rows(),
            "{suffix}: complete prestate"
        );
        assert_eq!(step.affected().inserted(), 0, "{suffix}");
        assert_eq!(step.affected().updated(), 0, "{suffix}");
        match suffix {
            "S216" => {
                assert!(step.before_exists() && step.after_exists());
                assert_eq!(step.affected().deleted(), 0);
                assert_eq!(
                    step.after_schema()
                        .expect("after ALTER typed schema")
                        .columns()
                        .len(),
                    2
                );
                assert_eq!(
                    step.after().rows(),
                    literal_state(
                        "orders",
                        &["id", "amount"],
                        &serde_json::json!([[1, null], [1, null], [2, null], [null, null]])
                    )
                    .rows()
                );
                assert_eq!(final_states.len(), 1);
            }
            "S217" => {
                assert!(step.before_exists() && !step.after_exists());
                assert_eq!(step.affected().deleted(), 4);
                assert!(step.after_schema().is_none());
                assert!(final_states.is_empty());
            }
            "S219" => {
                assert!(step.before_exists() && step.after_exists());
                assert_eq!(step.affected().deleted(), 4);
                assert_eq!(step.after().rows().len(), 0);
                assert!(step.after_schema().is_some());
                assert_eq!(final_states.len(), 1);
                assert_eq!(
                    final_states[0]
                        .exact_domains(step.after_schema().expect("live empty schema"))
                        .expect("empty typed domains")[0]
                        .values(),
                    &[]
                );
            }
            _ => panic!("unreviewed original lifecycle case"),
        }
    }
}
