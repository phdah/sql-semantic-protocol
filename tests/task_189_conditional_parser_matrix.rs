//! TASK-189 conditional SQL parser boundaries are frozen, never inferred from native engine support.
use serde_json::Value;
use sql_semantic_protocol::dialect_from_name;
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

#[test]
fn every_conditional_parser_cell_matches_the_task_owned_freeze() {
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json")).unwrap();
    let dialects = matrix["required_dialects"].as_array().unwrap();
    let mut observed = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut frozen = BTreeMap::<String, BTreeMap<String, String>>::new();

    for case in matrix["cases"].as_array().unwrap() {
        if case["proof"] != "Residual_expected" {
            continue;
        }
        let id = case["id"].as_str().unwrap().to_string();
        let sql = case["sql"].as_str().unwrap();
        let mut parser_cells = BTreeMap::new();
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let parser = dialect_from_name(name).unwrap();
            let status = if Parser::parse_sql(parser.as_ref(), sql).is_ok() {
                "parsed"
            } else {
                "parser_boundary"
            };
            parser_cells.insert(name.to_string(), status.to_string());
        }
        observed.insert(id.clone(), parser_cells);
        frozen.insert(
            id,
            serde_json::from_value(case["parser_status_by_dialect"].clone()).unwrap(),
        );
    }

    assert_eq!(
        observed.len(),
        6,
        "all conditional cases must be classified"
    );
    assert_eq!(
        observed, frozen,
        "replace unverified parser cells with observed parser boundaries, then review the freeze"
    );
}

#[test]
fn every_conditional_native_session_cell_has_a_conservative_owned_disposition() {
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json")).unwrap();
    assert_eq!(matrix["version"], 4);
    let dialects = matrix["required_dialects"].as_array().unwrap();
    let mut native_duckdb = 0;
    let mut residual_vendor = 0;
    let mut parser_boundaries = 0;
    for case in matrix["cases"].as_array().unwrap() {
        if case["proof"] != "Residual_expected" {
            continue;
        }
        assert_eq!(case["conditional_owner"], "TASK-189");
        assert!(
            !case["reason"].as_str().unwrap().is_empty(),
            "conditional native law must have a specific reason"
        );
        assert_eq!(
            case["native_execution_disposition_by_dialect"]
                .as_object()
                .unwrap()
                .len(),
            dialects.len()
        );
        for dialect in dialects {
            let dialect = dialect.as_str().unwrap();
            let parser = case["parser_status_by_dialect"][dialect].as_str().unwrap();
            let native = case["native_execution_disposition_by_dialect"][dialect]
                .as_str()
                .unwrap();
            match (parser, dialect) {
                ("parser_boundary", _) => {
                    assert_eq!(native, "parser_boundary");
                    parser_boundaries += 1;
                }
                ("parsed", "duckdb") => {
                    assert_eq!(native, "duckdb_native_oracle_executed_in_ci");
                    native_duckdb += 1;
                }
                ("parsed", _) => {
                    assert_eq!(native, "residual_native_vendor_or_session_unattested");
                    residual_vendor += 1;
                }
                _ => panic!("unreviewed conditional parser or execution cell"),
            }
        }
    }
    assert_eq!(
        (native_duckdb, residual_vendor, parser_boundaries),
        (6, 65, 7)
    );
}
