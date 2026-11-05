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

fn integer_or_null(value: &LiteralValue) -> Option<i64> {
    match value {
        LiteralValue::Null => None,
        LiteralValue::Number(number) => Some(number.parse().unwrap()),
        other => panic!("unproved physical INTEGER: {other:?}"),
    }
}

#[test]
fn every_parent_checkpoint_matches_native_catalog_schema_and_complete_bag() {
    let freeze = frozen();
    let program = freeze["cases"][0]["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|statement| statement.as_str().unwrap())
        .collect::<Vec<_>>();
    let proof = physical_ordered_lifecycle_plan(&analyze(&program), &[]);
    let PhysicalOrderedLifecycleProof::Exact(proof) = proof else {
        panic!("parent exact lifecycle required: {proof:?}");
    };
    let db = Connection::open_in_memory().unwrap();

    for (index, sql) in program.iter().enumerate() {
        // A SELECT reads the current state, but never mutates the catalog.
        if index != 3 && index != 5 {
            db.execute_batch(sql).unwrap();
        }
        let checkpoint = &proof.checkpoints[index];
        let mut statement = db
            .prepare(
                "SELECT column_name, data_type FROM information_schema.columns \
                 WHERE table_name = 'stage' ORDER BY ordinal_position",
            )
            .unwrap();
        let native_columns = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();

        if index == 6 {
            assert!(checkpoint.states.is_empty());
            assert!(checkpoint.schemas.is_empty());
            assert!(native_columns.is_empty(), "DROP removes the catalog");
            assert!(db.prepare("SELECT * FROM stage").is_err());
            continue;
        }
        assert_eq!(checkpoint.states.len(), 1);
        assert_eq!(checkpoint.schemas.len(), 1);
        let schema = &checkpoint.schemas[0];
        let expected_columns = schema
            .columns()
            .iter()
            .map(|column| column.name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            native_columns
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>(),
            expected_columns,
            "step {index}: authoritative schema and native catalog"
        );
        assert!(
            native_columns.iter().all(|(_, kind)| kind == "INTEGER"),
            "step {index}: independent typed native schema"
        );

        let state = &checkpoint.states[0];
        if native_columns.len() == 1 {
            let mut stmt = db.prepare("SELECT a FROM stage").unwrap();
            let mut actual = stmt
                .query_map([], |row| row.get::<_, Option<i64>>(0))
                .unwrap()
                .collect::<duckdb::Result<Vec<_>>>()
                .unwrap();
            let mut expected = state
                .rows()
                .iter()
                .map(|row| integer_or_null(&row.values()["a"]))
                .collect::<Vec<_>>();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "step {index}: physical a bag");
        } else {
            let mut actual = native_rows(&db, "SELECT a, b FROM stage");
            let mut expected = state
                .rows()
                .iter()
                .map(|row| {
                    (
                        integer_or_null(&row.values()["a"]),
                        integer_or_null(&row.values()["b"]).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "step {index}: complete (a,b) bag");
        }
    }
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
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let records = [
        ("task_157_ddl_identity_acceptance", [7, 1, 1]),
        ("task_158_alter_column_transition_acceptance", [4, 2, 2]),
        ("task_159_ddl_lifecycle_acceptance", [6, 2, 10]),
        ("task_160_ordered_lifecycle_acceptance", [3, 2, 2]),
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
        let record = &manifest[records[index].0];
        assert_eq!(record["owner"], child["owner"]);
        assert_eq!(record["child_signed_off"], true);
        assert_eq!(record["mandatory_cells_verified"], true);
        assert!(
            record["ci_verified"] == true
                || record["implementation_ci_verified"] == true,
            "child {index} lacks a recorded verified implementation head"
        );
        let mut strengths = [0usize; 3];
        for case in child["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            assert!(id.starts_with("83-C"));
            assert!(case_ids.insert(id), "duplicate child fixture: {id}");
            let outcome = case["expected_outcome"]
                .as_str()
                .or_else(|| case["classification"].as_str())
                .unwrap();
            let rank = match outcome {
                "Exact" => 0,
                "Impossible" => 1,
                "Residual" => 2,
                other => panic!("{id}: unclassified child proof outcome {other}"),
            };
            strengths[rank] += 1;
        }
        assert_eq!(
            strengths, records[index].1,
            "child {index}: original exact/impossible/residual evidence changed"
        );
    }
    assert_eq!(case_ids.len(), 42, "preserve all 42 child-frozen cases");

    let mapping = freeze["original_parent_ac_mapping"].as_object().unwrap();
    assert_eq!(mapping.len(), 6);
    for ac in 1..=6 {
        let owners = mapping[&ac.to_string()].as_array().unwrap();
        assert!(!owners.is_empty());
        for owner in owners {
            let owner = owner.as_str().unwrap();
            assert!(child_ids.contains(owner));
            let child = children
                .iter()
                .find(|child| child["owner"] == owner)
                .unwrap();
            let original_ac = child["original_parent_ac"]
                .as_array()
                .or_else(|| child["original_parent_ac_mapping"].as_array())
                .unwrap();
            assert!(
                original_ac.iter().any(|covered| covered.as_u64() == Some(ac)),
                "{owner} has no original parent AC {ac} evidence"
            );
        }
    }
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0]["requirement"], "mandatory");
    assert_eq!(cases[1]["requirement"], "mandatory");
}
