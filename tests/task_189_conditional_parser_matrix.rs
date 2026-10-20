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

    assert_eq!(observed.len(), 6, "all conditional cases must be classified");
    assert_eq!(
        observed, frozen,
        "replace unverified parser cells with observed parser boundaries, then review the freeze"
    );
}
