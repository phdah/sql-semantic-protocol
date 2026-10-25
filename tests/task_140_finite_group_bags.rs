//! TASK-140 public complete-bag proof for finite fanout followed by grouping.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, FiniteColumnType, LiteralValue,
    PhysicalFiniteTableProof, SetMode, SqlInput, ValueDomain,
};

fn analyzed(sql: &str, dialect_name: &str) -> (sql_semantic_protocol::AnalysisBundle, String) {
    let dialect = dialect_from_name(dialect_name).expect("exposed parser dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref())
        .expect("valid query");
    let target = bundle
        .layers()
        .last()
        .expect("one terminal layer")
        .id()
        .to_string();
    (bundle, target)
}

fn source_free(sql: &str) -> PhysicalFiniteTableProof {
    let (bundle, target) = analyzed(sql, "duckdb");
    physical_finite_table_plan(&bundle, &target, None)
}

fn duckdb_bag(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("independent DuckDB");
    let mut stmt = db.prepare(sql).expect("valid native oracle SQL");
    stmt.query_map([], |row| {
        let v = row.get::<_, Option<i64>>(0)?;
        let n = row.get::<_, i64>(1)?;
        Ok(BTreeMap::from([
            (
                "v".to_string(),
                v.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
            ),
            ("n".to_string(), LiteralValue::Number(n.to_string())),
        ]))
    })
    .expect("native rows")
    .collect::<Result<Vec<_>, _>>()
    .expect("complete native bag")
}

fn native_source_bag(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("native DuckDB");
    let mut stmt = db.prepare(sql).expect("literal physical source SQL");
    stmt.query_map([], |r| {
        let v = r.get::<_, Option<i64>>(0)?;
        Ok(BTreeMap::from([(
            "v".to_string(),
            v.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        )]))
    })
    .expect("native complete physical rows")
    .collect::<Result<Vec<_>, _>>()
    .expect("all source rows")
}

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut keys = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

#[test]
fn frozen_inventory_names_both_directions_and_join_and_adapter_obligations() {
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-140-acceptance-freeze.json"))
            .expect("versioned task-owned acceptance inventory");
    assert_eq!(matrix["owner"], "TASK-140");
    assert_eq!(
        matrix["fixture_ids"],
        serde_json::json!(["77-C140-P", "77-C140-N"])
    );
    assert_eq!(matrix["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(matrix["adapter_sources"].as_array().unwrap().len(), 3);
    assert!(matrix["variants"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["family"] == "unnest_join_group"));
    assert_eq!(matrix["verification"]["task_done"], true);
    assert_eq!(matrix["verification"]["semantic_exact_cases_verified"], 7);
    assert_eq!(matrix["verification"]["parser_cells_currently_certified"], 117);
    assert_eq!(matrix["verification"]["adapter_cells_currently_certified"], 27);
}

#[test]
fn finite_unnest_duplicates_group_into_exact_counts_with_complete_source_identities() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) GROUP BY v";
    let PhysicalFiniteTableProof::Exact(proof) = source_free(sql) else {
        panic!("closed literal UNNEST followed by GROUP BY should be exact");
    };
    assert_eq!(proof.source_row_ids, ["unnest:1", "unnest:2", "unnest:3"]);
    assert_eq!(
        proof.contributors,
        [
            ["unnest:1".to_string(), "unnest:2".to_string()].to_vec(),
            ["unnest:3".to_string()].to_vec()
        ]
    );
    assert_eq!(proof.terminal.len(), 2);
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
    assert_eq!(proof.terminal_column_types["v"], FiniteColumnType::Integer);
    assert_eq!(proof.terminal_column_types["n"], FiniteColumnType::Integer);
    assert!(proof
        .terminal_domains
        .values()
        .all(|value| !matches!(value, sql_semantic_protocol::ValueDomain::Unknown(_))));
}

#[test]
fn absent_group_is_impossible_from_complete_finite_source() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) GROUP BY v";
    let (bundle, target) = analyzed(sql, "duckdb");
    let wanted = BTreeMap::from([
        ("v".to_string(), LiteralValue::Number("3".to_string())),
        ("n".to_string(), LiteralValue::Number("1".to_string())),
    ]);
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, Some(&wanted)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn finite_values_group_preserves_null_as_one_group_and_duplicate_producers() {
    let sql = "SELECT v, COUNT(*) AS n FROM (VALUES (1),(1),(NULL)) AS t(v) GROUP BY v";
    let PhysicalFiniteTableProof::Exact(proof) = source_free(sql) else {
        panic!("complete VALUES bag with NULL grouping must be exact");
    };
    assert_eq!(proof.source_row_ids, ["values:0", "values:1", "values:2"]);
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
    assert!(proof
        .terminal
        .iter()
        .any(|row| row["v"] == LiteralValue::Null
            && row["n"] == LiteralValue::Number("1".to_string())));
    assert_eq!(proof.contributors.iter().map(Vec::len).sum::<usize>(), 3);
}

