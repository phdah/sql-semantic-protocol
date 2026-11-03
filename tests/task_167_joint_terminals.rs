//! TASK-167: two exact terminals share one physical source assignment.
//! The independent DuckDB execution below checks complete source/terminal bags.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_terminal_bag_plan,
    AnalysisBundle, ConfiguredSqlInput, ConstraintValue, LiteralValue, OutcomeGoal,
    OutputDistribution, OutputValueCount, PhysicalJointOtherExact, PhysicalJointTerminalProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const JOIN: &str = "SELECT l.k AS k, COUNT(*) AS n FROM l JOIN r ON l.k = r.k WHERE l.k = 1 GROUP BY l.k HAVING COUNT(*) >= 2";
const WINDOW: &str = "SELECT k, COUNT(*) OVER (PARTITION BY k) AS copies FROM l";
const SET: &str = "SELECT k, v FROM l UNION ALL SELECT k, v FROM r";

fn catalog() -> RelationCatalog {
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                [("id", "VARCHAR"), ("k", "INTEGER"), ("v", "INTEGER")]
                    .into_iter()
                    .map(|(column, kind)| {
                        SchemaColumn::from_sql_type(column, kind, "postgresql").unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    RelationCatalog::from_schemas(&schemas).unwrap()
}

fn analyze(join: &str, other: &str, dialect: &str) -> AnalysisBundle {
    let inputs = [SqlInput::inline(join), SqlInput::inline(other)];
    let ids = ["task-167-join", "task-167-other"];
    let parser = dialect_from_name(dialect).unwrap();
    let configured = inputs
        .iter()
        .zip(ids)
        .map(|(sql, id)| ConfiguredSqlInput::new(id, sql, dialect, parser.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap()
}

fn physical_row(id: &str, k: Option<i64>, v: i64) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".into(), LiteralValue::Text(id.into())),
        (
            "k".into(),
            k.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        ),
        ("v".into(), LiteralValue::Number(v.to_string())),
    ]))
    .unwrap()
}

fn sources(null_row: bool) -> Vec<PhysicalWriteState> {
    let mut left = vec![
        physical_row("l0", Some(1), 10),
        physical_row("l1", Some(1), 20),
    ];
    if null_row {
        left.push(physical_row("l2", None, 30));
    }
    vec![
        PhysicalWriteState::new("l", left).unwrap(),
        PhysicalWriteState::new("r", vec![physical_row("r0", Some(1), 10)]).unwrap(),
    ]
}

fn histogram(column: &str, values: &[(ConstraintValue, u64)]) -> OutputDistribution {
    OutputDistribution::new(
        column,
        values
            .iter()
            .map(|(value, rows)| OutputValueCount::new(value.clone(), *rows))
            .collect(),
    )
    .unwrap()
}

fn goals(
    bundle: &AnalysisBundle,
    secondary_rows: u64,
    join_rows: u64,
    set: bool,
) -> Vec<OutcomeGoal> {
    vec![
        OutcomeGoal::new(
            bundle.layers()[0].id(),
            Some(join_rows),
            Some(join_rows),
            vec![histogram("n", &[(ConstraintValue::Integer(2), join_rows)])],
        )
        .unwrap(),
        OutcomeGoal::new(
            bundle.layers()[1].id(),
            Some(secondary_rows),
            None,
            if set {
                vec![histogram(
                    "v",
                    &[
                        (ConstraintValue::Integer(10), 2),
                        (ConstraintValue::Integer(20), 1),
                    ],
                )]
            } else {
                vec![histogram(
                    "copies",
                    &[
                        (ConstraintValue::Integer(1), 1),
                        (ConstraintValue::Integer(2), 2),
                    ],
                )]
            },
        )
        .unwrap(),
    ]
}

fn normalized(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut output = rows.iter().map(|row| format!("{row:?}")).collect::<Vec<_>>();
    output.sort();
    output
}

fn row(values: &[(&str, Option<i64>)]) -> BTreeMap<String, LiteralValue> {
    values
        .iter()
        .map(|(key, value)| {
            (
                (*key).to_string(),
                value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
            )
        })
        .collect()
}

#[test]
fn frozen_matrix_has_stable_cases_and_all_parser_dialects() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-167-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 1);
    let matrix = freeze["matrix"].as_array().unwrap();
    assert_eq!(matrix.len(), 6);
    for case in matrix {
        assert!(case["id"].as_str().unwrap().starts_with("85-C167-"));
        let dialects = case["dialect_cells"].as_array().unwrap();
        assert_eq!(dialects.len(), DIALECTS.len());
        for (expected, actual) in DIALECTS.iter().zip(dialects) {
            assert_eq!(actual["dialect"].as_str(), Some(*expected));
        }
    }
}

