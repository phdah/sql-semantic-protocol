//! Public API verification of exact finite closed-world three-source bags.
//! DuckDB exercises the same immutable cases independently.
use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_chain_bag_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalFiniteChainExact, PhysicalFiniteChainProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn attested_pk_fk_constraints() -> Vec<RelationConstraintSet> {
    let evidence = || {
        vec![ConstraintEvidence::new(
            ConstraintProvenance::new(
                ConstraintSourceKind::ExternalMetadata,
                "task-115-enforced-source-evidence",
            )
            .expect("authoritative constraint provenance"),
            ConstraintEnforcement::Enforced,
        )]
    };
    vec![
        RelationConstraintSet::new(
            "l",
            vec![
                RelationConstraint::primary_key(vec!["id".to_string()], evidence()).expect("l PK"),
            ],
        )
        .expect("l constraints"),
        RelationConstraintSet::new(
            "r",
            vec![
                RelationConstraint::primary_key(vec!["id".to_string()], evidence()).expect("r PK"),
                RelationConstraint::foreign_key(
                    vec!["tid".to_string()],
                    "t",
                    vec!["tid".to_string()],
                    evidence(),
                )
                .expect("r->t FK"),
            ],
        )
        .expect("r constraints"),
        RelationConstraintSet::new(
            "t",
            vec![
                RelationConstraint::primary_key(vec!["tid".to_string()], evidence()).expect("t PK"),
            ],
        )
        .expect("t constraints"),
    ]
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    ["l", "r", "t"]
        .into_iter()
        .map(|source| {
            let rows = case["physical_source_bags"][source]
                .as_array()
                .expect("frozen source bag")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("fully named physical row")
                        .iter()
                        .map(|(name, value)| {
                            let literal = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(value) = value.as_i64() {
                                LiteralValue::Number(value.to_string())
                            } else {
                                LiteralValue::Text(
                                    value.as_str().expect("typed string identity").to_string(),
                                )
                            };
                            (name.to_string(), literal)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("complete source row")
                })
                .collect();
            PhysicalWriteState::new(source, rows).expect("physical source identity")
        })
        .collect()
}

