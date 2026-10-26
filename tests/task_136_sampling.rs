//! TASK-136 public semantics: exact deterministic pages versus sampled Residuals.
//! Complete physical bags and DuckDB output are independent of the SQL analyzer.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name,
    physical_finite_page_bag_plan, to_json, ConditionClause, ConfiguredSqlInput,
    LiteralValue, PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState,
    Protocol, ProtocolStatement, RelationCatalog, RelationSchema,
    ResidualConditionReason, SamplingMethod, SamplingSeedSyntax, SamplingSyntax,
    SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

fn analyze(sql: &str, dialect: &str) -> Protocol {
    let parser = dialect_from_name(dialect).unwrap();
    analyze_sql(sql, dialect, parser.as_ref()).unwrap()
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "s",
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap(),
    ])
    .unwrap()
}

fn source(keys: &[i64]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        keys.iter()
            .map(|key| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    LiteralValue::Number(key.to_string()),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn proof(sql: &str, dialect: &str, keys: &[i64]) -> PhysicalFinitePageProof {
    let parser = dialect_from_name(dialect).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "TASK-136",
        &input,
        dialect,
        parser.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap();
    let target = bundle.layers().last().unwrap().id();
    physical_finite_page_bag_plan(&bundle, target, &[source(keys)])
}

fn duckdb_output(sql: &str, keys: &[i64]) -> Vec<i64> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER);").unwrap();
    for key in keys {
        db.execute_batch(&format!("INSERT INTO s VALUES ({key});")).unwrap();
    }
    let count: i64 = db.query_row("SELECT COUNT(*) FROM s", [], |row| row.get(0)).unwrap();
    assert_eq!(count, keys.len() as i64, "complete physical input bag");
    db.prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<i64>, _>>()
        .unwrap()
}

fn exact_values(proof: PhysicalFinitePageProof) -> Vec<i64> {
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("mandatory deterministic case must be exact: {proof:?}");
    };
    exact.terminal
        .iter()
        .map(|row| match row.get("k").unwrap() {
            LiteralValue::Number(value) => value.parse().unwrap(),
            other => panic!("unexpected value: {other:?}"),
        })
        .collect()
}

#[test]
fn deterministic_positive_and_impossible_page_bags_match_duckdb() {
    let keys = [1, 2, 3];
    let scenarios = [
        ("SELECT k FROM s ORDER BY k LIMIT 2", vec![1, 2]),
        ("SELECT k FROM s ORDER BY k FETCH FIRST 2 ROWS ONLY", vec![1, 2]),
        ("SELECT k FROM s ORDER BY k LIMIT 1 OFFSET 9", vec![]),
    ];
    for (sql, expected) in scenarios {
        assert_eq!(duckdb_output(sql, &keys), expected, "{sql}");
        assert_eq!(exact_values(proof(sql, "generic", &keys)), expected, "{sql}");
    }
}

#[test]
fn sample_method_quantity_and_seed_are_canonical_but_not_physical_proofs() {
    let sql = "SELECT k FROM s TABLESAMPLE BERNOULLI (50) REPEATABLE (7)";
    let protocol = analyze(sql, "postgresql");
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("expected parsed query");
    };
    let [sample] = query.sampling() else {
        panic!("one sampled source must be retained");
    };
    assert_eq!(sample.source(), "s");
    assert_eq!(sample.source_instance(), 0);
    assert_eq!(sample.syntax(), SamplingSyntax::TableSample);
    assert_eq!(sample.method(), Some(SamplingMethod::Bernoulli));
    assert_eq!(sample.quantity().and_then(|quantity| quantity.value()), Some(&LiteralValue::Number("50".to_string())));
    let seed = sample.seed().expect("a seed must be preserved");
    assert_eq!(seed.syntax(), SamplingSeedSyntax::Repeatable);
    assert_eq!(seed.value(), Some(&LiteralValue::Number("7".to_string())));
    assert!(query.condition_exactness().residual_conditions().iter().any(|item| {
        item.reason() == ResidualConditionReason::TableSample
            && item.clause() == ConditionClause::RowSetOperator
    }));
    let document = to_json(&protocol);
    assert!(document.contains("\"sampling\""));
    assert!(document.contains("\"membership\":\"residual\""));
    assert!(document.contains("\"repeatable\""));
}

#[test]
fn source_sampling_blocks_closed_world_page_even_when_order_is_total() {
    let sampled = "SELECT k FROM s TABLESAMPLE SYSTEM (50) ORDER BY k LIMIT 2";
    let outcome = proof(sampled, "generic", &[1, 2, 3]);
    assert!(matches!(outcome, PhysicalFinitePageProof::Residual { .. }), "{outcome:?}");
    assert_eq!(exact_values(proof("SELECT k FROM s ORDER BY k LIMIT 2", "generic", &[1, 2, 3])), vec![1, 2]);
}

#[test]
fn tied_page_boundaries_do_not_claim_an_exact_source_identity() {
    let uncertain = proof("SELECT k FROM s ORDER BY k LIMIT 1", "generic", &[1, 1, 2]);
    assert!(matches!(uncertain, PhysicalFinitePageProof::Residual { .. }), "{uncertain:?}");
}

#[test]
fn all_exposed_dialects_keep_sampling_opaque_or_reject_the_parser_form() {
    const DIALECTS: [&str; 13] = [
        "generic", "mysql", "postgresql", "hive", "sqlite", "snowflake",
        "redshift", "mssql", "clickhouse", "bigquery", "ansi", "duckdb", "databricks",
    ];
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        let result = analyze_sql(
            "SELECT k FROM s TABLESAMPLE SYSTEM (50)",
            dialect,
            parser.as_ref(),
        );
        match result {
            Ok(protocol) => {
                let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                    panic!("{dialect}: parsed statement is not query");
                };
                assert_eq!(query.sampling().len(), 1, "{dialect}");
                assert!(query.condition_exactness().residual_conditions().iter().any(|item| {
                    item.reason() == ResidualConditionReason::TableSample
                }), "{dialect}: TABLESAMPLE must remain residual");
            }
            Err(error) => {
                // Native parser boundaries are tracked separately from semantic support.
                assert!(!error.to_string().is_empty(), "{dialect}");
            }
        }
    }
}
