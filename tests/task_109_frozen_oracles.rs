//! TASK-109 executable acceptance contract.
//! The expected row bags and truth-partition identities are independently frozen
//! and are not generated from the implementation's witness enumeration.

mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    to_bundle_json, ConfiguredSqlInput, JoinKind, JoinPopulationPattern, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessObligation,
};

const MATRIX: &str = include_str!("../docs/task-109-acceptance-freeze.json");

fn fixture_bundle(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .iter()
        .map(|name| {
            RelationSchema::new(
                *name,
                ["k", "a"]
                    .iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(*column, "INTEGER", "postgresql")
                            .expect("signed INTEGER schema")
                    })
                    .collect(),
            )
            .expect("relation schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let parser = dialect_from_name(dialect).expect("registered parser dialect");
    let text = SqlInput::inline(sql);
    let input = [ConfiguredSqlInput::new(
        "task109-oracle",
        &text,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&input, &catalog).expect("canonical SQL analysis")
}

fn source_insert(db: &Connection, relation: &str, bag: &[Value]) {
    for row in bag {
        let id = row["id"].as_str().expect("frozen physical row identity");
        // All frozen values are typed signed INTEGER or SQL NULL.
        // Only fixed fixture literals are interpolated, never consumer SQL.
        let k = row["k"].as_i64().map_or("NULL".into(), |n| n.to_string());
        let a = row["a"].as_i64().map_or("NULL".into(), |n| n.to_string());
        assert!(id.chars().all(|ch| ch.is_ascii_alphanumeric()));
        db.execute_batch(&format!(
            "INSERT INTO {relation}(rid,k,a) VALUES ('{id}',{k},{a})"
        ))
        .expect("exact physical source row");
    }
}

fn queried_bag(db: &Connection, sql: &str, cols: usize) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("independent SQL execution");
    let rows = statement
        .query_map([], |row| {
            let mut tuple = Vec::with_capacity(cols);
            for index in 0..cols {
                tuple.push(row.get::<_, Option<i64>>(index)?);
            }
            Ok(tuple)
        })
        .expect("executed rows");
    let mut output = rows
        .map(|row| row.expect("typed INTEGER output"))
        .collect::<Vec<_>>();
    output.sort();
    output
}

fn pair_bag(db: &Connection, predicate: &str, not_true: bool) -> Vec<Vec<String>> {
    let truth = if not_true { "IS NOT TRUE" } else { "IS TRUE" };
    let sql = format!("SELECT l.rid, r.rid FROM l CROSS JOIN r WHERE ({predicate}) {truth}");
    let mut stmt = db.prepare(&sql).expect("SQL three-valued predicate");
    let rows = stmt
        .query_map([], |row| {
            Ok(vec![row.get::<_, String>(0)?, row.get::<_, String>(1)?])
        })
        .expect("identity bag");
    let mut result = rows
        .map(|r| r.expect("typed identities"))
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn assert_typed_domain(id: &str, actual: &[Vec<Option<i64>>], spec: &Value) {
    if actual.is_empty() {
        assert_eq!(spec["type"], "empty", "{id}: empty bag domain");
        assert_eq!(
            spec["nullable"], false,
            "{id}: absence has no nullable rows"
        );
        return;
    }
    let domains = if spec.get("type").is_some() {
        vec![spec]
    } else {
        vec![&spec["k"], &spec["a"]]
    };
    for (idx, domain) in domains.iter().enumerate() {
        let nulls = actual.iter().filter(|row| row[idx].is_none()).count();
        assert_eq!(
            domain["nullable"],
            nulls > 0,
            "{id}/column {idx}: NULL membership"
        );
        let non_null = actual.iter().filter_map(|row| row[idx]).collect::<Vec<_>>();
        if !non_null.is_empty() {
            let lower = *non_null.iter().min().expect("nonempty minimum");
            let upper = *non_null.iter().max().expect("nonempty maximum");
            assert_eq!(domain["type"], "integer", "{id}: typed numerical bounds");
            assert_eq!(domain["lower"], lower.to_string(), "{id}: lower bound");
            assert_eq!(domain["upper"], upper.to_string(), "{id}: upper bound");
            assert_eq!(domain["lower_inclusive"], true);
            assert_eq!(domain["upper_inclusive"], true);
        }
    }
}

#[test]
fn every_frozen_variant_executes_complete_physical_and_terminal_bags() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen task inventory");
    let variants = matrix["variants"].as_array().expect("mandatory variants");
    assert_eq!(
        variants.len(),
        5,
        "review changes to frozen scope explicitly"
    );
    for item in variants {
        let id = item["id"].as_str().expect("immutable variant identity");
        let oracle = &item["execution_oracle"];
        let db = Connection::open_in_memory().expect("independent DuckDB oracle");
        db.execute_batch(
            "CREATE TABLE l(rid VARCHAR, k INTEGER, a INTEGER);
             CREATE TABLE r(rid VARCHAR, k INTEGER, a INTEGER);",
        )
        .expect("complete schema");
        for name in ["l", "r"] {
            source_insert(
                &db,
                name,
                item["source_bags"][name].as_array().expect("closed bag"),
            );
            let actual: i64 = db
                .query_row(&format!("SELECT COUNT(*) FROM {name}"), [], |row| {
                    row.get(0)
                })
                .expect("physical cardinality");
            assert_eq!(
                actual as usize,
                item["source_bags"][name].as_array().unwrap().len(),
                "{id}/{name}: source closed"
            );
        }
        let columns = oracle["output_columns"]
            .as_u64()
            .expect("typed projection arity") as usize;
        let actual = queried_bag(&db, item["sql"].as_str().expect("SQL variant"), columns);
        let mut expected: Vec<Vec<Option<i64>>> =
            serde_json::from_value(oracle["expected_output_bag"].clone())
                .expect("frozen typed expected output bag");
        expected.sort();
        assert_eq!(
            actual, expected,
            "{id}: complete terminal values and multiplicities"
        );
        assert_eq!(
            actual.len() as u64,
            item["matched_count"].as_u64().unwrap(),
            "{id}: output rows"
        );
        assert_typed_domain(id, &actual, &item["value_domain"]);

        let predicate = oracle["truth_predicate"].as_str().expect("3VL oracle");
        let positive = pair_bag(&db, predicate, false);
        let negative = pair_bag(&db, predicate, true);
        let mut expected_positive: Vec<Vec<String>> =
            serde_json::from_value(item["matched"].clone()).expect("frozen source row pairs");
        let mut expected_negative: Vec<Vec<String>> =
            serde_json::from_value(item["not_true_pairs"].clone()).expect("frozen rejected pairs");
        expected_positive.sort();
        expected_negative.sort();
        assert_eq!(positive, expected_positive, "{id}: matched row identities");
        assert_eq!(
            negative, expected_negative,
            "{id}: false or unknown identities"
        );
        assert_eq!(
            positive.len() + negative.len(),
            oracle["closed_world_pair_count"].as_u64().unwrap() as usize,
            "{id}: no omitted physical pair"
        );
    }
}

#[test]
fn each_frozen_variant_has_canonical_join_ast_and_same_dialect_semantics() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen matrix");
    for item in matrix["variants"].as_array().expect("mandatory") {
        let id = item["id"].as_str().unwrap();
        let mut reference: Option<Value> = None;
        for &dialect in common::DIALECTS {
            let b = fixture_bundle(item["sql"].as_str().unwrap(), dialect);
            let doc: Value = serde_json::from_str(&to_bundle_json(&b)).expect("canonical wire");
            let mut statement = doc["inputs"][0]["statements"][0].clone();
            assert_eq!(statement["kind"], "query", "{id}/{dialect}");
            let joins = statement["joins"].as_array().expect("canonical joins");
            assert_eq!(joins.len(), 1, "{id}/{dialect}: binary shape");
            assert_eq!(joins[0]["kind"], "inner", "{id}/{dialect}");
            let mut equality_count = 0;
            let mut scalar_count = 0;
            fn count_conjuncts(v: &Value, equality_count: &mut usize, scalar_count: &mut usize) {
                if v["kind"] == "and" {
                    for item in v["operands"].as_array().expect("AND operands") {
                        count_conjuncts(item, equality_count, scalar_count);
                    }
                } else if v["kind"] == "comparison" && v["operator"] == "eq" {
                    *equality_count += 1;
                } else {
                    *scalar_count += 1;
                }
            }
            count_conjuncts(
                &joins[0]["condition"],
                &mut equality_count,
                &mut scalar_count,
            );
            assert_eq!(
                equality_count,
                item["canonical_ast_expectation"]["key_equality_conjuncts"]
                    .as_u64()
                    .unwrap() as usize,
                "{id}/{dialect}: AST equalities"
            );
            assert_eq!(
                equality_count + scalar_count,
                item["canonical_ast_expectation"]["on_conjuncts"]
                    .as_u64()
                    .unwrap() as usize,
                "{id}/{dialect}: ON conjunction"
            );
            let where_kind = item["canonical_ast_expectation"]["where_kind"]
                .as_str()
                .unwrap();
            if where_kind == "none" {
                assert!(
                    statement["predicates"]["where"].is_null(),
                    "{id}/{dialect}: no WHERE"
                );
            } else {
                assert_eq!(
                    statement["predicates"]["where"]["kind"], where_kind,
                    "{id}/{dialect}: WHERE kind"
                );
            }
            statement["diagnostics"] = Value::Null;
            if let Some(reference) = &reference {
                assert_eq!(
                    &statement, reference,
                    "{id}/{dialect}: canonical parser equivalence"
                );
            } else {
                reference = Some(statement);
            }
        }
    }
}

