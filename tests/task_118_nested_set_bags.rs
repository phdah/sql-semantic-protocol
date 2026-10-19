//! TASK-118: complete nested set source/terminal bags against independent DuckDB.
//! Public seam: configured SQL to canonical graph to whole-source physical proof.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_set_tree_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteSetTreeProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-118-acceptance-freeze.json"))
        .expect("task 118 finite acceptance matrix")
}

fn source_schema(name: &str, model: bool) -> RelationSchema {
    let cols = if model {
        vec!["k", "v"]
    } else {
        vec!["id", "k", "v"]
    };
    RelationSchema::new(
        name,
        cols.into_iter()
            .map(|column| {
                SchemaColumn::from_sql_type(
                    column,
                    if column == "id" { "VARCHAR" } else { "INTEGER" },
                    "postgresql",
                )
                .expect("typed schema column")
            })
            .collect(),
    )
    .expect("source schema")
}

fn physical_states(case: &Value) -> Vec<PhysicalWriteState> {
    let entries = case["source_bags"]
        .as_object()
        .expect("all source populations");
    entries
        .iter()
        .map(|(name, bag)| {
            let rows = bag
                .as_array()
                .expect("physical bag")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("whole row")
                        .iter()
                        .map(|(key, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(n) = value.as_i64() {
                                LiteralValue::Number(n.to_string())
                            } else {
                                LiteralValue::Text(
                                    value.as_str().expect("physical identity").to_string(),
                                )
                            };
                            (key.to_string(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("source row")
                })
                .collect();
            PhysicalWriteState::new(name, rows).expect("source identity")
        })
        .collect()
}

fn bundle_for(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let statements = case["statements"].as_array().expect("producer SQL");
    let mut queries = statements
        .iter()
        .map(|value| value.as_str().expect("producer statement").to_string())
        .collect::<Vec<_>>();
    queries.push(case["sql"].as_str().expect("terminal SQL").to_string());
    let inputs = queries
        .iter()
        .map(SqlInput::inline)
        .collect::<Vec<_>>();
    let names = (0..inputs.len())
        .map(|n| format!("task118-{n}"))
        .collect::<Vec<_>>();
    let dialect_instance = dialect_from_name(dialect).expect("dialect");
    let configured = inputs
        .iter()
        .zip(names.iter())
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, dialect, dialect_instance.as_ref()))
        .collect::<Vec<_>>();
    let mut schemas = case["source_bags"]
        .as_object()
        .expect("sources")
        .keys()
        .map(|relation| source_schema(relation, false))
        .collect::<Vec<_>>();
    if !statements.is_empty() {
        schemas.extend(["sa", "sb", "sc"].map(|name| source_schema(name, true)));
    }
    let catalog = RelationCatalog::from_schemas(&schemas).expect("source schema catalog");
    analyze_configured_inputs_with_catalog(&configured, &catalog)
        .unwrap_or_else(|error| panic!("{}: {error:?}", case["id"]))
}

fn key(values: &Value) -> Vec<Option<i128>> {
    values
        .as_array()
        .expect("positional values")
        .iter()
        .map(|value| value.as_i64().map(i128::from))
        .collect()
}

fn frozen_histogram(case: &Value) -> BTreeMap<Vec<Option<i128>>, u64> {
    case["expected_terminal_tuple_histogram"]
        .as_array()
        .expect("terminal histogram")
        .iter()
        .map(|entry| {
            (
                key(&entry["tuple"]),
                entry["count"].as_u64().expect("count"),
            )
        })
        .collect()
}

fn duckdb_histogram(connection: &Connection, sql: &str) -> BTreeMap<Vec<Option<i128>>, u64> {
    let mut stmt = connection
        .prepare(&format!(
            "SELECT k,v,COUNT(*) FROM ({sql}) AS result GROUP BY k,v"
        ))
        .expect("DuckDB parser");
    stmt.query_map([], |row| {
        let k = row.get::<_, Option<i64>>(0)?;
        let v = row.get::<_, Option<i64>>(1)?;
        let count = row.get::<_, u64>(2)?;
        Ok((vec![k.map(i128::from), v.map(i128::from)], count))
    })
    .expect("DuckDB terminal")
    .collect::<Result<BTreeMap<_, _>, _>>()
    .expect("entire DuckDB bag")
}

fn seed_database(connection: &Connection, case: &Value) {
    for (relation, bag) in case["source_bags"].as_object().expect("source bags") {
        connection
            .execute_batch(&format!(
                "CREATE TABLE {relation} (id VARCHAR,k INTEGER,v INTEGER);"
            ))
            .expect("physical source table");
        for row in bag.as_array().expect("source rows") {
            let value = |field: &str| {
                row[field]
                    .as_i64()
                    .map_or_else(|| "NULL".to_string(), |n| n.to_string())
            };
            connection
                .execute_batch(&format!(
                    "INSERT INTO {relation} (id,k,v) VALUES ('{}',{},{});",
                    row["id"].as_str().expect("source identity"),
                    value("k"),
                    value("v"),
                ))
                .expect("source snapshot");
        }
    }
    for statement in case["statements"].as_array().expect("producer statements") {
        connection
            .execute_batch(statement.as_str().expect("producer SQL"))
            .expect("independent DuckDB producer");
    }
}

