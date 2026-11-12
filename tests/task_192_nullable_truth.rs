//! TASK-192: nullable disjunction and IS TRUE/IS FALSE have different 3VL laws.
//! Exactness requires the original full positive/negative/UNKNOWN partition,
//! not just agreement on which WHERE rows happen to survive.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name, physical_source_plan,
    BooleanTruthCase, BooleanWitnessDirection, ComposedSemantics, ConditionExactnessStatus,
    ConfiguredSqlInput, Predicate, ProtocolStatement, RelationCatalog, SqlInput, ValueDomain,
    WitnessDirection, WitnessFormula, WitnessObligation,
};

const ORIGINAL: &str = include_str!("../docs/task-177-predicate-oracles.json");
const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn original_case<'a>(oracles: &'a Value, id: &str) -> &'a Value {
    oracles["cases"]
        .as_array()
        .expect("original independently pinned predicates")
        .iter()
        .find(|row| row["id"] == id)
        .expect("original case")
}

fn physical_ids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut stmt = db.prepare(sql).expect("native full rowid query");
    let rows = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("native physical identities");
    let mut found: Vec<_> = rows.map(|row| row.expect("physical row")).collect();
    found.sort();
    found
}

fn expected_ids(case: &Value, key: &str) -> Vec<i64> {
    let mut ids: Vec<_> =
        serde_json::from_value(case[key].clone()).expect("physical row identities");
    ids.sort();
    ids
}

#[test]
fn original_s012_s016_and_s017_preserve_native_three_valued_truth_partitions() {
    let prior: Value = serde_json::from_str(ORIGINAL).expect("independent source oracle");
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(prior["source_setup"].as_str().expect("source DDL and rows"))
        .expect("complete independent source bag");
    assert_eq!(
        physical_ids(&db, "SELECT id FROM orders"),
        vec![-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );

    for (id, canonical_truth) in [
        (
            "88-C177-S012",
            "((amount > 1 OR amount IS NULL) AND id > 0)",
        ),
        ("88-C177-S016", "((amount > 0) AND amount IS NOT NULL)"),
        (
            "88-C177-S017",
            "((NOT (amount > 0)) AND amount IS NOT NULL)",
        ),
    ] {
        let case = original_case(&prior, id);
        let sql = case["sql"].as_str().expect("unchanged original query");
        let predicate = case["predicate"].as_str().expect("original predicate");
        let original_terminal = physical_ids(&db, sql);
        assert_eq!(
            original_terminal,
            expected_ids(case, "expected_selected_ids"),
            "{id}: original selected ID occurrences"
        );
        for (truth, key) in [
            ("IS TRUE", "expected_selected_ids"),
            ("IS FALSE", "expected_false_ids"),
            ("IS UNKNOWN", "expected_unknown_ids"),
        ] {
            let native = physical_ids(
                &db,
                &format!("SELECT id FROM orders WHERE ({predicate}) {truth}"),
            );
            assert_eq!(
                native,
                expected_ids(case, key),
                "{id}: original {truth} source partition"
            );
            let normalized = physical_ids(
                &db,
                &format!("SELECT id FROM orders WHERE ({canonical_truth}) {truth}"),
            );
            assert_eq!(
                normalized, native,
                "{id}: normalized expression changed the original {truth} partition"
            );
        }
        let rejected = physical_ids(
            &db,
            &format!("SELECT id FROM orders WHERE ({predicate}) IS NOT TRUE"),
        );
        let normalized_rejected = physical_ids(
            &db,
            &format!("SELECT id FROM orders WHERE ({canonical_truth}) IS NOT TRUE"),
        );
        assert_eq!(rejected, normalized_rejected, "{id}: NOT TRUE source bag");
        let mut expected_rejected = expected_ids(case, "expected_false_ids");
        expected_rejected.extend(expected_ids(case, "expected_unknown_ids"));
        expected_rejected.sort();
        assert_eq!(
            rejected, expected_rejected,
            "{id}: every rejected occurrence"
        );
    }
}

#[test]
fn exact_nullable_original_predicates_do_not_erase_id_or_null_truth() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("43 frozen original SQLs");
    let mut visited = 0;
    for case in freeze["cases"].as_array().expect("43 original samples") {
        let id = case["id"].as_str().expect("case ID");
        if !matches!(id, "88-C177-S012" | "88-C177-S016" | "88-C177-S017") {
            continue;
        }
        let sql = case["sql"].as_str().expect("unchanged original SQL");
        for dialect in freeze["dialects"]
            .as_array()
            .expect("13 public parser dialects")
        {
            let name = dialect.as_str().expect("name");
            let parser = dialect_from_name(name).expect("supported public parser");
            let protocol = analyze_sql(sql, name, parser.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let [ProtocolStatement::Query(query)] = protocol.statements() else {
                panic!("{id}/{name}: expected full query");
            };
            assert_eq!(
                query.condition_exactness().status(),
                ConditionExactnessStatus::Exact,
                "{id}/{name}: truth-preserving supported predicate; residuals={:?} source={:?}",
                query.condition_exactness().residual_conditions(),
                query.predicates().where_predicate()
            );
            let Some(where_predicate) = query.predicates().where_predicate() else {
                panic!("{id}/{name}: WHERE dropped");
            };
            assert!(
                matches!(where_predicate, Predicate::And(_)),
                "{id}/{name}: 3VL guard lost"
            );
            match id {
                "88-C177-S012" => assert!(
                    matches!(query.output().columns()[0].domain(), ValueDomain::Ranges(_)),
                    "{id}/{name}: independent id > 0 interval must survive nullable OR"
                ),
                "88-C177-S016" | "88-C177-S017" => assert!(
                    matches!(
                        query.output().columns()[0].domain(),
                        ValueDomain::Unbounded | ValueDomain::Unknown(_)
                    ),
                    "{id}/{name}: a filter on amount cannot invent finite id values"
                ),
                _ => unreachable!(),
            }
            visited += 1;
        }
    }
    assert_eq!(visited, 39);
}

#[test]
fn typed_nullable_predicates_have_exact_true_and_not_true_boolean_witnesses() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("43 original cases");
    for dialect in freeze["dialects"].as_array().expect("13 public parsers") {
        let name = dialect.as_str().expect("dialect");
        let parser = dialect_from_name(name).expect("parser");
        let ddl = SqlInput::inline("CREATE TABLE orders (id INTEGER, amount INTEGER)");
        let schema = sql_semantic_protocol::analyze_inputs(&[ddl], name, parser.as_ref())
            .expect("typed direct SQL DDL");
        let catalog =
            RelationCatalog::from_schemas(schema.source_schemas()).expect("typed original source");
        for id in ["88-C177-S012", "88-C177-S016", "88-C177-S017"] {
            let case = freeze["cases"]
                .as_array()
                .expect("cases")
                .iter()
                .find(|row| row["id"] == id)
                .expect("case");
            let sql = case["sql"].as_str().expect("source SQL");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "task192-nullable-truth",
                &input,
                name,
                parser.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let layer = bundle.layers().last().expect("one output layer");
            let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
                panic!("{id}/{name}: source complete query cannot be unresolved");
            };
            assert!(
                composed.boolean_witnesses().iter().any(|item| {
                    matches!(
                        item.witness().qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ) && matches!(
                        item.witness().rejected(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                    )
                }),
                "{id}/{name}: need an exact typed source-row TRUE and NOT TRUE witness"
            );
        }
    }
}

