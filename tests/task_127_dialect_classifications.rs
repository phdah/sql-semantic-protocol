//! TASK-127: all 13 sqlparser entrypoints are observed, including parser boundaries.
//! A successful parse is not a physical JOIN/GROUP proof or native vendor execution.
use serde_json::{Map, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, ConfiguredSqlInput,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;

#[test]
fn frozen_qualify_parser_and_canonical_cells_match_all_thirteen_dialects() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json")).unwrap();
    let dialects = freeze["required_dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    let mut observations = Vec::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let schemas = case["source_schema"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(relation, columns)| {
                RelationSchema::new(
                    relation.clone(),
                    columns
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(name, ty)| {
                            let parsed_type = ty.as_str().unwrap().split_whitespace().next().unwrap();
                            SchemaColumn::from_sql_type(name, parsed_type, "postgresql").unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
        let mut cells = Map::new();
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let parser = dialect_from_name(name).unwrap();
            let classification = if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                "parser_boundary"
            } else {
                let input = SqlInput::inline(sql);
                match analyze_configured_inputs_with_catalog(
                    &[ConfiguredSqlInput::new(id, &input, name, parser.as_ref())],
                    &catalog,
                ) {
                    Ok(bundle) => {
                        let canonical = bundle.inputs().iter().flat_map(|input| input.statements())
                            .filter_map(|statement| match statement {
                                ProtocolStatement::Query(query) => Some(query),
                                _ => None,
                            })
                            .any(|query| query.predicates().qualify_predicate().is_some()
                                && query.diagnostics().is_empty());
                        if canonical { "parsed" } else { "semantic_residual" }
                    }
                    Err(_) => "semantic_residual",
                }
            };
            cells.insert(name.to_owned(), Value::String(classification.to_owned()));
        }
        observations.push((id, cells));
    }
    let mismatch = observations.iter().any(|(id, cells)| {
        let case = cases.iter().find(|case| case["id"] == *id).unwrap();
        dialects.iter().any(|dialect| {
            let name = dialect.as_str().unwrap();
            case["parser_status_by_dialect"][name] != cells[name]
        })
    });
    assert!(
        !mismatch,
        "TASK-127 must record observed parser/canonical boundaries in frozen matrix: {observations:?}"
    );
}
