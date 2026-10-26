//! All 130 frozen TASK-136 parser/canonical-dialect obligations are executable.
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    LiteralValue, ConfiguredSqlInput, PhysicalFinitePageProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[
        RelationSchema::new("s",vec![
            SchemaColumn::from_sql_type("k","INTEGER","postgresql").unwrap()
        ]).unwrap()
    ]).unwrap()
}
fn literal(v: &Value) -> LiteralValue {
    v.as_i64().map_or(LiteralValue::Null,|n|LiteralValue::Number(n.to_string()))
}
fn state(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new("s",case["source"].as_array().unwrap().iter().map(|v|
        PhysicalWriteRow::new(BTreeMap::from([("k".to_string(),literal(v))])).unwrap()
    ).collect()).unwrap()
}
#[test]
fn all_130_declared_parser_cells_preserve_the_frozen_canonical_outcome() {
    let freeze: Value = serde_json::from_str(include_str!("../docs/task-136-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"],2);
    let mut mandatory = 0;
    let mut conditional = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let sql = case["sql"].as_str().unwrap();
        let cells = case["dialect_cells"].as_array().unwrap();
        assert_eq!(cells.len(),13,"{}: every dialect present",case["id"]);
        for cell in cells {
            let dialect = cell["dialect"].as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            let parsed = Parser::parse_sql(parser.as_ref(),sql);
            assert_eq!(parsed.is_ok(),cell["parser"]=="parsed",
                "{}/{dialect}: parser boundary diverges: {parsed:?}",case["id"]);
            if cell["parser"]=="parser_boundary" {
                conditional += 1;
                continue;
            }
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                case["id"].as_str().unwrap(),&input,dialect,parser.as_ref()
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured,&catalog()).unwrap();
            let target = bundle.layers().last().unwrap().id();
            let output = physical_finite_page_bag_plan(&bundle,target,&[state(case)]);
            if case["classification"]=="mandatory" {
                mandatory += 1;
                assert_eq!(cell["canonical"],"exact_physical_bag");
                let PhysicalFinitePageProof::Exact(exact) = output else {
                    panic!("{}/{dialect}: mandatory exact proof missing: {output:?}",case["id"]);
                };
                let expected = case["output"].as_array().unwrap().iter().map(literal).collect::<Vec<_>>();
                let actual = exact.terminal.iter().map(|row|row["k"].clone()).collect::<Vec<_>>();
                assert_eq!(actual,expected,"{}/{dialect}: terminal bag",case["id"]);
                let selected = exact.rows.iter().filter(|r|r.survives)
                    .map(|r|r.source_row_id.clone()).collect::<Vec<_>>();
                let rejected = exact.rows.iter().filter(|r|!r.survives)
                    .map(|r|r.source_row_id.clone()).collect::<Vec<_>>();
                let ids = |field: &str| case[field].as_array().unwrap().iter()
                    .map(|x|x.as_str().unwrap().to_string()).collect::<Vec<_>>();
                assert_eq!(selected,ids("selected"));
                assert_eq!(rejected,ids("rejected"));
                assert_eq!(exact.terminal_domains["k"].admits_null(),
                    case["terminal_domain"]["k"]["nullable"].as_bool());
            } else {
                conditional += 1;
                assert_eq!(cell["canonical"],"residual_membership");
                assert!(matches!(output,PhysicalFinitePageProof::Residual { .. }),
                    "{}/{dialect}: conditional membership claimed exact: {output:?}",case["id"]);
                assert_eq!(cell["native_execution"],"unverified");
            }
        }
    }
    assert_eq!(mandatory,65,"5 mandatory cases x 13 dialects");
    assert_eq!(conditional,65,"5 conditional cases x 13 dialects");
}
