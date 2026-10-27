//! TASK-145 frozen source/target and SQL TRUTH acceptance matrix.
//! Every frozen case is checked via the public API against an independent DuckDB bag.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, ConstraintValue, ColumnValueKind, ColumnValueSource, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-145-acceptance-freeze.json"))
        .expect("frozen TASK-145 matrix")
}

fn bag_row(row: &Value) -> (Option<i64>, Option<i64>) {
    (row["id"].as_i64(), row["score"].as_i64())
}

fn physical_row(pair: (Option<i64>, Option<i64>)) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".into(), pair.0.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()))),
        ("score".into(), pair.1.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()))),
    ])).unwrap()
}

fn state(relation: &str, bag: &[Value]) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, bag.iter().map(|r| physical_row(bag_row(r))).collect()).unwrap()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-145").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn prepared(case: &Value) -> (sql_semantic_protocol::AnalysisBundle, Vec<PhysicalWriteState>) {
    let schemas = ["target", "empty_source"].into_iter().map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "score"].into_iter().map(|name| {
                SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap()
            }).collect(),
        ).unwrap()
    }).collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let configured = ConfiguredSqlInput::new("task-145-matrix", &sql, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap();
    let id = case["id"].as_str().unwrap();
    let mut constraints = Vec::new();
    if id == "80-C145-N-unique-violation" {
        constraints.push(RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap());
    }
    let mut set = RelationConstraintSet::new("target", constraints).unwrap();
    let defaults = if id == "80-C145-P-default-values" {
        vec![("id", 5_i64), ("score", 7_i64)]
    } else if id == "80-C145-P-default-omission" {
        vec![("score", 7_i64)]
    } else {
        vec![]
    };
    if !defaults.is_empty() {
        set = set.with_column_value_sources(defaults.into_iter().map(|(column, value)| {
            ColumnValueSource::new(
                column, ColumnValueKind::Default, Some(ConstraintValue::Integer(value)), evidence()
            ).unwrap()
        }).collect());
    }
    if id == "80-C145-N-unique-violation" || id.contains("default-") {
        bundle.enrich_relation_constraints(&[set]);
    }
    let mut snapshots = vec![state("target", case["prior_target_bag"].as_array().unwrap())];
    if id == "80-C145-P-zero-source" {
        snapshots.push(PhysicalWriteState::new("empty_source", vec![]).unwrap());
    }
    (bundle, snapshots)
}

fn duckdb_rows(db: &Connection) -> Vec<(Option<i64>, Option<i64>)> {
    let mut statement = db.prepare("SELECT id, score FROM target").unwrap();
    statement.query_map([], |row| {
        Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
    }).unwrap().map(|r| r.unwrap()).collect()
}

fn oracle(case: &Value) -> Vec<(Option<i64>, Option<i64>)> {
    let id = case["id"].as_str().unwrap();
    let db = Connection::open_in_memory().unwrap();
    let ddl = match id {
        "80-C145-P-default-values" => "CREATE TABLE target(id INTEGER DEFAULT 5, score INTEGER DEFAULT 7)",
        "80-C145-P-default-omission" => "CREATE TABLE target(id INTEGER, score INTEGER DEFAULT 7)",
        "80-C145-N-unique-violation" => "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER)",
        _ => "CREATE TABLE target(id INTEGER, score INTEGER)",
    };
    db.execute_batch(ddl).unwrap();
    db.execute_batch("CREATE TABLE empty_source(id INTEGER, score INTEGER)").unwrap();
    for row in case["prior_target_bag"].as_array().unwrap() {
        let (id, score) = bag_row(row);
        let literal = |v: Option<i64>| v.map_or("NULL".into(), |v| v.to_string());
        db.execute_batch(&format!("INSERT INTO target VALUES ({}, {})", literal(id), literal(score))).unwrap();
    }
    let pre = duckdb_rows(&db);
    assert_eq!(pre.len(), case["prior_target_bag"].as_array().unwrap().len());
    let sql = case["sql"].as_str().unwrap();
    if id == "80-C145-N-unique-violation" {
        assert!(db.execute_batch(sql).is_err(), "{sql}: expected PK rejection");
    } else {
        db.execute_batch(sql).unwrap_or_else(|error| panic!("{sql}: {error}"));
    }
    duckdb_rows(&db)
}

fn sorted(mut bag: Vec<(Option<i64>, Option<i64>)>) -> Vec<(Option<i64>, Option<i64>)> {
    bag.sort_unstable();
    bag
}

