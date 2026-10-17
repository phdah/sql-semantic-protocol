//! TASK-113: 182 independently observed parser/canonical dialect classifications.
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, ComparisonOperator,
    ConfiguredSqlInput, JoinKind, Predicate, ProtocolStatement, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};

const MATRIX: &str = include_str!("../docs/task-113-acceptance-freeze.json");

#[test]
fn frozen_task_113_parser_cells_match_all_registered_dialects() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            let identity = if relation == "l" { "lid" } else { "rid" };
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type(identity, "VARCHAR", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let mut observations = Vec::new();
    for case in matrix["cases"].as_array().unwrap() {
        let mut cells = serde_json::Map::new();
        for name in matrix["dialects"].as_array().unwrap() {
            let dialect = name.as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            let sql = SqlInput::inline(case["sql"].as_str().unwrap());
            let configured = ConfiguredSqlInput::new("task113", &sql, dialect, parser.as_ref());
            let status = match analyze_configured_inputs_with_catalog(&[configured], &catalog) {
                Ok(bundle) => {
                    let query = bundle.inputs()[0]
                        .statements()
                        .iter()
                        .find_map(|statement| {
                            if let ProtocolStatement::Query(query) = statement {
                                Some(query)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    let join = &query.joins()[0];
                    let kind = match join.kind() {
                        JoinKind::Cross => "cross",
                        JoinKind::Inner => "inner",
                        _ => "other",
                    };
                    let condition = match join.condition() {
                        None => "no_predicate",
                        Some(Predicate::Comparison(c)) => match c.operator() {
                            ComparisonOperator::Eq => "eq",
                            ComparisonOperator::Lt => "lt",
                            ComparisonOperator::IsNotDistinctFrom => "is_not_distinct_from",
                            _ => "other_comparison",
                        },
                        Some(Predicate::Or(_)) => "or",
                        _ => "other",
                    };
                    if query.diagnostics().is_empty()
                        && case["canonical_ast_expectation"]["join_kind"] == kind
                        && case["canonical_ast_expectation"]["condition"] == condition
                    {
                        "parser_equivalent"
                    } else {
                        "semantic_residual"
                    }
                }
                Err(_) => "parser_boundary",
            };
            cells.insert(dialect.to_string(), json!(status));
        }
        observations.push(json!({"id":case["id"],"cells":cells}));
    }
    assert_eq!(observations.len(), 14);
    assert_eq!(matrix["dialects"].as_array().unwrap().len(), 13);
    for (observed, frozen) in observations.iter().zip(matrix["cases"].as_array().unwrap()) {
        assert_eq!(observed["id"], frozen["id"]);
        for dialect in matrix["dialects"].as_array().unwrap() {
            let name = dialect.as_str().unwrap();
            assert_eq!(
                observed["cells"][name], frozen["dialects"][name]["parser"],
                "{name}: parser and canonical classification of {}",
                frozen["id"]
            );
            assert_eq!(
                frozen["dialects"][name]["canonical"], "equivalent_when_parsed",
                "{name}: semantic shape must remain canonical"
            );
        }
    }
}
