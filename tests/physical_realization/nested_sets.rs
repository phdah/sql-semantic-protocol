//! Full-output tests for nested set-operator physical populations.

use super::*;

#[test]
fn three_and_four_branch_set_trees_map_complete_source_bags() {
    let schema = |relation: &str, column: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        schema("a", "k"),
        schema("b", "k"),
        schema("c", "k"),
        schema("d", "k"),
        schema("sa", "id"),
        schema("sb", "id"),
        schema("sc", "id"),
        schema("sd", "id"),
    ];
    let producers = [
        "CREATE TABLE sa AS SELECT k AS id FROM a",
        "CREATE TABLE sb AS SELECT k AS id FROM b",
        "CREATE TABLE sc AS SELECT k AS id FROM c",
        "CREATE TABLE sd AS SELECT k AS id FROM d",
    ];

    for &dialect in DIALECTS {
        for (query, source_count, wanted) in [
            ("SELECT id FROM sa UNION ALL SELECT id FROM sb UNION ALL SELECT id FROM sc", 3_usize, 3_u64),
            ("SELECT id FROM sa UNION SELECT id FROM sb UNION SELECT id FROM sc UNION SELECT id FROM sd", 4, 2),
        ] {
            let mut statements = producers[..source_count].to_vec();
            statements.push(query);
            let mut bundle = bundle_with_schemas(&statements, dialect, &schemas);
            let target = bundle.layers()[source_count].id().to_string();
            bundle.set_outcome_goals(
                &[OutcomeGoal::new(&target, Some(wanted), None, vec![]).unwrap()]
            ).unwrap();
            let Some(OutcomeWitness::SetTuples { tuples, case, .. }) =
                bundle.outcome_goals()[0].witness()
            else {
                panic!("{dialect}: missing nested set tuple law: {:?}", bundle.outcome_goals());
            };
            assert_eq!(case.obligations().len(), source_count);
            assert_eq!(
                case.output_tuple_count().checked_mul(*tuples),
                Some(wanted),
                "{dialect}: source bags must yield exact selected count"
            );
            let physical = physical_joint_source_plan(&bundle, &[(&target, wanted)]);
            let WitnessDirection::Feasible(cases) = physical.outcome() else {
                panic!("{dialect}: nested operator proof unbound: {physical:?}");
            };
            assert!(cases.iter().any(|proof| proof.obligations().iter().any(|o| {
                matches!(o, WitnessObligation::OperatorPopulation {
                    witness: OutcomeWitness::SetTuples { .. }
                })
            })), "{dialect}: missing canonical nested physical operator witness");

            let db = Connection::open_in_memory().unwrap();
            db.execute_batch(
                "CREATE TABLE a(k INTEGER); CREATE TABLE b(k INTEGER);
                 CREATE TABLE c(k INTEGER); CREATE TABLE d(k INTEGER);",
            ).unwrap();
            for obligation in case.obligations() {
                assert!(!obligation.boundary().is_intermediate());
                assert!(["a", "b", "c", "d"].contains(&obligation.boundary().relation()));
                for key in 0..*tuples {
                    for _ in 0..obligation.matching_tuple_count() {
                        db.execute_batch(&format!(
                            "INSERT INTO {} VALUES ({key});",
                            obligation.boundary().relation()
                        )).unwrap();
                    }
                }
            }
            db.execute_batch(&statements[..source_count].join(";")).unwrap();
            let mut statement = db.prepare(&format!(
                "SELECT id FROM ({query}) ORDER BY id"
            )).unwrap();
            let result = statement.query_map([], |row| row.get::<_, i64>(0))
                .unwrap().collect::<Result<Vec<_>, _>>().unwrap();
            let expected = (0..*tuples).flat_map(|key| {
                std::iter::repeat_n(key as i64, case.output_tuple_count() as usize)
            }).collect::<Vec<_>>();
            assert_eq!(result, expected, "{dialect}: nested full-output differential");
        }
    }
}

#[test]
fn mixed_nested_except_all_keeps_branch_multiplicity_and_null_laws() {
    let schema = |relation: &str, column: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
        ).unwrap()
    };
    let schemas = [
        schema("a", "k"), schema("b", "k"), schema("c", "k"),
        schema("sa", "id"), schema("sb", "id"), schema("sc", "id")
    ];
    let query = "(SELECT id FROM sa UNION ALL SELECT id FROM sb) EXCEPT ALL SELECT id FROM sc";
    let statements = [
        "CREATE TABLE sa AS SELECT k AS id FROM a",
        "CREATE TABLE sb AS SELECT k AS id FROM b",
        "CREATE TABLE sc AS SELECT k AS id FROM c",
        query,
    ];
    for dialect in ["generic", "postgresql", "duckdb"] {
        let bundle = bundle_with_schemas(&statements, dialect, &schemas);
        let goal = bundle.layers()[3].id();
        let direction = physical_joint_source_plan(&bundle, &[(goal, 2)]);
        assert!(
            matches!(direction.outcome(), WitnessDirection::Feasible(_)),
            "{dialect}: nested bag difference cannot be composed: {direction:?}"
        );
    }
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE a(k INTEGER); CREATE TABLE b(k INTEGER); CREATE TABLE c(k INTEGER);
         INSERT INTO a VALUES (0),(0),(NULL);
         INSERT INTO b VALUES (0),(1),(NULL);
         INSERT INTO c VALUES (0),(0),(0),(NULL);
         CREATE TABLE sa AS SELECT k AS id FROM a;
         CREATE TABLE sb AS SELECT k AS id FROM b;
         CREATE TABLE sc AS SELECT k AS id FROM c;"
    ).unwrap();
    let sql = format!("SELECT id FROM ({query}) ORDER BY id NULLS LAST");
    let values = db.prepare(&sql).unwrap().query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(values, vec![Some(1), None]);
}
