use serde_json::Value;
use sql_semantic_protocol::dialect_from_name;
use sqlparser::parser::Parser;
#[test]
fn parser_classification_probe() {
    let fixture: Value = serde_json::from_str(include_str!("../docs/task-136-acceptance-freeze.json")).unwrap();
    let mut output = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let mut supported = Vec::new();
        let mut rejected = Vec::new();
        for cell in case["dialect_cells"].as_array().unwrap() {
            let d = cell["dialect"].as_str().unwrap();
            let dialect = dialect_from_name(d).unwrap();
            if Parser::parse_sql(dialect.as_ref(), case["sql"].as_str().unwrap()).is_ok() {
                supported.push(d);
            } else {
                rejected.push(d);
            }
        }
        output.push(format!("{} parsed=[{}] rejected=[{}]",case["id"],supported.join(","),rejected.join(",")));
    }
    panic!("TASK136 PARSER MAP\n{}",output.join("\n"));
}