#[test]
fn complete_physical_set_trees_match_independent_source_and_terminal_oracles() {
    let freeze = freeze();
    assert_eq!(freeze["case_count"], 5);
    let cases = freeze["cases"].as_array().expect("mandatory finite cases");
    let kinds = cases
        .iter()
        .flat_map(|case| case["variants"].as_array().expect("named variants"))
        .map(|item| item.as_str().expect("variant").to_string())
        .collect::<BTreeSet<_>>();
    for required in freeze["required_variants"].as_array().expect("variants") {
        assert!(
            kinds.contains(required.as_str().expect("variant")),
            "uncovered {required}"
        );
    }
    for case in cases {
        let id = case["id"].as_str().expect("fixture identity");
        let bundle = bundle_for(case, "generic");
        let target = bundle.layers().last().expect("terminal layer").id();
        let snapshots = physical_states(case);
        let proof = physical_finite_set_tree_plan(&bundle, target, &snapshots);
        let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
            panic!("{id}: missing complete closed-world proof: {proof:?}");
        };
        let expected = frozen_histogram(case);
        let observed = exact
            .tuples
            .iter()
            .filter(|tuple| tuple.output_count > 0)
            .map(|tuple| (tuple.values.clone(), tuple.output_count))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(observed, expected, "{id}: exact nullable tuple frequencies");
        assert_eq!(
            exact.terminal.len(),
            case["expected_count"].as_u64().expect("frozen count") as usize,
            "{id}: complete named terminal bag",
        );
        assert_eq!(
            exact.sources,
            snapshots
                .iter()
                .map(|state| state.relation().to_string())
                .collect::<Vec<_>>(),
            "{id}: one physical assignment per distinct source",
        );
        for branch in &exact.branches {
            let source = snapshots
                .iter()
                .find(|state| state.relation() == branch.source)
                .expect("shared source identity");
            let mut all = branch
                .selected_row_ids
                .iter()
                .chain(branch.false_row_ids.iter())
                .chain(branch.unknown_row_ids.iter())
                .cloned()
                .collect::<Vec<_>>();
            all.sort();
            let mut physical_ids = source
                .rows()
                .iter()
                .map(|row| match row.values().get("id") {
                    Some(LiteralValue::Text(id)) => id.clone(),
                    _ => panic!("source identity missing"),
                })
                .collect::<Vec<_>>();
            physical_ids.sort();
            assert_eq!(
                all, physical_ids,
                "{id}: complete SQL TRUE/NOT TRUE partition"
            );
        }
        for (position, domain) in exact.terminal_domains.iter().enumerate() {
            let expected_domain = &case["typed_terminal_domains"][position];
            assert_eq!(
                domain.bounds(),
                match (
                    expected_domain["lower"].as_i64(),
                    expected_domain["upper"].as_i64()
                ) {
                    (Some(a), Some(b)) => Some((i128::from(a), i128::from(b))),
                    _ => None,
                },
                "{id}: inclusive terminal integer bounds",
            );
            assert_eq!(
                domain.contains_null,
                expected_domain["contains_null"]
                    .as_bool()
                    .expect("NULL contract"),
                "{id}: terminal NULL evidence",
            );
        }
        let db = Connection::open_in_memory().expect("independent DuckDB");
        seed_database(&db, case);
        assert_eq!(
            duckdb_histogram(&db, case["sql"].as_str().expect("SQL")),
            expected,
            "{id}: independent entire SQL tree oracle",
        );
    }
}

#[test]
fn negative_set_count_is_closed_world_absence_not_a_missing_positive_example() {
    let freeze = freeze();
    let negative = freeze["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "72-C118-N-nested")
        .expect("negative case");
    let bundle = bundle_for(negative, "postgresql");
    let proof = physical_finite_set_tree_plan(
        &bundle,
        bundle.layers().last().expect("target").id(),
        &physical_states(negative),
    );
    let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
        panic!("negative count unproved: {proof:?}");
    };
    assert!(exact.terminal.is_empty());
    assert_eq!(exact.tuples.len(), 1);
    assert_eq!(exact.tuples[0].output_count, 0);
    assert_eq!(
        exact.tuples[0]
            .branch_row_ids
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![1, 1, 2],
        "all three physical populations contribute to absence",
    );
}

#[test]
fn missing_shared_source_and_duplicate_row_id_fail_closed() {
    let case = &freeze()["cases"][2];
    let bundle = bundle_for(case, "generic");
    let target = bundle.layers().last().expect("target").id();
    let states = physical_states(case);
    assert!(matches!(
        physical_finite_set_tree_plan(&bundle, target, &states[..1]),
        PhysicalFiniteSetTreeProof::Residual { .. },
    ));
    let mut duplicate = states[0].rows().to_vec();
    duplicate.push(states[0].rows()[0].clone());
    let wrong = PhysicalWriteState::new(states[0].relation(), duplicate).expect("wrong source");
    assert!(matches!(
        physical_finite_set_tree_plan(&bundle, target, &[wrong, states[1].clone()]),
        PhysicalFiniteSetTreeProof::Residual { .. },
    ));
}
