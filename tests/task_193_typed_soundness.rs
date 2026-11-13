//! TASK-193 exact finite DuckDB terminal bag soundness against emitted
//! source-typed canonical output domains for 24 original SQL variants ×
//! 13 dialects. The observed source bag constrains *test instances*, not the
//! universally valid SQL domain; unbounded domains remain conservative.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, ComposedSemantics,
    ConfiguredSqlInput, LiteralValue, RelationCatalog, SetMode, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-193-acceptance-freeze.json");

fn catalog() -> RelationCatalog {
    let name = "duckdb";
    let dialect = dialect_from_name(name).expect("project dialect");
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders(id INTEGER, amount INTEGER);
             CREATE TABLE customers(id INTEGER, amount INTEGER);
             CREATE TABLE products(id INTEGER)",
        )],
        name,
        dialect.as_ref(),
    )
    .expect("typed physical schema");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("schema-backed domain evidence")
}

fn admits(domain: &ValueDomain, value: &Value) -> bool {
    match domain {
        ValueDomain::Unbounded | ValueDomain::Unknown(_) => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(set) => {
            let observed = if value.is_null() {
                LiteralValue::Null
            } else if let Some(v) = value.as_i64() {
                LiteralValue::Number(v.to_string())
            } else {
                return false;
            };
            let included = set.values().iter().any(|v| v.value() == &observed);
            match set.mode() {
                SetMode::Include => included,
                SetMode::Exclude => !included,
            }
        }
        ValueDomain::Ranges(ranges) => {
            let Some(v) = value.as_i64().map(i128::from) else {
                return false;
            };
            ranges.ranges().iter().any(|range| {
                let min = range.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(n) = raw.parse::<i128>() else {
                        return false;
                    };
                    v > n || (v == n && bound.inclusive())
                });
                let max = range.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(n) = raw.parse::<i128>() else {
                        return false;
                    };
                    v < n || (v == n && bound.inclusive())
                });
                min && max
            })
        }
        // A newly introduced domain cannot claim proof without an updated test.
        _ => false,
    }
}

#[test]
fn all_312_original_source_typed_public_cells_admit_complete_literal_terminal_bags() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("task-owned 24 × 13 freeze");
    let direct = catalog();
    assert_eq!(freeze["case_count"], 24);
    assert_eq!(freeze["parser_cells"], 312);
    let mut cells = BTreeSet::new();
    let mut checked_rows = 0;
    for case in freeze["cases"].as_array().expect("24 original cases") {
        let id = case["id"].as_str().expect("original case ID");
        let sql = case["sql"].as_str().expect("original SQL");
        let terminals = case["expected_terminal_bag"]
            .as_array()
            .expect("independently pinned complete DuckDB output multiset");
        let expected_columns = case["expected_columns"].as_u64().expect("arity") as usize;
        for dialect_name in freeze["dialects"].as_array().expect("13 dialect names") {
            let name = dialect_name.as_str().expect("dialect");
            let cell_id = format!("{id}/{name}");
            assert!(cells.insert(cell_id.clone()), "{cell_id}: duplicate");
            let dialect = dialect_from_name(name).expect("one of 13 parsers");
            let source = SqlInput::inline(sql);
            let selected = [ConfiguredSqlInput::new(
                &cell_id,
                &source,
                name,
                dialect.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&selected, &direct)
                .unwrap_or_else(|error| panic!("{cell_id}: source-typed analysis: {error}"));
            let ComposedSemantics::Resolved(semantics) = bundle
                .layers()
                .last()
                .expect("one original SQL layer")
                .composed_semantics()
            else {
                panic!("{cell_id}: original SQL may not silently lose its typed output");
            };
            let outputs = semantics.output().columns();
            assert_eq!(
                outputs.len(),
                expected_columns,
                "{cell_id}: projection arity"
            );
            for row in terminals {
                let values = row.as_array().expect("native literal output row");
                assert_eq!(values.len(), outputs.len(), "{cell_id}: native width");
                for (column, value) in outputs.iter().zip(values) {
                    assert!(
                        admits(column.domain(), value),
                        "{cell_id}: native literal {value} excluded by a claimed domain {:?}",
                        column.domain()
                    );
                    checked_rows += 1;
                }
            }
        }
    }
    assert_eq!(cells.len(), 312);
    assert!(
        checked_rows > 1500,
        "verify every duplicate and NULL terminal occurrence"
    );
}
