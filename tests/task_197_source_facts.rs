//! TASK-197: exact per-case source-fact authority and no-evidence boundaries.
//!
//! A source metadata adapter cannot prove a physical SQL program executed.

use std::collections::BTreeSet;

use serde_json::Value;

const SOURCE: &str = include_str!("../docs/task-197-source-fact-matrix.json");
const ORIGINAL: &str = include_str!("../docs/task-197-acceptance-freeze.json");

#[test]
fn every_original_case_has_exact_authoritative_source_columns_or_explicit_non_applicability() {
    let recorded: Value = serde_json::from_str(SOURCE).expect("owned v1 source fact matrix");
    let original: Value = serde_json::from_str(ORIGINAL).expect("immutable task-owned SQL");
    assert_eq!(recorded["schema_version"], 1);
    assert_eq!(recorded["task_id"], "TASK-197");
    assert_eq!(recorded["case_count"], 163);
    let fact_sets = &recorded["relation_source_fact_sets"];
    for name in ["integer_amount", "floating_amount", "decimal_amount"] {
        for relation in ["orders", "customers", "products"] {
            assert!(fact_sets[name][relation]["id"].is_string());
        }
    }
    assert_eq!(fact_sets["integer_amount"]["orders"]["amount"], "INTEGER");
    assert_eq!(fact_sets["floating_amount"]["orders"]["amount"], "DOUBLE");
    assert_eq!(
        fact_sets["decimal_amount"]["orders"]["amount"],
        "DECIMAL(10,2)"
    );

    let all = recorded["cases"]
        .as_array()
        .expect("all original SQL source facts");
    let frozen = original["cases"]
        .as_array()
        .expect("immutable 163 original SQL");
    assert_eq!(all.len(), 163);
    let mut seen = BTreeSet::new();
    let mut source_free = 0;
    let mut reads = 0;
    let mut writes = 0;
    for (fact, case) in all.iter().zip(frozen) {
        let id = fact["id"].as_str().expect("original case identity");
        assert!(seen.insert(id), "{id}: duplicated source fact authority");
        assert_eq!(fact["id"], case["id"], "{id}: original inventory drift");
        assert_eq!(fact["sql"], case["sql"], "{id}: SQL drift");
        assert_eq!(fact["owner"], case["owner"], "{id}: owner drift");
        assert_eq!(fact["source_relations"], case["source_relations_mentioned"]);
        assert_eq!(fact["producer_execution"], "unattested");
        assert_eq!(fact["parser_cell_count"], 13);
        assert!(fact["source_semantic_oracle"]
            .as_str()
            .is_some_and(|path| path.ends_with("-acceptance-freeze.json")));
        assert!(fact["independent_per_dialect_review"]
            .as_str()
            .is_some_and(|path| path.ends_with("-independent-semantic-review.json")));

        let relations = fact["source_relations"]
            .as_array()
            .expect("metadata source scope");
        if relations.is_empty() {
            assert_eq!(fact["source_fact_set"], Value::Null);
            assert_eq!(fact["metadata_scope"], "no_external_source_fact");
            assert_eq!(
                fact["exact_available_columns"]
                    .as_object()
                    .expect("no source facts")
                    .len(),
                0
            );
            source_free += 1;
        } else {
            let set = fact["source_fact_set"]
                .as_str()
                .expect("authoritative source type set");
            for relation in relations {
                let name = relation.as_str().expect("physical relation identity");
                assert_eq!(
                    fact["exact_available_columns"][name], fact_sets[set][name],
                    "{id}: source type fact differs across declared adapters"
                );
            }
            assert!(fact["source_authority"]["direct_sql"].is_string());
            assert!(fact["source_authority"]["dbt_manifest"].is_string());
            assert!(fact["source_authority"]["dbt_warehouse_catalog"].is_string());
            assert!(fact["source_authority"]["odcs"].is_string());
        }
        match fact["statement_class"].as_str().expect("statement class") {
            "query" => reads += 1,
            "dml" => {
                assert_eq!(
                    fact["metadata_scope"],
                    "DML_program_requires_original_SQL_metadata_only"
                );
                writes += 1;
            }
            "ddl" => assert_eq!(
                fact["metadata_scope"],
                "DDL_production_or_lifecycle_facts_cannot_be_executed_by_metadata_adapters"
            ),
            other => panic!("{id}: unclassified statement kind {other}"),
        }
    }
    assert_eq!(seen.len(), 163);
    assert_eq!(source_free, 2);
    assert!(reads > 100);
    assert!(writes > 10);
}