#[test]
fn frozen_positive_counts_have_exact_closed_source_witnesses() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen matrix");
    for item in matrix["variants"].as_array().unwrap() {
        if item["matched_count"].as_u64().unwrap() == 0 {
            // A zero on one frozen bag is not a universal impossible result.
            continue;
        }
        let id = item["id"].as_str().unwrap();
        let b = fixture_bundle(item["sql"].as_str().unwrap(), "postgresql");
        let wanted = item["matched_count"].as_u64().unwrap();
        let proof = physical_joint_source_plan(&b, &[(b.layers()[0].id(), wanted)]);
        let WitnessDirection::Feasible(cases) = proof.outcome() else {
            panic!("{id}: missing constructive source-level exact row count: {proof:?}");
        };
        assert!(
            cases.iter().any(|case| {
                let mut closed = 0;
                let mut proper = false;
                for o in case.obligations() {
                    match o {
                        WitnessObligation::ClosedWorld { .. } => closed += 1,
                        WitnessObligation::JoinPopulation {
                            join_kind: JoinKind::Inner,
                            output_rows,
                            closed_world: true,
                            pattern:
                                JoinPopulationPattern::CommonMatched
                                | JoinPopulationPattern::DistinctMatched
                                | JoinPopulationPattern::GroupedMatched { .. },
                            ..
                        } if *output_rows == wanted => proper = true,
                        _ => {}
                    }
                }
                proper && closed == 2
            }),
            "{id}: full join source law, not an unconstrained sampled pair"
        );
    }
}