#[test]
fn every_frozen_mandatory_cell_matches_full_duckdb_bag_and_typed_domain() {
    let matrix = freeze();
    assert_eq!(matrix["owner"], "TASK-145");
    let mut seen = BTreeSet::new();
    let mut exact_count = 0;
    let mut impossible_count = 0;
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert!(seen.insert(id), "duplicate fixture {id}");
        assert_eq!(case["owner"], "TASK-145");
        assert_eq!(case["column_instances"], serde_json::json!(["target.id", "target.score"]));
        if case["classification"] != "mandatory" {
            continue;
        }
        let expected = case["expected_target_bag"].as_array().unwrap()
            .iter().map(bag_row).collect::<Vec<_>>();
        let native = oracle(case);
        assert_eq!(sorted(native.clone()), sorted(expected.clone()), "{id}: independent native bag");
        let (bundle, snapshots) = prepared(case);
        let result = physical_ordered_write_state_plan(&bundle, &snapshots);
        if case["proof"] == "Impossible" {
            assert!(matches!(result, PhysicalWriteStatePlan::Impossible { .. }), "{id}: {result:?}");
            impossible_count += 1;
            continue;
        }
        let PhysicalWriteStatePlan::Feasible { transitions, final_states } = result else {
            panic!("{id}: exact physical plan required: {result:?}");
        };
        exact_count += 1;
        assert_eq!(transitions.len(), 1, "{id}");
        let transition = &transitions[0];
        let input = snapshots.iter().find(|s| s.relation() == "target").unwrap();
        assert_eq!(transition.before().rows(), input.rows(), "{id}: complete prestate");
        let target = final_states.iter().find(|s| s.relation() == "target").unwrap();
        assert_eq!(transition.after(), target, "{id}: complete terminal state");
        let physical = target.rows().iter().map(|r| {
            let get = |name: &str| match r.values().get(name).unwrap() {
                LiteralValue::Number(v) => Some(v.parse::<i64>().unwrap()),
                LiteralValue::Null => None,
                v => panic!("{id}: unsafe value {v:?}"),
            };
            (get("id"), get("score"))
        }).collect::<Vec<_>>();
        assert_eq!(physical, expected, "{id}: complete physical ordered rows");
        assert_eq!(transition.affected().inserted() as usize, physical.len()-input.rows().len(), "{id}");
        assert_eq!(transition.affected().updated(), 0);
        assert_eq!(transition.affected().deleted(), 0);
        let source_row_ids: Vec<String> = (0..physical.len()).map(|i| format!("target#{i}")).collect();
        assert_eq!(serde_json::json!(source_row_ids), case["source_row_ids"], "{id}: stable row IDs");
        // Full bag frequencies, NULL, inclusive bounds, and NOT TRUE absent-row witness.
        let schema = bundle.source_schemas().iter().find(|s| s.relation() == "target").unwrap();
        let domains = target.exact_domains(schema).unwrap();
        for domain in domains {
            let name = domain.column();
            let attested = &case["domain"][name];
            let mut counts = BTreeMap::<Option<i64>, u64>::new();
            for (value, n) in domain.values() {
                let key = match value {
                    LiteralValue::Number(v) => Some(v.parse::<i64>().unwrap()),
                    LiteralValue::Null => None,
                    _ => panic!("noninteger domain"),
                };
                counts.insert(key, *n);
            }
            let mut expected_counts = BTreeMap::<Option<i64>, u64>::new();
            for pair in &physical {
                let key = if name == "id" { pair.0 } else { pair.1 };
                *expected_counts.entry(key).or_default() += 1;
            }
            assert_eq!(counts, expected_counts, "{id}: domain count");
            assert_eq!(attested["nullable"], expected_counts.contains_key(&None), "{id}:{name} NULL");
            let values = attested["values"].as_array().unwrap().iter().map(Value::as_i64).collect::<BTreeSet<_>>();
            assert_eq!(values, expected_counts.keys().copied().collect(), "{id}:{name} domain set");
            if let Some(bounds) = attested["bounds"].as_object() {
                let numeric = expected_counts.keys().filter_map(|x| *x).collect::<Vec<_>>();
                if !numeric.is_empty() {
                    assert_eq!(bounds["lower"], *numeric.iter().min().unwrap());
                    assert_eq!(bounds["upper"], *numeric.iter().max().unwrap());
                    assert_eq!(bounds["inclusive"], serde_json::json!([true,true]));
                }
            }
        }
        assert!(!physical.contains(&(Some(999999),Some(999999))), "{id}: NOT TRUE physical absence");
        assert!(!native.contains(&(Some(999999),Some(999999))), "{id}: NOT TRUE native absence");
        for other in &snapshots {
            if other.relation() != "target" {
                let after = final_states.iter().find(|s| s.relation()==other.relation()).unwrap();
                assert_eq!(after, other, "{id}: untouched physical source");
            }
        }
    }
    assert_eq!(seen.len(), 12);
    assert_eq!(exact_count, 8);
    assert_eq!(impossible_count, 1);
}

#[test]
fn every_frozen_conditional_case_fails_closed() {
    let matrix = freeze();
    let mut n = 0;
    for case in matrix["cases"].as_array().unwrap() {
        if case["classification"] != "conditional" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        assert_eq!(case["release_disposition"], "conditional_residual_pending_independent_review");
        let (bundle, snapshots) = prepared(case);
        let result = physical_ordered_write_state_plan(&bundle, &snapshots);
        assert!(matches!(result, PhysicalWriteStatePlan::Residual { .. }), "{id}: {result:?}");
        n += 1;
    }
    assert_eq!(n, 3);
}
