//! Canonical operator-local constructive proof translation and fail-closed regression tests.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, local_constructive_witnesses,
    to_bundle_json, BooleanTruthCase, ComposedSemantics, ConfiguredSqlInput, ProofStrength,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessFormula,
    WitnessObligation, WitnessOperator,
};

fn catalog() -> RelationCatalog {
    let mut schemas = Vec::new();
    for relation in ["t", "l", "r"] {
        let schema = RelationSchema::new(
            relation,
            ["a", "b", "k"]
                .into_iter()
                .map(|column| {
                    SchemaColumn::from_sql_type(column, "INTEGER", "postgresql")
                        .expect("integer schema column")
                })
                .collect(),
        )
        .expect("typed source");
        schemas.push(schema);
    }
    RelationCatalog::from_schemas(&schemas).expect("relation catalog")
}

fn bundle(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let dialect_impl = dialect_from_name(dialect).expect("known dialect");
    let input = SqlInput::inline(sql);
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "proof",
            &input,
            dialect,
            dialect_impl.as_ref(),
        )],
        &catalog(),
    )
    .expect("analysis succeeds")
}

#[test]
fn coupled_boolean_reuses_one_row_identity_and_preserves_sql_not_true() {
    for &dialect in DIALECTS {
        let bundle = bundle("SELECT a FROM t WHERE a > 2 OR b < 0", dialect);
        let ComposedSemantics::Resolved(ref resolved) = bundle.layers()[0].composed_semantics()
        else {
            panic!("expected resolved composition for {dialect}");
        };
        let proofs = local_constructive_witnesses(resolved);
        let proof = proofs
            .iter()
            .find(|proof| proof.operator() == WitnessOperator::Boolean)
            .expect("coupled boolean normalized");
        for (direction, truth) in [
            (proof.qualifying(), BooleanTruthCase::True),
            (proof.rejected(), BooleanTruthCase::NotTrue),
        ] {
            let WitnessDirection::Feasible(cases) = direction else {
                panic!("expected exact typed {truth:?} witness for {dialect}: {direction:?}");
            };
            assert_eq!(cases.len(), 1);
            assert_eq!(cases[0].strength(), ProofStrength::Sufficient);
            let [WitnessObligation::Predicate(WitnessFormula::RowTruth {
                row,
                predicate: _,
                truth: actual,
            })] = cases[0].obligations()
            else {
                panic!("expected one coupled row truth");
            };
            assert_eq!(*actual, truth);
            assert_eq!(row.relation(), "t");
            assert_eq!(row.name(), "candidate");
        }

        let wire: serde_json::Value =
            serde_json::from_str(&to_bundle_json(&bundle)).expect("valid JSON");
        let encoded = &wire["layers"][0]["composed_semantics"]["constructive_witnesses"];
        assert_eq!(encoded[0]["operator"], "boolean");
        assert_eq!(encoded[0]["rejected"]["status"], "feasible");
        assert_eq!(
            encoded[0]["rejected"]["cases"][0]["obligations"][0]["formula"]["truth"],
            "not_true"
        );
        assert_eq!(to_bundle_json(&bundle), to_bundle_json(&bundle));
    }
}

#[test]
fn unmatched_join_encodes_closed_world_partner_absence_not_just_an_example() {
    let b = bundle("SELECT l.a FROM l LEFT JOIN r ON l.k = r.k", "postgresql");
    let ComposedSemantics::Resolved(ref resolved) = b.layers()[0].composed_semantics() else {
        panic!("expected resolved join");
    };
    let proof = local_constructive_witnesses(resolved)
        .into_iter()
        .find(|proof| proof.operator() == WitnessOperator::Join)
        .expect("join proof");
    let WitnessDirection::Feasible(cases) = proof.qualifying() else {
        panic!("expected qualifying join cases");
    };
    assert!(cases.iter().any(|case| case
        .obligations()
        .iter()
        .any(|obligation| matches!(obligation, WitnessObligation::JoinPair { .. }))));
    assert!(cases
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| matches!(
            obligation,
            WitnessObligation::NoMatchingPartner {
                closed_world: true,
                ..
            }
        ))));
    let WitnessDirection::Feasible(rejected) = proof.rejected() else {
        panic!("expected rejected join witness");
    };
    assert!(rejected
        .iter()
        .all(|case| case.obligations().iter().any(|obligation| matches!(
            obligation,
            WitnessObligation::NoMatchingPartner {
                closed_world: true,
                ..
            }
        ))));
}

#[test]
fn group_cases_preserve_bounded_counts_and_non_null_contributions() {
    let b = bundle(
        "SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2",
        "postgresql",
    );
    let ComposedSemantics::Resolved(ref resolved) = b.layers()[0].composed_semantics() else {
        panic!("expected resolved grouped query");
    };
    let proof = local_constructive_witnesses(resolved)
        .into_iter()
        .find(|proof| proof.operator() == WitnessOperator::Group)
        .expect("group witness");
    let WitnessDirection::Feasible(cases) = proof.qualifying() else {
        panic!(
            "expected a typed group construction: {:?}",
            proof.qualifying()
        );
    };
    assert!(cases
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| {
            matches!(obligation, WitnessObligation::Group { rows, .. } if rows.minimum() >= 2)
        })));
    let WitnessDirection::Feasible(rejected) = proof.rejected() else {
        panic!(
            "expected a rejected group construction: {:?}",
            proof.rejected()
        );
    };
    assert!(!rejected.is_empty());
}

#[test]
fn ranked_witness_preserves_strict_order_and_closed_world_predecessors() {
    let b = bundle(
        "SELECT ROW_NUMBER() OVER (ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn <= 2",
        "snowflake",
    );
    let ComposedSemantics::Resolved(ref resolved) = b.layers()[0].composed_semantics() else {
        panic!("expected resolved window");
    };
    let proof = local_constructive_witnesses(resolved)
        .into_iter()
        .find(|proof| proof.operator() == WitnessOperator::Window)
        .expect("window witness");
    let WitnessDirection::Feasible(cases) = proof.qualifying() else {
        panic!("expected ranking proof: {:?}", proof.qualifying());
    };
    assert!(cases
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::Ranked {
                    strict_unique: true,
                    closed_world: true,
                    ..
                }
            )
        })));
}

#[test]
fn independent_rows_and_sql_null_are_observed_by_duckdb_oracle() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE t (a INTEGER, b INTEGER);
         INSERT INTO t VALUES (3, NULL), (NULL, -1), (NULL, NULL), (1, 1);",
    )
    .expect("seed");
    let surviving: i64 = db
        .query_row("SELECT COUNT(*) FROM t WHERE a > 2 OR b < 0", [], |row| {
            row.get(0)
        })
        .expect("count");
    let rejected: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM t WHERE (a > 2 OR b < 0) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!((surviving, rejected), (2, 2));
    let b = bundle("SELECT a FROM t WHERE a > 2 OR b < 0", "duckdb");
    let ComposedSemantics::Resolved(ref resolved) = b.layers()[0].composed_semantics() else {
        panic!("expected resolved");
    };
    let proof = local_constructive_witnesses(resolved)
        .into_iter()
        .find(|proof| proof.operator() == WitnessOperator::Boolean)
        .expect("boolean proof");
    assert!(matches!(proof.qualifying(), WitnessDirection::Feasible(_)));
    assert!(matches!(proof.rejected(), WitnessDirection::Feasible(_)));
}
