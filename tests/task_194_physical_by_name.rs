//! TASK-194: original UNION BY NAME has a closed-source, tuple-level Exact
//! physical proof only when both typed leaf outputs have the same unique names.
//! Other name alignment stays Residual; no vendor-execution claim is made.

use std::collections::BTreeMap;

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
