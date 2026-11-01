//! TASK-156 parser-dialect, computed expression and explicit vendor/UPSERT boundaries.
//! Parser acceptance is not execution evidence and never promotes a residual to Exact.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, Expression, MergeAction, ProtocolStatement, SqlInput,
    WriteKind,
};
use sqlparser::parser::Parser;

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-156-acceptance-freeze.json")).unwrap()
}

fn classify(dialect: &str, sql: &str, kind: &str) -> &'static str {
    let parser = dialect_from_name(dialect).unwrap();
    if Parser::parse_sql(parser.as_ref(), sql).is_err() {
        return "parser_boundary";
    }
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
        .unwrap_or_else(|err| panic!("{dialect}: parsed syntax has no canonical result: {err}"));
    match bundle.inputs()[0].statements().first().unwrap() {
        ProtocolStatement::Unsupported(_) => "canonical_unsupported",
        ProtocolStatement::Query(query) => match query.write() {
            Some(write)
                if kind == "merge"
                    && write.kind() == WriteKind::ConditionalMutation
                    && write.match_condition().is_some()
                    && query.sources().len() == 1
                    && write.merge_clauses().len() == 1 =>
            {
                let action = write.merge_clauses()[0].action();
                let expression = match action {
                    MergeAction::Update { assignments } => {
                        assert_eq!(assignments.len(), 1);
                        Some(assignments[0].value().expression())
                    }
                    MergeAction::Insert { values, .. } => {
                        assert_eq!(values.len(), 1);
                        assert_eq!(values[0].len(), 2);
                        Some(values[0][1].expression())
                    }
                    MergeAction::Delete | MergeAction::Unsupported(_) => None,
                };
                if expression.is_some_and(|expr| matches!(expr, Expression::Binary(_))) {
                    "parsed_canonical"
                } else {
                    "canonical_unsupported"
                }
            }
            _ => "canonical_unsupported",
        },
        _ => "canonical_unsupported",
    }
}

#[test]
fn all_thirteen_parser_dialects_have_explicit_frozen_computed_and_vendor_cells() {
    let frozen = frozen();
    let variants = frozen["dialect_syntax"].as_array().unwrap();
    let dialects = frozen["dialects"].as_array().unwrap();
    assert_eq!(variants.len(), 5);
    assert_eq!(dialects.len(), 13);
    let mut results = BTreeMap::new();
    for dialect in dialects {
        let name = dialect.as_str().unwrap();
        let observed = variants
            .iter()
            .map(|variant| {
                classify(
                    name,
                    variant["sql"].as_str().unwrap(),
                    variant["kind"].as_str().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(observed.len(), 5);
        for (index, verdict) in observed.iter().enumerate() {
            if index < 3 {
                assert_eq!(
                    *verdict, "parsed_canonical",
                    "{name}: shared computed MERGE variant {index} must normalize"
                );
            } else {
                assert_ne!(
                    *verdict, "parsed_canonical",
                    "{name}: vendor law cannot claim portable computed MERGE"
                );
            }
        }
        results.insert(name.to_owned(), observed);
    }
    assert_eq!(results.values().map(Vec::len).sum::<usize>(), 65);
    assert_eq!(frozen["dialect_cells_required"], 65);
    assert_eq!(
        serde_json::to_value(results).unwrap(),
        frozen["dialect_expected"],
        "every advertised parser outcome requires a frozen explicit expected verdict"
    );
}
