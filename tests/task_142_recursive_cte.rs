//! TASK-142: canonical finite recursion and independent DuckDB bag oracles.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, RelationCatalog, SqlInput,
};

fn bundle(sql: &str, dialect_name: &str) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name(dialect_name).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "recursive",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let catalog = RelationCatalog::from_schemas(&[]).unwrap();
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

fn requested(value: i64) -> BTreeMap<String, LiteralValue> {
    BTreeMap::from([("x".to_string(), LiteralValue::Number(value.to_string()))])
}

#[test]
fn guarded_recursive_union_proves_complete_bags_and_absence() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-142-acceptance-freeze.json")).unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    let mut checked = 0;
    for case in cases {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        checked += 1;
        let sql = case["sql"].as_str().unwrap();
        let bundle = bundle(sql, "duckdb");
        let target = bundle.layers().last().unwrap();
        let PhysicalCteProof::Exact(proof) =
            physical_finite_cte_bag_plan(&bundle, target.id(), &[], None)
        else {
            panic!("{}: missing complete guarded recursion proof", case["id"]);
        };
        assert!(proof.sources.is_empty(), "source-free seed");
        let expected = case["expected_terminal_bag"].as_array().unwrap();
        assert_eq!(proof.terminal.len(), expected.len());
        for (index, row) in proof.terminal.iter().enumerate() {
            assert_eq!(
                row.values["x"],
                LiteralValue::Number(expected[index].as_i64().unwrap().to_string())
            );
            assert_eq!(
                row.source_row_ids,
                vec![case["expected_source_row_ids"][index]
                    .as_str()
                    .unwrap()
                    .to_string()]
            );
        }
        let domain = &proof.terminal_domains["x"];
        assert!(!domain.null_allowed);
        assert_eq!(domain.integer_bounds, Some((1, 3)));
        assert_eq!(proof.false_pairs["terminal/cte:0:n"].len(), 1);
        let positive = requested(case["positive"]["tuple"]["x"].as_i64().unwrap());
        let absent = requested(case["not_true"]["tuple"]["x"].as_i64().unwrap());
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle, target.id(), &[], Some(&positive)),
            PhysicalCteProof::Exact(_)
        ));
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle, target.id(), &[], Some(&absent)),
            PhysicalCteProof::Impossible { .. }
        ));

        // Independent native engine oracle: exact values, duplicates, and absence.
        let db = Connection::open_in_memory().unwrap();
        let mut stmt = db.prepare(sql).unwrap();
        let physical = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            physical,
            expected
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>()
        );
        assert!(!physical.contains(&case["not_true"]["tuple"]["x"].as_i64().unwrap()));
    }
    assert_eq!(checked, 2);
}

#[test]
fn cyclic_recursive_union_all_stays_residual() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-142-acceptance-freeze.json")).unwrap();
    let cycle = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "78-C142-N-CYCLE")
        .unwrap();
    let bundle = bundle(cycle["sql"].as_str().unwrap(), "duckdb");
    let target = bundle.layers().last().unwrap();
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, target.id(), &[], Some(&requested(1))),
        PhysicalCteProof::Residual { .. }
    ));
}

#[test]
fn recursive_overflow_and_large_physical_bags_fail_closed() {
    for sql in [
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 20000) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 0 FROM n WHERE x < 3) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x - 1 FROM n WHERE x < 3) SELECT x FROM n",
    ] {
        let bundle = bundle(sql, "duckdb");
        let target = bundle.layers().last().unwrap();
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle, target.id(), &[], None),
            PhysicalCteProof::Residual { .. }
        ));
    }
}

#[test]
fn frozen_negative_case_is_closed_world_impossible_in_protocol_and_duckdb() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-142-acceptance-freeze.json")).unwrap();
    let negative = freeze["cases"].as_array().unwrap().iter()
        .find(|case| case["id"] == "78-C142-N-ABSENCE").unwrap();
    let sql = negative["sql"].as_str().unwrap();
    let wanted = negative["wanted"]["x"].as_i64().unwrap();
    let bundle = bundle(sql, "duckdb");
    let target = bundle.layers().last().unwrap();
    let PhysicalCteProof::Exact(proof) =
        physical_finite_cte_bag_plan(&bundle, target.id(), &[], None)
    else {
        panic!("mandatory negative fixture must have a complete source-free bag");
    };
    let source_values = proof.stage_bags["terminal/cte:0:n"].iter()
        .map(|row| row.values["x"].clone()).collect::<Vec<_>>();
    assert_eq!(source_values, vec![
        LiteralValue::Number("1".into()),
        LiteralValue::Number("2".into()),
        LiteralValue::Number("3".into())
    ]);
    assert_eq!(proof.terminal_domains["x"].integer_bounds, Some((1, 3)));
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, target.id(), &[], Some(&requested(wanted))),
        PhysicalCteProof::Impossible { .. }
    ));
    assert!(matches!(
        physical_finite_cte_bag_plan(
            &bundle, target.id(), &[],
            Some(&BTreeMap::from([("x".to_string(), LiteralValue::Null)]))
        ),
        PhysicalCteProof::Impossible { .. }
    ));

    let db = Connection::open_in_memory().unwrap();
    let all_rows = db.prepare(sql).unwrap()
        .query_map([], |row| row.get::<_, i64>(0)).unwrap()
        .collect::<duckdb::Result<Vec<_>>>().unwrap();
    assert_eq!(all_rows, vec![1, 2, 3]);
    let absent_sql = format!("SELECT NOT EXISTS (SELECT 1 FROM ({sql}) AS finite WHERE x = {wanted})");
    let absent: bool = db.query_row(&absent_sql, [], |row| row.get(0)).unwrap();
    assert!(absent, "the independent SQL closed-world nonmembership oracle");
    let unknown_sql = format!("SELECT COUNT(*) FROM ({sql}) AS finite WHERE x = NULL");
    let matched_unknown: i64 = db.query_row(&unknown_sql, [], |row| row.get(0)).unwrap();
    assert_eq!(matched_unknown, 0, "SQL UNKNOWN is NOT TRUE, not a matching row");
}
