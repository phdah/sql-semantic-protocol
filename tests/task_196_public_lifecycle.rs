//! TASK-196 public physical lifecycle proof over original DML programs.
//!
//! A complete physical source assignment supplies the row identities; only
//! known-Exact results can certify affected counts and final typed bags.
//! Original unimplemented analyzer variants stay explicit review blockers.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ColumnValueKind, ColumnValueSource, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");
const OCCURRENCES: &str = include_str!("../docs/task-196-original-write-occurrences.json");

fn parse(contents: &str) -> Value {
    serde_json::from_str(contents).expect("independently pinned task-owned evidence")
}

fn integer(value: &Value) -> LiteralValue {
    value
        .as_i64()
        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
}

fn row(pair: &Value) -> PhysicalWriteRow {
    let tuple = pair.as_array().expect("native nullable integer pair");
    assert_eq!(tuple.len(), 2);
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_owned(), integer(&tuple[0])),
        ("amount".to_owned(), integer(&tuple[1])),
    ]))
    .expect("well-typed complete source row")
}

fn state(relation: &str, source: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        source
            .as_array()
            .expect("physical source bag")
            .iter()
            .map(row)
            .collect(),
    )
    .expect("source rows retain duplicate physical occurrences")
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["orders", "customers"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["id", "amount"]
                        .into_iter()
                        .map(|name| {
                            SchemaColumn::from_sql_type(name, "INTEGER", "duckdb")
                                .expect("frozen integer schema")
                        })
                        .collect(),
                )
                .expect("two-column table")
            })
            .collect::<Vec<_>>(),
    )
    .expect("authoritative source columns")
}

fn bundle(sql: &str, enforce_primary_key: bool) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").expect("public parser");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task196-original-sql",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog())
        .expect("original SQL public adapter");
    if enforce_primary_key || sql.contains("THEN INSERT (id)") {
        // The frozen DuckDB DDL has an ordinary nullable amount column.
        // An omitted amount is the implicitly attested SQL NULL default,
        // not an arbitrary application-side missing value.
        let evidence = vec![ConstraintEvidence::new(
            ConstraintProvenance::new(
                ConstraintSourceKind::ExternalMetadata,
                "task-196-native-orders-ddl",
            )
            .expect("native column DDL provenance"),
            ConstraintEnforcement::Enforced,
        )];
        let constraints = if enforce_primary_key {
            vec![
                RelationConstraint::primary_key(vec!["id".to_owned()], evidence.clone())
                    .expect("PK law"),
            ]
        } else {
            Vec::new()
        };
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new("orders", constraints)
            .expect("source constraint set")
            .with_column_value_sources(vec![ColumnValueSource::new(
                "amount",
                ColumnValueKind::Default,
                Some(ConstraintValue::Null),
                evidence,
            )
            .expect("DDL-attested implicit nullable default")])]);
    }
    bundle
}

fn bag(state: &PhysicalWriteState) -> Vec<Vec<LiteralValue>> {
    let mut values = state
        .rows()
        .iter()
        .map(|row| {
            vec![
                row.values().get("id").expect("id").clone(),
                row.values().get("amount").expect("amount").clone(),
            ]
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|row| format!("{row:?}"));
    values
}

fn expected_bag(value: &Value) -> Vec<Vec<LiteralValue>> {
    let mut result = value
        .as_array()
        .expect("complete known native output")
        .iter()
        .map(|entry| {
            entry
                .as_array()
                .expect("pair")
                .iter()
                .map(integer)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    result.sort_by_key(|row| format!("{row:?}"));
    result
}

#[test]
fn all_portable_original_dml_proofs_reconcile_complete_target_source_and_affected_bags() {
    let freeze = parse(FREEZE);
    let occurrences = parse(OCCURRENCES);
    let mut exact = 0;
    for case in occurrences["cases"].as_array().expect("16 source laws") {
        let id = case["id"].as_str().expect("original SQL identity");
        let native = freeze["cases"]
            .as_array()
            .expect("original cases")
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("matching original DML");
        let oracle = &native["native_oracle"];
        let source = &oracle["complete_source_bags"];
        let initial = if source["customers"].is_null() {
            vec![state("orders", &source["orders"])]
        } else {
            vec![
                state("orders", &source["orders"]),
                state("customers", &source["customers"]),
            ]
        };
        let pk = id == "88-C177-S198";
        let sql = native["sql"].as_str().expect("immutable original SQL");
        let bundle = bundle(sql, pk);
        let result = physical_ordered_write_state_plan(&bundle, &initial);
        if id == "88-C177-S198" {
            // The original DuckDB ON CONFLICT no-op has a native complete-bag
            // witness, but portable PK/default check ordering is not attested.
            assert!(
                matches!(result, PhysicalWriteStatePlan::Residual { .. }),
                "{id}: engine-dependent constraint validation precedence must remain unapproved"
            );
            continue;
        }

        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = result
        else {
            panic!("{id}: source-complete portable DML should be provable: {result:?}");
        };
        assert_eq!(transitions.len(), 1, "{id}: one original mutation");
        let transition = &transitions[0];
        assert_eq!(
            transition.before(),
            &initial[0],
            "{id}: exact source identity"
        );
        let final_target = final_states
            .iter()
            .find(|state| state.relation() == "orders")
            .expect("preserved physical target identity");
        assert_eq!(
            bag(final_target),
            expected_bag(&oracle["expected_poststate_bag"]),
            "{id}: exact full physical final bag"
        );
        assert_eq!(
            bag(transition.after()),
            bag(final_target),
            "{id}: transition output owns the final target"
        );
        if initial.len() == 2 {
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "customers"),
                Some(&initial[1]),
                "{id}: source table must not mutate"
            );
        }
        let law = &case["independent_law"];
        assert_eq!(
            transition.affected().inserted(),
            law["inserted"].as_u64().expect("insert count"),
            "{id}"
        );
        assert_eq!(
            transition.affected().updated(),
            law["updated"].as_u64().expect("update count"),
            "{id}"
        );
        assert_eq!(
            transition.affected().deleted(),
            law["deleted"].as_u64().expect("delete count"),
            "{id}"
        );
        let returned = &oracle["expected_returning_bag"];
        if let Some(expected) = returned.as_array() {
            let physical = transition
                .returning()
                .expect("original RETURNING projection");
            let mut actual = physical
                .rows()
                .iter()
                .map(|row| row.values().get("id").expect("RETURNING id").clone())
                .collect::<Vec<_>>();
            actual.sort_by_key(|value| format!("{value:?}"));
            let mut wanted = expected
                .iter()
                .map(|entry| integer(&entry.as_array().expect("RETURNING one-column tuple")[0]))
                .collect::<Vec<_>>();
            wanted.sort_by_key(|value| format!("{value:?}"));
            assert_eq!(
                actual, wanted,
                "{id}: complete RETURNING physical occurrences"
            );
        }
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "orders")
            .expect("authoritative target source columns");
        let domains = final_target
            .exact_domains(schema)
            .expect("fully known physical bag");
        assert_eq!(domains.len(), 2, "{id}: typed id and amount domains");
        for domain in domains {
            assert_eq!(
                domain.values().iter().map(|(_, count)| *count).sum::<u64>(),
                final_target.rows().len() as u64,
                "{id}: typed frequency must include NULL and all duplicates"
            );
        }
        exact += 1;
    }
    assert_eq!(exact, 15);
}
