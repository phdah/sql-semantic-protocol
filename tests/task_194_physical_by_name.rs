//! TASK-194: original UNION BY NAME has a closed-source, tuple-level Exact
//! physical proof only when both typed leaf outputs have the same unique names.
//! Other name alignment stays Residual; no vendor-execution claim is made.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_set_tree_membership_plan, physical_finite_set_tree_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-194-acceptance-freeze.json");

fn literal(value: &Value) -> LiteralValue {
    if let Some(n) = value.as_i64() {
        LiteralValue::Number(n.to_string())
    } else {
        assert!(value.is_null(), "closed-world integer or SQL NULL");
        LiteralValue::Null
    }
}

fn sources() -> (RelationCatalog, Vec<PhysicalWriteState>) {
    let fixture: Value = serde_json::from_str(FREEZE).expect("frozen sources");
    let mut schemas = Vec::new();
    let mut states = Vec::new();
    for relation in ["orders", "customers"] {
        schemas.push(
            RelationSchema::new(
                relation,
                ["__physical_row_id", "id", "amount"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            column,
                            if column == "__physical_row_id" {
                                "VARCHAR"
                            } else {
                                "INTEGER"
                            },
                            "duckdb",
                        )
                        .expect("authoritative physical datatype")
                    })
                    .collect(),
            )
            .expect("typed physical source"),
        );
        let rows = fixture["complete_source_bags"][relation]
            .as_array()
            .expect("every source row")
            .iter()
            .enumerate()
            .map(|(ordinal, row)| {
                let tuple = row.as_array().expect("two physical columns");
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "__physical_row_id".to_string(),
                        LiteralValue::Text(format!("{relation}#{ordinal}")),
                    ),
                    ("id".to_string(), literal(&tuple[0])),
                    ("amount".to_string(), literal(&tuple[1])),
                ]))
                .expect("stable source-row identity distinct from duplicated/NULL SQL id")
            })
            .collect();
        states.push(PhysicalWriteState::new(relation, rows).expect("complete source state"));
    }
    (
        RelationCatalog::from_schemas(&schemas).expect("canonical typed source catalog"),
        states,
    )
}

fn analyze(
    sql: &str,
    dialect_name: &str,
    catalog: &RelationCatalog,
) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name(dialect_name).expect("one of 13 SQL parser dialects");
    let input = SqlInput::inline(sql);
    let chosen = [ConfiguredSqlInput::new(
        "task194-physical-by-name",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&chosen, catalog).expect("source-typed set SQL")
}

