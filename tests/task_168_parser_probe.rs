//! TASK-168 parser probe: will become an immutable dialect classification test.
mod common;
use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, ConfiguredSqlInput,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

#[test]
fn task_168_frozen_qualify_parser_classes() {
    let sql = "SELECT k,v FROM l QUALIFY ROW_NUMBER() OVER (PARTITION BY k ORDER BY v) = 1";
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "l",
        [("id", "VARCHAR"), ("k", "INTEGER"), ("v", "INTEGER")]
            .into_iter()
            .map(|(name, kind)| SchemaColumn::from_sql_type(name, kind, "postgresql").unwrap())
            .collect(),
    )
    .unwrap()]).unwrap();
    let mut summary = Vec::new();
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        let input = SqlInput::inline(sql);
        let configured = ConfiguredSqlInput::new("task168-qualify", &input, dialect, parser.as_ref());
        let result = analyze_configured_inputs_with_catalog(&[configured], &catalog);
        let class = if result.is_ok() { "accepted" } else { "boundary" };
        summary.push(format!("{dialect}:{class}"));
    }
    panic!("TASK-168 PROBE {}", summary.join(","));
}