fn json_terminal(rows: Vec<BTreeMap<String, LiteralValue>>) -> Vec<Value> {
    let mut values = rows
        .into_iter()
        .map(|row| {
            Value::Object(
                row.into_iter()
                    .map(|(column, value)| {
                        let json = match value {
                            LiteralValue::Null => Value::Null,
                            LiteralValue::Number(value) => {
                                json!(value.parse::<i64>().expect("integer domain"))
                            }
                            LiteralValue::Text(value) => json!(value),
                            LiteralValue::Boolean(value) => json!(value),
                            _ => panic!("unsupported terminal literal"),
                        };
                        (column, json)
                    })
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    values.sort_by_key(Value::to_string);
    values
}

fn frozen_triplets(case: &Value, kind: &str) -> Vec<[String; 3]> {
    let mut triples = case[kind]
        .as_array()
        .expect("complete P or NOT TRUE bag")
        .iter()
        .map(|triplet| {
            let components = triplet.as_array().expect("source identities");
            std::array::from_fn(|index| {
                components[index]
                    .as_str()
                    .expect("stable identity")
                    .to_string()
            })
        })
        .collect::<Vec<_>>();
    triples.sort();
    triples
}

#[test]
fn all_frozen_bags_match_normalized_sql_constructive_proof_in_thirteen_dialects() {
    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("task-115 immutable cases");
    for case in frozen["cases"].as_array().expect("all cases") {
        let case_id = case["id"].as_str().expect("id");
        for dialect in frozen["dialects"].as_array().expect("thirteen dialects") {
            let dialect = dialect.as_str().expect("canonical dialect name");
            let source_schemas = ["l", "r", "t"]
                .into_iter()
                .map(|source| {
                    let columns = case["source_column_types"][source]
                        .as_object()
                        .expect("typed physical columns")
                        .iter()
                        .map(|(column, declaration)| {
                            let sql_type = declaration
                                .as_str()
                                .expect("typed column")
                                .split_whitespace()
                                .next()
                                .expect("SQL datatype");
                            SchemaColumn::from_sql_type(column, sql_type, "postgresql")
                                .expect("attested physical scalar type")
                        })
                        .collect();
                    RelationSchema::new(source, columns).expect("source schema")
                })
                .chain(std::iter::once(
                    RelationSchema::new(
                        "stage",
                        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                            .expect("stage key")],
                    )
                    .expect("stage schema"),
                ))
                .collect::<Vec<_>>();
            let catalog = RelationCatalog::from_schemas(&source_schemas).expect("typed catalog");
            let sql = case["sql"].as_str().expect("canonical SQL");
            let statements = sql
                .split(';')
                .map(str::trim)
                .filter(|statement| !statement.is_empty())
                .map(SqlInput::inline)
                .collect::<Vec<_>>();
            let ids = (0..statements.len())
                .map(|index| format!("task-115-{case_id}-{index}"))
                .collect::<Vec<_>>();
            let parser = dialect_from_name(dialect).expect("supported dialect");
            let configured = statements
                .iter()
                .zip(&ids)
                .map(|(sql, id)| ConfiguredSqlInput::new(id, sql, dialect, parser.as_ref()))
                .collect::<Vec<_>>();
            let mut analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{case_id}/{dialect}: {error:?}"));
            if case["variant"] == "pk_fk_constrained_sources" {
                analyzed.enrich_relation_constraints(&attested_pk_fk_constraints());
            }
            let target = analyzed.layers().last().expect("frozen terminal layer");
            let proof = physical_finite_chain_bag_plan(&analyzed, target.id(), &snapshots(case));
            let PhysicalFiniteChainProof::Exact(exact) = proof else {
                panic!("{case_id}/{dialect}: finite canonical proof failed: {proof:?}");
            };
            let PhysicalFiniteChainExact {
                sources,
                filtered_true_ids,
                filtered_not_true_ids,
                mut selected,
                mut rejected,
                terminal,
                terminal_domains,
                ..
            } = *exact;
            assert_eq!(sources, ["l", "r", "t"].map(str::to_string));
            selected.sort();
            rejected.sort();
            assert_eq!(
                selected,
                frozen_triplets(case, "selected_physical_triplets"),
                "{case_id}/{dialect}: SQL TRUE physical identity bag"
            );
            assert_eq!(
                rejected,
                frozen_triplets(case, "rejected_not_true_triplets"),
                "{case_id}/{dialect}: SQL NOT TRUE physical identity bag"
            );
            let mut actual_true = filtered_true_ids;
            let mut actual_not_true = filtered_not_true_ids;
            actual_true.sort();
            actual_not_true.sort();
            let frozen_ids = |name: &str| {
                let mut ids = case[name]
                    .as_array()
                    .expect("filter identity bag")
                    .iter()
                    .map(|v| v.as_str().expect("stable source identity").to_string())
                    .collect::<Vec<_>>();
                ids.sort();
                ids
            };
            assert_eq!(
                actual_true,
                frozen_ids("filtered_true_ids"),
                "{case_id}/{dialect}"
            );
            assert_eq!(
                actual_not_true,
                frozen_ids("filtered_not_true_ids"),
                "{case_id}/{dialect}"
            );
            let grouped = case["variant"] == "downstream_group_count";
            let mut expected = case["expected_terminal_bag"]
                .as_array()
                .expect("terminal bag")
                .iter()
                .map(|row| {
                    if grouped {
                        json!({"k": row["k"], "n": row["n"]})
                    } else {
                        json!({"k": row["k"]})
                    }
                })
                .collect::<Vec<_>>();
            expected.sort_by_key(Value::to_string);
            assert_eq!(
                json_terminal(terminal),
                expected,
                "{case_id}/{dialect}: exact canonical terminal bag"
            );
            for (column, actual_domain) in terminal_domains {
                let expected_domain = &case["terminal_domains"][&column];
                assert_eq!(
                    actual_domain.null_allowed(),
                    expected_domain["null_allowed"]
                        .as_bool()
                        .expect("NULL domain"),
                    "{case_id}/{dialect}: {column} NULL domain"
                );
                let expected_allowed = expected_domain["allowed"]
                    .as_array()
                    .expect("finite value domain")
                    .iter()
                    .map(|value| i128::from(value.as_i64().expect("integer value")))
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual_domain.allowed(),
                    expected_allowed,
                    "{case_id}/{dialect}: {column} finite values"
                );
                let expected_bounds = match (
                    expected_domain["lower"]["value"].as_i64(),
                    expected_domain["upper"]["value"].as_i64(),
                ) {
                    (Some(lower), Some(upper)) => {
                        assert_eq!(expected_domain["lower"]["inclusive"], true);
                        assert_eq!(expected_domain["upper"]["inclusive"], true);
                        Some((i128::from(lower), i128::from(upper)))
                    }
                    _ => None,
                };
                assert_eq!(
                    actual_domain.bounds(),
                    expected_bounds,
                    "{case_id}/{dialect}: {column} inclusive bounds"
                );
            }
        }
    }
}

#[test]
fn incomplete_or_contradictory_physical_evidence_never_certifies_a_join_chain() {
    let schemas = [
        (
            "l",
            vec![("id", "VARCHAR"), ("k", "INTEGER"), ("a", "INTEGER")],
        ),
        ("r", vec![("id", "VARCHAR"), ("k", "INTEGER")]),
        ("t", vec![("id", "VARCHAR"), ("k", "INTEGER")]),
    ]
    .into_iter()
    .map(|(relation, columns)| {
        RelationSchema::new(
            relation,
            columns
                .into_iter()
                .map(|(name, kind)| {
                    SchemaColumn::from_sql_type(name, kind, "postgresql").expect("attested scalar")
                })
                .collect(),
        )
        .expect("physical relation")
    })
    .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("typed source catalog");
    let parser = dialect_from_name("postgresql").expect("portable SQL dialect");
    let sql = SqlInput::inline("SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k WHERE l.a>0");
    let configured = [ConfiguredSqlInput::new(
        "task-115-negative",
        &sql,
        "postgresql",
        parser.as_ref(),
    )];
    let analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analysis");
    let target = analyzed.layers().last().expect("terminal");
    let row = |pairs: Vec<(&str, LiteralValue)>| {
        PhysicalWriteRow::new(
            pairs
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        )
        .expect("complete physical row")
    };
    let text = |value: &str| LiteralValue::Text(value.to_string());
    let number = |value: i64| LiteralValue::Number(value.to_string());
    let left = row(vec![("id", text("l1")), ("k", number(1)), ("a", number(1))]);
    let right = row(vec![("id", text("r1")), ("k", number(1))]);
    let third = row(vec![("id", text("t1")), ("k", number(1))]);
    let make = |left_rows: Vec<PhysicalWriteRow>| {
        vec![
            PhysicalWriteState::new("l", left_rows).expect("left"),
            PhysicalWriteState::new("r", vec![right.clone()]).expect("middle"),
            PhysicalWriteState::new("t", vec![third.clone()]).expect("right"),
        ]
    };
    let valid = make(vec![left.clone()]);
    assert!(
        matches!(
            physical_finite_chain_bag_plan(&analyzed, target.id(), &valid),
            PhysicalFiniteChainProof::Exact(_)
        ),
        "the same complete source bag should be provable"
    );
    assert!(
        matches!(
            physical_finite_chain_bag_plan(&analyzed, target.id(), &make(vec![left.clone(), left])),
            PhysicalFiniteChainProof::Impossible { .. }
        ),
        "duplicate physical row identities are inconsistent"
    );
    let wrong_type = row(vec![
        ("id", text("l1")),
        ("k", text("not an integer")),
        ("a", number(1)),
    ]);
    assert!(
        !matches!(
            physical_finite_chain_bag_plan(&analyzed, target.id(), &make(vec![wrong_type])),
            PhysicalFiniteChainProof::Exact(_)
        ),
        "invalid physical scalar type cannot certify a joined bag"
    );
    assert!(
        !matches!(
            physical_finite_chain_bag_plan(&analyzed, target.id(), &[]),
            PhysicalFiniteChainProof::Exact(_)
        ),
        "missing complete source bags are not empty valid populations"
    );

    let unsupported =
        SqlInput::inline("SELECT l.k FROM l JOIN r ON l.k=r.k JOIN t ON r.k=t.k WHERE l.a + 1 > 0");
    let configured = [ConfiguredSqlInput::new(
        "task-115-unproved",
        &unsupported,
        "postgresql",
        parser.as_ref(),
    )];
    let analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analysis");
    let target = analyzed.layers().last().expect("terminal");
    assert!(
        matches!(
            physical_finite_chain_bag_plan(&analyzed, target.id(), &valid),
            PhysicalFiniteChainProof::Residual { .. }
        ),
        "unimplemented computed predicates must not masquerade as finite TRUE"
    );
}

#[test]
fn enforced_pk_fk_on_frozen_bags_rejects_duplicate_parent_and_missing_reference() {
    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("frozen matrix");
    let case = inventory["cases"]
        .as_array()
        .expect("eight cases")
        .iter()
        .find(|case| case["id"] == "71-C115-P-CONSTRAINTS")
        .expect("positive constrained proof cell");
    let schemas = ["l", "r", "t"]
        .into_iter()
        .map(|relation| {
            let columns = case["source_column_types"][relation]
                .as_object()
                .expect("source schema")
                .iter()
                .map(|(column, declaration)| {
                    SchemaColumn::from_sql_type(
                        column,
                        declaration
                            .as_str()
                            .expect("SQL type")
                            .split_whitespace()
                            .next()
                            .expect("base type"),
                        "postgresql",
                    )
                    .expect("typed source column")
                })
                .collect();
            RelationSchema::new(relation, columns).expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let dialect = dialect_from_name("postgresql").expect("dialect");
    let sql = SqlInput::inline(case["sql"].as_str().expect("frozen SQL"));
    let configured = [ConfiguredSqlInput::new(
        "task-115-constrained-impossible",
        &sql,
        "postgresql",
        dialect.as_ref(),
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("source schema analysis");
    bundle.enrich_relation_constraints(&attested_pk_fk_constraints());
    let target = bundle.layers().last().expect("joined layer").id().to_string();
    let baseline = snapshots(case);
    assert!(matches!(
        physical_finite_chain_bag_plan(&bundle, &target, &baseline),
        PhysicalFiniteChainProof::Exact(_)
    ));

    let mut repeated_tid = baseline.clone();
    let parent = &baseline[2];
    let mut duplicate_values = parent.rows()[0].values().clone();
    duplicate_values.insert("id".to_string(), LiteralValue::Text("t2".to_string()));
    let duplicate = PhysicalWriteRow::new(duplicate_values).expect("complete parent row");
    repeated_tid[2] = PhysicalWriteState::new(
        "t",
        vec![parent.rows()[0].clone(), duplicate],
    )
    .expect("duplicate candidate");
    assert!(
        matches!(
            physical_finite_chain_bag_plan(&bundle, &target, &repeated_tid),
            PhysicalFiniteChainProof::Impossible { .. }
        ),
        "enforced primary key forbids duplicate t.tid even when row ids differ"
    );

    let mut orphan_tid = baseline.clone();
    let child = &baseline[1];
    let mut orphan_values = child.rows()[0].values().clone();
    orphan_values.insert("tid".to_string(), LiteralValue::Number("99".to_string()));
    let orphan = PhysicalWriteRow::new(orphan_values).expect("complete child row");
    orphan_tid[1] =
        PhysicalWriteState::new("r", vec![orphan, child.rows()[1].clone()])
            .expect("orphan candidate");
    assert!(
        !matches!(
            physical_finite_chain_bag_plan(&bundle, &target, &orphan_tid),
            PhysicalFiniteChainProof::Exact(_)
        ),
        "unmatched enforced FK must remain a typed residual or impossible"
    );
}