fn decoded_terminal(result: &[BTreeMap<String, LiteralValue>]) -> Vec<Vec<Option<i64>>> {
    let mut tuples = result
        .iter()
        .map(|row| {
            ["id", "amount"]
                .into_iter()
                .map(|name| match row.get(name).expect("both typed columns") {
                    LiteralValue::Number(n) => Some(n.parse::<i64>().expect("closed SQL INTEGER")),
                    LiteralValue::Null => None,
                    other => panic!("unexpected source value: {other:?}"),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    tuples.sort();
    tuples
}

#[test]
fn all_13_parsers_certify_original_by_name_complete_physical_bags() {
    let fixture: Value = serde_json::from_str(FREEZE).expect("frozen case");
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "88-C177-S083")
        .expect("original BY NAME SQL");
    let expected = {
        let mut result = case["expected_terminal_bag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r.as_array()
                    .unwrap()
                    .iter()
                    .map(Value::as_i64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        result.sort();
        result
    };
    let (catalog, states) = sources();
    for dialect in fixture["dialects"]
        .as_array()
        .expect("13 original parser cells")
    {
        let name = dialect.as_str().expect("dialect");
        let bundle = analyze(case["sql"].as_str().expect("original SQL"), name, &catalog);
        let target = bundle.layers().last().expect("terminal set").id();
        let plan = physical_finite_set_tree_plan(&bundle, target, &states);
        let PhysicalFiniteSetTreeProof::Exact(exact) = plan else {
            panic!("88-C177-S083/{name}: original BY NAME physical proof must be exact: {plan:?}");
        };
        assert_eq!(
            decoded_terminal(&exact.terminal),
            expected,
            "{name}: SQL tuple bag"
        );
        assert_eq!(exact.branches.len(), 2);
        assert_eq!(exact.branches[0].selected_row_ids.len(), 6);
        assert_eq!(exact.branches[1].selected_row_ids.len(), 5);
        assert_eq!(exact.tuples.iter().map(|t| t.output_count).sum::<u64>(), 7);
        assert_eq!(
            exact
                .tuples
                .iter()
                .map(
                    |t| t.branch_row_ids.iter().map(Vec::len).sum::<usize>() as u64
                        - t.output_count
                )
                .sum::<u64>(),
            4,
            "{name}: 4 duplicate source occurrences discarded by UNION BY NAME DISTINCT"
        );
        let requested = BTreeMap::from([
            ("id".to_string(), LiteralValue::Number("999".to_string())),
            ("amount".to_string(), LiteralValue::Number("10".to_string())),
        ]);
        assert!(
            matches!(
                physical_finite_set_tree_membership_plan(&bundle, target, &states, &requested),
                PhysicalFiniteSetTreeProof::Impossible { .. }
            ),
            "{name}: absent whole tuple is Impossible in the complete physical bag"
        );
    }
}

#[test]
fn unsafe_by_name_alignment_cannot_be_promoted_to_a_physical_proof() {
    let (catalog, states) = sources();
    for sql in [
        "SELECT id, amount FROM orders UNION BY NAME SELECT id FROM customers",
        "SELECT id AS value, amount AS value FROM orders UNION BY NAME SELECT amount AS value, id AS other FROM customers",
    ] {
        let bundle = analyze(sql, "generic", &catalog);
        let target = bundle.layers().last().expect("set").id();
        assert!(
            matches!(
                physical_finite_set_tree_plan(&bundle, target, &states),
                PhysicalFiniteSetTreeProof::Residual { .. }
            ),
            "{sql}: unmatched or duplicated names have no sound bijection"
        );
    }
}

#[test]
fn by_name_page_orders_in_leaf_coordinates_before_aligning_the_terminal_tuple() {
    // Right-side SELECT order (amount, id) differs from terminal (id, amount).
    // Ordering the already-aligned tuple instead would pick (3, 50), not (9, 2).
    let (catalog, _) = sources();
    let state = |relation: &str, tuples: &[(i64, i64)]| {
        let rows = tuples
            .iter()
            .enumerate()
            .map(|(ordinal, (id, amount))| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "__physical_row_id".to_string(),
                        LiteralValue::Text(format!("{relation}#{ordinal}")),
                    ),
                    ("id".to_string(), LiteralValue::Number(id.to_string())),
                    (
                        "amount".to_string(),
                        LiteralValue::Number(amount.to_string()),
                    ),
                ]))
                .expect("independent physical source identity")
            })
            .collect();
        PhysicalWriteState::new(relation, rows).expect("complete source population")
    };
    let states = vec![
        state("orders", &[(1, 10)]),
        state("customers", &[(3, 50), (9, 2)]),
    ];
    let sql = "SELECT id, amount FROM orders UNION BY NAME \
               (SELECT amount, id FROM customers \
                ORDER BY amount ASC NULLS LAST, id ASC NULLS LAST LIMIT 1)";
    let bundle = analyze(sql, "duckdb", &catalog);
    let target = bundle.layers().last().expect("terminal set").id();
    let plan = physical_finite_set_tree_plan(&bundle, target, &states);
    let PhysicalFiniteSetTreeProof::Exact(exact) = plan else {
        panic!("name-aligned branch page must be exact: {plan:?}");
    };
    assert_eq!(
        exact.branches[1].selected_row_ids,
        vec!["customers#1"],
        "page membership uses amount first, not final output id"
    );

    let db = Connection::open_in_memory().expect("independent DuckDB oracle");
    db.execute_batch(
        "CREATE TABLE orders(id INTEGER, amount INTEGER);
         CREATE TABLE customers(id INTEGER, amount INTEGER);
         INSERT INTO orders VALUES (1, 10);
         INSERT INTO customers VALUES (3, 50), (9, 2);",
    )
    .expect("complete independent DuckDB source population");
    let mut query = db
        .prepare(&format!("SELECT id, amount FROM ({sql}) AS result ORDER BY id"))
        .expect("native BY NAME query");
    let native = query
        .query_map([], |row| {
            Ok(vec![
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
            ])
        })
        .expect("execute native SQL")
        .collect::<Result<Vec<_>, _>>()
        .expect("complete terminal bag");
    assert_eq!(
        decoded_terminal(&exact.terminal),
        native,
        "physical proof must reproduce DuckDB's actual terminal tuples"
    );
    assert_eq!(native, vec![vec![Some(1), Some(10)], vec![Some(9), Some(2)]]);
}
