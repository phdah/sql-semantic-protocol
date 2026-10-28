//! TASK-148: negative and duplicate-sensitive closed-world write-script oracles.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStateGap, PhysicalWriteStatePlan, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

type Row = (i64, Option<i64>);

fn state(name: &str, values: &[Row]) -> PhysicalWriteState {
    PhysicalWriteState::new(name, values.iter().map(|(id, score)| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".into(), LiteralValue::Number(id.to_string())),
            ("score".into(), score.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))),
        ])).unwrap()
    }).collect()).unwrap()
}

fn bundle(sql: &[&str], primary_key: bool) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["target","source"].map(|relation| RelationSchema::new(
        relation,
        ["id","score"].into_iter().map(|column| {
            SchemaColumn::from_sql_type(column,"INTEGER","duckdb").unwrap()
        }).collect(),
    ).unwrap());
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let inputs = sql.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
    let ids = (0..sql.len()).map(|i| format!("task148-negative-{i}")).collect::<Vec<_>>();
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = inputs.iter().zip(&ids).map(|(sql,id)| {
        ConfiguredSqlInput::new(id,sql,"duckdb",dialect.as_ref())
    }).collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured,&catalog).unwrap();
    if primary_key {
        let evidence = vec![ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task148-negative").unwrap(),
            ConstraintEnforcement::Enforced,
        )];
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",vec![RelationConstraint::primary_key(vec!["id".into()],evidence).unwrap()],
        ).unwrap()]);
    }
    bundle
}

fn native_bag(db: &Connection) -> Vec<Row> {
    let mut stmt = db.prepare("SELECT id,score FROM target ORDER BY id,score NULLS FIRST").unwrap();
    stmt.query_map([],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,Option<i64>>(1)?)))
        .unwrap().map(|row|row.unwrap()).collect()
}

#[test]
fn null_predicate_is_not_true_and_never_reaches_target() {
    let sql = "INSERT INTO target(id,score) SELECT id,score FROM source WHERE score > 10";
    let source = state("source",&[(1,Some(10)),(2,None),(3,Some(30))]);
    let prior = state("target",&[(0,Some(0))]);
    let plan = physical_ordered_write_state_plan(&bundle(&[sql],false),&[source.clone(),prior.clone()]);
    let PhysicalWriteStatePlan::Feasible{transitions,final_states} = plan else {
        panic!("the complete nullable source must be proven: {plan:?}");
    };
    assert_eq!(transitions.len(),1);
    assert_eq!(transitions[0].before(),&prior);
    assert_eq!(transitions[0].after().rows(),state("target",&[(0,Some(0)),(3,Some(30))]).rows());
    assert_eq!(transitions[0].affected().inserted(),1);
    assert_eq!(final_states.iter().find(|s|s.relation()=="source"),Some(&source));

    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE source(id INTEGER,score INTEGER);
        CREATE TABLE target(id INTEGER,score INTEGER);
        INSERT INTO source VALUES (1,10),(2,NULL),(3,30);
        INSERT INTO target VALUES (0,0);").unwrap();
    assert_eq!(db.query_row::<i64,_,_>(
        "SELECT COUNT(*) FROM source WHERE score > 10",[],|row|row.get(0)).unwrap(),1);
    assert_eq!(db.query_row::<i64,_,_>(
        "SELECT COUNT(*) FROM source WHERE (score > 10) IS NOT TRUE",[],|row|row.get(0)).unwrap(),2);
    db.execute_batch(sql).unwrap();
    assert_eq!(native_bag(&db),vec![(0,Some(0)),(3,Some(30))]);
}