#[test]
fn original_cross_column_or_and_not_have_exact_typed_boolean_witnesses_not_scalar_exactness() {
    // These bounded witnesses are constructive row-local evidence, not a
    // promotion of a residual *whole-query* scalar-domain contract.
    let freeze: Value = serde_json::from_str(FREEZE).expect("original 43 SQL cases");
    let prior: Value = serde_json::from_str(ORIGINAL).expect("frozen native source bags");
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(prior["source_setup"].as_str().expect("complete source"))
        .expect("original native population");

    for id in ["88-C177-S010", "88-C177-S011"] {
        let oracle = original_case(&prior, id);
        let predicate = oracle["predicate"]
            .as_str()
            .expect("original Boolean condition");
        let canonical = match id {
            "88-C177-S010" => "(amount > 1 OR id > 0)",
            "88-C177-S011" => "NOT (amount > 1)",
            _ => unreachable!("fixed original witness cases"),
        };
        for (test, key) in [
            ("IS TRUE", "expected_selected_ids"),
            ("IS FALSE", "expected_false_ids"),
            ("IS UNKNOWN", "expected_unknown_ids"),
        ] {
            assert_eq!(
                physical_ids(
                    &db,
                    &format!("SELECT id FROM orders WHERE ({canonical}) {test}")
                ),
                expected_ids(oracle, key),
                "{id}: independent native {test} partition"
            );
            assert_eq!(
                physical_ids(
                    &db,
                    &format!("SELECT id FROM orders WHERE ({predicate}) {test}")
                ),
                expected_ids(oracle, key),
                "{id}: original native {test} partition"
            );
        }
        let mut complement = expected_ids(oracle, "expected_false_ids");
        complement.extend(expected_ids(oracle, "expected_unknown_ids"));
        complement.sort();
        assert_eq!(
            physical_ids(
                &db,
                &format!("SELECT id FROM orders WHERE ({predicate}) IS NOT TRUE")
            ),
            complement,
            "{id}: duplicate-safe rejected source occurrences"
        );

        let case = freeze["cases"]
            .as_array()
            .expect("43 variants")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original SQL");
        let sql = case["sql"].as_str().expect("original text");

        for name in freeze["dialects"].as_array().expect("13 dialects") {
            let name = name.as_str().expect("dialect");
            let parser = dialect_from_name(name).expect("public parser");
            let ddl = SqlInput::inline("CREATE TABLE orders (id INTEGER, amount INTEGER)");
            let schema = sql_semantic_protocol::analyze_inputs(&[ddl], name, parser.as_ref())
                .expect("typed original source schema");
            let catalog =
                RelationCatalog::from_schemas(schema.source_schemas()).expect("source catalog");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "task192-residual-witness",
                &input,
                name,
                parser.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let layer = bundle.layers().last().expect("query layer");
            let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
                panic!("{id}/{name}: missing typed source");
            };
            assert_eq!(
                composed.condition_exactness().status(),
                if id == "88-C177-S011" {
                    ConditionExactnessStatus::Exact
                } else {
                    ConditionExactnessStatus::Residual
                },
                "{id}/{name}: signed NOT-ordered comparison has a typed scalar interval, unlike cross-column OR"
            );
            assert!(
                composed.boolean_witnesses().iter().any(|item| {
                    matches!(
                        item.witness().qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ) && matches!(
                        item.witness().rejected(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                    )
                }),
                "{id}/{name}: expected exact row-local TRUE/NOT TRUE witness"
            );
            // Physical TRUE/NOT TRUE obligations are constructive for both
            // cases. Only the typed single-column ordered NOT has an exact
            // scalar complement; cross-column OR remains a residual domain.
            let physical = physical_source_plan(&bundle, layer.id());
            assert_eq!(physical.sources(), &["orders".to_string()]);
            for (direction, truth) in [
                (physical.qualifying(), BooleanTruthCase::True),
                (physical.rejected(), BooleanTruthCase::NotTrue),
            ] {
                let WitnessDirection::Feasible(cases) = direction else {
                    panic!("{id}/{name}: missing physical {truth:?} row witness: {physical:?}");
                };
                assert!(
                    cases
                        .iter()
                        .any(|case| case.obligations().iter().any(|obligation| {
                            matches!(
                                obligation,
                                WitnessObligation::Predicate(WitnessFormula::RowTruth {
                                    truth: actual,
                                    ..
                                }) if *actual == truth
                            )
                        })),
                    "{id}/{name}: physical row witness has wrong TRUE or NOT TRUE direction"
                );
            }
        }
    }
}

