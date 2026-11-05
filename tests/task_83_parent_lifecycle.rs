//! TASK-83: independent parent DDL/DML/read closure at the public physical seam.
//! The native DuckDB snapshots are external oracles, not generated expectations.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_lifecycle_plan, LiteralValue,
    PhysicalOrderedLifecycleProof, SqlInput,
};

fn frozen() -> Value {
    serde_json::from_str(include_str!(
        "../docs/task-83-parent-acceptance-freeze.json"
    ))
    .unwrap()
}

fn analyze(program: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = program
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    analyze_inputs(&inputs, "duckdb", dialect.as_ref()).unwrap()
}

fn native_rows(db: &Connection, sql: &str) -> Vec<(Option<i64>, i64)> {
    let mut statement = db.prepare(sql).unwrap();
    statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn independent_parent_lifecycle_proves_both_reads_and_intermediate_catalogs() {
    let freeze = frozen();
    let case = &freeze["cases"][0];
    assert_eq!(case["id"], "83-PARENT-LIFECYCLE-P");
    assert_eq!(case["classification"], "Exact");
    let program = case["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sql| sql.as_str().unwrap())
        .collect::<Vec<_>>();
    let analysis = analyze(&program);
    let plan = physical_ordered_lifecycle_plan(&analysis, &[]);
    let PhysicalOrderedLifecycleProof::Exact(proof) = plan else {
        panic!("complete physical DDL/read script should be Exact: {plan:?}");
    };

    assert_eq!(proof.checkpoints.len(), program.len());
    assert_eq!(proof.transitions.len(), 5);
    assert_eq!(proof.reads.len(), 2);
    assert!(
        proof.final_states.is_empty(),
        "DROP removes the table identity"
    );
    assert_eq!(proof.checkpoints[0].states.len(), 1);
    assert!(proof.checkpoints[0].states[0].rows().is_empty());
    assert_eq!(proof.checkpoints[1].states[0].rows().len(), 4);
    assert_eq!(proof.checkpoints[2].states[0].rows().len(), 4);
    assert_eq!(proof.checkpoints[4].states.len(), 1);
    assert!(proof.checkpoints[4].states[0].rows().is_empty());
    assert_eq!(proof.checkpoints[4].schemas[0].columns().len(), 2);
    assert!(proof.checkpoints[6].states.is_empty());
    assert!(proof.checkpoints[6].schemas.is_empty());

    let selected = &proof.reads[0];
    assert_eq!(selected.source.rows().len(), 4);
    assert_eq!(selected.output.rows().len(), 1);
    assert_eq!(selected.surviving_source_ordinals, vec![3]);
    assert_eq!(selected.rejected_source_ordinals, vec![0, 1, 2]);
    let domains = selected.exact_domains().unwrap();
    assert_eq!(domains.len(), 2);
    assert_eq!(
        domains[0].values(),
        &[(LiteralValue::Number("3".into()), 1)]
    );
    assert_eq!(
        domains[1].values(),
        &[(LiteralValue::Number("2".into()), 1)]
    );

    // Compare the protocol's typed frequencies, NULLs and inclusive
    // interval endpoints against the independent frozen parent oracle.
    for (read, expected) in proof
        .reads
        .iter()
        .zip([&case["first_read"], &case["second_read"]])
    {
        assert_eq!(
            read.source.rows().len(),
            expected["source_count"].as_u64().unwrap() as usize
        );
        let frozen_rows = expected["rows"].as_array().unwrap();
        assert_eq!(read.output.rows().len(), frozen_rows.len());
        for (actual, expected_row) in read.output.rows().iter().zip(frozen_rows) {
            let values = expected_row
                .as_object()
                .unwrap()
                .iter()
                .map(|(column, value)| {
                    let value = value.as_i64().map_or(LiteralValue::Null, |number| {
                        LiteralValue::Number(number.to_string())
                    });
                    (column.clone(), value)
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(actual.values(), &values, "complete frozen SELECT row");
        }
        for (actual, field) in [
            (&read.surviving_source_ordinals, "true_ordinals"),
            (&read.rejected_source_ordinals, "not_true_ordinals"),
        ] {
            let expected = expected[field]
                .as_array()
                .unwrap()
                .iter()
                .map(|ordinal| ordinal.as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            assert_eq!(actual, &expected, "frozen source membership: {field}");
        }
        let domains = read.exact_domains().unwrap();
        assert_eq!(domains.len(), 2);
        for domain in &domains {
            let target = &expected["typed_domains"][domain.column()];
            assert_eq!(target["type"], "INTEGER");
            assert_eq!(
                domain.values().len(),
                target["frequencies"].as_object().unwrap().len()
            );
            for (value, frequency) in domain.values() {
                let key = match value {
                    LiteralValue::Null => "NULL".to_string(),
                    LiteralValue::Number(number) => number.clone(),
                    other => panic!("unexpected parent INTEGER: {other:?}"),
                };
                assert_eq!(*frequency, target["frequencies"][&key].as_u64().unwrap());
            }
            let numbers = domain
                .values()
                .iter()
                .filter_map(|(value, _)| match value {
                    LiteralValue::Number(number) => Some(number.parse::<i64>().unwrap()),
                    LiteralValue::Null => None,
                    other => panic!("unexpected parent INTEGER: {other:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(numbers.iter().min().copied(), target["lower"].as_i64());
            assert_eq!(numbers.iter().max().copied(), target["upper"].as_i64());
            assert_eq!(
                target["nullable"].as_bool().unwrap(),
                domain
                    .values()
                    .iter()
                    .any(|(value, _)| *value == LiteralValue::Null)
            );
            assert_eq!(
                target["lower_inclusive"].as_bool().unwrap(),
                !numbers.is_empty()
            );
            assert_eq!(
                target["upper_inclusive"].as_bool().unwrap(),
                !numbers.is_empty()
            );
        }
    }

    let emptied = &proof.reads[1];
    assert!(emptied.source.rows().is_empty());
    assert!(emptied.output.rows().is_empty());
    assert!(emptied.surviving_source_ordinals.is_empty());
    assert!(emptied.rejected_source_ordinals.is_empty());
    for domain in emptied.exact_domains().unwrap() {
        assert!(domain.values().is_empty());
    }
    let effects = case["write_effects"].as_array().unwrap();
    assert_eq!(effects.len(), proof.transitions.len());
    for (transition, expected) in proof.transitions.iter().zip(effects) {
        assert_eq!(
            transition.affected().inserted(),
            expected["inserted"].as_u64().unwrap()
        );
        assert_eq!(
            transition.affected().deleted(),
            expected["deleted"].as_u64().unwrap()
        );
        assert_eq!(transition.affected().updated(), 0);
    }
    assert_eq!(
        proof.final_states.len(),
        case["final_live_catalog"].as_array().unwrap().len()
    );

    let db = Connection::open_in_memory().unwrap();
    for statement in &program[..3] {
        db.execute_batch(statement).unwrap();
    }
    let mut rows = native_rows(&db, "SELECT a, b FROM stage");
    rows.sort();
    assert_eq!(
        rows,
        vec![(None, 2), (Some(1), 2), (Some(1), 2), (Some(3), 2)]
    );
    assert_eq!(native_rows(&db, program[3]), vec![(Some(3), 2)]);
    db.execute_batch(program[4]).unwrap();
    assert!(native_rows(&db, program[5]).is_empty());
    let column_count = db
        .query_row(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'stage'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(column_count, 2, "TRUNCATE preserves typed schema");
    db.execute_batch(program[6]).unwrap();
    assert!(db.prepare("SELECT a, b FROM stage").is_err());
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'stage'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
}

#[test]
fn independent_parent_drop_before_read_is_impossible_in_both_engines() {
    let freeze = frozen();
    let case = &freeze["cases"][1];
    assert_eq!(case["id"], "83-PARENT-DROP-READ-N");
    let program = case["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sql| sql.as_str().unwrap())
        .collect::<Vec<_>>();
    let plan = physical_ordered_lifecycle_plan(&analyze(&program), &[]);
    let PhysicalOrderedLifecycleProof::Impossible { reason } = plan else {
        panic!("a proven dropped relation cannot supply a SELECT: {plan:?}");
    };
    assert!(reason.contains(case["expected_reason_contains"].as_str().unwrap()));

    let db = Connection::open_in_memory().unwrap();
    for statement in &program[..3] {
        db.execute_batch(statement).unwrap();
    }
    assert!(db.prepare(program[3]).is_err());
}

#[test]
fn parent_registry_preserves_child_inventories_and_original_ac_ownership() {
    let freeze = frozen();
    assert_eq!(freeze["owner"], "TASK-83");
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["independent_parent_signoff"], false);
    let paths = freeze["child_freezes"].as_array().unwrap();
    assert_eq!(paths.len(), 4);
    let children: [Value; 4] = [
        serde_json::from_str(include_str!("../docs/task-157-acceptance-freeze.json")).unwrap(),
        serde_json::from_str(include_str!("../docs/task-158-acceptance-freeze.json")).unwrap(),
        serde_json::from_str(include_str!("../docs/task-159-acceptance-freeze.json")).unwrap(),
        serde_json::from_str(include_str!("../docs/task-160-acceptance-freeze.json")).unwrap(),
    ];
    let mut child_ids = BTreeSet::new();
    let mut case_ids = BTreeSet::new();
    for (index, child) in children.iter().enumerate() {
        let id = format!("TASK-{}", index + 157);
        assert_eq!(child["owner"], id);
        assert_eq!(child["parent"], "TASK-83");
        assert_eq!(
            paths[index],
            format!("docs/task-{}-acceptance-freeze.json", index + 157)
        );
        assert_eq!(child["dialects"].as_array().unwrap().len(), 13);
        child_ids.insert(id);
        for case in child["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            assert!(id.starts_with("83-C"));
            assert!(case_ids.insert(id), "duplicate child fixture: {id}");
        }
    }
    assert_eq!(case_ids.len(), 42, "preserve all 42 child-frozen cases");

    let mapping = freeze["original_parent_ac_mapping"].as_object().unwrap();
    assert_eq!(mapping.len(), 6);
    for ac in 1..=6 {
        let owners = mapping[&ac.to_string()].as_array().unwrap();
        assert!(!owners.is_empty());
        for owner in owners {
            assert!(child_ids.contains(owner.as_str().unwrap()));
        }
    }
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0]["requirement"], "mandatory");
    assert_eq!(cases[1]["requirement"], "mandatory");
}
