//! TASK-117: independent two-column NULL, duplicate, and absence oracles.
//! The expected tuple counts come from a frozen, versioned source-bag matrix.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_set_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteSetProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../docs/task-117-acceptance-freeze.json"))
        .expect("frozen TASK-117 matrix")
}

fn source_states(case: &Value) -> Vec<PhysicalWriteState> {
    ["a", "b"]
        .into_iter()
        .map(|relation| {
            let rows = case["source_bags"][relation]
                .as_array()
                .expect("complete source rows")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("fully named row")
                        .iter()
                        .map(|(column, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(number) = value.as_i64() {
                                LiteralValue::Number(number.to_string())
                            } else {
                                LiteralValue::Text(
                                    value.as_str().expect("string identity").to_string(),
                                )
                            };
                            (column.to_string(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("typed source row")
                })
                .collect();
            PhysicalWriteState::new(relation, rows).expect("source relation")
        })
        .collect()
}

fn source_schema(relation: &str) -> RelationSchema {
    RelationSchema::new(
        relation,
        ["id", "k", "v"]
            .into_iter()
            .map(|column| {
                let datatype = if column == "id" { "VARCHAR" } else { "INTEGER" };
                SchemaColumn::from_sql_type(column, datatype, "postgresql")
                    .expect("typed fixture scalar")
            })
            .collect(),
    )
    .expect("typed relation schema")
}

fn tuple_key(row: &Value) -> [Option<i128>; 2] {
    let components = row.as_array().expect("two positional values");
    std::array::from_fn(|index| components[index].as_i64().map(i128::from))
}

fn expected_histogram(case: &Value) -> BTreeMap<[Option<i128>; 2], u64> {
    case["full_terminal_tuple_histogram"]
        .as_array()
        .expect("complete expected output")
        .iter()
        .map(|row| {
            (
                tuple_key(&row["tuple"]),
                row["count"].as_u64().expect("expected multiplicity"),
            )
        })
        .collect()
}

fn duckdb_histogram(db: &Connection, sql: &str) -> BTreeMap<[Option<i128>; 2], u64> {
    let mut statement = db
        .prepare(&format!(
            "SELECT k, v, COUNT(*) FROM ({sql}) result GROUP BY k,v"
        ))
        .expect("valid independent SQL oracle");
    statement
        .query_map([], |row| {
            let k = row.get::<_, Option<i64>>(0)?;
            let v = row.get::<_, Option<i64>>(1)?;
            let count = row.get::<_, u64>(2)?;
            Ok(([k.map(i128::from), v.map(i128::from)], count))
        })
        .expect("query oracle")
        .collect::<Result<BTreeMap<_, _>, _>>()
        .expect("collect complete output")
}

fn seed_duckdb(db: &Connection, case: &Value) {
    db.execute_batch(
        "CREATE TABLE a (id VARCHAR, k INTEGER, v INTEGER);
         CREATE TABLE b (id VARCHAR, k INTEGER, v INTEGER);",
    )
    .expect("physical tables");
    for relation in ["a", "b"] {
        for row in case["source_bags"][relation]
            .as_array()
            .expect("source rows")
        {
            let id = row["id"].as_str().expect("source identity");
            let v = if row["v"].is_null() {
                "NULL".to_string()
            } else {
                row["v"].as_i64().expect("finite integer").to_string()
            };
            let sql = format!(
                "INSERT INTO {relation} (id,k,v) VALUES ('{id}',{},{v})",
                row["k"].as_i64().expect("finite integer")
            );
            db.execute_batch(&sql).expect("closed physical source rows");
        }
    }
}

#[test]
fn frozen_six_operators_prove_exact_two_column_source_and_terminal_bags() {
    let freeze = fixture();
    assert_eq!(freeze["case_count"], 12);
    let schemas = [source_schema("a"), source_schema("b")];
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    for case in freeze["cases"].as_array().expect("all cases") {
        let id = case["id"].as_str().expect("fixture id");
        let sql = case["sql"].as_str().expect("frozen sql");
        let dialect = dialect_from_name("generic").expect("generic dialect");
        let input = SqlInput::inline(sql);
        let configured = ConfiguredSqlInput::new(id, &input, "generic", dialect.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog)
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        let target = bundle.layers().last().expect("terminal layer").id();
        let states = source_states(case);
        let proof = physical_finite_set_bag_plan(&bundle, target, &states);
        let PhysicalFiniteSetProof::Exact(exact) = proof else {
            panic!("{id}: no exact closed-world bag proof: {proof:?}");
        };
        assert_eq!(exact.sources, ["a".to_string(), "b".to_string()], "{id}");

        let actual = exact
            .tuples
            .iter()
            .filter(|tuple| tuple.output_count > 0)
            .map(|tuple| (tuple.values, tuple.output_count))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            actual,
            expected_histogram(case),
            "{id}: full tuple histogram"
        );
        assert_eq!(
            exact.terminal.len(),
            case["expected_count"].as_u64().expect("count") as usize,
            "{id}: terminal multiplicity"
        );

        for entry in &exact.tuples {
            for (side, identities) in [("a", &entry.left_row_ids), ("b", &entry.right_row_ids)] {
                let expected = case["source_bags"][side]
                    .as_array()
                    .expect("source")
                    .iter()
                    .filter(|row| {
                        [
                            row["k"].as_i64().map(i128::from),
                            row["v"].as_i64().map(i128::from),
                        ] == entry.values
                    })
                    .map(|row| row["id"].as_str().expect("id").to_string())
                    .collect::<Vec<_>>();
                assert_eq!(*identities, expected, "{id}: closed source identities");
            }
        }
        for (index, domain) in exact.terminal_domains.iter().enumerate() {
            let frozen = &case["terminal_value_domains"][index];
            let expected_bounds = match (frozen["lower"].as_i64(), frozen["upper"].as_i64()) {
                (Some(low), Some(high)) => Some((i128::from(low), i128::from(high))),
                _ => None,
            };
            assert_eq!(domain.bounds(), expected_bounds, "{id}: typed bounds");
            assert_eq!(
                domain.contains_null,
                frozen["contains_null"].as_bool().expect("NULL evidence"),
                "{id}: exact NULL domain"
            );
        }

        let db = Connection::open_in_memory().expect("DuckDB");
        seed_duckdb(&db, case);
        assert_eq!(
            duckdb_histogram(&db, sql),
            expected_histogram(case),
            "{id}: independent SQL terminal bag"
        );
    }
}

#[test]
fn absent_except_membership_is_proven_from_both_complete_source_bags() {
    let freeze = fixture();
    let case = freeze["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "72-C117-N-saturating_difference")
        .expect("negative absence fixture");
    let catalog =
        RelationCatalog::from_schemas(&[source_schema("a"), source_schema("b")]).expect("catalog");
    let dialect = dialect_from_name("generic").expect("parser");
    let input = SqlInput::inline(case["sql"].as_str().expect("SQL"));
    let configured = ConfiguredSqlInput::new("negative", &input, "generic", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).expect("bundle");
    let proof = physical_finite_set_bag_plan(
        &bundle,
        bundle.layers().last().expect("layer").id(),
        &source_states(case),
    );
    let PhysicalFiniteSetProof::Exact(exact) = proof else {
        panic!("negative closed-world proof was not exact: {proof:?}");
    };
    assert!(exact.terminal.is_empty());
    assert_eq!(exact.tuples.len(), 2);
    assert!(exact.tuples.iter().all(|tuple| tuple.output_count == 0));
    assert_eq!(exact.tuples[0].left_row_ids.len(), 2);
    assert_eq!(exact.tuples[0].right_row_ids.len(), 3);
}
