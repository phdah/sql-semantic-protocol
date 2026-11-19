//! TASK-180 independent final-gate replay of all original TASK-196 DuckDB
//! lifecycle and DML source/terminal bags. Native syntax failure and metadata
//! authority limitations are not promoted into vendor-equivalent semantics.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const WRITES: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn read() -> Value {
    serde_json::from_str(WRITES).expect("independent original SQL/native review freeze")
}

fn bag(db: &Connection, sql: &str, expected: &Value, id: &str) {
    let mut expected_rows: Vec<Vec<Option<i64>>> =
        serde_json::from_value(expected.clone()).expect("frozen full native integer/NULL bag");
    expected_rows.sort();
    let columns = expected_rows.first().map_or(1, Vec::len);
    assert!(expected_rows.iter().all(|row| row.len() == columns));
    let mut statement = db
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{id}: original source/terminal SQL: {error}"));
    let actual = statement
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|error| panic!("{id}: native source/terminal rows: {error}"));
    let mut actual_rows: Vec<_> = actual
        .map(|row| row.expect("native complete physical row"))
        .collect();
    actual_rows.sort();
    assert_eq!(
        actual_rows, expected_rows,
        "{id}: whole native bag and NULL multiplicity"
    );
}

#[test]
fn original_schema_and_write_state_bags_replay_without_vendor_overclaim() {
    let frozen = read();
    let mut seen = BTreeSet::new();
    let mut dml = 0;
    let mut schema = 0;
    let mut constraint_negatives = 0;
    let mut returning = 0;
    let mut failed_vendor = 0;
    let mut metadata_only = 0;
    for case in frozen["cases"]
        .as_array()
        .expect("all 38 original SQL cases")
    {
        let id = case["id"].as_str().expect("stable original ID");
        assert!(seen.insert(id), "{id}: duplicated original DDL/DML variant");
        let sql = case["sql"].as_str().expect("unaltered original SQL");
        let oracle = &case["native_oracle"];
        match oracle["kind"]
            .as_str()
            .expect("independent native category")
        {
            "original_native_dml_complete_target_bag" => {
                let db = Connection::open_in_memory().expect("native isolated DML catalog");
                db.execute_batch(oracle["source_setup"].as_str().expect("source setup"))
                    .unwrap_or_else(|error| panic!("{id}: source population: {error}"));
                bag(
                    &db,
                    "SELECT id, amount FROM orders",
                    &oracle["complete_source_bags"]["orders"],
                    id,
                );
                let customers = &oracle["complete_source_bags"]["customers"];
                if !customers.is_null() {
                    bag(&db, "SELECT id, amount FROM customers", customers, id);
                }
                if oracle["expected_returning_bag"].is_null() {
                    db.execute_batch(sql)
                        .unwrap_or_else(|error| panic!("{id}: original DML execution: {error}"));
                } else {
                    bag(&db, sql, &oracle["expected_returning_bag"], id);
                    returning += 1;
                }
                bag(
                    &db,
                    "SELECT id, amount FROM orders",
                    &oracle["expected_poststate_bag"],
                    id,
                );
                if !customers.is_null() {
                    bag(&db, "SELECT id, amount FROM customers", customers, id);
                }
                dml += 1;
            }
            "original_native_schema_lifecycle_source_and_terminal" => {
                let db = Connection::open_in_memory().expect("native isolated schema catalog");
                let setup = oracle["source_setup"]
                    .as_str()
                    .expect("schema source setup");
                if !setup.is_empty() {
                    db.execute_batch(setup)
                        .unwrap_or_else(|error| panic!("{id}: native setup: {error}"));
                }
                bag(
                    &db,
                    oracle["before_query"].as_str().expect("prestate SQL"),
                    &oracle["before_bag"],
                    id,
                );
                db.execute_batch(sql)
                    .unwrap_or_else(|error| panic!("{id}: original schema DDL: {error}"));
                let followup = oracle["followup_sql"].as_str().expect("post DDL input");
                if !followup.is_empty() {
                    db.execute_batch(followup)
                        .unwrap_or_else(|error| panic!("{id}: reviewed followup: {error}"));
                }
                let terminal = oracle["terminal_query"].as_str().expect("terminal SQL");
                bag(&db, terminal, &oracle["terminal_bag"], id);
                if let Some(invalid) = oracle["must_reject_after"].as_str() {
                    assert!(
                        db.execute_batch(invalid).is_err(),
                        "{id}: nonconstructive constraint/lifecycle state was accepted"
                    );
                    bag(&db, terminal, &oracle["terminal_bag"], id);
                    constraint_negatives += 1;
                }
                schema += 1;
            }
            "unapproved_vendor_or_unsupported_dml_boundary" => {
                assert!(matches!(id, "88-C177-S199" | "88-C177-S200"));
                assert!(oracle["expected_poststate_bag"].is_null());
                let db = Connection::open_in_memory().expect("native rejection fixture");
                db.execute_batch(
                    "CREATE TABLE orders(id INTEGER, amount INTEGER);
                     INSERT INTO orders VALUES (1,10),(2,NULL);
                     CREATE TABLE customers(id INTEGER, amount INTEGER);
                     INSERT INTO customers VALUES (1,100);",
                )
                .expect("complete source states");
                let expected_orders = serde_json::json!([[1, 10], [2, null]]);
                let expected_customers = serde_json::json!([[1, 100]]);
                bag(&db, "SELECT id, amount FROM orders", &expected_orders, id);
                bag(
                    &db,
                    "SELECT id, amount FROM customers",
                    &expected_customers,
                    id,
                );
                assert!(
                    db.execute_batch(sql).is_err(),
                    "{id}: original vendor syntax is not certified by DuckDB"
                );
                bag(&db, "SELECT id, amount FROM orders", &expected_orders, id);
                bag(
                    &db,
                    "SELECT id, amount FROM customers",
                    &expected_customers,
                    id,
                );
                failed_vendor += 1;
            }
            "metadata_adapter_only_requires_authoritative_fixture" => {
                assert!(matches!(id, "88-C177-S191" | "88-C177-S193"));
                assert!(oracle["expected_poststate_bag"].is_null());
                assert_eq!(oracle["boundary_owner"], "TASK-197");
                metadata_only += 1;
            }
            other => panic!("{id}: unreviewed physical SQL category {other}"),
        }
    }
    assert_eq!(seen.len(), 38);
    assert_eq!((dml, schema, failed_vendor, metadata_only), (16, 18, 2, 2));
    assert_eq!(returning, 3);
    assert!(constraint_negatives >= 5);
}

