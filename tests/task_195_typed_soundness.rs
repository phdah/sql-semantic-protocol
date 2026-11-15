//! TASK-195 original DuckDB output occurrence soundness across the 13
//! parser dialects and authoritative direct source column types.
//! Source fixture observations are never promoted to universal SQL bounds.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, ComposedSemantics,
    ConfiguredSqlInput, LiteralValue, RelationCatalog, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeSet;

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn catalog() -> RelationCatalog {
    let parser = dialect_from_name("duckdb").expect("source types dialect");
    let ddl = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders(id INTEGER, amount INTEGER);
             CREATE TABLE customers(id INTEGER, amount INTEGER);
             CREATE TABLE products(id INTEGER)",
        )],
        "duckdb",
        parser.as_ref(),
    )
    .expect("authoritative direct DDL");
    RelationCatalog::from_schemas(ddl.source_schemas()).expect("typed source catalog")
}

fn admits(domain: &ValueDomain, value: &Value) -> bool {
    match domain {
        ValueDomain::Unbounded | ValueDomain::Unknown(_) => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(set) => {
            let found = set.values().iter().any(|entry| match entry.value() {
                LiteralValue::Null => value.is_null(),
                LiteralValue::Number(number) => value
                    .as_f64()
                    .zip(number.parse::<f64>().ok())
                    .is_some_and(|(actual, expected)| (actual - expected).abs() < 1e-9),
                _ => false,
            });
            match set.mode() {
                SetMode::Include => found,
                SetMode::Exclude => !found,
            }
        }
        ValueDomain::Ranges(ranges) => {
            let Some(observed) = value.as_f64() else {
                return false;
            };
            ranges.ranges().iter().any(|range| {
                let lower = range.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(number) = raw.parse::<f64>() else {
                        return false;
                    };
                    observed > number || (observed == number && bound.inclusive())
                });
                let upper = range.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(number) = raw.parse::<f64>() else {
                        return false;
                    };
                    observed < number || (observed == number && bound.inclusive())
                });
                lower && upper
            })
        }
        // Never make a membership claim for a future unexamined domain kind.
        _ => false,
    }
}

#[test]
fn all_28_original_deterministic_bags_respect_accepted_typed_output_domains() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("task-owned original SQL");
    let catalog = catalog();
    let mut checked = BTreeSet::new();
    let mut cases = 0;
    let mut checked_occurrences = 0;
    for case in frozen["cases"].as_array().expect("36 named original SQL") {
        let Some(terminal) = case["native_oracle"]["expected_terminal_bag"].as_array() else {
            continue;
        };
        cases += 1;
        let id = case["id"].as_str().expect("original ID");
        let sql = case["sql"].as_str().expect("immutable original SQL");
        let arity = case["native_oracle"]["columns"]
            .as_u64()
            .expect("native width") as usize;
        for cell in case["cells"].as_array().expect("13 named dialects") {
            if cell["original_observed_status"] == "R" {
                continue;
            }
            let name = cell["dialect"].as_str().expect("public dialect");
            let key = format!("{id}/{name}");
            assert!(checked.insert(key.clone()), "{key}: duplicate cell");
            let parser = dialect_from_name(name).expect("public parser");
            let source = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                &key,
                &source,
                name,
                parser.as_ref(),
            )];
            let analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{key}: source schema analysis: {error}"));
            let ComposedSemantics::Resolved(semantics) = analyzed
                .layers()
                .last()
                .expect("typed terminal")
                .composed_semantics()
            else {
                panic!("{key}: no supported canonical output");
            };
            let domains = semantics.output().columns();
            assert_eq!(domains.len(), arity, "{key}: projected column width");
            for row in terminal {
                let tuple = row.as_array().expect("literal terminal tuple");
                assert_eq!(tuple.len(), arity, "{key}: terminal width");
                for (column, value) in domains.iter().zip(tuple) {
                    assert!(
                        admits(column.domain(), value),
                        "{key}: claimed domain {:?} excluded native terminal {value}",
                        column.domain()
                    );
                    checked_occurrences += 1;
                }
            }
        }
    }
    assert_eq!(cases, 28);
    assert_eq!(
        checked.len(),
        363,
        "28 originals × 13 dialects minus one named parser rejection"
    );
    assert!(
        checked_occurrences >= 1000,
        "all NULL and peer duplicates verified"
    );
}
