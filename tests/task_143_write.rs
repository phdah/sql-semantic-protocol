//! TASK-143 downstream ordered writes from a filtered, computed lexical producer.
//! Independent DuckDB oracle validates complete physical source, intermediate and target bags.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    CheckCompareOperator, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

const SQL: &str = "INSERT INTO target(k, shifted) WITH stage AS
    (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1)
    SELECT k, shifted FROM stage";

fn state(name: &str, columns: &[&str], rows: &[Vec<Option<i64>>]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        name.to_string(),
        rows.iter()
            .map(|row| {
                PhysicalWriteRow::new(
                    columns
                        .iter()
                        .zip(row)
                        .map(|(column, value)| {
                            (
                                column.to_string(),
                                value.map_or(LiteralValue::Null, |v| {
                                    LiteralValue::Number(v.to_string())
                                }),
                            )
                        })
                        .collect::<BTreeMap<_, _>>(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn case(raw: &[Option<i64>], prior: &[(i64, i64)], expected: &[(i64, i64)]) {
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "raw".to_string(),
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
        RelationSchema::new(
            "target".to_string(),
            vec![
                SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap(),
                SchemaColumn::from_sql_type("shifted", "INTEGER", "duckdb").unwrap(),
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(SQL);
    let inputs = [ConfiguredSqlInput::new(
        "task143",
        &sql,
        "duckdb",
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let raw_state = state(
        "raw",
        &["k"],
        &raw.iter().map(|v| vec![*v]).collect::<Vec<_>>(),
    );
    let target_state = state(
        "target",
        &["k", "shifted"],
        &prior
            .iter()
            .map(|(k, s)| vec![Some(*k), Some(*s)])
            .collect::<Vec<_>>(),
    );
    let initial = [raw_state.clone(), target_state.clone()];
    let plan = physical_ordered_write_state_plan(&bundle, &initial);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &plan
    else {
        panic!("TASK-143 must prove exact source-derived write, found: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    let transition = &transitions[0];
    assert_eq!(transition.before().rows(), target_state.rows());
    assert_eq!(transition.intermediate_states().len(), 1);
    assert_eq!(transition.intermediate_states()[0].relation(), "stage");
    let expected_stage = raw
        .iter()
        .filter_map(|v| v.filter(|k| *k > 1))
        .map(|k| vec![Some(k), Some(k + 10)])
        .collect::<Vec<_>>();
    assert_eq!(
        transition.intermediate_states()[0].rows(),
        state("stage", &["k", "shifted"], &expected_stage).rows(),
        "complete materialized stage must derive only from physical raw"
    );
    let target = final_states
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    let final_rows = expected
        .iter()
        .map(|(k, s)| vec![Some(*k), Some(*s)])
        .collect::<Vec<_>>();
    assert_eq!(
        target.rows(),
        state("target", &["k", "shifted"], &final_rows).rows()
    );
    assert_eq!(
        transition.affected().inserted() as usize,
        expected.len() - prior.len()
    );
    assert_eq!(
        final_states.len(),
        2,
        "CTE stage is not an independent physical insertion"
    );
    assert!(final_states.iter().any(|s| s == &raw_state));
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    assert_eq!(target.exact_domains(schema).unwrap().len(), 2);

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw(k INTEGER); CREATE TABLE target(k INTEGER, shifted INTEGER);",
    )
    .unwrap();
    for k in raw {
        db.execute("INSERT INTO raw VALUES (?1)", params![k])
            .unwrap();
    }
    for (k, shifted) in prior {
        db.execute("INSERT INTO target VALUES (?1,?2)", params![k, shifted])
            .unwrap();
    }
    let mut producer = db
        .prepare(
            "WITH stage AS (SELECT k,k+10 AS shifted FROM raw WHERE k>1)
         SELECT k,shifted FROM stage ORDER BY k,shifted",
        )
        .unwrap();
    let mut observed_stage = producer
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    let mut expected_stage_sorted = expected_stage
        .iter()
        .map(|v| (v[0].unwrap(), v[1].unwrap()))
        .collect::<Vec<_>>();
    observed_stage.sort();
    expected_stage_sorted.sort();
    assert_eq!(observed_stage, expected_stage_sorted);
    drop(producer);
    db.execute_batch(SQL).unwrap();
    let mut terminal = db
        .prepare("SELECT k,shifted FROM target ORDER BY k,shifted")
        .unwrap();
    let observed = terminal
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort();
    assert_eq!(
        observed, expected_sorted,
        "native complete target poststate"
    );
}

#[test]
fn filtered_computed_cte_write_produces_exact_target_and_intermediate_bags() {
    case(
        &[Some(1), Some(2), Some(3)],
        &[(99, 109)],
        &[(99, 109), (2, 12), (3, 13)],
    );
}

#[test]
fn duplicate_and_null_rejections_cannot_be_inserted_as_free_producer_rows() {
    case(&[Some(1), Some(2), Some(2), None], &[], &[(2, 12), (2, 12)]);
}

#[test]
fn rejected_computed_cte_insert_is_impossible_and_preserves_original_target() {
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "raw".to_string(),
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
        RelationSchema::new(
            "target".to_string(),
            vec![
                SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap(),
                SchemaColumn::from_sql_type("shifted", "INTEGER", "duckdb").unwrap(),
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let source = SqlInput::inline(SQL);
    let configured = [ConfiguredSqlInput::new(
        "task143",
        &source,
        "duckdb",
        dialect.as_ref(),
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-143").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::check_comparison(
            "shifted",
            CheckCompareOperator::Gte,
            ConstraintValue::Integer(50),
            evidence,
        )
        .unwrap()],
    )
    .unwrap()]);
    let initial = [
        state("raw", &["k"], &[vec![Some(1)], vec![Some(2)]]),
        state("target", &["k", "shifted"], &[vec![Some(99), Some(109)]]),
    ];
    let plan = physical_ordered_write_state_plan(&bundle, &initial);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "computed CTE insert violating enforced target check must be Impossible: {plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE raw(k INTEGER);
        CREATE TABLE target(k INTEGER,shifted INTEGER CHECK(shifted >= 50));
        INSERT INTO raw VALUES(1),(2);
        INSERT INTO target VALUES(99,109);",
    )
    .unwrap();
    assert!(db.execute_batch(SQL).is_err());
    let rows = db
        .prepare("SELECT k,shifted FROM target")
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(rows, vec![(99, 109)], "failed native insert must be atomic");
}