fn typed_original_source(relation: &str, literal_bag: &Value) -> PhysicalWriteState {
    let rows = literal_bag
        .as_array()
        .expect("complete native source identity bag");
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|row| {
                let source = row.as_array().expect("id,amount tuple");
                assert_eq!(source.len(), 2);
                let values = ["id", "amount"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| {
                        (
                            name.to_string(),
                            source[index].as_i64().map_or(LiteralValue::Null, |number| {
                                LiteralValue::Number(number.to_string())
                            }),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                PhysicalWriteRow::new(values).expect("complete source row")
            })
            .collect(),
    )
    .expect("named physical source occurrences")
}

fn native_target_rowids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut query = db.prepare(sql).expect("source-row identity query");
    let iter = query
        .query_map([], |row| row.get::<usize, i64>(0))
        .expect("full physical truth partition");
    let mut ordinals: Vec<_> = iter.map(|row| row.expect("one physical rowid")).collect();
    ordinals.sort();
    ordinals
}

#[test]
fn original_typed_update_and_delete_match_native_complete_bags_and_all_13_dialect_physical_writes()
{
    let freeze = read();
    let catalog = RelationCatalog::from_schemas(
        &["orders", "customers"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["id", "amount"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "duckdb")
                                .expect("authoritative SQL schema")
                        })
                        .collect(),
                )
                .expect("typed relation")
            })
            .collect::<Vec<_>>(),
    )
    .expect("complete independent source type authority");

    let mut checked = 0;
    for suffix in ["S203", "S206"] {
        let id = format!("88-C177-{suffix}");
        let frozen = freeze["cases"]
            .as_array()
            .expect("original 38 variants")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original untyped historical U case");
        let oracle = &frozen["native_oracle"];
        let native = Connection::open_in_memory().expect("native independent source");
        native
            .execute_batch(oracle["source_setup"].as_str().expect("closed source"))
            .expect("source bags with duplicated keys and NULL");
        let before = oracle["complete_source_bags"]["orders"].clone();
        let customers = oracle["complete_source_bags"]["customers"].clone();
        bag(&native, "SELECT id, amount FROM orders", &before, &id);
        bag(&native, "SELECT id, amount FROM customers", &customers, &id);

        let predicate = "id IN (SELECT id FROM customers)";
        for (truth, expected) in [
            ("IS TRUE", vec![0]),
            ("IS FALSE", vec![1, 2, 3]),
            ("IS UNKNOWN", vec![4]),
        ] {
            assert_eq!(
                native_target_rowids(
                    &native,
                    &format!("SELECT rowid FROM orders WHERE ({predicate}) {truth}")
                ),
                expected,
                "{id}: three-valued original target row identity"
            );
        }
        assert_eq!(
            native_target_rowids(
                &native,
                &format!("SELECT rowid FROM orders WHERE ({predicate}) IS NOT TRUE")
            ),
            vec![1, 2, 3, 4],
            "{id}: a NULL target key is rejected without losing duplicate false rows"
        );

        let sql = frozen["sql"].as_str().expect("original DML SQL");
        native
            .execute_batch(sql)
            .expect("original native DML outcome");
        bag(
            &native,
            "SELECT id, amount FROM orders",
            &oracle["expected_poststate_bag"],
            &id,
        );
        bag(&native, "SELECT id, amount FROM customers", &customers, &id);
        let expected = typed_original_source("orders", &oracle["expected_poststate_bag"]);
        let source_states = [
            typed_original_source("orders", &before),
            typed_original_source("customers", &customers),
        ];
        for dialect in freeze["dialects"].as_array().expect("13 exposed parsers") {
            let dialect = dialect.as_str().expect("named dialect");
            let parser = dialect_from_name(dialect).expect("public SQL dialect");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "task180-original-typed-DML",
                &input,
                dialect,
                parser.as_ref(),
            )];
            let parsed = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{id}/{dialect}: {error}"));
            let [ProtocolStatement::Query(query)] = parsed.inputs()[0].statements() else {
                panic!("{id}/{dialect}: typed original DML was silently dropped");
            };
            assert_eq!(
                query
                    .sources()
                    .iter()
                    .map(|source| source.name())
                    .collect::<Vec<_>>(),
                vec!["customers"],
                "{id}/{dialect}: source relation identity must not be dropped"
            );
            let plan = physical_ordered_write_state_plan(&parsed, &source_states);
            let PhysicalWriteStatePlan::Feasible {
                transitions,
                final_states,
            } = plan
            else {
                panic!("{id}/{dialect}: expected a source-complete typed DML plan: {plan:?}");
            };
            assert_eq!(transitions.len(), 1);
            assert_eq!(
                transitions[0].affected_target_ordinals(),
                &[0],
                "{id}/{dialect}: only the true physical row may change"
            );
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "orders"),
                Some(&expected),
                "{id}/{dialect}: complete native target after-state"
            );
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "customers"),
                Some(&source_states[1]),
                "{id}/{dialect}: source duplicates are read-only"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 26);
}
