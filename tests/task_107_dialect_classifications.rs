//! Cross-dialect TASK-107 classifications derived from *actual* parser and
//! canonical-emitter calls. No dialect parser is treated as an engine oracle.
//!
//! The checked-in matrix holds one case per (TASK-66 variant, exposed dialect).
//! Each case is re-executed here; inventory drift or a changed AST normalization
//! invalidates the recorded classification rather than passing silently.

use serde_json::{json, Value};
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};
use sqlparser::parser::Parser;

const FREEZE: &str = include_str!("../docs/task-107-acceptance-freeze.json");

fn canonical(sql: &str, dialect: &str, parser: &dyn sqlparser::dialect::Dialect) -> Option<Value> {
    let protocol = analyze_sql(sql, dialect, parser).ok()?;
    let mut value: Value = serde_json::from_str(&to_json(&protocol)).ok()?;
    if let Some(input) = value["inputs"]
        .as_array_mut()
        .and_then(|inputs| inputs.first_mut())
    {
        input["dialect"] = Value::Null;
    }
    Some(value)
}

#[test]
fn all_frozen_dialect_cells_are_actually_parsed_and_equivalence_checked() {
    let freeze: Value = serde_json::from_str(FREEZE).unwrap();
    let variants = freeze["inventory_cells"].as_array().expect("42 variants");
    assert_eq!(variants.len(), 42);
    let mut observed = serde_json::Map::new();
    let mut missing = 0usize;

    for cell in variants {
        let id = cell["id"].as_str().expect("case id");
        let sql = cell["sql_shape"].as_str().expect("frozen SQL");
        let mut baseline: Option<Value> = None;
        let mut outcomes = serde_json::Map::new();
        let dialects = cell["dialect_cells"].as_object().expect("all dialects");
        assert_eq!(dialects.len(), 13);
        for (name, expected) in dialects {
            let dialect = dialect_from_name(name)
                .unwrap_or_else(|| panic!("{id}: unsupported public dialect {name}"));
            let parsed = Parser::parse_sql(dialect.as_ref(), sql);
            let (parser_status, canonical_status) = match parsed {
                Err(_) => ("parse_rejected", "not_applicable_parse"),
                Ok(_) => match canonical(sql, name, dialect.as_ref()) {
                    Some(value) => {
                        if let Some(reference) = &baseline {
                            if value == *reference {
                                ("parsed", "equivalent")
                            } else {
                                // Parser trees and emitted semantics can diverge.
                                // The review must assess meaning-changing dialect
                                // assumptions; never claim this is runtime parity.
                                ("parsed", "assumption_recorded")
                            }
                        } else {
                            baseline = Some(value);
                            ("parsed", "equivalent")
                        }
                    }
                    None => {
                        // Parsing alone does not establish semantic analysis.
                        ("parsed", "assumption_recorded")
                    }
                },
            };
            outcomes.insert(
                name.to_owned(),
                json!({
                    "parser": parser_status,
                    "canonical": canonical_status
                }),
            );
            if expected["parser"] == "unverified" || expected["canonical"] == "unverified" {
                missing += 1;
            } else {
                assert_eq!(
                    expected["parser"], parser_status,
                    "{id}/{name}: parser evidence changed; SQL={sql}"
                );
                assert_eq!(
                    expected["canonical"], canonical_status,
                    "{id}/{name}: canonical outcome changed; SQL={sql}"
                );
            }
        }
        observed.insert(id.to_owned(), Value::Object(outcomes));
    }
    assert_eq!(observed.len(), 42);
    if missing != 0 {
        // First execution deliberately produces the finite observed matrix.
        // Commit the outcome classifications before this task can be closed.
        panic!(
            "TASK107_CLASSIFICATIONS_START\\n{}\\nTASK107_CLASSIFICATIONS_END\\n{missing} unverified dialect cells",
            Value::Object(observed)
        );
    }
}
