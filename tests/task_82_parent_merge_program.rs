//! TASK-82 parent-level cross-child MERGE acceptance regression.
//! Independently exercise matched, by-target, by-source, NULL and sequential
//! computed updates against native DuckDB and the public physical-state API.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const MERGE_BRANCHES: &str = "MERGE INTO target USING source ON target.id = source.id
    WHEN MATCHED THEN UPDATE SET score = source.score
    WHEN NOT MATCHED BY TARGET THEN INSERT (id, score) VALUES (source.id, source.score)
    WHEN NOT MATCHED BY SOURCE THEN DELETE";
const MERGE_COMPUTED: &str = "MERGE INTO target AS t USING source AS s ON t.id = s.id
    WHEN MATCHED THEN UPDATE SET score = t.score + s.score";

fn row(id: Option<i64>, score: i64) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        (
            "id".to_owned(),
            id.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
        ("score".to_owned(), LiteralValue::Number(score.to_string())),
    ]))
    .unwrap()
}

fn state(relation: &str, rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter().map(|(id, score)| row(*id, *score)).collect(),
    )
    .unwrap()
}

fn rows(state: &PhysicalWriteState) -> Vec<String> {
    let mut bag = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    bag.sort();
    bag
}

fn native_state(db: &Connection, relation: &str) -> PhysicalWriteState {
    assert!(["target", "source"].contains(&relation));
    let mut stmt = db
        .prepare(&format!("SELECT id, score FROM {relation}"))
        .unwrap();
    let bag = stmt
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    state(relation, &bag)
}

fn native_on_ordinals(db: &Connection, relation: &str, matched: bool) -> Vec<i64> {
    assert!(["target", "source"].contains(&relation));
    let peer = if relation == "target" { "source" } else { "target" };
    let predicate = if matched { "EXISTS" } else { "NOT EXISTS" };
    let sql = format!(
        "SELECT ordinal FROM (
           SELECT row_number() OVER (ORDER BY rowid) - 1 AS ordinal, id FROM {relation}
         ) AS physical
         WHERE {predicate} (SELECT 1 FROM {peer} AS other WHERE physical.id = other.id)
         ORDER BY ordinal"
    );
    let mut stmt = db.prepare(&sql).unwrap();
    stmt.query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn parent_merge_program_preserves_closed_bags_and_sql_null_rejection() {
    let dialect = dialect_from_name("duckdb").unwrap();
    let catalog = RelationCatalog::from_schemas(
        &["target", "source"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["id", "score"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let sql = [SqlInput::inline(MERGE_BRANCHES), SqlInput::inline(MERGE_COMPUTED)];
    let inputs = [
        ConfiguredSqlInput::new("task82-branches", &sql[0], "duckdb", dialect.as_ref()),
        ConfiguredSqlInput::new("task82-computed", &sql[1], "duckdb", dialect.as_ref()),
    ];
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();

    let before = state("target", &[(Some(1), 10), (Some(2), 20), (None, 90)]);
    let source = state(
        "source",
        &[(Some(1), 100), (Some(3), 300), (None, 500)],
    );
    let after_first = state("target", &[(Some(1), 100), (Some(3), 300), (None, 500)]);
    let after_second = state("target", &[(Some(1), 200), (Some(3), 600), (None, 500)]);

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER);
         CREATE TABLE source(id INTEGER, score INTEGER);",
    )
    .unwrap();
    for (relation, bag) in [("target", &before), ("source", &source)] {
        for row in bag.rows() {
            let id = match row.values().get("id").unwrap() {
                LiteralValue::Null => None,
                LiteralValue::Number(value) => Some(value.parse::<i64>().unwrap()),
                value => panic!("unexpected id: {value:?}"),
            };
            let score = match row.values().get("score").unwrap() {
                LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
                value => panic!("unexpected score: {value:?}"),
            };
            db.execute(
                &format!("INSERT INTO {relation}(id,score) VALUES (?1,?2)"),
                params![id, score],
            )
            .unwrap();
        }
    }

    // This independent ON oracle includes both FALSE and UNKNOWN as NOT TRUE.
    assert_eq!(native_on_ordinals(&db, "target", true), vec![0]);
    assert_eq!(native_on_ordinals(&db, "target", false), vec![1, 2]);
    assert_eq!(native_on_ordinals(&db, "source", true), vec![0]);
    assert_eq!(native_on_ordinals(&db, "source", false), vec![1, 2]);

    let plan = physical_ordered_write_state_plan(&bundle, &[before.clone(), source.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("parent cross-child MERGE must be exact: {plan:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(rows(transitions[0].before()), rows(&before));
    assert_eq!(rows(transitions[0].after()), rows(&after_first));
    assert_eq!(transitions[0].affected().inserted(), 2);
    assert_eq!(transitions[0].affected().updated(), 1);
    assert_eq!(transitions[0].affected().deleted(), 2);
    assert_eq!(rows(transitions[1].before()), rows(&after_first));
    assert_eq!(rows(transitions[1].after()), rows(&after_second));
    assert_eq!(transitions[1].affected().updated(), 2);
    assert_eq!(transitions[1].affected().inserted(), 0);
    assert_eq!(transitions[1].affected().deleted(), 0);

    db.execute_batch(MERGE_BRANCHES).unwrap();
    assert_eq!(rows(&native_state(&db, "target")), rows(&after_first));
    assert_eq!(rows(&native_state(&db, "source")), rows(&source));
    db.execute_batch(MERGE_COMPUTED).unwrap();
    assert_eq!(rows(&native_state(&db, "target")), rows(&after_second));
    assert_eq!(rows(&native_state(&db, "source")), rows(&source));
    assert_eq!(
        rows(final_states.iter().find(|state| state.relation() == "target").unwrap()),
        rows(&after_second)
    );

    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
    let domains = after_second.exact_domains(schema).unwrap();
    let id = domains.iter().find(|domain| domain.column() == "id").unwrap();
    let score = domains
        .iter()
        .find(|domain| domain.column() == "score")
        .unwrap();
    let id_frequencies = id
        .values()
        .iter()
        .map(|(value, count)| {
            (
                match value {
                    LiteralValue::Null => "NULL".to_owned(),
                    LiteralValue::Number(value) => value.clone(),
                    other => panic!("unexpected id domain value: {other:?}"),
                },
                *count,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let score_frequencies = score
        .values()
        .iter()
        .map(|(value, count)| {
            (
                match value {
                    LiteralValue::Number(value) => value.clone(),
                    other => panic!("unexpected score domain value: {other:?}"),
                },
                *count,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        id_frequencies,
        BTreeMap::from([
            ("NULL".to_owned(), 1),
            ("1".to_owned(), 1),
            ("3".to_owned(), 1),
        ])
    );
    assert_eq!(
        score_frequencies,
        BTreeMap::from([
            ("200".to_owned(), 1),
            ("500".to_owned(), 1),
            ("600".to_owned(), 1),
        ])
    );
}
