//! TASK-165 frozen public-API acceptance runner, not a parser-only smoke test.
//! Compares every mandatory fixture and all 13 dialect paths to an independent
//! DuckDB source/terminal bag and the emitted canonical goal verdict.

use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, ConfiguredSqlInput,
    ConstraintValue, OutcomeGoal, OutcomeGoalStatus, OutputDistribution, OutputValueCount,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../docs/task-165-acceptance-freeze.json"))
        .expect("TASK-165 frozen JSON")
}

fn typed_bucket(value: &Value) -> ConstraintValue {
    if value.is_null() {
        ConstraintValue::Null
    } else {
        ConstraintValue::Integer(value.as_i64().expect("signed INTEGER fixture"))
    }
}

fn request_for(case: &Value, layer_id: &str) -> OutcomeGoal {
    let requested = &case["requested"];
    let rows = requested["rows"].as_u64().expect("output row target");
    let distributions = requested["histogram"]
        .as_array()
        .map(|values| {
            vec![OutputDistribution::new(
                "k",
                values
                    .iter()
                    .map(|bucket| {
                        let count = if bucket["rows"] == "u64::MAX" {
                            u64::MAX
                        } else {
                            bucket["rows"].as_u64().expect("bucket count")
                        };
                        OutputValueCount::new(typed_bucket(&bucket["value"]), count)
                    })
                    .collect(),
            )
            .expect("histogram")]
        })
        .unwrap_or_default();
    OutcomeGoal::new(
        layer_id,
        Some(rows),
        requested["groups"].as_u64(),
        distributions,
    )
    .expect("frozen goal")
}

fn source_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").expect("source datatype")],
    )
    .expect("source schema")])
    .expect("relation catalog")
}

fn observed_bag(db: &Connection, sql: &str, grouped: bool) -> Vec<Value> {
    let mut statement = db.prepare(sql).expect("DuckDB SQL");
    let rows = statement
        .query_map([], |row| {
            let k: Option<i64> = row.get(0)?;
            if grouped {
                let n: i64 = row.get(1)?;
                Ok(json!({"k": k, "n": n}))
            } else {
                Ok(json!(k))
            }
        })
        .expect("DuckDB query");
    let mut bag = rows
        .map(|row| row.expect("DuckDB typed occurrence"))
        .collect::<Vec<_>>();
    bag.sort_by_key(Value::to_string);
    bag
}

