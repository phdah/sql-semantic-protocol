//! TASK-196 typed scalar IN for original correlated-source UPDATE and DELETE.
//!
//! Every public dialect parser uses the same canonical statement when both
//! membership keys have proven signed-integer types. Missing or floating
//! metadata must fail closed rather than guess membership or NULL behavior.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name,
    physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, ProtocolStatement, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn schema(table: &str, data_type: &str) -> RelationSchema {
    RelationSchema::new(
        table,
        ["id", "amount"]
            .into_iter()
            .map(|column| {
                SchemaColumn::from_sql_type(
                    column,
                    if column == "id" { data_type } else { "INTEGER" },
                    "duckdb",
                )
                .expect("independent typed source")
            })
            .collect(),
    )
    .expect("canonical relation schema")
}

fn catalog(data_type: &str) -> RelationCatalog {
    RelationCatalog::from_schemas(&[schema("orders", data_type), schema("customers", data_type)])
        .expect("two independent complete source schemas")
}

fn state(relation: &str, rows: &[(Option<i64>, Option<i64>)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|(id, amount)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".to_owned(),
                        id.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                    ),
                    (
                        "amount".to_owned(),
                        amount.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                    ),
                ]))
                .expect("physical NULL or integer")
            })
            .collect(),
    )
    .expect("named source")
}

fn sql<'a>(freeze: &'a Value, suffix: &str) -> &'a str {
    freeze["cases"]
        .as_array()
        .expect("original SQL")
        .iter()
        .find(|case| case["id"] == format!("88-C177-{suffix}"))
        .expect("frozen subquery mutation")["sql"]
        .as_str()
        .expect("literal original SQL")
}

#[test]
fn both_original_scalar_in_mutations_have_exact_13_parser_dialect_physical_outcomes() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("task matrix");
    let initial = [
        state(
            "orders",
            &[
                (Some(1), Some(10)),
                (Some(2), Some(20)),
                (Some(2), Some(20)),
                (Some(3), None),
                (None, Some(7)),
            ],
        ),
        state(
            "customers",
            &[
                (Some(1), Some(100)),
                (Some(4), Some(40)),
                (Some(4), Some(40)),
            ],
        ),
    ];
    let mut cells = 0;
    for (suffix, expected) in [
        (
            "S203",
            state(
                "orders",
                &[
                    (Some(1), Some(1)),
                    (Some(2), Some(20)),
                    (Some(2), Some(20)),
                    (Some(3), None),
                    (None, Some(7)),
                ],
            ),
        ),
        (
            "S206",
            state(
                "orders",
                &[
                    (Some(2), Some(20)),
                    (Some(2), Some(20)),
                    (Some(3), None),
                    (None, Some(7)),
                ],
            ),
        ),
    ] {
        let original_sql = sql(&frozen, suffix);
        for dialect in frozen["dialects"].as_array().expect("13 public dialects") {
            let name = dialect.as_str().expect("named dialect");
            let parser = dialect_from_name(name).expect("public parser");
            let input = SqlInput::inline(original_sql);
            let configured = [ConfiguredSqlInput::new(
                "task196-original-in",
                &input,
                name,
                parser.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog("INTEGER"))
                .unwrap_or_else(|err| panic!("{suffix}/{name}: {err}"));
            assert!(
                matches!(
                    bundle.inputs()[0].statements()[0],
                    ProtocolStatement::Query(_)
                ),
                "{suffix}/{name}: typed original IN must not vanish or be unsupported"
            );
            let result = physical_ordered_write_state_plan(&bundle, &initial);
            let PhysicalWriteStatePlan::Feasible {
                transitions,
                final_states,
            } = result
            else {
                panic!("{suffix}/{name}: complete typed physical source must certify SQL IN: {result:?}");
            };
            let [transition] = transitions.as_slice() else {
                panic!("{suffix}/{name}: one original write");
            };
            assert_eq!(
                transition.affected_target_ordinals(),
                &[0],
                "{suffix}/{name}"
            );
            assert_eq!(
                transition.affected().updated() + transition.affected().deleted(),
                1,
                "{suffix}/{name}"
            );
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "orders"),
                Some(&expected)
            );
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "customers"),
                Some(&initial[1])
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 26);
}

#[test]
fn absent_metadata_and_float_in_subquery_mutations_never_become_exact() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let initial = [
        state("orders", &[(Some(1), Some(10)), (None, Some(7))]),
        state("customers", &[(Some(1), Some(100)), (None, Some(40))]),
    ];
    for suffix in ["S203", "S206"] {
        let original = sql(&frozen, suffix);
        let parser = dialect_from_name("duckdb").expect("source dialect");
        let untyped = analyze_inputs(&[SqlInput::inline(original)], "duckdb", parser.as_ref())
            .expect("parsed but explicitly unsupported core without typed source");
        assert!(
            matches!(
                physical_ordered_write_state_plan(&untyped, &initial),
                PhysicalWriteStatePlan::Residual { .. }
            ),
            "{suffix}: missing types cannot infer a successful mutation"
        );
        let input = SqlInput::inline(original);
        let configured = [ConfiguredSqlInput::new(
            "task196-float-in",
            &input,
            "duckdb",
            parser.as_ref(),
        )];
        let float = analyze_configured_inputs_with_catalog(&configured, &catalog("DOUBLE"))
            .expect("typed floating point parse");
        assert!(
            matches!(
                physical_ordered_write_state_plan(&float, &initial),
                PhysicalWriteStatePlan::Residual { .. }
            ),
            "{suffix}: float membership needs engine-specific comparison laws"
        );
    }
}

#[test]
fn unproved_nested_membership_shapes_fail_closed_without_mutating_source_rows() {
    let original_state = [
        state("orders", &[(Some(1), Some(10)), (None, Some(7))]),
        state("customers", &[(Some(1), Some(100)), (None, Some(40))]),
    ];
    let parser = dialect_from_name("duckdb").expect("named parser");
    for sql in [
        "UPDATE orders SET amount = 1 WHERE id IN (SELECT id + 1 FROM customers)",
        "UPDATE orders SET amount = 1 WHERE id IN (SELECT id FROM customers WHERE amount > 0)",
        "DELETE FROM orders WHERE id NOT IN (SELECT id FROM customers)",
        "DELETE FROM orders WHERE id IN (SELECT orders.id FROM customers)",
        "DELETE FROM orders WHERE id IN (SELECT id FROM customers WHERE customers.id = orders.id)",
    ] {
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "task196-negative-in",
            &input,
            "duckdb",
            parser.as_ref(),
        )];
        let analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog("INTEGER"))
            .unwrap_or_else(|error| panic!("negative SQL must remain analyzable: {error}"));
        assert!(
            matches!(
                physical_ordered_write_state_plan(&analyzed, &original_state),
                PhysicalWriteStatePlan::Residual { .. }
            ),
            "{sql}: computed, filtered, correlated or negated IN is not a proved exact write"
        );
    }
}
