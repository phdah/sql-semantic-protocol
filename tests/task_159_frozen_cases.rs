//! TASK-159 frozen end-to-end complete-bag oracle, owned by this protocol repo.
//! Expected values come from the precommitted acceptance contract, never the analyzer.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_write_state_plan, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, SqlInput,
};
use std::collections::{BTreeMap, BTreeSet};

fn matrix() -> Value {
    serde_json::from_str(include_str!("../docs/task-159-acceptance-freeze.json")).unwrap()
}
fn physical_rows(rows: &Value) -> Vec<PhysicalWriteRow> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let values = row
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    let value = match v {
                        Value::Null => LiteralValue::Null,
                        Value::Number(n) => LiteralValue::Number(n.to_string()),
                        other => panic!("unfrozen value {other:?}"),
                    };
                    (k.clone(), value)
                })
                .collect();
            PhysicalWriteRow::new(values).unwrap()
        })
        .collect()
}
fn bag(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut out = rows
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    out.sort();
    out
}
fn native_exists(conn: &Connection, relation: &str) -> bool {
    conn.query_row(
        "SELECT count(*) FROM information_schema.tables WHERE table_name = ?",
        [relation],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}
fn native_columns(conn: &Connection, relation: &str) -> Vec<String> {
    let mut query = conn
        .prepare(
            "SELECT column_name, data_type FROM information_schema.columns              WHERE table_name = ? ORDER BY ordinal_position"
        )
        .unwrap();
    query
        .query_map([relation], |row| {
            let name: String = row.get(0)?;
            let datatype: String = row.get(1)?;
            Ok(format!("{name} {datatype}"))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn native_rows(conn: &Connection, relation: &str) -> Vec<PhysicalWriteRow> {
    let mut q = conn.prepare(&format!("SELECT a FROM {relation}")).unwrap();
    q.query_map([], |r| {
        let a: Option<i64> = r.get(0)?;
        Ok(PhysicalWriteRow::new(BTreeMap::from([(
            "a".to_string(),
            a.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        )]))
        .unwrap())
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}
fn assert_domain(
    state: &PhysicalWriteState,
    schema: &sql_semantic_protocol::RelationSchema,
    expected: &Value,
) {
    let domain = state.exact_domains(schema).unwrap();
    assert_eq!(domain.len(), 1);
    let frequencies = domain[0]
        .values()
        .iter()
        .map(|(v, n)| {
            let k = match v {
                LiteralValue::Number(n) => n.clone(),
                LiteralValue::Null => "NULL".to_string(),
                other => panic!("unfrozen domain {other:?}"),
            };
            (k, *n)
        })
        .collect::<BTreeMap<_, _>>();
    let expected_freq = expected["a"]["frequencies"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.to_owned(), v.as_u64().unwrap()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(frequencies, expected_freq, "exact typed frequency mismatch");
    assert_eq!(
        frequencies.contains_key("NULL"),
        expected["a"]["nullable"].as_bool().unwrap()
    );
    let nums = frequencies
        .keys()
        .filter(|k| k.as_str() != "NULL")
        .map(|k| k.parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(nums.iter().min().copied(), expected["a"]["lower"].as_i64());
    assert_eq!(nums.iter().max().copied(), expected["a"]["upper"].as_i64());
    assert_eq!(
        !nums.is_empty(),
        expected["a"]["lower_inclusive"].as_bool().unwrap()
    );
    assert_eq!(
        !nums.is_empty(),
        expected["a"]["upper_inclusive"].as_bool().unwrap()
    );
    assert_eq!(expected["a"]["type"], "INTEGER");
}

#[test]
fn every_mandatory_frozen_lifecycle_cell_has_independent_native_and_physical_oracles() {
    let matrix = matrix();
    assert_eq!(matrix["owner"], "TASK-159");
    assert_eq!(matrix["version"], 3);
    assert_eq!(matrix["parent"], "TASK-83");
    let cases = matrix["cases"].as_array().unwrap();
    let mut seen = BTreeSet::new();
    let mut exact = 0;
    let mut impossible = 0;
    for case in cases.iter().filter(|case| case["mandatory"] == true) {
        let id = case["id"].as_str().unwrap();
        assert!(seen.insert(id));
        let program = case["program"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sql| sql.as_str().unwrap())
            .collect::<Vec<_>>();
        let parser = dialect_from_name("duckdb").unwrap();
        let inputs = program
            .iter()
            .map(|sql| SqlInput::inline(*sql))
            .collect::<Vec<_>>();
        let analyzed = analyze_inputs(&inputs, "duckdb", parser.as_ref()).unwrap();
        let db = Connection::open_in_memory().unwrap();
        let expected = case["expected_outcome"].as_str().unwrap();
        let physical = physical_ordered_write_state_plan(&analyzed, &[]);

        if expected == "Impossible" {
            impossible += 1;
            assert!(
                matches!(physical, PhysicalWriteStatePlan::Impossible { .. }),
                "{id}"
            );
            for sql in &program[..program.len() - 1] {
                db.execute_batch(sql).unwrap();
            }
            assert!(
                db.execute_batch(program[program.len() - 1]).is_err(),
                "{id}"
            );
            let expected_states = case["expected_unchanged_states"].as_array();
            if let Some(states) = expected_states {
                for state in states {
                    let relation = state["relation"].as_str().unwrap();
                    assert!(
                        native_exists(&db, relation),
                        "{id}: impossible operation removed {relation}"
                    );
                    assert_eq!(
                        bag(&native_rows(&db, relation)),
                        bag(&physical_rows(&state["rows"]))
                    );
                }
            } else {
                let relation = case["before_relation"].as_str().unwrap_or("t");
                assert!(
                    !native_exists(&db, relation),
                    "{id}: impossible operation changed target"
                );
            }
            continue;
        }
        assert_eq!(expected, "Exact", "{id}");
        exact += 1;
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = physical
        else {
            panic!("{id}: required portable Exact proof was not constructed");
        };
        assert_eq!(transitions.len(), program.len(), "{id}: ordered SQL layers");
        let last = transitions.last().unwrap();
        assert_eq!(
            last.before_exists(),
            case["before_exists"].as_bool().unwrap(),
            "{id}"
        );
        assert_eq!(
            last.after_exists(),
            case["after_exists"].as_bool().unwrap(),
            "{id}"
        );
        assert_eq!(
            last.affected().inserted(),
            case["affected"]["inserted"].as_u64().unwrap()
        );
        assert_eq!(
            last.affected().updated(),
            case["affected"]["updated"].as_u64().unwrap()
        );
        assert_eq!(
            last.affected().deleted(),
            case["affected"]["deleted"].as_u64().unwrap()
        );
        let ordinals = case["affected_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        assert_eq!(
            last.affected_target_ordinals(),
            ordinals,
            "{id}: affected row occurrences"
        );

        for (index, sql) in program.iter().enumerate() {
            let step = &transitions[index];
            if step.before_exists() {
                assert!(
                    native_exists(&db, step.before().relation()),
                    "{id}: before #{index}"
                );
                assert_eq!(
                    bag(step.before().rows()),
                    bag(&native_rows(&db, step.before().relation())),
                    "{id}: pre #{index}"
                );
            }
            db.execute_batch(sql).unwrap();
            if step.after_exists() {
                assert!(
                    native_exists(&db, step.after().relation()),
                    "{id}: after #{index}"
                );
                assert_eq!(
                    bag(step.after().rows()),
                    bag(&native_rows(&db, step.after().relation())),
                    "{id}: post #{index}"
                );
            } else {
                assert!(
                    !native_exists(&db, step.after().relation()),
                    "{id}: absent after #{index}"
                );
            }
        }
        let expected_pre = case["prestate"]
            .is_array()
            .then(|| physical_rows(&case["prestate"]));
        assert_eq!(
            bag(last.before().rows()),
            bag(&expected_pre.unwrap_or_default()),
            "{id}: frozen prestate"
        );
        if case["before_exists"] == true {
            assert_domain(
                last.before(),
                last.before_schema().unwrap(),
                &case["pre_typed_domains"],
            );
        }
        if case["after_exists"] == true {
            let expected_post = physical_rows(&case["poststate"]);
            assert_eq!(
                bag(last.after().rows()),
                bag(&expected_post),
                "{id}: poststate"
            );
            assert_domain(
                last.after(),
                last.after_schema().unwrap(),
                &case["post_typed_domains"],
            );
            let native = native_columns(&db, case["after_relation"].as_str().unwrap());
            let expected = case["after_schema"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_string())
                .collect::<Vec<_>>();
            assert_eq!(native, expected, "{id}: native final schema");
            assert_eq!(final_states.len(), 1, "{id}");
            assert_eq!(
                bag(final_states[0].rows()),
                bag(&expected_post),
                "{id}: terminal bag"
            );
            assert_eq!(
                final_states[0].relation(),
                case["after_relation"].as_str().unwrap()
            );
        } else {
            assert!(
                final_states.is_empty(),
                "{id}: absent table is not an empty table"
            );
            assert!(last.after_schema().is_none());
        }
        let all = (0..last.before().rows().len())
            .map(|i| Value::from(i as u64))
            .collect::<Vec<_>>();
        assert_eq!(
            case["source_truth"]["true_ordinals"],
            Value::from(all),
            "{id}: positive truth"
        );
        assert_eq!(
            case["source_truth"]["not_true_ordinals"],
            serde_json::json!([]),
            "{id}"
        );
        assert_eq!(
            case["source_truth"]["unknown_ordinals"],
            serde_json::json!([]),
            "{id}"
        );
    }
    assert_eq!(exact, 6, "every mandatory Exact cell must be replayed");
    assert_eq!(
        impossible, 2,
        "every mandatory Impossible cell must be refuted"
    );
    assert_eq!(seen.len(), 8);
}
