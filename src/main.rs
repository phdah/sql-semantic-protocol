use std::env;
use std::io::{Error as IoError, ErrorKind};

use sql_semantic_protocol::{analyze_sql, to_json};
use sqlparser::dialect::GenericDialect;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sql = env::args().skip(1).collect::<Vec<_>>().join(" ");

    if sql.trim().is_empty() {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "usage: sql-semantic-protocol <sql>",
        )
        .into());
    }

    let dialect = GenericDialect {};
    let protocol = analyze_sql(&sql, "generic", &dialect)?;
    println!("{}", to_json(&protocol));

    Ok(())
}
