//! TASK-180: prove source-typed ALTER SET DEFAULT as an ordered schema law.
//! Existing physical occurrences are not rewritten; omitted-column values
//! use the new default only after the certified ALTER. Catalog-free statements
//! remain unsupported rather than inventing an untyped DDL effect.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name,
    physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn typed_catalog() -> RelationCatalog {
    let schema = RelationSchema::new(
        "orders",
        ["id", "amount"]
            .into_iter()
            .map(|name| {
                SchemaColumn::from_sql_type(name, "INTEGER", "duckdb")
                    .expect("independently typed column")
            })
            .collect(),
    )
    .expect("complete typed relation");
    RelationCatalog::from_schemas(&[schema]).expect("one independently attested catalog")
}

fn original_s220() -> String {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original 38 SQL identities");
    freeze["cases"]
        .as_array()
        .expect("frozen variants")
        .iter()
        .find(|case| case["id"] == "88-C177-S220")
        .expect("source SQL ALTER SET DEFAULT")["sql"]
        .as_str()
        .expect("original SQL")
        .to_owned()
}

fn integer_bag(db: &Connection, sql: &str) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("native complete bag");
    let rows = statement
        .query_map([], |row| {
            Ok(vec![
                row.get::<usize, Option<i64>>(0)?,
                row.get::<usize, Option<i64>>(1)?,
            ])
        })
        .expect("native result rows");
    let mut bag: Vec<_> = rows.map(|row| row.expect("native nullable row")).collect();
    bag.sort();
    bag
}

#[test]
fn scalar_set_default_changes_only_future_inserted_rows_in_all_thirteen_dialects() {
    let source = original_s220();
    let body = format!(
        "CREATE TABLE orders(id INTEGER, amount INTEGER); \
         INSERT INTO orders VALUES (1,10),(2,NULL),(2,NULL); \
         {source}; INSERT INTO orders(id) VALUES (3)"
    );
    let expected = vec![
        vec![Some(1), Some(10)],
        vec![Some(2), None],
        vec![Some(2), None],
        vec![Some(3), Some(0)],
    ];
    let original_before = vec![
        vec![Some(1), Some(10)],
        vec![Some(2), None],
        vec![Some(2), None],
    ];
    let db = Connection::open_in_memory().expect("native default law");
    db.execute_batch(&body)
        .expect("original DuckDB SET DEFAULT followed by INSERT");
    assert_eq!(
        integer_bag(&db, "SELECT id, amount FROM orders"),
        expected,
        "complete DuckDB bag includes both pre-ALTER NULL occurrences"
    );

    let freeze: Value = serde_json::from_str(FREEZE).expect("13 parsers");
    let mut covered = 0;
    for dialect in freeze["dialects"].as_array().expect("13 project dialects") {
        let name = dialect.as_str().expect("dialect name");
        let parser = dialect_from_name(name).expect("public SQL dialect");
        let input = SqlInput::inline(&body);
        let configured = [ConfiguredSqlInput::new(
            "task180-default-sequence",
            &input,
            name,
            parser.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &typed_catalog())
            .unwrap_or_else(|error| panic!("{name}: source-attested parse: {error}"));
        let statement = &bundle.inputs()[0].statements()[2];
        let ProtocolStatement::Query(alter) = statement else {
            panic!("{name}: typed original SET DEFAULT was not represented");
        };
        let write = alter.write().expect("typed original DDL write");
        let [sql_semantic_protocol::PhysicalColumnAlterStep::SetDefault { column, default }] =
            write.column_alter_steps()
        else {
            panic!("{name}: one typed SET DEFAULT step required");
        };
        assert_eq!(column, "amount");
        assert_eq!(default, &LiteralValue::Number("0".to_string()));

        let outcome = physical_ordered_write_state_plan(&bundle, &[]);
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = outcome
        else {
            panic!("{name}: complete in-script CREATE/INSERT/ALTER/INSERT must preserve defaults: {outcome:?}");
        };
        assert_eq!(transitions.len(), 4);
        let alter_transition = &transitions[2];
        assert_eq!(alter_transition.affected().inserted(), 0);
        assert_eq!(alter_transition.affected().deleted(), 0);
        assert_eq!(alter_transition.affected().updated(), 0);
        let before: Vec<Vec<Option<i64>>> = alter_transition
            .before()
            .rows()
            .iter()
            .map(|row| {
                ["id", "amount"]
                    .into_iter()
                    .map(
                        |name| match row.values().get(name).expect("complete prestate") {
                            LiteralValue::Null => None,
                            LiteralValue::Number(value) => {
                                Some(value.parse().expect("signed integer"))
                            }
                            unexpected => panic!("{name}: unexpected prestate {unexpected:?}"),
                        },
                    )
                    .collect()
            })
            .collect();
        let mut before = before;
        before.sort();
        assert_eq!(
            before, original_before,
            "{name}: ALTER cannot rewrite existing NULLs"
        );
        assert_eq!(
            alter_transition.after().rows(),
            alter_transition.before().rows(),
            "{name}: SET DEFAULT changes only metadata"
        );

        let result = final_states
            .iter()
            .find(|state| state.relation() == "orders")
            .expect("complete final source and target");
        let mut rows: Vec<Vec<Option<i64>>> = result
            .rows()
            .iter()
            .map(|row| {
                ["id", "amount"]
                    .into_iter()
                    .map(
                        |column| match row.values().get(column).expect("complete target") {
                            LiteralValue::Null => None,
                            LiteralValue::Number(value) => {
                                Some(value.parse().expect("signed integer"))
                            }
                            other => panic!("{name}: unsupported output literal {other:?}"),
                        },
                    )
                    .collect()
            })
            .collect();
        rows.sort();
        assert_eq!(rows, expected, "{name}: full target bag and future default");
        covered += 1;
    }
    assert_eq!(covered, 13);
}

#[test]
fn missing_authoritative_schema_and_unproved_default_expressions_fail_closed() {
    let original = original_s220();
    let parser = dialect_from_name("duckdb").expect("project DuckDB dialect");
    let raw = analyze_inputs(&[SqlInput::inline(&original)], "duckdb", parser.as_ref())
        .expect("raw original statement remains an explicit unsupported boundary");
    assert!(
        matches!(
            raw.inputs()[0].statements()[0],
            ProtocolStatement::Unsupported(_)
        ),
        "original untyped U history is immutable"
    );

    for sql in [
        "ALTER TABLE orders ALTER COLUMN missing SET DEFAULT 0",
        "ALTER TABLE orders ALTER COLUMN amount SET DEFAULT random()",
    ] {
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "task180-invalid-default",
            &input,
            "duckdb",
            parser.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &typed_catalog())
            .expect("unsupported shape must remain explicit");
        assert!(
            matches!(
                bundle.inputs()[0].statements()[0],
                ProtocolStatement::Unsupported(_)
            ),
            "{sql}: unproved column or expression cannot create an effect"
        );
    }

    let program = format!(
        "CREATE TABLE orders(id INTEGER, amount INTEGER); \
         INSERT INTO orders VALUES (1,10); \
         ALTER TABLE orders ALTER COLUMN amount SET DEFAULT 'not an integer'; \
         INSERT INTO orders(id) VALUES (2)"
    );
    let input = SqlInput::inline(&program);
    let configured = [ConfiguredSqlInput::new(
        "task180-incompatible-default",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &typed_catalog())
        .expect("typed invalid default remains a checked operation");
    assert!(
        !matches!(
            physical_ordered_write_state_plan(&bundle, &[]),
            PhysicalWriteStatePlan::Feasible { .. }
        ),
        "an invalid future-column default must not yield an exact write program"
    );
}
