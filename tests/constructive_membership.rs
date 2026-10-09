//! Operator-local parity for EXISTS/IN and set tuple membership.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, local_constructive_witnesses,
    to_bundle_json, ComposedSemantics, ConfiguredSqlInput, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, WitnessDirection, WitnessObligation, WitnessOperator,
};

fn analyze(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .iter()
        .map(|relation| {
            RelationSchema::new(
                *relation,
                vec![SchemaColumn::from_sql_type("k", "BIGINT", "postgresql").expect("type")],
            )
            .expect("schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let dialect = dialect_from_name("postgresql").expect("dialect");
    let sql_input = SqlInput::inline(sql);
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "proof",
            &sql_input,
            "postgresql",
            dialect.as_ref(),
        )],
        &catalog,
    )
    .expect("analysis")
}

fn proof(
    sql: &str,
    kind: WitnessOperator,
) -> (
    sql_semantic_protocol::ConstructiveWitness,
    serde_json::Value,
) {
    let bundle = analyze(sql);
    let layer = bundle.layers().last().expect("query layer");
    let ComposedSemantics::Resolved(ref semantics) = layer.composed_semantics() else {
        panic!("resolved semantics expected");
    };
    let proof = local_constructive_witnesses(semantics)
        .into_iter()
        .find(|witness| witness.operator() == kind)
        .expect("operator evidence");
    let json = serde_json::from_str(&to_bundle_json(&bundle)).expect("json");
    (proof, json)
}

#[test]
fn exists_membership_keeps_correlated_keys_and_closed_world_absence() {
    let (w, json) = proof(
        "SELECT l.k FROM l WHERE EXISTS (SELECT 1 FROM r WHERE r.k = l.k)",
        WitnessOperator::Subquery,
    );
    let WitnessDirection::Feasible(passing) = w.qualifying() else {
        panic!("expected EXISTS matching witness: {:?}", w.qualifying());
    };
    let WitnessDirection::Feasible(failing) = w.rejected() else {
        panic!("expected EXISTS absent witness: {:?}", w.rejected());
    };
    assert!(passing.iter().any(|case| {
        case.obligations().iter().any(|obligation| {
        matches!(obligation, WitnessObligation::Membership { correlations, closed_world: true, .. }
            if correlations.len() == 1)
    })
    }));
    assert!(failing
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::Membership {
                    case: sql_semantic_protocol::SubqueryMembershipCase::NoCandidates,
                    ..
                }
            )
        })));
    assert!(json.to_string().contains("\"constructive_witnesses\""));
}

#[test]
fn not_in_retains_independent_nullable_rejection_cases() {
    let (w, _) = proof(
        "SELECT l.k FROM l WHERE l.k NOT IN (SELECT r.k FROM r)",
        WitnessOperator::Subquery,
    );
    let WitnessDirection::Feasible(passing) = w.qualifying() else {
        panic!("positive NOT IN cases: {:?}", w.qualifying());
    };
    let WitnessDirection::Feasible(failing) = w.rejected() else {
        panic!("negative NOT IN cases: {:?}", w.rejected());
    };
    assert!(passing.len() >= 2);
    assert!(failing.len() >= 3);
    assert!(failing
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::Membership {
                    case: sql_semantic_protocol::SubqueryMembershipCase::NoMatchNullCandidate,
                    ..
                }
            )
        })));
}

#[test]
fn set_all_duplicate_count_and_closed_world_zero_are_preserved() {
    let (w, _) = proof(
        "SELECT k FROM l EXCEPT ALL SELECT k FROM r",
        WitnessOperator::Set,
    );
    let WitnessDirection::Feasible(passing) = w.qualifying() else {
        panic!("set positive cases: {:?}", w.qualifying());
    };
    let WitnessDirection::Feasible(failing) = w.rejected() else {
        panic!("set rejected cases: {:?}", w.rejected());
    };
    assert!(passing
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::SetResultTuple {
                    matching_rows: 1,
                    nulls_equal: true
                }
            )
        })));
    assert!(failing
        .iter()
        .any(|case| case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::SetResultTuple {
                    matching_rows: 0,
                    nulls_equal: true
                }
            )
        })));
    assert!(passing.iter().all(|case| case
        .obligations()
        .iter()
        .filter(|obligation| matches!(
            obligation,
            WitnessObligation::SetTuple {
                closed_world: true,
                ..
            }
        ))
        .count()
        == 2));
}

#[test]
fn sql_engine_oracle_agrees_on_duplicate_set_membership_and_null_truth() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(k BIGINT); CREATE TABLE r(k BIGINT);
         INSERT INTO l VALUES (1), (1), (NULL);
         INSERT INTO r VALUES (1), (NULL);",
    )
    .expect("rows");
    let except_rows: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT k FROM l EXCEPT ALL SELECT k FROM r)",
            [],
            |row| row.get(0),
        )
        .expect("count");
    let membership_rows: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l WHERE k NOT IN (SELECT k FROM r)",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!((except_rows, membership_rows), (1, 0));
    let (set_proof, _) = proof(
        "SELECT k FROM l EXCEPT ALL SELECT k FROM r",
        WitnessOperator::Set,
    );
    let (in_proof, _) = proof(
        "SELECT l.k FROM l WHERE l.k NOT IN (SELECT r.k FROM r)",
        WitnessOperator::Subquery,
    );
    assert!(matches!(
        set_proof.qualifying(),
        WitnessDirection::Feasible(_)
    ));
    assert!(matches!(in_proof.rejected(), WitnessDirection::Feasible(_)));
}

