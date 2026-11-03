//! TASK-166 frozen, independent shared-source acceptance runner.
//! One task-owned JSON case defines complete source/terminal bags, requested
//! (possibly impossible) goals and SQL TRUE/NOT TRUE occurrence partitions.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_distribution_plan,
    to_bundle_json, AnalysisBundle, ClosedWorldCoverage, ConfiguredSqlInput, ConstraintValue,
    GroupBoundaryKind, OutcomeGoal, OutcomeWitness, OutputDistribution, OutputValueCount,
    ProtocolStatement, RelationCatalog, RelationSchema, RowQuantifier, SchemaColumn, SqlInput,
    WitnessDirection, WitnessObligation,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-166-acceptance-freeze.json"))
        .expect("versioned TASK-166 freeze")
}

fn sorted_bag(values: &[Value]) -> Vec<Value> {
    let mut bag = values.to_vec();
    bag.sort_by_key(Value::to_string);
    bag
}

fn canonical_value(value: &Value) -> ConstraintValue {
    if value.is_null() {
        ConstraintValue::Null
    } else {
        ConstraintValue::Integer(value.as_i64().expect("i64 INTEGER frozen value"))
    }
}

fn histogram(column: &str, buckets: &Value) -> Vec<OutputDistribution> {
    let Some(buckets) = buckets.as_array() else {
        return Vec::new();
    };
    vec![OutputDistribution::new(
        column,
        buckets
            .iter()
            .map(|bucket| {
                OutputValueCount::new(
                    canonical_value(&bucket["value"]),
                    bucket["rows"].as_u64().expect("frozen bucket count"),
                )
            })
            .collect(),
    )
    .expect("complete typed histogram")]
}

