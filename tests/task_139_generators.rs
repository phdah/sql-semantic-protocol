//! TASK-139 source-free generator proofs and fail-closed vendor boundaries.
//!
//! DuckDB UNNEST is an independent array-element bag oracle, not a
//! certification of Snowflake FLATTEN or any other vendor implementation.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, FiniteColumnType,
    FiniteTableKind, LiteralValue, PhysicalFiniteTableProof, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

const DIALECTS: [&str; 13] = [
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

fn source_free_proof(
    sql: &str,
    dialect_name: &str,
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> Option<PhysicalFiniteTableProof> {
    let dialect = dialect_from_name(dialect_name).expect("project dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref()).ok()?;
    let layer = bundle.layers().last()?;
    Some(physical_finite_table_plan(&bundle, layer.id(), wanted))
}

fn expected_bag(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("DuckDB");
    let mut statement = db.prepare(sql).expect("independent array oracle");
    statement
        .query_map([], |row| {
            let value = row.get::<_, Option<i64>>(0)?;
            Ok(BTreeMap::from([(
                "v".to_string(),
                value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
            )]))
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("complete oracle bag")
}

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rows = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn frozen_generator_matrix_does_not_claim_unverified_parser_or_adapter_cells() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-139-acceptance-freeze.json"))
            .expect("versioned task-owned matrix");
    let manifest: Value = serde_json::from_str(include_str!("../docs/coverage-manifest.json"))
        .expect("coverage manifest");
    assert_eq!(freeze["owner"], "TASK-139");
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["77-C139-P", "77-C139-N"])
    );
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), DIALECTS.len());
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 7);
    assert_eq!(freeze["validation"]["task_done"], false);
    assert_eq!(
        manifest["task_139_generator_acceptance"]["freeze"],
        "docs/task-139-acceptance-freeze.json"
    );
    for case in freeze["cases"].as_array().unwrap() {
        assert_eq!(
            case["dialect_matrix"].as_array().unwrap().len(),
            DIALECTS.len()
        );
        assert_eq!(case["adapter_matrix"].as_array().unwrap().len(), 3);
        assert!(case["dialect_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .all(|cell| cell["certified"] == false));
        assert!(case["adapter_matrix"]
            .as_array()
            .unwrap()
            .iter()
            .all(|cell| cell["certified"] == false));
    }
}

#[test]
fn typed_explode_literal_bags_preserve_duplicates_null_and_closed_world_absence() {
    for (sql, reference, expected_ids) in [
        (
            "SELECT EXPLODE([5, 6]) AS v",
            "SELECT v FROM UNNEST([5,6]) AS t(v)",
            vec!["explode:1", "explode:2"],
        ),
        (
            "SELECT EXPLODE([5, NULL, 5]) AS v",
            "SELECT v FROM UNNEST([5,NULL,5]) AS t(v)",
            vec!["explode:1", "explode:2", "explode:3"],
        ),
    ] {
        let Some(PhysicalFiniteTableProof::Exact(result)) = source_free_proof(sql, "generic", None)
        else {
            panic!("typed source-free EXPLODE must have an exact complete bag: {sql}");
        };
        assert_eq!(result.row_ids, expected_ids);
        assert_eq!(sorted(&result.terminal), sorted(&expected_bag(reference)));
        assert_eq!(result.terminal_column_types["v"], FiniteColumnType::Integer);
        assert!(!matches!(
            result.terminal_domains["v"],
            ValueDomain::Unknown(_)
        ));

        let missing = BTreeMap::from([("v".to_string(), LiteralValue::Number("7".to_string()))]);
        assert!(matches!(
            source_free_proof(sql, "generic", Some(&missing)),
            Some(PhysicalFiniteTableProof::Impossible { .. })
        ));
    }
}

#[test]
fn accepted_from_explode_literal_forms_retain_generator_identity_and_complete_bag() {
    let sql = "SELECT * FROM EXPLODE([5, 6]) AS t(v)";
    let oracle = expected_bag("SELECT v FROM UNNEST([5,6]) AS t(v)");
    let mut accepted = 0;
    for dialect in DIALECTS {
        let Some(proof) = source_free_proof(sql, dialect, None) else {
            continue; // Explicit parser boundary, not invented portable syntax.
        };
        if let PhysicalFiniteTableProof::Exact(table) = proof {
            accepted += 1;
            assert_eq!(table.row_ids, ["explode:1", "explode:2"]);
            assert_eq!(sorted(&table.terminal), sorted(&oracle));
            assert_eq!(table.terminal_column_types["v"], FiniteColumnType::Integer);
        }
    }
    assert!(
        accepted > 0,
        "at least one exposed parser must recognize FROM EXPLODE"
    );
}

#[test]
fn unproved_vendor_tables_opaque_inputs_and_shapes_are_never_exact() {
    for sql in [
        "SELECT * FROM EXPLODE(opaque_array) AS t(v)",
        "SELECT * FROM FLATTEN([5,6]) AS t(v)",
        "SELECT * FROM EXPLODE([5,6]) WITH ORDINALITY AS t(v, ord)",
        "SELECT * FROM custom_table_generator([5,6]) AS t(v)",
        "SELECT * FROM EXPLODE([5,6]) AS t(v) WHERE v > 5",
    ] {
        for dialect in DIALECTS {
            if let Some(proof) = source_free_proof(sql, dialect, None) {
                assert!(!matches!(proof, PhysicalFiniteTableProof::Exact(_)),
                    "{dialect}: opaque or unproved generator cannot acquire a complete source bag: {sql}");
            }
        }
    }
}

#[test]
fn from_explode_is_canonical_not_a_physical_relation() {
    let sql = "SELECT * FROM EXPLODE([5, 6]) AS t(v)";
    for dialect_name in DIALECTS {
        let parser = dialect_from_name(dialect_name).unwrap();
        let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, parser.as_ref())
        else {
            continue;
        };
        let Some(query) = bundle.inputs()[0]
            .statements()
            .iter()
            .find_map(|s| match s {
                sql_semantic_protocol::ProtocolStatement::Query(query) => Some(query),
                _ => None,
            })
        else {
            continue;
        };
        if let Some(table) = query.finite_table() {
            assert_eq!(table.kind(), FiniteTableKind::Explode);
            assert!(query.sources().is_empty());
            assert_eq!(table.rows().len(), 2);
        }
    }
}

#[test]
fn task_139_parser_classification_probe() {
    let matrix: Value = serde_json::from_str(include_str!("../docs/task-139-acceptance-freeze.json"))
        .expect("matrix");
    let mut observed = Vec::new();
    for case in matrix["cases"].as_array().expect("cases") {
        let mut cells = Vec::new();
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).expect("dialect");
            let status = match analyze_inputs(
                &[SqlInput::inline(case["sql"].as_str().unwrap())],
                dialect,
                parser.as_ref(),
            ) {
                Err(_) => "parser_boundary",
                Ok(bundle) => match bundle.layers().last() {
                    None => "residual",
                    Some(layer) => match physical_finite_table_plan(&bundle, layer.id(), None) {
                        PhysicalFiniteTableProof::Exact(_) => "exact",
                        PhysicalFiniteTableProof::Residual { .. } | PhysicalFiniteTableProof::Impossible { .. } => "residual",
                    },
                },
            };
            cells.push(format!("{dialect}:{status}"));
        }
        observed.push(format!("{} {}", case["id"].as_str().unwrap(), cells.join(",")));
    }
    panic!("TASK139_DISCOVERY\\n{}", observed.join("\\n"));
}
