//! TASK-157: frozen full-bag CREATE and materialized-snapshot row-state oracles.
//! DuckDB supplies independent portable SQL execution evidence. DuckDB does not
//! implement CREATE MATERIALIZED VIEW; the equivalent stored CTAS snapshot is
//! tested separately from the canonical materialization identity.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_view_read_plan,
    physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue, PhysicalViewProof,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, WriteDefinitionMode,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-157-acceptance-freeze.json")).unwrap()
}

fn row(value: &Value) -> PhysicalWriteRow {
    let v = value["k"]
        .as_i64()
        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()));
    PhysicalWriteRow::new(BTreeMap::from([("k".to_owned(), v)])).unwrap()
}

fn state(name: &str, values: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(name, values.as_array().unwrap().iter().map(row).collect()).unwrap()
}

fn catalog(case: &Value) -> RelationCatalog {
    let mut names = vec![
        "raw".to_owned(),
        case["target"].as_str().unwrap().to_owned(),
    ];
    names.sort();
    names.dedup();
    RelationCatalog::from_schemas(
        &names
            .into_iter()
            .map(|name| {
                RelationSchema::new(
                    name,
                    vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn bundle(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let sql = case["sql"].as_array().unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = sql
        .iter()
        .map(|value| SqlInput::inline(value.as_str().unwrap()))
        .collect::<Vec<_>>();
    let ids = (0..inputs.len())
        .map(|index| format!("ddl-{index}"))
        .collect::<Vec<_>>();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog(case)).unwrap()
}

fn initial(case: &Value) -> Vec<PhysicalWriteState> {
    case["initial"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, values)| state(name, values))
        .collect()
}

fn sorted_rows(state: &PhysicalWriteState) -> Vec<String> {
    let mut values = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn assert_bag(actual: &PhysicalWriteState, expected: &Value, case_id: &str) {
    let expected = state(actual.relation(), expected);
    assert_eq!(
        sorted_rows(actual),
        sorted_rows(&expected),
        "{case_id}: exact row multiset"
    );
}

fn assert_domains(actual: &PhysicalWriteState, schema: &RelationSchema, domain: &Value) {
    let columns = actual.exact_domains(schema).unwrap();
    assert_eq!(columns.len(), 1);
    let values = columns[0].values();
    let mut actual_freq = BTreeMap::<String, u64>::new();
    let mut integers = Vec::<i64>::new();
    for (value, frequency) in values {
        let key = match value {
            LiteralValue::Null => "NULL".to_owned(),
            LiteralValue::Number(n) => {
                integers.push(n.parse().unwrap());
                n.clone()
            }
            other => panic!("unsupported typed integer oracle: {other:?}"),
        };
        actual_freq.insert(key, *frequency);
    }
    let expected = domain["frequencies"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| (key.clone(), value.as_u64().unwrap()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_freq, expected);
    assert_eq!(integers.iter().min().copied(), domain["lower"].as_i64());
    assert_eq!(integers.iter().max().copied(), domain["upper"].as_i64());
    assert_eq!(
        actual_freq.contains_key("NULL"),
        domain["nullable"].as_bool().unwrap()
    );
    assert_eq!(
        !integers.is_empty(),
        domain["lower_inclusive"].as_bool().unwrap()
    );
    assert_eq!(
        !integers.is_empty(),
        domain["upper_inclusive"].as_bool().unwrap()
    );
}

fn native(case: &Value, materialized_emulated: bool) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE raw(k INTEGER)").unwrap();
    for value in case["initial"]["raw"].as_array().unwrap() {
        db.execute("INSERT INTO raw VALUES (?1)", params![value["k"].as_i64()])
            .unwrap();
    }
    if case["initial"]["copy_stage"].is_array() {
        db.execute_batch("CREATE TABLE copy_stage(k INTEGER)")
            .unwrap();
        for value in case["initial"]["copy_stage"].as_array().unwrap() {
            db.execute(
                "INSERT INTO copy_stage VALUES (?1)",
                params![value["k"].as_i64()],
            )
            .unwrap();
        }
    }
    if case["setup_sql"].is_array() {
        for stmt in case["setup_sql"].as_array().unwrap() {
            db.execute_batch(stmt.as_str().unwrap()).unwrap();
        }
    }
    for (index, sql) in case["sql"].as_array().unwrap().iter().enumerate() {
        let stmt = sql.as_str().unwrap();
        if materialized_emulated && index == 0 {
            assert!(stmt.starts_with("CREATE MATERIALIZED VIEW "));
            db.execute_batch(&stmt.replacen("CREATE MATERIALIZED VIEW ", "CREATE TABLE ", 1))
                .unwrap();
        } else if case["classification"] == "Impossible" && index == 0 {
            assert!(
                db.execute_batch(stmt).is_err(),
                "duplicate CREATE must fail"
            );
        } else {
            db.execute_batch(stmt).unwrap();
        }
    }
    db
}

fn native_bag(db: &Connection, target: &str) -> Vec<Option<i64>> {
    assert!([
        "empty_stage",
        "copy_stage",
        "stage.copy_stage",
        "view_stage",
        "snap_stage",
        "raw"
    ]
    .contains(&target));
    let mut statement = db.prepare(&format!("SELECT k FROM {target}")).unwrap();
    let mut result = statement
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn frozen_bag(rows: &Value) -> Vec<Option<i64>> {
    let mut values = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["k"].as_i64())
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[test]
fn frozen_table_create_replace_and_duplicate_oracles_match_native_duckdb() {
    for case in freeze()["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if id.contains("logical-view")
            || id.contains("materialized")
            || case["classification"] == "Residual"
        {
            continue;
        }
        let analyzed = bundle(case);
        let result = physical_ordered_write_state_plan(&analyzed, &initial(case));
        let db = native(case, false);
        let target = case["target"].as_str().unwrap();
        if case["classification"] == "Impossible" {
            assert!(
                matches!(result, PhysicalWriteStatePlan::Impossible { .. }),
                "{id}: {result:?}"
            );
            assert_eq!(native_bag(&db, target), frozen_bag(&case["before"]));
            continue;
        }
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = result
        else {
            panic!("{id}: expected a certified CREATE/CTAS output: {result:?}");
        };
        assert_eq!(transitions.len(), 1, "{id}: single DDL transition");
        let transition = &transitions[0];
        let expected_source_ids = case["source_truth"]["true_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| vec![format!("raw#{}", value.as_u64().unwrap())])
            .collect::<Vec<_>>();
        assert_eq!(
            transition.definition_source_row_ids(),
            expected_source_ids.as_slice(),
            "{id}: each physical output must retain its exact original source occurrence"
        );
        assert_eq!(transition.after().relation(), target);
        assert_bag(transition.after(), &case["after"], id);
        assert_eq!(
            transition.affected().inserted() as usize,
            case["after"].as_array().unwrap().len()
        );
        assert_eq!(
            transition.affected().deleted() as usize,
            case["before"].as_array().map_or(0, Vec::len)
        );
        assert_eq!(transition.affected().updated(), 0);
        let final_state = final_states
            .iter()
            .find(|s| s.relation() == target)
            .unwrap();
        assert_bag(final_state, &case["after"], id);
        let schema = analyzed
            .source_schemas()
            .iter()
            .find(|s| s.relation() == target)
            .unwrap();
        assert_domains(final_state, schema, &case["domain"]);
        assert_eq!(
            native_bag(&db, target),
            frozen_bag(&case["after"]),
            "{id}: DuckDB full snapshot"
        );
        assert_eq!(
            native_bag(&db, "raw"),
            frozen_bag(&case["initial"]["raw"]),
            "{id}: CREATE must preserve the original complete physical source"
        );
    }
}

#[test]
fn materialized_view_freezes_creation_bag_while_source_changes() {
    let freeze = freeze();
    let case = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "83-C157-P-materialized")
        .unwrap();
    let analyzed = bundle(case);
    let definition = analyzed.inputs()[0].statements().first().unwrap();
    let sql_semantic_protocol::ProtocolStatement::Query(query) = definition else {
        panic!("materialized view definition must normalize");
    };
    assert_eq!(
        query.write().unwrap().definition_mode(),
        Some(WriteDefinitionMode::CreateMaterializedView)
    );
    let result = physical_ordered_write_state_plan(&analyzed, &initial(case));
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = result
    else {
        panic!("materialized snapshot and source append must be exact: {result:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(
        transitions[0].definition_source_row_ids(),
        [
            vec!["raw#0".to_owned()],
            vec!["raw#1".to_owned()],
            vec!["raw#2".to_owned()],
        ],
        "materialized snapshot must retain creation-time producer IDs after source INSERT"
    );
    let snapshot = final_states
        .iter()
        .find(|s| s.relation() == "snap_stage")
        .unwrap();
    let source = final_states.iter().find(|s| s.relation() == "raw").unwrap();
    assert_bag(snapshot, &case["after"], "snap_stage");
    assert_eq!(source.rows().len(), 6);
    let schema = analyzed
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "snap_stage")
        .unwrap();
    assert_domains(snapshot, schema, &case["domain"]);
    let db = native(case, true);
    assert_eq!(native_bag(&db, "snap_stage"), frozen_bag(&case["after"]));
    assert_eq!(native_bag(&db, "raw").len(), 6);
}

#[test]
fn if_not_exists_does_not_silently_invent_an_exact_creation() {
    let freeze = freeze();
    let case = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "83-C157-N-conditional")
        .unwrap();
    let result = physical_ordered_write_state_plan(&bundle(case), &initial(case));
    assert!(
        matches!(result, PhysicalWriteStatePlan::Residual { .. }),
        "{result:?}"
    );
}

#[test]
fn logical_view_re_evaluates_after_canonical_source_append() {
    let frozen = freeze();
    let case = frozen["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "83-C157-P-logical-view")
        .unwrap();
    let analyzed = bundle(case);
    assert_eq!(analyzed.layers().len(), 3);
    let target_layer = analyzed.layers().last().unwrap();
    let result = physical_finite_view_read_plan(&analyzed, target_layer.id(), &initial(case), None);
    let PhysicalViewProof::Exact(proof) = result else {
        panic!("VIEW must re-evaluate the canonical post-append physical leaf: {result:?}");
    };
    assert_eq!(proof.view_name, "view_stage");
    assert_eq!(proof.source_bag.len(), 6);
    assert_eq!(proof.view_bag.len(), 4);
    assert_eq!(proof.terminal.len(), 4);
    assert_eq!(proof.terminal[0].source_row_ids, ["raw#0"]);
    assert_eq!(proof.terminal[1].source_row_ids, ["raw#1"]);
    assert_eq!(proof.terminal[2].source_row_ids, ["raw#2"]);
    assert_eq!(proof.terminal[3].source_row_ids, ["raw#5"]);
    let mut values = proof
        .terminal
        .iter()
        .map(|row| match &row.values["k"] {
            LiteralValue::Number(value) => Some(value.parse::<i64>().unwrap()),
            LiteralValue::Null => None,
            other => panic!("unexpected view value {other:?}"),
        })
        .collect::<Vec<_>>();
    values.sort();
    assert_eq!(values, frozen_bag(&case["after"]));
    assert_eq!(proof.terminal_domains["k"].integer_bounds, Some((1, 3)));
    assert!(!proof.terminal_domains["k"].null_allowed);
    let missing = BTreeMap::from([("k".to_owned(), LiteralValue::Number("0".to_owned()))]);
    assert!(matches!(
        physical_finite_view_read_plan(
            &analyzed,
            target_layer.id(),
            &initial(case),
            Some(&missing)
        ),
        PhysicalViewProof::Impossible { .. }
    ));
    let db = native(case, false);
    assert_eq!(native_bag(&db, "view_stage"), frozen_bag(&case["after"]));
}

#[test]
fn materialized_snapshot_cannot_be_reinterpreted_as_a_logical_view() {
    let frozen = freeze();
    let case = frozen["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "83-C157-P-materialized")
        .unwrap();
    let analyzed = bundle(case);
    let read = analyzed.layers().last().unwrap();
    assert!(matches!(
        physical_finite_view_read_plan(&analyzed, read.id(), &initial(case), None),
        PhysicalViewProof::Residual { .. }
    ));
}

#[test]
fn frozen_positive_and_not_true_source_identities_match_duckdb_sql_three_valued_logic() {
    let freeze = freeze();
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if id == "83-C157-P-empty-table" || case["classification"] == "Residual" {
            continue; // No source query or attested conditional execution.
        }
        let materialized = id == "83-C157-P-materialized";
        let db = native(case, materialized);
        let predicate = if id == "83-C157-N-duplicate" || id == "83-C157-N-null-duplicate" {
            "TRUE"
        } else {
            "k > 0"
        };
        // A materialized snapshot's source membership is fixed before the
        // subsequent INSERT, not recomputed from the later six-row input.
        let scope = if materialized { " AND rowid < 5" } else { "" };
        for (column, truth) in [
            ("true_ordinals", "IS TRUE"),
            ("not_true_ordinals", "IS NOT TRUE"),
        ] {
            let query =
                format!("SELECT rowid FROM raw WHERE ({predicate}) {truth}{scope} ORDER BY rowid");
            let mut statement = db.prepare(&query).unwrap();
            let actual = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            let expected = case["source_truth"][column]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                actual, expected,
                "{id}/{column}: exact source occurrence membership"
            );
        }
    }
}

#[test]
fn unsupported_transaction_between_view_definition_and_read_blocks_exact_proof() {
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = [
        SqlInput::inline("CREATE VIEW view_stage AS SELECT k FROM raw WHERE k > 0"),
        SqlInput::inline("BEGIN"),
        SqlInput::inline("INSERT INTO raw VALUES (3)"),
        SqlInput::inline("SELECT k FROM view_stage"),
    ];
    let ids = ["view", "transaction", "append", "read"];
    let configured = sql
        .iter()
        .zip(ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "raw",
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
    ])
    .unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    assert!(bundle.inputs().iter().any(|input| {
        input.statements().iter().any(|statement| {
            matches!(statement, sql_semantic_protocol::ProtocolStatement::Unsupported(_))
        })
    }));
    assert_eq!(bundle.layers().len(), 3);
    let leaf = PhysicalWriteState::new(
        "raw",
        vec![PhysicalWriteRow::new(BTreeMap::from([(
            "k".to_owned(),
            LiteralValue::Number("1".to_owned()),
        )]))
        .unwrap()],
    )
    .unwrap();
    let read = bundle.layers().last().unwrap();
    assert!(matches!(
        physical_finite_view_read_plan(&bundle, read.id(), &[leaf], None),
        PhysicalViewProof::Residual { .. }
    ));
}