#[test]
fn branch_tuple_counts_are_identical_to_existing_operator_local_cases() {
    use sql_semantic_protocol::{ProtocolStatement, SetWitnessDirection};
    let analyzed = analyze("SELECT k FROM l EXCEPT ALL SELECT k FROM r");
    let ProtocolStatement::Query(query) = &analyzed.inputs()[0].statements()[0] else {
        panic!("expected query");
    };
    let set = query.set_operation().expect("set");
    let (original_positive, original_negative) = set.witness_directions();
    let ComposedSemantics::Resolved(ref semantics) = analyzed.layers()[0].composed_semantics() else {
        panic!("resolved");
    };
    let normalized = local_constructive_witnesses(semantics).into_iter()
        .find(|proof| proof.operator() == WitnessOperator::Set).expect("set proof");
    for (original, current) in [
        (original_positive, normalized.qualifying()),
        (original_negative, normalized.rejected()),
    ] {
        let (SetWitnessDirection::Exact(cases), WitnessDirection::Feasible(converted)) =
            (original, current) else {
            panic!("both directions must remain exact");
        };
        assert_eq!(cases.len(), converted.len());
        for (case, translated) in cases.iter().zip(converted) {
            assert_eq!(
                case.obligations().len(),
                translated.obligations().iter().filter(|item|
                    matches!(item, WitnessObligation::SetTuple { .. })
                ).count()
            );
            assert!(translated.obligations().iter().any(|item| matches!(item,
                WitnessObligation::SetResultTuple { matching_rows, nulls_equal: true }
                    if *matching_rows == case.output_tuple_count()
            )));
            for original_branch in case.obligations() {
                assert!(translated.obligations().iter().any(|item| matches!(
                    item, WitnessObligation::SetTuple {
                        branch_identity, matching_rows, closed_world: true, ..
                    } if branch_identity == original_branch.branch_identity()
                        && *matching_rows == original_branch.matching_tuple_count()
                )));
            }
        }
    }
}

#[test]
fn membership_case_counts_match_legacy_exact_truth_directions() {
    use sql_semantic_protocol::{ProtocolStatement, SubqueryMembershipDirection};
    let analyzed = analyze(
        "SELECT l.k FROM l WHERE l.k NOT IN (SELECT r.k FROM r)"
    );
    let ProtocolStatement::Query(query) = &analyzed.inputs()[0].statements()[0] else {
        panic!("expected query");
    };
    let original = &query.subquery_witnesses()[0];
    let ComposedSemantics::Resolved(ref semantics) = analyzed.layers()[0].composed_semantics() else {
        panic!("resolved");
    };
    let normalized = local_constructive_witnesses(semantics).into_iter()
        .find(|proof| proof.operator() == WitnessOperator::Subquery).expect("membership proof");
    for (before, after) in [
        (original.qualifying(), normalized.qualifying()),
        (original.rejected(), normalized.rejected()),
    ] {
        let (SubqueryMembershipDirection::Exact(old), WitnessDirection::Feasible(new)) =
            (before, after) else {
            panic!("both directions must remain exact");
        };
        assert_eq!(old.len(), new.len());
        for (law, case) in old.iter().zip(new) {
            assert!(case.obligations().iter().any(|item| matches!(
                item, WitnessObligation::Membership { case: candidate, .. } if law == candidate
            )));
        }
    }
}

#[test]
fn intermediate_boundaries_require_producer_realization_instead_of_direct_writes() {
    use sql_semantic_protocol::{local_pending_producers, GroupBoundaryKind};
    let b = analyze(
        "WITH left_cte AS (SELECT k FROM l WHERE k > 0), right_cte AS (SELECT k FROM r)
         SELECT k FROM left_cte UNION ALL SELECT k FROM right_cte"
    );
    let ComposedSemantics::Resolved(ref semantics) = b.layers()[0].composed_semantics() else {
        panic!("resolved");
    };
    let intermediate = semantics.set_operations().iter().flat_map(|op| op.operation().branches())
        .filter_map(|branch| branch.witness_boundary())
        .any(|boundary| boundary.is_intermediate());
    let pending = local_pending_producers(semantics);
    if intermediate {
        assert!(!pending.is_empty());
        for obligation in pending {
            let WitnessObligation::Producer { boundary, physical_sources } = obligation else {
                panic!("only unresolved producer requirements");
            };
            assert_eq!(boundary.kind(), GroupBoundaryKind::Intermediate);
            assert!(physical_sources.iter().all(|source| source == "l" || source == "r"));
        }
    } else {
        let normalized = local_constructive_witnesses(semantics);
        let set = normalized.iter().find(|proof| proof.operator() == WitnessOperator::Set)
            .expect("set evidence");
        assert!(matches!(set.qualifying(), WitnessDirection::Residual { .. }));
    }
}

#[test]
fn unsupported_set_modifiers_do_not_get_normalized_as_constructive() {
    let (w, _) = proof(
        "SELECT k FROM l UNION ALL SELECT k FROM r LIMIT 1",
        WitnessOperator::Set,
    );
    assert!(matches!(w.qualifying(), WitnessDirection::Residual { .. }));
}