#[test]
fn duplicate_source_rows_remain_distinct_in_final_bag_and_exact_domain() {
    let sql="INSERT INTO target(id,score) SELECT id,score FROM source WHERE TRUE";
    let source=state("source",&[(1,Some(10)),(1,Some(10)),(2,Some(20))]);
    let bundle=bundle(&[sql],false);
    let plan=physical_ordered_write_state_plan(&bundle,&[source.clone(),state("target",&[])]);
    let PhysicalWriteStatePlan::Feasible{transitions,final_states}=plan else {
        panic!("unconstrained INSERT must retain the full duplicate bag: {plan:?}");
    };
    assert_eq!(transitions[0].affected().inserted(),3);
    assert_eq!(final_states.iter().find(|s|s.relation()=="source"),Some(&source));
    assert_eq!(transitions[0].after().rows(),source.rows());
    let schema=bundle.source_schemas().iter().find(|s|s.relation()=="target").unwrap();
    let domains=transitions[0].after().exact_domains(schema).unwrap();
    assert_eq!(domains[0].column(),"id");
    assert_eq!(domains[0].values(),&[
        (LiteralValue::Number("1".into()),2),(LiteralValue::Number("2".into()),1),
    ]);

    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE source(id INTEGER,score INTEGER);
        CREATE TABLE target(id INTEGER,score INTEGER);
        INSERT INTO source VALUES (1,10),(1,10),(2,20);").unwrap();
    db.execute_batch(sql).unwrap();
    assert_eq!(native_bag(&db),vec![(1,Some(10)),(1,Some(10)),(2,Some(20))]);
}

#[test]
fn intra_statement_duplicate_primary_key_is_impossible_without_partial_commit() {
    let sql="INSERT INTO target(id,score) SELECT id,score FROM source WHERE TRUE";
    let source=state("source",&[(1,Some(10)),(1,Some(20))]);
    let target=state("target",&[]);
    let plan=physical_ordered_write_state_plan(&bundle(&[sql],true),&[source,target]);
    assert!(matches!(plan,PhysicalWriteStatePlan::Impossible{..}),"{plan:?}");

    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE source(id INTEGER,score INTEGER);
        CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);
        INSERT INTO source VALUES (1,10),(1,20);").unwrap();
    assert!(db.execute_batch(sql).is_err());
    assert!(native_bag(&db).is_empty(),"failed statement must not partially commit");
}

#[test]
fn unreadable_source_population_is_residual_not_inferred_from_target() {
    let sql="INSERT INTO target(id,score) SELECT id,score FROM source WHERE TRUE";
    let result=physical_ordered_write_state_plan(
        &bundle(&[sql],false),&[state("target",&[])],
    );
    assert!(matches!(result,PhysicalWriteStatePlan::Residual{
        gap:PhysicalWriteStateGap::MissingPopulation,..}),"{result:?}");
}

#[test]
fn later_insert_select_reads_target_written_by_prior_statement() {
    let statements=[
        "INSERT INTO target(id,score) VALUES (1,10),(2,20)",
        "INSERT INTO target(id,score) SELECT id,score FROM target WHERE id > 1",
    ];
    let result=physical_ordered_write_state_plan(
        &bundle(&statements,false),&[state("target",&[]),state("source",&[])],
    );
    let PhysicalWriteStatePlan::Feasible{transitions,final_states}=result else {
        panic!("later INSERT must read the committed preceding target: {result:?}");
    };
    assert_eq!(transitions.len(),2);
    assert_eq!(transitions[0].after().rows(),state("target",&[(1,Some(10)),(2,Some(20))]).rows());
    assert_eq!(transitions[1].before(),transitions[0].after());
    assert_eq!(transitions[1].affected().inserted(),1);
    assert_eq!(transitions[1].after().rows(),
        state("target",&[(1,Some(10)),(2,Some(20)),(2,Some(20))]).rows());
    assert_eq!(final_states.iter().find(|s|s.relation()=="target"),Some(transitions[1].after()));

    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER,score INTEGER);").unwrap();
    for sql in statements {db.execute_batch(sql).unwrap();}
    assert_eq!(native_bag(&db),vec![(1,Some(10)),(2,Some(20)),(2,Some(20))]);
}