#[test]
fn reviewed_conditional_joins_fail_closed_without_inventing_exact_pair_laws() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("versioned matrix");
    let cases = matrix["conditional_boundary"]["cases"].as_array().expect("review cases");
    assert_eq!(cases.len(), 4);
    for case in cases {
        let id = case["id"].as_str().expect("named boundary");
        assert_eq!(case["owner"], "TASK-109", "{id}: bounded owner");
        assert_eq!(case["review_disposition"], "accepted_fail_closed_nonmandatory");
        assert!(case["missing_evidence"].as_str().is_some_and(|note| !note.is_empty()));
        let sql = case["sql"].as_str().expect("SQL variant");
        let analyzed = if id == "J109-STRING" {
            let schemas = ["l", "r"].into_iter().map(|name| {
                RelationSchema::new(
                    name,
                    ["k", "a"].into_iter().map(|col| {
                        SchemaColumn::from_sql_type(
                            col,
                            if col == "k" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        ).expect("physical source type")
                    }).collect(),
                ).expect("schema")
            }).collect::<Vec<_>>();
            let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
            let source = SqlInput::inline(sql);
            let parser = dialect_from_name("postgresql").expect("PostgreSQL");
            let input = [ConfiguredSqlInput::new("task109-conditional", &source,
                "postgresql", parser.as_ref())];
            analyze_configured_inputs_with_catalog(&input, &catalog).expect("typed analysis")
        } else {
            fixture_bundle(sql, "postgresql")
        };
        let proof = physical_joint_source_plan(
            &analyzed,
            &[(analyzed.layers()[0].id(), 4)],
        );
        assert!(
            matches!(proof.outcome(), WitnessDirection::Residual { .. }),
            "{id}: absent evidence must remain typed residual, not fabricated feasibility/impossibility: {proof:?}"
        );
    }
}
