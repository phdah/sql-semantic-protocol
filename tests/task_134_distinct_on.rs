//! TASK-134 deterministic and unresolved DISTINCT ON physical winner selection.
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    physical_finite_page_membership_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn prove(
    sql: &str,
) -> (
    PhysicalFinitePageProof,
    sql_semantic_protocol::AnalysisBundle,
    PhysicalWriteState,
) {
    let parser = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "distinct_on",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let schema = RelationSchema::new(
        "s",
        vec![
            SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let source = PhysicalWriteState::new(
        "s",
        [(1, 10), (1, 20), (2, 30)]
            .into_iter()
            .map(|(k, v)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("k".to_string(), LiteralValue::Number(k.to_string())),
                    ("v".to_string(), LiteralValue::Number(v.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let layer = bundle.layers().last().unwrap().id();
    let proof = physical_finite_page_bag_plan(&bundle, layer, std::slice::from_ref(&source));
    (proof, bundle, source)
}

#[test]
fn distinct_on_ordered_winner_is_certified_with_full_source_and_terminal_oracle() {
    let sql = "SELECT DISTINCT ON (k) k, v FROM s ORDER BY k, v DESC LIMIT 2";
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE s(k INTEGER, v INTEGER); INSERT INTO s VALUES (1,10),(1,20),(2,30);",
    )
    .unwrap();
    let native = db
        .prepare(sql)
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(native, vec![(1, 20), (2, 30)]);
    let (proof, bundle, source) = prove(sql);
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("unambiguous winner must be exact: {proof:?}");
    };
    let terminal = exact
        .terminal
        .iter()
        .map(|row| {
            let integer = |name: &str| match row.get(name).unwrap() {
                LiteralValue::Number(n) => n.parse::<i64>().unwrap(),
                other => panic!("unknown scalar {other:?}"),
            };
            (integer("k"), integer("v"))
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal, native);
    assert_eq!(
        exact.terminal_contributors,
        vec![vec!["s#1".to_string()], vec!["s#2".to_string()]]
    );
    assert_eq!(
        exact
            .rows
            .iter()
            .filter(|row| !row.survives)
            .map(|row| row.source_row_id.as_str())
            .collect::<Vec<_>>(),
        vec!["s#0"]
    );
    assert!(matches!(exact.terminal_domains["v"], ValueDomain::Set(_)));
    let layer = bundle.layers().last().unwrap().id();
    let rejected = BTreeMap::from([
        ("k".to_string(), LiteralValue::Number("1".into())),
        ("v".to_string(), LiteralValue::Number("10".into())),
    ]);
    assert!(matches!(
        physical_finite_page_membership_plan(&bundle, layer, &[source], &rejected),
        PhysicalFinitePageProof::Impossible { .. }
    ));
}

#[test]
fn distinct_on_tied_nonkey_projection_is_residual() {
    let (proof, _, _) = prove("SELECT DISTINCT ON (k) k, v FROM s ORDER BY k LIMIT 2");
    assert!(
        matches!(proof, PhysicalFinitePageProof::Residual { .. }),
        "{proof:?}"
    );
}


#[test]
fn distinct_page_must_not_split_a_nonunique_output_sort_peer_group() {
    let (uncertain, _, _) = prove("SELECT DISTINCT k, v FROM s ORDER BY k LIMIT 1");
    assert!(matches!(uncertain, PhysicalFinitePageProof::Residual { .. }), "{uncertain:?}");

    let (certain, _, _) = prove("SELECT DISTINCT k, v FROM s ORDER BY k LIMIT 2");
    let PhysicalFinitePageProof::Exact(exact) = certain else {
        panic!("both complete tied outputs are retained: {certain:?}");
    };
    assert_eq!(exact.terminal.len(), 2);
    assert_eq!(exact.rows.iter().filter(|row| row.survives).count(), 2);
    assert!(exact.terminal.iter().all(|row|
        matches!(row.get("k"), Some(LiteralValue::Number(k)) if k == "1")
    ));
}
