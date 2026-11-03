//! TASK-167 v2 acceptance matrix: each case and canonical dialect is executed
//! against a single complete source assignment. Native DuckDB, typed protocol,
//! rejected membership and full terminal bags have independent frozen oracles.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_terminal_bag_plan,
    to_bundle_json, to_physical_joint_terminal_json, AnalysisBundle, ConfiguredSqlInput,
    ConstraintValue, LiteralValue, OutcomeGoal, OutputDistribution, OutputValueCount,
    PhysicalJointOtherExact, PhysicalJointTerminalProof, PhysicalWriteRow, PhysicalWriteState,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput,
    ValueDomain,
};

const DIALECTS: [&str; 13] = [
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-167-acceptance-freeze.json"))
        .expect("active corrected v2 TASK-167 contract")
}

fn literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(number) => LiteralValue::Number(number.to_string()),
        Value::String(text) => LiteralValue::Text(text.clone()),
        other => panic!("unsupported frozen source literal: {other}"),
    }
}

fn state(relation: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.as_array()
            .expect("closed physical rows")
            .iter()
            .map(|row| {
                PhysicalWriteRow::new(
                    row.as_object()
                        .expect("physical row with column values")
                        .iter()
                        .map(|(column, value)| (column.clone(), literal(value)))
                        .collect(),
                )
                .expect("valid row")
            })
            .collect(),
    )
    .expect("named physical relation")
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    ["l", "r"]
        .into_iter()
        .map(|relation| state(relation, &case["source_bags"][relation]))
        .collect()
}