fn catalog(typed: bool) -> RelationCatalog {
    let schemas = if typed {
        vec![RelationSchema::new(
            "t",
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()]
    } else {
        Vec::new()
    };
    RelationCatalog::from_schemas(&schemas).unwrap()
}

fn analyze(case: &Value, dialect: &str) -> AnalysisBundle {
    let sqls = case["sql_statements"]
        .as_array()
        .expect("two SQL statements");
    assert_eq!(sqls.len(), 2);
    let parser = dialect_from_name(dialect).expect("advertised parser");
    let inputs = sqls
        .iter()
        .map(|sql| SqlInput::inline(sql.as_str().expect("SQL text")))
        .collect::<Vec<_>>();
    let names = ["task-166-a", "task-166-b"];
    let configured = inputs
        .iter()
        .zip(names)
        .map(|(sql, name)| ConfiguredSqlInput::new(name, sql, dialect, parser.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(
        &configured,
        &catalog(case["id"] != "85-C166-C-missing-schema"),
    )
    .unwrap_or_else(|error| panic!("{}/{dialect}: {error}", case["id"]))
}

fn goals(case: &Value, bundle: &AnalysisBundle) -> [OutcomeGoal; 2] {
    let layers = bundle.layers();
    assert_eq!(layers.len(), 2, "both complete terminal layers required");
    let a = &case["goals"]["terminal_a"];
    let b = &case["goals"]["terminal_b"];
    [
        OutcomeGoal::new(
            layers[0].id(),
            Some(a["rows"].as_u64().unwrap()),
            None,
            histogram("whole", &a["histogram"]),
        )
        .unwrap(),
        OutcomeGoal::new(
            layers[1].id(),
            Some(b["rows"].as_u64().unwrap()),
            None,
            histogram("filtered", &b["histogram"]),
        )
        .unwrap(),
    ]
}

fn check_feasible(case: &Value, goals: &[OutcomeGoal; 2], result: &WitnessDirection) {
    let WitnessDirection::Feasible(cases) = result else {
        panic!("{} expected Exact, found {result:?}", case["id"]);
    };
    let [proof] = cases.as_slice() else {
        panic!("{} must have one common physical bag", case["id"]);
    };
    let mut populations = Vec::new();
    let mut closed = 0;
    let mut physical_rows = 0;
    let mut outputs = BTreeMap::<String, (u64, Option<u64>)>::new();
    for obligation in proof.obligations() {
        match obligation {
            WitnessObligation::OperatorPopulation {
                witness:
                    OutcomeWitness::SourceRows {
                        relation,
                        rows,
                        columns,
                    },
            } => populations.push((relation, rows, columns)),
            WitnessObligation::ClosedWorld { boundary, coverage }
                if boundary.kind() == GroupBoundaryKind::Physical
                    && boundary.relation() == "t"
                    && *coverage == ClosedWorldCoverage::EntireRelation =>
            {
                closed += 1;
            }
            WitnessObligation::Rows {
                boundary,
                quantifier: RowQuantifier::ForAll,
                bounds,
                closed_world: true,
                ..
            } if boundary.relation() == "t"
                && bounds.minimum() == 3
                && bounds.maximum() == Some(3) =>
            {
                physical_rows += 1;
            }
            WitnessObligation::OutputRows { layer_id, bounds } => {
                assert!(
                    outputs
                        .insert(layer_id.clone(), (bounds.minimum(), bounds.maximum()))
                        .is_none(),
                    "no duplicate terminal obligations"
                );
            }
            _ => {}
        }
    }
    assert_eq!(
        populations.len(),
        1,
        "single physical-source occurrence bag"
    );
    assert_eq!(closed, 1);
    assert_eq!(physical_rows, 1);
    assert_eq!(populations[0].0.as_str(), "t");
    assert_eq!(*populations[0].1, 3);
    assert_eq!(populations[0].2.len(), 1);
    let source_column = &populations[0].2[0];
    assert_eq!(source_column.column(), "k");
    let expected = histogram("whole", &case["goals"]["terminal_a"]["histogram"]);
    assert_eq!(source_column.values(), expected[0].values());
    assert_eq!(outputs.len(), 2);
    for goal in goals {
        assert_eq!(
            outputs.get(goal.layer_id()),
            Some(&(goal.rows().unwrap(), goal.rows())),
            "both outputs must be proven from that same source"
        );
    }
}

fn observed_bag(db: &Connection, sql: &str) -> Vec<Value> {
    let mut stmt = db.prepare(sql).expect("frozen native DuckDB SQL");
    let rows = stmt
        .query_map([], |row| {
            let value: Option<i64> = row.get(0)?;
            Ok(json!(value))
        })
        .expect("execute whole terminal");
    let values = rows
        .map(|row| row.expect("full result row"))
        .collect::<Vec<_>>();
    sorted_bag(&values)
}

fn observed_integer_domain(bag: &[Value]) -> Value {
    let mut integers = bag.iter().filter_map(Value::as_i64).collect::<Vec<_>>();
    integers.sort();
    let lower = integers.first().copied();
    let upper = integers.last().copied();
    json!({
        "type": "signed_integer",
        "bits": 32,
        "lower": lower,
        "upper": upper,
        "lower_inclusive": lower.map(|_| true),
        "upper_inclusive": upper.map(|_| true),
        "nullable": bag.iter().any(Value::is_null)
    })
}

fn check_native_bags(case: &Value) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(ordinal INTEGER, k INTEGER);")
        .unwrap();
    let source = case["native_snapshots"]["source_bag"]
        .as_array()
        .expect("physical source");
    for (ordinal, value) in source.iter().enumerate() {
        let scalar = value
            .as_i64()
            .map_or_else(|| "NULL".to_string(), |n| n.to_string());
        db.execute_batch(&format!("INSERT INTO t VALUES ({ordinal}, {scalar});"))
            .unwrap();
    }
    assert_eq!(
        observed_bag(&db, "SELECT k FROM t"),
        sorted_bag(source),
        "{}: physical input mismatch",
        case["id"]
    );
    assert_eq!(
        observed_integer_domain(&observed_bag(&db, "SELECT k FROM t")),
        case["expected"]["typed_domains"]["t.k"],
        "{}: native physical INTEGER/NULL range and inclusivity",
        case["id"]
    );
    let statements = case["sql_statements"].as_array().unwrap();
    assert_eq!(
        observed_integer_domain(&observed_bag(&db, statements[0].as_str().unwrap())),
        case["expected"]["typed_domains"]["terminal_a"],
        "{}: native A output domain",
        case["id"]
    );
    assert_eq!(
        observed_bag(&db, statements[0].as_str().unwrap()),
        sorted_bag(
            case["native_snapshots"]["terminal_a_bag"]
                .as_array()
                .unwrap()
        ),
        "{}: A terminal bag mismatch",
        case["id"]
    );
    if case["native_snapshots"]["execute_terminal_b"] == true {
        assert_eq!(
            observed_integer_domain(&observed_bag(&db, statements[1].as_str().unwrap())),
            case["expected"]["typed_domains"]["terminal_b"],
            "{}: native B output domain",
            case["id"]
        );
        assert_eq!(
            observed_bag(&db, statements[1].as_str().unwrap()),
            sorted_bag(
                case["native_snapshots"]["terminal_b_bag"]
                    .as_array()
                    .unwrap()
            ),
            "{}: B terminal bag mismatch",
            case["id"]
        );
        let condition = if case["id"] == "85-C166-P-null" {
            "k IS NULL"
        } else {
            "k = 1"
        };
        let selected = db
            .prepare(&format!(
                "SELECT ordinal FROM t WHERE {condition} ORDER BY ordinal"
            ))
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let rejected = db
            .prepare(&format!(
                "SELECT ordinal FROM t WHERE ({condition}) IS NOT TRUE ORDER BY ordinal"
            ))
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let expected_selected = case["expected"]["positive_rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_i64().unwrap())
            .collect::<Vec<_>>();
        let expected_rejected = case["expected"]["not_true_rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            selected, expected_selected,
            "{}: SQL TRUE source indices",
            case["id"]
        );
        assert_eq!(
            rejected, expected_rejected,
            "{}: SQL NOT TRUE source indices",
            case["id"]
        );
        assert_eq!(selected.len() + rejected.len(), source.len());
    }
}

#[test]
fn every_frozen_case_and_parser_dialect_is_accounted_for() {
    let fixture = freeze();
    assert_eq!(fixture["version"], 2);
    let cases = fixture["cases"].as_array().expect("frozen cases");
    let dialects = fixture["dialects"].as_array().expect("13 dialects");
    assert_eq!(cases.len(), 6);
    assert_eq!(dialects.len(), 13);
    assert_eq!(fixture["dialect_cell_count"], 78);
    let expected_ids = BTreeSet::from([
        "85-C166-P-shared",
        "85-C166-P-null",
        "85-C166-N-upper",
        "85-C166-N-histogram",
        "85-C166-C-opaque",
        "85-C166-C-missing-schema",
    ]);
    let ids = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids, expected_ids);
    for number in ["1", "2", "3", "4", "5"] {
        for id in fixture["original_child_ac_map"][number].as_array().unwrap() {
            assert!(
                ids.contains(id.as_str().unwrap()),
                "unmapped child AC {number}"
            );
        }
    }
    let mut checked = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let expected = case["expected"]["classification"].as_str().unwrap();
        let tiers = case["requirement"].as_str().unwrap();
        assert_eq!(
            tiers == "mandatory",
            ["Exact", "Impossible"].contains(&expected)
        );
        assert_eq!(case["dialect_parser_cells"].as_array().unwrap().len(), 13);
        assert_eq!(case["native_snapshots"]["source_bag"], case["source_bag"]);
        assert_eq!(
            case["native_snapshots"]["terminal_a_bag"],
            case["terminal_a"]
        );
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            assert!(
                case["dialect_parser_cells"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|cell| {
                        cell["dialect"] == name
                            && cell["verdict"] == expected
                            && cell["expected"] == "canonical_two_query_layers"
                    }),
                "{id}/{name}: no frozen parser-classification cell"
            );
            let mut bundle = analyze(case, name);
            assert_eq!(bundle.inputs().len(), 2, "{id}/{name}: two SQL inputs");
            for input in bundle.inputs() {
                assert!(
                    matches!(
                        input.statements().first(),
                        Some(ProtocolStatement::Query(_))
                    ),
                    "{id}/{name}: canonical query required, not parser acceptance alone"
                );
            }
            let targets = goals(case, &bundle);
            let plan = physical_joint_distribution_plan(&bundle, &targets);
            match expected {
                "Exact" => check_feasible(case, &targets, &plan),
                "Impossible" => assert_eq!(plan, WitnessDirection::Impossible, "{id}/{name}"),
                "Residual" => assert!(
                    matches!(plan, WitnessDirection::Residual { .. }),
                    "{id}/{name}: unexpected exactness or false impossibility: {plan:?}"
                ),
                other => panic!("unreviewed verdict {other}"),
            };
            bundle.set_outcome_goals(&targets).expect("public goal API");
            let emitted: Value = serde_json::from_str(&to_bundle_json(&bundle))
                .expect("valid canonical JSON protocol");
            assert_eq!(emitted["outcome_goals"].as_array().unwrap().len(), 2);
            for requested in &targets {
                assert!(
                    emitted["outcome_goals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|goal| {
                            goal["layer_id"] == requested.layer_id()
                                && goal["requested"]["rows"] == requested.rows().unwrap()
                        }),
                    "{id}/{name}: canonical requested goal must survive emission"
                );
            }
            assert_eq!(
                emitted["source_schemas"]
                    .as_array()
                    .is_none_or(Vec::is_empty),
                id == "85-C166-C-missing-schema",
                "{id}/{name}: canonical schema authority"
            );
            if id != "85-C166-C-missing-schema" {
                let schema = &emitted["source_schemas"][0];
                assert_eq!(schema["relation"], "t", "{id}/{name}: source identity");
                assert_eq!(schema["columns"][0]["name"], "k");
                assert_eq!(schema["columns"][0]["data_type"]["kind"], "signed_integer");
                assert_eq!(schema["columns"][0]["data_type"]["bits"], 32);
            }
            checked += 1;
        }
        if case["native_snapshots"]["execute_terminal_b"] == true {
            check_native_bags(case);
        } else {
            // Opaque is intentionally an unexecutable vendor function: its
            // whole-source and A-terminal bags are independently executable.
            check_native_bags(case);
        }
    }
    assert_eq!(
        checked, 78,
        "all frozen parser-dialect classifications checked"
    );
}
