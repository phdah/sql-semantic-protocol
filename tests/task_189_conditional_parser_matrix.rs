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
    assert_eq!(matrix["version"], 5);
    let dialects = matrix["required_dialects"].as_array().unwrap();
    let mut native_duckdb = 0;
    let mut native_sqlite = 0;
    let mut sqlite_unsupported = 0;
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
                ("parsed", "sqlite") if case["id"] == "74-C189-C1" => {
                    assert_eq!(native, "sqlite_native_unsupported_executed_in_ci");
                    sqlite_unsupported += 1;
                }
                ("parsed", "sqlite") => {
                    assert_eq!(native, "sqlite_native_oracle_executed_in_ci");
                    native_sqlite += 1;
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
        (
            native_duckdb,
            native_sqlite,
            sqlite_unsupported,
            residual_vendor,
            parser_boundaries
        ),
        (6, 5, 1, 59, 7)
    );
}

#[test]
fn exact_mandatory_cells_and_reviewed_residuals_are_distinct() {
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json")).unwrap();
    assert_eq!(matrix["version"], 5);
    assert!(matrix["mandatory_unverified"].as_array().unwrap().is_empty());
    let dialects = matrix["required_dialects"].as_array().unwrap();
    let adapters = matrix["required_adapters"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    assert_eq!(adapters.len(), 3);

    let mut mandatory = 0;
    let mut conditional = 0;
    for case in matrix["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(matrix["numeric_oracle_cases"].as_array().unwrap())
    {
        match case["proof"].as_str().unwrap() {
            "Exact" => {
                mandatory += 1;
                assert!(!case["full_terminal_bag"].as_array().unwrap().is_empty());
                assert!(!case["typed_terminal_domains"].as_array().unwrap().is_empty());
                assert_eq!(
                    case["source_bag"].as_array().unwrap().len(),
                    case["source_row_ids"].as_array().unwrap().len()
                );
                for dialect in dialects {
                    let dialect = dialect.as_str().unwrap();
                    assert_eq!(
                        case["parser_status_by_dialect"][dialect], "parsed",
                        "{}/{}",
                        case["id"],
                        dialect
                    );
                }
                for adapter in adapters {
                    let adapter = adapter.as_str().unwrap();
                    assert_eq!(
                        case["expected_adapter_cells"][adapter], "Exact_required",
                        "{}/{}",
                        case["id"],
                        adapter
                    );
                }
            }
            "Residual_expected" => {
                conditional += 1;
                assert!(
                    case["release_disposition"]
                        .as_str()
                        .unwrap()
                        .starts_with("Reviewed conditional Residual:"),
                    "{}: conditional release requires an explicit review",
                    case["id"]
                );
                for adapter in adapters {
                    let adapter = adapter.as_str().unwrap();
                    assert_eq!(
                        case["expected_adapter_cells"][adapter], "Residual_required",
                        "{}/{}",
                        case["id"],
                        adapter
                    );
                }
            }
            other => panic!("unreviewed TASK-189 proof classification: {other}"),
        }
    }
    assert_eq!((mandatory, conditional), (10, 8));
}