#[test]
fn unrelated_not_and_cross_column_or_remain_conservative() {
    let parser = dialect_from_name("duckdb").expect("DuckDB public parser");
    for sql in [
        "SELECT id FROM orders WHERE NOT (amount > 1)",
        "SELECT id FROM orders WHERE amount > 1 OR id > 0",
    ] {
        let protocol = analyze_sql(sql, "duckdb", parser.as_ref()).expect("public query");
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("one original query");
        };
        assert_ne!(
            query.condition_exactness().status(),
            ConditionExactnessStatus::Exact,
            "{sql}: no blanket removal of true residuals"
        );
    }
}

#[test]
fn floating_source_without_nan_and_signed_zero_laws_cannot_be_exact() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let parser = dialect_from_name("duckdb").expect("source DDL dialect");
    let ddl = SqlInput::inline("CREATE TABLE orders (id INTEGER, amount DOUBLE)");
    let source = sql_semantic_protocol::analyze_inputs(&[ddl], "duckdb", parser.as_ref())
        .expect("floating source schema");
    let catalog = RelationCatalog::from_schemas(source.source_schemas())
        .expect("physical floating type facts");

    for id in ["88-C177-S012", "88-C177-S016", "88-C177-S017"] {
        let sql = freeze["cases"]
            .as_array()
            .expect("43 SQL cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original case")["sql"]
            .as_str()
            .expect("original SQL");
        for dialect_name in freeze["dialects"].as_array().expect("13 parser dialects") {
            let name = dialect_name.as_str().expect("dialect");
            let dialect = dialect_from_name(name).expect("parser");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "task192-unattested-float",
                &input,
                name,
                dialect.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let layer = bundle.layers().last().expect("query");
            let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
                panic!("{id}/{name}: source query");
            };
            assert_ne!(
                composed.condition_exactness().status(),
                ConditionExactnessStatus::Exact,
                "{id}/{name}: DOUBLE without an attested NaN/ordering law cannot certify integer-style exactness"
            );
        }
    }
}
