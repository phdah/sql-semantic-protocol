use std::{env, fs};

use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::SnowflakeDialect;
use sqlparser::parser::Parser;

use crate::debug_probe::debug_probe;
use crate::semantic::extract_schema;

mod debug_probe;
mod semantic;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sql = r#"
        SELECT
            DATE_TRUNC('day', created_at) AS day,
            COUNT(*) AS n
        FROM my_db.my_schema.events
        WHERE created_at >= DATEADD(day, -7, CURRENT_TIMESTAMP())
        GROUP BY 1
        QUALIFY ROW_NUMBER() OVER (ORDER BY day DESC) <= 10
    "#;

    let output_file = env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("ast.json"));
    let dialect = SnowflakeDialect {};
    let ast = Parser::parse_sql(&dialect, sql)?;

    let probe_input = 41;
    let probe_output = debug_probe(probe_input);
    println!("debug_probe({probe_input}) = {probe_output}");

    fs::write(&output_file, serde_json::to_string_pretty(&ast)?)?;
    println!("Wrote AST to {output_file}");

    for stmt in &ast {
        if let Statement::Query(query) = stmt {
            if let SetExpr::Select(select) = query.body.as_ref() {
                let schema = extract_schema(select);
                println!("\n{schema}");
            }
        }
    }

    Ok(())
}
