//! TASK-165 frozen public-API acceptance runner, not a parser-only smoke test.
//! Compares every mandatory fixture and all 13 dialect paths to an independent
//! DuckDB source/terminal bag and the emitted canonical goal verdict.

use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json,
    ConfiguredSqlInput, ConstraintValue, OutcomeGoal, OutcomeGoalStatus, OutputDistribution,
    OutputValueCount, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
            .expect("source datatype")],
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
    let mut cell_count = 0;
    for case in cases {
        let id = case["id"].as_str().expect("stable ID");
        let classification = case["expected"]["classification"].as_str().expect("verdict");
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
            let sources = [ConfiguredSqlInput::new(id, &input, dialect, parser.as_ref())];
            let mut bundle = analyze_configured_inputs_with_catalog(&sources, &source_catalog())
                .unwrap_or_else(|error| panic!("{id}/{dialect}: parser: {error}"));
            let target = bundle.layers().last().expect("output layer").id().to_string();
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
                json["outcome_goals"][0]["requested"]["rows"],
                case["requested"]["rows"],
                "{id}/{dialect}"
            );
            if classification == "Exact" {
                let kind = case["expected"]["witness"].as_str();
                if let Some(kind) = kind {
                    assert_eq!(json["outcome_goals"][0]["witness"]["kind"], json!(kind), "{id}/{dialect}");
                } else {
                    assert_eq!(
                        json["outcome_goals"][0]["witness"]["kind"],
                        "empty_sources",
                        "{id}/{dialect}"
                    );
                }
            } else {
                assert!(json["outcome_goals"][0].get("witness").is_none(), "{id}/{dialect}");
            }

            // An unmaterializable overflow population is an arithmetic
            // contradiction, not a fake engine-execution claim.
            if let (Some(source), Some(expected)) = (source, expected) {
                let db = Connection::open_in_memory().expect("DuckDB");
                db.execute_batch("CREATE TABLE t(k INTEGER)").expect("source DDL");
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
                assert_eq!(terminal, frozen_terminal, "{id}/{dialect}: complete terminal");
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
        let mut bundle = analyze_inputs(&[SqlInput::inline("SELECT k FROM t")], dialect, parser.as_ref())
            .expect("syntax parsed without catalog");
        let layer = bundle.layers()[0].id().to_string();
        bundle.set_outcome_goals(&[request_for(case, &layer)]).expect("goal");
        assert_eq!(
            bundle.outcome_goals()[0].status(),
            OutcomeGoalStatus::Residual,
            "{dialect}: no authoritative datatype or full-bag construction"
        );
    }
}