fn schemas() -> Vec<RelationSchema> {
    ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                [("id", "VARCHAR"), ("k", "INTEGER"), ("v", "INTEGER")]
                    .into_iter()
                    .map(|(column, ty)| {
                        SchemaColumn::from_sql_type(column, ty, "postgresql").unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

fn analyze(case: &Value, dialect: &str) -> AnalysisBundle {
    let parser = dialect_from_name(dialect).expect("project dialect");
    let sql = case["sql"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| SqlInput::inline(v.as_str().unwrap()))
        .collect::<Vec<_>>();
    let configured = sql
        .iter()
        .enumerate()
        .map(|(index, value)| {
            ConfiguredSqlInput::new(
                if index == 0 {
                    "task-167-joined"
                } else {
                    "task-167-other"
                },
                value,
                dialect,
                parser.as_ref(),
            )
        })
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(
        &configured,
        &RelationCatalog::from_schemas(&schemas()).unwrap(),
    )
    .unwrap_or_else(|e| panic!("{}/{dialect}: {e}", case["id"]))
}

fn requested_buckets(column: &str, values: &Value) -> OutputDistribution {
    let buckets = values
        .as_object()
        .unwrap()
        .iter()
        .map(|(value, count)| {
            OutputValueCount::new(
                if value == "null" {
                    ConstraintValue::Null
                } else {
                    ConstraintValue::Integer(value.parse().expect("frozen signed INTEGER"))
                },
                count.as_u64().expect("nonnegative frequency"),
            )
        })
        .collect();
    OutputDistribution::new(column, buckets).unwrap()
}

fn requested_goal(layer_id: &str, goal: &Value) -> OutcomeGoal {
    let distributions = goal["histogram"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(column, values)| requested_buckets(column, values))
        .collect();
    OutcomeGoal::new(
        layer_id,
        goal["rows"].as_u64(),
        goal["groups"].as_u64(),
        distributions,
    )
    .unwrap()
}

fn goals(case: &Value, bundle: &AnalysisBundle) -> [OutcomeGoal; 2] {
    let layers = bundle.layers();
    assert_eq!(layers.len(), 2);
    [
        requested_goal(layers[0].id(), &case["goal"]["join"]),
        requested_goal(layers[1].id(), &case["goal"]["secondary"]),
    ]
}

fn expected_rows(value: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    value
        .as_array()
        .expect("frozen actual terminal rows")
        .iter()
        .map(|row| {
            row.as_object()
                .unwrap()
                .iter()
                .map(|(column, value)| (column.to_string(), literal(value)))
                .collect()
        })
        .collect()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut output = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    output.sort();
    output
}

/// Domain bounds are derived only from complete, exact finite value sets.
/// Every bound in this frozen signed-integer family is inclusive.
fn check_domain(domain: &ValueDomain, frozen: &Value, case_id: &str, column: &str) {
    let ValueDomain::Set(values) = domain else {
        panic!("{case_id}/{column}: expected exact finite domain, got {domain:?}");
    };
    assert_eq!(values.mode(), SetMode::Include);
    let actual = values
        .values()
        .iter()
        .map(|value| format!("{:?}", value.value()))
        .collect::<BTreeSet<_>>();
    let expected = frozen["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(literal)
        .map(|value| format!("{value:?}"))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{case_id}/{column}: all exact values");
    assert_eq!(
        domain.admits_null(),
        Some(frozen["contains_null"].as_bool().unwrap()),
        "{case_id}/{column}: SQL NULL domain"
    );
    let signed = values
        .values()
        .iter()
        .filter_map(|value| match value.value() {
            LiteralValue::Number(s) => Some(s.parse::<i128>().unwrap()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let min = signed.iter().min().copied().map(|value| json!(value));
    let max = signed.iter().max().copied().map(|value| json!(value));
    assert_eq!(
        min.unwrap_or(Value::Null),
        frozen["lower"],
        "{case_id}/{column}"
    );
    assert_eq!(
        max.unwrap_or(Value::Null),
        frozen["upper"],
        "{case_id}/{column}"
    );
    assert_eq!(frozen["lower_inclusive"], true);
    assert_eq!(frozen["upper_inclusive"], true);
    assert_eq!(frozen["type"], "INTEGER");
}

fn check_domains(domains: &BTreeMap<String, ValueDomain>, expected: &Value, case_id: &str) {
    assert_eq!(domains.len(), expected.as_object().unwrap().len());
    for (name, expected_domain) in expected.as_object().unwrap() {
        check_domain(
            domains
                .get(name)
                .expect("every terminal domain accounted for"),
            expected_domain,
            case_id,
            name,
        );
    }
}

fn check_set_domains(
    exact: &sql_semantic_protocol::PhysicalFiniteSetTreeExact,
    expected: &Value,
    case_id: &str,
) {
    assert_eq!(exact.terminal_domains.len(), 2);
    for (index, name) in ["k", "v"].into_iter().enumerate() {
        let actual = &exact.terminal_domains[index];
        let frozen = &expected[name];
        let values = actual
            .values
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        let expected_values = frozen["values"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|value| !value.is_null())
            .map(Value::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(values, expected_values, "{case_id}/{name} tuple positions");
        assert_eq!(actual.contains_null, frozen["contains_null"]);
        let bounds = actual.bounds();
        assert_eq!(
            bounds.map(|bounds| json!(bounds.0)),
            Some(frozen["lower"].clone())
        );
        assert_eq!(
            bounds.map(|bounds| json!(bounds.1)),
            Some(frozen["upper"].clone())
        );
        assert_eq!(frozen["lower_inclusive"], true);
        assert_eq!(frozen["upper_inclusive"], true);
    }
}

fn pairs(value: &Value) -> Vec<[String; 2]> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            [
                pair[0].as_str().unwrap().to_string(),
                pair[1].as_str().unwrap().to_string(),
            ]
        })
        .collect()
}

fn assert_proof(case: &Value, proof: PhysicalJointTerminalProof) {
    let id = case["id"].as_str().unwrap();
    match case["expected"].as_str().unwrap() {
        "Exact" => {
            let PhysicalJointTerminalProof::Exact(exact) = proof else {
                panic!("{id}: frozen exact plan is {proof:?}");
            };
            assert_eq!(
                exact.sources,
                snapshots(case),
                "{id}: one complete shared assignment"
            );
            assert_eq!(
                exact.grouped.selected,
                pairs(&case["join_pair_membership"]["true"]),
                "{id}: SQL TRUE pair identities"
            );
            assert_eq!(
                exact.grouped.rejected,
                pairs(&case["join_pair_membership"]["not_true"]),
                "{id}: SQL NOT TRUE pair identities"
            );
            assert_eq!(
                bag(&exact.grouped.terminal),
                bag(&expected_rows(&case["observed_terminal_bags"]["join"])),
            );
            check_domains(
                &exact.grouped.terminal_domains,
                &case["typed_output_domains"]["joined"],
                id,
            );
            match &exact.other {
                PhysicalJointOtherExact::Window(window) => {
                    assert_eq!(case["secondary"], "window");
                    assert_eq!(
                        bag(&window.terminal),
                        bag(&expected_rows(&case["observed_terminal_bags"]["other"])),
                        "{id}: full window terminal bag"
                    );
                    assert_eq!(window.source_row_ids.len(), exact.sources[0].rows().len());
                    check_domains(
                        &window.terminal_domains,
                        &case["typed_output_domains"]["other"],
                        id,
                    );
                }
                PhysicalJointOtherExact::Set(set) => {
                    assert_eq!(case["secondary"], "set");
                    assert_eq!(
                        bag(&set.terminal),
                        bag(&expected_rows(&case["observed_terminal_bags"]["other"])),
                        "{id}: full positional set bag"
                    );
                    check_set_domains(set, &case["typed_output_domains"]["other"], id);
                    assert_eq!(set.sources, vec!["l".to_string(), "r".to_string()]);
                    let output_count: u64 = set.tuples.iter().map(|tuple| tuple.output_count).sum();
                    assert_eq!(output_count, set.terminal.len() as u64);
                }
            }
        }
        "Impossible" => assert!(
            matches!(proof, PhysicalJointTerminalProof::Impossible { .. }),
            "{id}: expected contradictory goal with fixed source, got {proof:?}"
        ),
        "Residual" => assert!(
            matches!(proof, PhysicalJointTerminalProof::Residual { .. }),
            "{id}: opaque law must remain Residual, got {proof:?}"
        ),
        other => panic!("{id}: unknown expected verdict {other}"),
    }
}

/// Read independent physical rows and terminal bags through native DuckDB SQL,
/// not from the semantic interpreter, and compare against frozen *observed*
/// bags even when the user's *requested* goal is intentionally impossible.
fn native_duckdb(case: &Value) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, k INTEGER, v INTEGER);
         CREATE TABLE r(id VARCHAR, k INTEGER, v INTEGER);",
    )
    .unwrap();
    for relation in ["l", "r"] {
        for row in case["source_bags"][relation].as_array().unwrap() {
            let scalar = |name: &str| row[name].as_i64().map_or("NULL".into(), |n| n.to_string());
            db.execute_batch(&format!(
                "INSERT INTO {relation} VALUES ('{}', {}, {});",
                row["id"].as_str().unwrap(),
                scalar("k"),
                scalar("v")
            ))
            .unwrap();
        }
        let actual = db
            .prepare(&format!("SELECT id,k,v FROM {relation} ORDER BY id"))
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let expected = case["source_bags"][relation]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["id"].as_str().unwrap().to_string(),
                    row["k"].as_i64(),
                    row["v"].as_i64(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected,
            "{}: complete {relation} physical bag",
            case["id"]
        );
    }
    let query = |sql: &str, columns: &[&str]| {
        let mut statement = db.prepare(sql).unwrap();
        let actual = statement
            .query_map([], |row| {
                let mut values = BTreeMap::new();
                for (index, column) in columns.iter().enumerate() {
                    values.insert(
                        (*column).to_string(),
                        row.get::<_, Option<i64>>(index)?
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    );
                }
                Ok(values)
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        bag(&actual)
    };
    let join_sql = case["sql"][0].as_str().unwrap();
    assert_eq!(
        query(join_sql, &["k", "n"]),
        bag(&expected_rows(&case["observed_terminal_bags"]["join"])),
        "{}: independent native full join bag",
        case["id"]
    );
    let selected = db
        .prepare(
            "SELECT l.id, r.id FROM l CROSS JOIN r WHERE (l.k = r.k AND l.k = 1) IS TRUE ORDER BY l.id, r.id"
        )
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let rejected = db
        .prepare(
            "SELECT l.id, r.id FROM l CROSS JOIN r WHERE (l.k = r.k AND l.k = 1) IS NOT TRUE ORDER BY l.id, r.id"
        )
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let physical_id = |side: &str, id: &str| {
        case["source_bags"][side]
            .as_array()
            .unwrap()
            .iter()
            .position(|row| row["id"] == id)
            .map(|index| format!("{side}#{index}"))
            .unwrap()
    };
    for (actual, expected) in [
        (selected, &case["join_pair_membership"]["true"]),
        (rejected, &case["join_pair_membership"]["not_true"]),
    ] {
        let mut canonical = actual
            .iter()
            .map(|(l, r)| [physical_id("l", l), physical_id("r", r)])
            .collect::<Vec<_>>();
        canonical.sort();
        let mut frozen = pairs(expected);
        frozen.sort();
        assert_eq!(
            canonical, frozen,
            "{}: SQL 3VL TRUE/NOT TRUE pairs",
            case["id"]
        );
    }
    if let Some(sql) = case["sql"][1].as_str() {
        if case["native_duckdb"]["status"] != "unavailable_opaque_sql_function" {
            let columns = if case["secondary"] == "set" {
                vec!["k", "v"]
            } else {
                vec!["k", "copies"]
            };
            assert_eq!(
                query(sql, &columns),
                bag(&expected_rows(&case["observed_terminal_bags"]["other"])),
                "{}: independent native complete other terminal",
                case["id"]
            );
        }
    }
}

#[test]
fn every_frozen_mandatory_case_is_checked_across_all_13_dialects_with_native_duckdb() {
    let document = freeze();
    assert_eq!(document["version"], 2);
    let matrix = document["matrix"].as_array().unwrap();
    assert_eq!(matrix.len(), 6);
    assert_eq!(
        matrix
            .iter()
            .filter(|case| case["requirement"] == "mandatory")
            .count(),
        5,
    );
    assert_eq!(
        document["dialects"].as_array().unwrap().len(),
        DIALECTS.len()
    );
    for case in matrix {
        let id = case["id"].as_str().unwrap();
        assert!(id.starts_with("85-C167-"));
        let cells = case["dialect_cells"].as_array().unwrap();
        assert_eq!(cells.len(), 13);
        for (index, dialect) in DIALECTS.iter().enumerate() {
            assert_eq!(
                cells[index]["dialect"], *dialect,
                "{id}: dialect cell identity"
            );
            assert_eq!(cells[index]["expected_verdict"], case["expected"]);
            let bundle = analyze(case, dialect);
            assert_eq!(bundle.layers().len(), 2, "{id}/{dialect}");
            assert!(
                bundle.inputs().iter().all(|unit| {
                    matches!(unit.statements().first(), Some(ProtocolStatement::Query(_)))
                }),
                "{id}/{dialect}: not merely parser acceptance"
            );
            let goals = goals(case, &bundle);
            let proof = physical_joint_terminal_bag_plan(&bundle, &goals, &snapshots(case));
            assert_proof(case, proof.clone());
            let sidecar = to_physical_joint_terminal_json(&bundle, &proof);
            let typed: Value = serde_json::from_str(&sidecar).unwrap();
            assert_eq!(typed["kind"], "physical_joint_terminal_evidence");
            assert_eq!(typed["protocol_version"], env!("CARGO_PKG_VERSION"));
            assert_eq!(
                typed["evidence"]["status"],
                case["expected"].as_str().unwrap().to_ascii_lowercase()
            );
            match &proof {
                PhysicalJointTerminalProof::Exact(_) => {
                    assert_eq!(
                        typed["evidence"]["grouped"]["layer_id"],
                        goals[0].layer_id()
                    );
                    assert_eq!(
                        typed["evidence"]["secondary"]["layer_id"],
                        goals[1].layer_id()
                    );
                    assert_eq!(typed["evidence"]["sources"].as_array().unwrap().len(), 2);
                    assert_eq!(
                        typed["evidence"]["grouped"]["selected"]
                            .as_array()
                            .unwrap()
                            .len(),
                        case["join_pair_membership"]["true"]
                            .as_array()
                            .unwrap()
                            .len()
                    );
                    assert_eq!(
                        typed["evidence"]["secondary"]["proof"]["terminal"]
                            .as_array()
                            .unwrap()
                            .len(),
                        case["observed_terminal_bags"]["other"]
                            .as_array()
                            .unwrap()
                            .len()
                    );
                    assert!(typed["evidence"]["grouped"]["domains"].is_object());
                    assert!(
                        typed["evidence"]["secondary"]["proof"]["domains"].is_object()
                            || typed["evidence"]["secondary"]["proof"]["domains"].is_array()
                    );
                    assert_eq!(
                        sidecar,
                        to_physical_joint_terminal_json(&bundle, &proof),
                        "{id}/{dialect}: byte-identical canonical emission"
                    );
                }
                _ => {
                    assert!(typed["evidence"]["grouped"].is_null());
                    assert!(typed["evidence"]["sources"].is_null());
                    assert!(typed["evidence"]["reason"].is_string());
                }
            }
            let mut emitted_bundle = bundle.clone();
            emitted_bundle.set_outcome_goals(&goals).unwrap();
            let emitted: Value = serde_json::from_str(&to_bundle_json(&emitted_bundle)).unwrap();
            assert_eq!(emitted["source_schemas"].as_array().unwrap().len(), 2);
            assert_eq!(emitted["outcome_goals"].as_array().unwrap().len(), 2);
            for target in &goals {
                assert!(emitted["outcome_goals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|goal| {
                        goal["layer_id"] == target.layer_id()
                            && goal["requested"]["rows"] == target.rows().unwrap()
                    }));
            }
        }
        if case["requirement"] == "mandatory" {
            native_duckdb(case);
        } else {
            // Vendor-defined opaque function semantics cannot be certified
            // from parser output or independent generated expectations.
            assert_eq!(case["expected"], "Residual");
            assert_eq!(
                case["native_duckdb"]["status"],
                "unavailable_opaque_sql_function"
            );
        }
    }
}