#[test]
fn every_frozen_dialect_cell_has_exact_proof_and_native_complete_bag_oracle() {
    let freeze = fixture();
    let dialects = freeze["dialects"].as_array().expect("dialect matrix");
    let cases = freeze["cases"].as_array().expect("acceptance cases");
    assert_eq!(dialects.len(), 13);
    assert_eq!(cases.len(), 8);
    assert_eq!(freeze["dialect_cells"], json!(dialects.len() * cases.len()));
    let mut unique_ids = std::collections::BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().expect("case ID");
        assert!(unique_ids.insert(id), "repeated frozen case ID: {id}");
        assert!(
            ["mandatory", "conditional"].contains(&case["requirement"].as_str().expect("tier")),
            "{id}: explicit tier required"
        );
    }
    assert_eq!(
        cases
            .iter()
            .filter(|case| case["requirement"] == "mandatory")
            .count(),
        7,
        "all seven mandatory cells remain included"
    );
    for criterion in ["1", "2", "3", "4", "5"] {
        let mapped = freeze["original_child_ac_map"][criterion]
            .as_array()
            .expect("all child criteria own explicit case mapping");
        assert!(!mapped.is_empty(), "AC {criterion} must be mapped");
        for id in mapped {
            assert!(
                unique_ids.contains(id.as_str().expect("mapped case ID")),
                "AC {criterion} references missing frozen case: {id}"
            );
        }
    }
    let mut cell_count = 0;
    for case in cases {
        let id = case["id"].as_str().expect("stable ID");
        let classification = case["expected"]["classification"]
            .as_str()
            .expect("verdict");
        if case["requirement"] == "conditional" {
            continue;
        }
        let sql = case["sql"].as_str().expect("SQL");
        let grouped = case["family"] == "ordinary_group_cardinality"
            || case["family"] == "incompatible_group_goal";
        let source = case["source_bag"].as_array();
        let expected = case["terminal_bag"].as_array();
        for dialect in dialects {
            let dialect = dialect.as_str().expect("dialect name");
            let parser = dialect_from_name(dialect).expect("canonical dialect");
            let input = SqlInput::inline(sql);
            let sources = [ConfiguredSqlInput::new(
                id,
                &input,
                dialect,
                parser.as_ref(),
            )];
            let mut bundle = analyze_configured_inputs_with_catalog(&sources, &source_catalog())
                .unwrap_or_else(|error| panic!("{id}/{dialect}: parser: {error}"));
            let target = bundle
                .layers()
                .last()
                .expect("output layer")
                .id()
                .to_string();
            bundle
                .set_outcome_goals(&[request_for(case, &target)])
                .expect("attach goal");
            let result = &bundle.outcome_goals()[0];
            let expected_status = match classification {
                "Exact" => OutcomeGoalStatus::Feasible,
                "Impossible" => OutcomeGoalStatus::Unsatisfiable,
                other => panic!("{id}: non-mandatory {other}"),
            };
            assert_eq!(result.status(), expected_status, "{id}/{dialect}");
            let json: Value =
                serde_json::from_str(&to_bundle_json(&bundle)).expect("emitted protocol");
            assert_eq!(
                json["outcome_goals"][0]["assessment"]["status"],
                json!(if expected_status == OutcomeGoalStatus::Feasible {
                    "feasible"
                } else {
                    "unsatisfiable"
                }),
                "{id}/{dialect}"
            );
            assert_eq!(
                json["outcome_goals"][0]["requested"]["rows"], case["requested"]["rows"],
                "{id}/{dialect}"
            );
            if classification == "Exact" {
                let kind = case["expected"]["witness"].as_str();
                if let Some(kind) = kind {
                    assert_eq!(
                        json["outcome_goals"][0]["witness"]["kind"],
                        json!(kind),
                        "{id}/{dialect}"
                    );
                } else {
                    assert_eq!(
                        json["outcome_goals"][0]["witness"]["kind"], "empty_sources",
                        "{id}/{dialect}"
                    );
                }
            } else {
                assert!(
                    json["outcome_goals"][0].get("witness").is_none(),
                    "{id}/{dialect}"
                );
            }
            if id == "85-C165-P-histogram" {
                // The protocol's complete physical source witness must itself
                // match the frozen physical bag, not only the caller's target.
                assert_eq!(
                    json["outcome_goals"][0]["witness"]["relation"], "t",
                    "{id}/{dialect}: witness must name physical source"
                );
                assert_eq!(
                    json["outcome_goals"][0]["witness"]["rows"],
                    json!(case["source_bag"].as_array().expect("full source").len()),
                    "{id}/{dialect}: whole input occurrence count"
                );
                assert_eq!(
                    json["outcome_goals"][0]["witness"]["columns"][0]["values"],
                    json!([
                        {"value":{"type":"null","value":null},"rows":1},
                        {"value":{"type":"integer","value":1},"rows":2}
                    ]),
                    "{id}/{dialect}: full typed NULL/integer histogram"
                );
                assert_eq!(
                    json["outcome_goals"][0]["witness"]["columns"][0]["column"], "k",
                    "{id}/{dialect}: canonical output/source column"
                );
                let witness_values = json["outcome_goals"][0]["witness"]["columns"][0]["values"]
                    .as_array()
                    .expect("complete typed physical buckets");
                let mut witness_bag = Vec::new();
                for bucket in witness_values {
                    let value = &bucket["value"];
                    let scalar = match value["type"].as_str().expect("canonical scalar type") {
                        "null" => Value::Null,
                        "integer" => value["value"].clone(),
                        other => panic!("{id}/{dialect}: unreviewed bucket {other}"),
                    };
                    for _ in 0..bucket["rows"].as_u64().expect("bucket rows") {
                        witness_bag.push(scalar.clone());
                    }
                }
                witness_bag.sort_by_key(Value::to_string);
                let mut frozen_bag = case["source_bag"].as_array().expect("source bag").clone();
                frozen_bag.sort_by_key(Value::to_string);
                assert_eq!(
                    witness_bag, frozen_bag,
                    "{id}/{dialect}: replay the actual canonical source witness"
                );
                let requested = request_for(case, &target);
                let stats = requested.distributions()[0]
                    .statistics()
                    .expect("checked histogram");
                assert_eq!(
                    stats.rows(),
                    case["expected"]["terminal_rows"].as_u64().expect("rows")
                );
                assert_eq!(
                    stats.distinct_non_null(),
                    case["expected"]["non_null_distinct"]
                        .as_u64()
                        .expect("distinct")
                );
                assert_eq!(
                    stats.null_rows(),
                    case["expected"]["null_rows"].as_u64().expect("nulls")
                );
                assert_eq!(
                    stats.duplicate_rows(),
                    case["expected"]["duplicate_rows"]
                        .as_u64()
                        .expect("duplicates")
                );
                let share = stats.null_fraction().expect("nonempty null share");
                assert_eq!(
                    json!({"numerator":share.numerator(),"denominator":share.denominator()}),
                    case["expected"]["null_fraction"],
                    "{id}/{dialect}: exact rational NULL share"
                );
            }

            if id == "85-C165-P-group" {
                let witness = &json["outcome_goals"][0]["witness"];
                assert_eq!(witness["relation"], "t", "{id}/{dialect}: physical source");
                assert_eq!(
                    witness["key"],
                    json!({"relation":"t","name":"k"}),
                    "{id}/{dialect}: source identity and grouped key"
                );
                assert_eq!(
                    witness["groups"],
                    json!(2),
                    "{id}/{dialect}: complete groups"
                );
                assert_eq!(
                    witness["rows_per_group"],
                    json!(1),
                    "{id}/{dialect}: complete physical occurrence count per group"
                );
                // The source generator interprets Groups as keys 0..groups-1.
                // Freeze and compare the *exact* constructed bag, not merely
                // another possible SQL population that happened to produce 2 rows.
                let groups = witness["groups"].as_u64().expect("group count");
                let rows_per_group = witness["rows_per_group"].as_u64().expect("occurrences");
                let mut generated = Vec::new();
                for key in 0..groups {
                    for _ in 0..rows_per_group {
                        generated.push(json!(key));
                    }
                }
                assert_eq!(
                    json!(generated),
                    case["source_bag"],
                    "{id}/{dialect}: reconstruct protocol witness physical inputs"
                );
                let mut generated_terminal = Vec::new();
                for key in 0..groups {
                    generated_terminal.push(json!({"k":key,"n":rows_per_group}));
                }
                assert_eq!(
                    json!(generated_terminal),
                    case["terminal_bag"],
                    "{id}/{dialect}: witness implies exact complete terminal bag"
                );
            }
            if id == "85-C165-P-zero" {
                assert_eq!(
                    json["outcome_goals"][0]["witness"]["relations"],
                    json!(["t"]),
                    "{id}/{dialect}: no forgotten physical source"
                );
            }

            // An unmaterializable overflow population is an arithmetic
            // contradiction, not a fake engine-execution claim.
            if let (Some(source), Some(expected)) = (source, expected) {
                let db = Connection::open_in_memory().expect("DuckDB");
                db.execute_batch("CREATE TABLE t(k INTEGER)")
                    .expect("source DDL");
                for value in source {
                    let literal = value.as_i64().map_or("NULL".to_string(), |n| n.to_string());
                    db.execute_batch(&format!("INSERT INTO t VALUES ({literal})"))
                        .expect("physical source occurrence");
                }
                let source_bag = observed_bag(&db, "SELECT k FROM t", false);
                let mut frozen_source = source.clone();
                frozen_source.sort_by_key(Value::to_string);
                assert_eq!(source_bag, frozen_source, "{id}/{dialect}: complete source");
                let terminal = observed_bag(&db, sql, grouped);
                let mut frozen_terminal = expected.clone();
                frozen_terminal.sort_by_key(Value::to_string);
                assert_eq!(
                    terminal, frozen_terminal,
                    "{id}/{dialect}: complete terminal"
                );
            }
            cell_count += 1;
        }
    }
    // Seven mandatory cases, and each of the 13 dialect identities is exercised.
    assert_eq!(cell_count, 91);
}

#[test]
fn conditional_unattested_source_is_residual_not_an_invented_exact_population() {
    use sql_semantic_protocol::analyze_inputs;
    let freeze = fixture();
    let case = freeze["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "85-C165-C-unknown")
        .expect("conditional ID");
    for dialect in freeze["dialects"].as_array().expect("dialects") {
        let dialect = dialect.as_str().expect("dialect");
        let parser = dialect_from_name(dialect).expect("dialect");
        let mut bundle = analyze_inputs(
            &[SqlInput::inline("SELECT k FROM t")],
            dialect,
            parser.as_ref(),
        )
        .expect("syntax parsed without catalog");
        let layer = bundle.layers()[0].id().to_string();
        bundle
            .set_outcome_goals(&[request_for(case, &layer)])
            .expect("goal");
        assert_eq!(
            bundle.outcome_goals()[0].status(),
            OutcomeGoalStatus::Residual,
            "{dialect}: no authoritative datatype or full-bag construction"
        );
    }
}