#[test]
fn opaque_fanout_cannot_claim_exact_closed_world_membership() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST(opaque_array) AS t(v) GROUP BY v";
    let (bundle, target) = analyzed(sql, "duckdb");
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, None),
        PhysicalFiniteTableProof::Residual { .. }
    ));
}

#[test]
fn joined_finite_fanout_proves_complete_bags_pairs_and_typed_terminal_domains() {
    let sql = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v GROUP BY t.v";
    let p = source_free(sql);
    let PhysicalFiniteTableProof::Exact(proof) = p else {
        panic!("two-source closed INNER equality bag must be exact: {p:?}");
    };
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
    assert_eq!(proof.source_row_ids.len(), 5, "five matched JOIN producers");
    assert_eq!(proof.contributors.iter().map(Vec::len).sum::<usize>(), 5);
    assert_eq!(
        proof.physical_input_ids["left"],
        ["left/unnest:1", "left/unnest:2", "left/unnest:3"]
    );
    assert_eq!(
        proof.physical_input_ids["right"],
        ["right/values:0", "right/values:1", "right/values:2"]
    );
    assert_eq!(proof.physical_inputs["left"].len(), 3);
    assert_eq!(proof.physical_inputs["right"].len(), 3);
    assert_eq!(
        sorted(&proof.physical_inputs["left"]),
        sorted(&native_source_bag("SELECT v FROM UNNEST([1,1,2]) AS t(v)"))
    );
    assert_eq!(
        sorted(&proof.physical_inputs["right"]),
        sorted(&native_source_bag(
            "SELECT v FROM (VALUES (1),(1),(2)) AS t(v)"
        ))
    );
    assert_eq!(
        proof.joined_selected,
        [
            ["left/unnest:1".to_string(), "right/values:0".to_string()],
            ["left/unnest:1".to_string(), "right/values:1".to_string()],
            ["left/unnest:2".to_string(), "right/values:0".to_string()],
            ["left/unnest:2".to_string(), "right/values:1".to_string()],
            ["left/unnest:3".to_string(), "right/values:2".to_string()]
        ],
        "every physical matched pair has deterministic provenance"
    );
    assert_eq!(proof.joined_selected.len(), 5);
    assert_eq!(proof.joined_not_true.len(), 4);
    assert_eq!(proof.joined_false.len(), 4);
    assert!(proof.joined_unknown.is_empty(), "no nullable join keys");
    assert_eq!(proof.terminal_column_types["v"], FiniteColumnType::Integer);
    assert_eq!(proof.terminal_column_types["n"], FiniteColumnType::Integer);
    let (bundle, target) = analyzed(sql, "duckdb");
    let wanted = BTreeMap::from([
        ("v".to_string(), LiteralValue::Number("3".to_string())),
        ("n".to_string(), LiteralValue::Number("1".to_string())),
    ]);
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, Some(&wanted)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn finite_join_null_keys_are_not_true_and_reversed_equality_is_equivalent() {
    let first = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2,NULL]) AS t(v) JOIN (VALUES(1),(1),(2),(NULL)) AS u(v) ON t.v = u.v GROUP BY t.v";
    let reverse = "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2,NULL]) AS t(v) JOIN (VALUES(1),(1),(2),(NULL)) AS u(v) ON u.v = t.v GROUP BY t.v";
    for sql in [first, reverse] {
        let p = source_free(sql);
        let PhysicalFiniteTableProof::Exact(proof) = p else {
            panic!("nullable equality must use SQL TRUE/NOT TRUE: {p:?}");
        };
        assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));
        assert_eq!(proof.joined_selected.len(), 5);
        assert_eq!(proof.joined_not_true.len(), 11);
        assert_eq!(proof.joined_false.len(), 4);
        assert_eq!(proof.joined_unknown.len(), 7);
        assert_eq!(
            proof.joined_not_true.len(),
            proof.joined_false.len() + proof.joined_unknown.len()
        );
        assert!(proof
            .joined_unknown
            .iter()
            .all(|pair| { pair[0] == "left/unnest:4" || pair[1] == "right/values:3" }));
        assert!(proof
            .joined_not_true
            .iter()
            .any(|ids| ids[0] == "left/unnest:4"));
        assert!(proof
            .joined_not_true
            .iter()
            .any(|ids| ids[1] == "right/values:3"));
        assert!(proof
            .terminal
            .iter()
            .all(|row| row["v"] != LiteralValue::Null));
    }
}

#[test]
fn non_equality_join_and_unproven_multiple_sources_fail_closed() {
    for sql in [
        "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v > u.v GROUP BY t.v",
        "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v AND t.v > 0 GROUP BY t.v",
        "SELECT t.v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) JOIN (VALUES(1),(1),(2)) AS u(v) ON t.v = u.v GROUP BY u.v",
    ] {
        let (bundle, target) = analyzed(sql, "duckdb");
        assert!(matches!(
            physical_finite_table_plan(&bundle, &target, None),
            PhysicalFiniteTableProof::Residual { .. }
        ), "{sql}: only attested exact join can be exact");
    }
}

