//! TASK-144: complete raw INSERT-CTE -> materialized target -> reused CTE read.
//! The terminal is proved from the original raw physical rows, not a caller
//! supplied post-write population. DuckDB independently executes both SQL steps.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_cte_read_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalOrderedCteProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const INSERT: &str = "INSERT INTO target(k, shifted) WITH stage AS \
    (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) \
    SELECT k, shifted FROM stage";
const READ: &str = "WITH copied AS (SELECT k, shifted FROM target) \
    SELECT a.shifted AS l, b.shifted AS r FROM copied a \
    JOIN copied b ON a.k = b.k";

fn num(value: i64) -> LiteralValue {
    LiteralValue::Number(value.to_string())
}

fn initial(raw: &[Option<i64>]) -> Vec<PhysicalWriteState> {
    let source_rows = raw
        .iter()
        .map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([(
                "k".to_string(),
                value.map_or(LiteralValue::Null, num),
            )]))
            .unwrap()
        })
        .collect();
    vec![
        PhysicalWriteState::new("raw", source_rows).unwrap(),
        PhysicalWriteState::new("target", vec![]).unwrap(),
    ]
}

fn analyze() -> sql_semantic_protocol::AnalysisBundle {
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "raw".to_string(),
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
        RelationSchema::new(
            "target".to_string(),
            vec![
                SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap(),
                SchemaColumn::from_sql_type("shifted", "INTEGER", "duckdb").unwrap(),
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let write = SqlInput::inline(INSERT);
    let read = SqlInput::inline(READ);
    let configured = [
        ConfiguredSqlInput::new("write", &write, "duckdb", dialect.as_ref()),
        ConfiguredSqlInput::new("terminal", &read, "duckdb", dialect.as_ref()),
    ];
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

fn canonical(raw: &[Option<i64>], absent: Option<i64>) -> PhysicalOrderedCteProof {
    let bundle = analyze();
    let target = bundle.layers().last().unwrap();
    let requested = absent.map(|value| {
        BTreeMap::from([("l".to_string(), num(value)), ("r".to_string(), num(value))])
    });
    physical_ordered_cte_read_plan(&bundle, target.id(), &initial(raw), requested.as_ref())
}

struct NativeBags {
    physical: Vec<Option<i64>>,
    stage: Vec<(i64, i64)>,
    target: Vec<(i64, i64)>,
    terminal: Vec<(i64, i64)>,
}

fn native(raw: &[Option<i64>]) -> NativeBags {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw(k INTEGER); CREATE TABLE target(k INTEGER, shifted INTEGER)",
    )
    .unwrap();
    for value in raw {
        db.execute("INSERT INTO raw(k) VALUES (?1)", params![value])
            .unwrap();
    }
    let physical = db
        .prepare("SELECT k FROM raw ORDER BY rowid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<duckdb::Result<Vec<Option<i64>>>>()
        .unwrap();
    let stage = db
        .prepare("SELECT k, k + 10 AS shifted FROM raw WHERE k > 1 ORDER BY rowid")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<(i64, i64)>>>()
        .unwrap();
    db.execute_batch(INSERT).unwrap();
    let target = db
        .prepare("SELECT k, shifted FROM target ORDER BY rowid")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<(i64, i64)>>>()
        .unwrap();
    let mut terminal = db
        .prepare(READ)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<(i64, i64)>>>()
        .unwrap();
    terminal.sort();
    NativeBags {
        physical,
        stage,
        target,
        terminal,
    }
}

fn assert_case(id: &str, raw: &[Option<i64>], shifted: &[i64], expected_count: usize) {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap();
    let case = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap();
    let expected_rows = case["expected_terminal"].as_array().unwrap();
    let NativeBags {
        physical,
        stage: native_stage,
        target: native_target,
        terminal: native_terminal,
    } = native(raw);
    assert_eq!(physical, raw, "{id}: complete initial source");
    let expected_stage = shifted
        .iter()
        .map(|value| (value - 10, *value))
        .collect::<Vec<_>>();
    assert_eq!(
        native_stage, expected_stage,
        "{id}: independent producer bag"
    );
    assert_eq!(
        native_target, expected_stage,
        "{id}: native materialized target"
    );
    assert_eq!(native_terminal.len(), expected_count);

    let PhysicalOrderedCteProof::Exact(proof) = canonical(raw, None) else {
        panic!("{id}: no source-complete ordered materialization + terminal proof");
    };
    assert_eq!(proof.transitions.len(), 1, "{id}: exactly one actual write");
    let transition = &proof.transitions[0];
    assert_eq!(transition.before().rows().len(), 0);
    assert_eq!(transition.after().rows().len(), shifted.len());
    assert_eq!(transition.intermediate_states().len(), 1);
    assert_eq!(
        transition.intermediate_states()[0].rows().len(),
        shifted.len()
    );
    assert_eq!(
        proof.final_states.len(),
        2,
        "{id}: no independent stage source"
    );
    let target = proof
        .final_states
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    let produced = target
        .rows()
        .iter()
        .map(|row| (row.values()["k"].clone(), row.values()["shifted"].clone()))
        .collect::<Vec<_>>();
    let expected = expected_stage
        .iter()
        .map(|(k, s)| (num(*k), num(*s)))
        .collect::<Vec<_>>();
    assert_eq!(
        produced, expected,
        "{id}: every target value is producer-derived"
    );
    assert_eq!(proof.read.sources, vec!["target"]);
    assert_eq!(proof.read.source_bags["target"].len(), shifted.len());
    assert_eq!(
        proof.read.stage_bags["terminal/cte:0:copied"].len(),
        shifted.len()
    );

    let mut canonical_rows = proof
        .read
        .terminal
        .iter()
        .map(|row| {
            (
                row.values["l"].clone(),
                row.values["r"].clone(),
                row.source_row_ids.clone(),
            )
        })
        .collect::<Vec<_>>();
    canonical_rows.sort_by(|left, right| left.2.cmp(&right.2));
    let mut expected_rows = expected_rows
        .iter()
        .map(|row| {
            (
                num(row["l"].as_i64().unwrap()),
                num(row["r"].as_i64().unwrap()),
                row["source_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().to_string())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    expected_rows.sort_by(|left, right| left.2.cmp(&right.2));
    assert_eq!(
        canonical_rows, expected_rows,
        "{id}: complete row identities/multiplicity"
    );
    assert_eq!(proof.read.terminal.len(), expected_count);
    let canonical_pairs = canonical_rows
        .iter()
        .map(|(left, right, _)| {
            let LiteralValue::Number(left) = left else {
                panic!("integer left")
            };
            let LiteralValue::Number(right) = right else {
                panic!("integer right")
            };
            (left.parse::<i64>().unwrap(), right.parse::<i64>().unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        canonical_pairs, native_terminal,
        "{id}: independent DuckDB output bag"
    );
    for column in ["l", "r"] {
        let domain = &proof.read.terminal_domains[column];
        assert!(!domain.null_allowed);
        let expected_domain = &case["domains"][column];
        assert_eq!(
            domain.integer_bounds,
            Some((
                expected_domain["lower"]["value"].as_i64().unwrap() as i128,
                expected_domain["upper"]["value"].as_i64().unwrap() as i128
            ))
        );
        assert_eq!(expected_domain["lower"]["inclusive"], true);
        assert_eq!(expected_domain["upper"]["inclusive"], true);
    }
    assert!(
        matches!(
            canonical(raw, Some(11)),
            PhysicalOrderedCteProof::Impossible { .. }
        ),
        "{id}: closed-world absence must be impossible"
    );
}

#[test]
fn multistep_filtered_cte_materialization_reuses_only_derived_rows() {
    assert_case(
        "78-C144-P-MULTISTEP",
        &[Some(1), Some(2), Some(3)],
        &[12, 13],
        2,
    );
}

#[test]
fn multistep_materialization_preserves_duplicate_occurrences_and_rejects_nulls() {
    assert_case(
        "78-C144-N-MULTISTEP-NULL",
        &[Some(1), Some(2), Some(2), None],
        &[12, 12],
        4,
    );
}

#[test]
fn duplicate_initial_target_population_is_impossible() {
    let bundle = analyze();
    let read_layer = bundle.layers().last().unwrap();
    let mut states = initial(&[Some(1), Some(2), Some(3)]);
    states.push(PhysicalWriteState::new("target", vec![]).unwrap());
    assert!(matches!(
        physical_ordered_cte_read_plan(&bundle, read_layer.id(), &states, None),
        PhysicalOrderedCteProof::Impossible { .. }
    ));
}