#[test]
fn window_and_join_use_one_closed_bag_for_all_thirteen_parser_dialects() {
    for dialect in DIALECTS {
        let bundle = analyze(JOIN, WINDOW, dialect);
        let mut assignment = sources(true);
        assignment.reverse(); // source list order must never change the actual proof
        let goal = goals(&bundle, 3, 1, false);
        let proof = physical_joint_terminal_bag_plan(&bundle, &goal, &assignment);
        let PhysicalJointTerminalProof::Exact(exact) = proof else {
            panic!("{dialect}: expected complete joint proof, got {proof:?}");
        };
        assert_eq!(exact.sources.len(), 2);
        assert_eq!(exact.grouped.selected.len(), 2);
        assert_eq!(exact.grouped.rejected.len(), 1);
        assert_eq!(
            exact.grouped.terminal,
            vec![row(&[("k", Some(1)), ("n", Some(2))])]
        );
        let PhysicalJointOtherExact::Window(window) = &exact.other else {
            panic!("{dialect}: second terminal should be window");
        };
        assert_eq!(
            normalized(&window.terminal),
            normalized(&[
                row(&[("k", Some(1)), ("copies", Some(2))]),
                row(&[("k", Some(1)), ("copies", Some(2))]),
                row(&[("k", None), ("copies", Some(1))]),
            ])
        );
        assert_eq!(window.source_row_ids.len(), 3);
        let reversed = goal.iter().rev().cloned().collect::<Vec<_>>();
        assert_eq!(
            physical_joint_terminal_bag_plan(&bundle, &reversed, &assignment),
            PhysicalJointTerminalProof::Exact(exact),
            "{dialect}: goal ordering cannot invent source independence"
        );
    }
}

#[test]
fn complete_source_bags_make_incompatible_join_or_window_goals_impossible() {
    let bundle = analyze(JOIN, WINDOW, "duckdb");
    for (joined, window) in [(2, 3), (1, 2)] {
        assert!(
            matches!(
                physical_joint_terminal_bag_plan(
                    &bundle,
                    &goals(&bundle, window, joined, false),
                    &sources(true)
                ),
                PhysicalJointTerminalProof::Impossible { .. }
            ),
            "contradictory count cannot be realized by changing an already attested source"
        );
    }
}

#[test]
fn set_and_join_reconcile_multiplicities_from_the_same_two_sources() {
    let bundle = analyze(
        "SELECT l.k AS k, COUNT(*) AS n FROM l JOIN r ON l.k = r.k WHERE l.k = 1 GROUP BY l.k HAVING COUNT(*) = 2",
        SET,
        "duckdb",
    );
    let assignment = sources(false);
    let exact =
        physical_joint_terminal_bag_plan(&bundle, &goals(&bundle, 3, 1, true), &assignment);
    let PhysicalJointTerminalProof::Exact(exact) = exact else {
        panic!("set branch and join must share complete bags: {exact:?}");
    };
    assert_eq!(exact.grouped.selected.len(), 2);
    let PhysicalJointOtherExact::Set(set) = &exact.other else {
        panic!("second terminal should be a canonical set tree");
    };
    assert_eq!(
        normalized(&set.terminal),
        normalized(&[
            row(&[("k", Some(1)), ("v", Some(10))]),
            row(&[("k", Some(1)), ("v", Some(10))]),
            row(&[("k", Some(1)), ("v", Some(20))]),
        ])
    );
    assert!(matches!(
        physical_joint_terminal_bag_plan(&bundle, &goals(&bundle, 4, 1, true), &assignment),
        PhysicalJointTerminalProof::Impossible { .. }
    ));
}

#[test]
fn unsupported_second_terminal_and_duplicate_physical_identity_fail_closed() {
    let b = analyze(JOIN, "SELECT opaque(k) AS x FROM l", "postgresql");
    assert!(matches!(
        physical_joint_terminal_bag_plan(&b, &goals(&b, 3, 1, false), &sources(true)),
        PhysicalJointTerminalProof::Residual { .. }
    ));
    let b = analyze(JOIN, WINDOW, "postgresql");
    let left_twice = vec![sources(true)[0].clone(), sources(true)[0].clone()];
    assert!(matches!(
        physical_joint_terminal_bag_plan(&b, &goals(&b, 3, 1, false), &left_twice),
        PhysicalJointTerminalProof::Residual { .. }
    ));
}

#[test]
fn independent_duckdb_oracle_attests_complete_source_and_both_terminal_bags() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, k INTEGER, v INTEGER);
         CREATE TABLE r(id VARCHAR, k INTEGER, v INTEGER);
         INSERT INTO l VALUES ('l0',1,10),('l1',1,20),('l2',NULL,30);
         INSERT INTO r VALUES ('r0',1,10);",
    )
    .unwrap();
    let source: Vec<(String, Option<i64>, i64)> = db
        .prepare("SELECT id,k,v FROM l ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        source,
        vec![
            ("l0".into(), Some(1), 10),
            ("l1".into(), Some(1), 20),
            ("l2".into(), None, 30),
        ]
    );
    let joined: Vec<(Option<i64>, i64)> = db
        .prepare(JOIN)
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(joined, vec![(Some(1), 2)]);
    let mut window = db
        .prepare("SELECT k, COUNT(*) OVER (PARTITION BY k) AS copies FROM l ORDER BY id")
        .unwrap();
    let window_rows = window
        .query_map([], |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(window_rows, vec![(Some(1), 2), (Some(1), 2), (None, 1)]);
    db.execute_batch("DELETE FROM l WHERE k IS NULL").unwrap();
    let set: Vec<(i64, i64)> = db
        .prepare(SET)
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut set = set;
    set.sort();
    assert_eq!(set, vec![(1, 10), (1, 10), (1, 20)]);
}