#[test]
fn frozen_mandatory_cases_reconcile_typed_null_domains_inclusive_bounds_and_full_pairs() {
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-140-acceptance-freeze.json"))
            .expect("frozen complete case matrix");
    let mut checked = 0;
    for case in matrix["variants"].as_array().expect("all cases") {
        let Some(domains) = case["domains"].as_object() else {
            continue;
        };
        let sql = case["sql"].as_str().expect("literal SQL");
        let PhysicalFiniteTableProof::Exact(proof) = source_free(sql) else {
            panic!("mandatory typed case {} must be exact", case["id"]);
        };
        for (name, expected) in domains {
            assert_eq!(proof.terminal_column_types[name], FiniteColumnType::Integer);
            let expected_values = expected["values"]
                .as_array()
                .expect("complete finite values");
            if expected_values.is_empty() {
                assert_eq!(proof.terminal_domains[name], ValueDomain::Empty);
                continue;
            }
            let ValueDomain::Set(domain) = &proof.terminal_domains[name] else {
                panic!("{sql}: {name}: finite outcome must be a typed Include domain");
            };
            assert_eq!(domain.mode(), SetMode::Include);
            assert_eq!(
                proof.terminal_domains[name].admits_null(),
                expected["nullable"].as_bool()
            );
            let mut actual = domain
                .values()
                .iter()
                .map(|literal| match literal.value() {
                    LiteralValue::Number(n) => {
                        serde_json::json!(n.parse::<i128>().expect("integer"))
                    }
                    LiteralValue::Null => Value::Null,
                    other => panic!("unsupported typed domain {other:?}"),
                })
                .collect::<Vec<_>>();
            actual.sort_by_key(|value| value.to_string());
            let mut specified = expected_values.to_vec();
            specified.sort_by_key(|value| value.to_string());
            assert_eq!(actual, specified, "{sql}: {name}: exact output set");
            let mut numeric = actual.iter().filter_map(Value::as_i64).collect::<Vec<_>>();
            numeric.sort_unstable();
            assert_eq!(
                numeric.first().copied(),
                expected["lower"]["value"].as_i64()
            );
            assert_eq!(numeric.last().copied(), expected["upper"]["value"].as_i64());
            assert_eq!(expected["lower"]["inclusive"], true);
            assert_eq!(expected["upper"]["inclusive"], true);
        }
        if let Some(partition) = case["complete_pair_partition"].as_object() {
            let parse_pairs = |rows: &Value| {
                rows.as_array()
                    .unwrap()
                    .iter()
                    .map(|pair| {
                        [
                            pair[0].as_str().unwrap().to_string(),
                            pair[1].as_str().unwrap().to_string(),
                        ]
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(proof.joined_selected, parse_pairs(&partition["selected"]));
            assert_eq!(proof.joined_not_true, parse_pairs(&partition["not_true"]));
            assert_eq!(proof.joined_false, parse_pairs(&partition["false"]));
            assert_eq!(proof.joined_unknown, parse_pairs(&partition["unknown"]));
            assert_eq!(
                proof.joined_selected.len() + proof.joined_not_true.len(),
                partition["complete_cartesian_count"].as_u64().unwrap() as usize
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked, 5,
        "all frozen mandatory typed-domain specifications"
    );
}

#[test]
fn bogus_source_alias_must_not_be_certified_as_an_exact_grouped_bag() {
    let sql = "SELECT wrong.v, COUNT(*) AS n FROM UNNEST([1,2]) AS t(v) GROUP BY wrong.v";
    let (bundle, target) = analyzed(sql, "duckdb");
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, None),
        PhysicalFiniteTableProof::Residual { .. }
    ));
}

#[test]
fn typed_empty_fanout_has_an_exact_empty_group_bag_and_impossible_membership() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST(CAST([] AS INTEGER[])) AS t(v) GROUP BY v";
    let PhysicalFiniteTableProof::Exact(proof) = source_free(sql) else {
        panic!("explicit integer-array type must certify the empty grouped bag");
    };
    assert!(proof.terminal.is_empty());
    assert!(proof.source_rows.is_empty());
    assert!(proof.source_row_ids.is_empty());
    assert_eq!(proof.terminal_domains["v"], ValueDomain::Empty);
    assert_eq!(proof.terminal_domains["n"], ValueDomain::Empty);
    assert_eq!(proof.terminal_column_types["v"], FiniteColumnType::Integer);
    assert_eq!(proof.terminal_column_types["n"], FiniteColumnType::Integer);
    assert_eq!(sorted(&proof.terminal), sorted(&duckdb_bag(sql)));

    let (bundle, target) = analyzed(sql, "duckdb");
    let wanted = BTreeMap::from([
        ("v".to_string(), LiteralValue::Number("1".to_string())),
        ("n".to_string(), LiteralValue::Number("1".to_string())),
    ]);
    assert!(matches!(
        physical_finite_table_plan(&bundle, &target, Some(&wanted)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}
