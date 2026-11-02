//! TASK-160: every frozen SQL statement has an explicit parser/canonical
//! classification in each of the thirteen advertised dialects.
//! Parser success is not evidence for a database-specific DDL execution law.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_lifecycle_plan,
    PhysicalOrderedLifecycleProof, ProtocolStatement, SqlInput, WriteKind,
};
use sqlparser::parser::Parser;

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-160-acceptance-freeze.json")).unwrap()
}

#[test]
fn each_frozen_lifecycle_statement_is_normalized_or_has_a_named_parser_boundary() {
    let freeze = frozen();
    let mut classifications = std::collections::BTreeMap::new();
    for dialect in freeze["dialects"].as_array().unwrap() {
        let dialect = dialect.as_str().unwrap();
        let parser = dialect_from_name(dialect).unwrap();
        let mut dialect_cases = Vec::new();
        for case in freeze["cases"].as_array().unwrap() {
            let mut statements = Vec::new();
            for sql in case["program"].as_array().unwrap() {
                let sql = sql.as_str().unwrap();
                if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                    statements.push("parser_boundary");
                    continue;
                }
                let analyzed =
                    analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).unwrap();
                let status = match &analyzed.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => "canonical_unsupported",
                    ProtocolStatement::Query(query) => {
                        match query.write().map(|write| write.kind()) {
                            Some(WriteKind::Definition) => "definition",
                            Some(WriteKind::Append) => "insert",
                            Some(WriteKind::SchemaAlter) => "alter",
                            Some(WriteKind::Lifecycle) => "lifecycle",
                            Some(_) => "unrelated_write",
                            None => "read",
                        }
                    }
                    _ => "unexpected_statement",
                };
                statements.push(status);
            }
            dialect_cases.push(statements);
        }
        classifications.insert(dialect.to_string(), dialect_cases);
    }
    assert_eq!(classifications.len(), 13);
    let total = classifications
        .values()
        .flat_map(|cases| cases.iter())
        .map(Vec::len)
        .sum::<usize>();
    let expected = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["program"].as_array().unwrap().len())
        .sum::<usize>()
        * 13;
    assert_eq!(total, expected);
    // The observed matrix is printed for the next immutable versioned freeze.
    // It never upgrades parser acceptance into a native DuckDB proof.
    println!(
        "TASK160_DIALECT_OBSERVED={}",
        serde_json::to_string(&classifications).unwrap()
    );
    for (dialect, cases) in &classifications {
        for (index, sequence) in cases.iter().enumerate() {
            let case = &freeze["cases"][index];
            if case["requirement"] != "mandatory" {
                continue;
            }
            // Every parsed canonical shared SQL statement must stay in the
            // intended operator family; failures to parse are explicit.
            for (sql, classification) in case["program"].as_array().unwrap().iter().zip(sequence) {
                let sql = sql.as_str().unwrap();
                let acceptable = if sql.starts_with("CREATE TABLE") {
                    ["definition", "parser_boundary"].as_slice()
                } else if sql.starts_with("INSERT INTO") {
                    ["insert", "parser_boundary"].as_slice()
                } else if sql.starts_with("ALTER TABLE") {
                    ["alter", "parser_boundary"].as_slice()
                } else if sql.starts_with("DROP TABLE") {
                    ["lifecycle", "parser_boundary"].as_slice()
                } else {
                    ["read", "parser_boundary"].as_slice()
                };
                assert!(
                    acceptable.contains(classification),
                    "{dialect}: {sql} unexpectedly lowered as {classification}"
                );
            }
        }
    }
    let observed = serde_json::to_value(classifications).unwrap();
    if freeze["dialect_expected"].is_null() {
        panic!("TASK160_DIALECT_OBSERVED={observed}");
    }
    assert_eq!(
        observed,
        freeze["dialect_expected"],
        "all parser and canonical outcomes must be pinned in the versioned acceptance freeze"
    );
}

#[test]
fn native_duckdb_validates_all_mandatory_full_script_classifications() {
    let freeze = frozen();
    let parser = dialect_from_name("duckdb").unwrap();
    for case in freeze["cases"].as_array().unwrap() {
        if case["requirement"] != "mandatory" {
            continue;
        }
        let sql = case["program"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sql| SqlInput::inline(sql.as_str().unwrap()))
            .collect::<Vec<_>>();
        let analyzed = analyze_inputs(&sql, "duckdb", parser.as_ref()).unwrap();
        let outcome = physical_ordered_lifecycle_plan(&analyzed, &[]);
        match case["classification"].as_str().unwrap() {
            "Exact" => assert!(
                matches!(outcome, PhysicalOrderedLifecycleProof::Exact(_)),
                "{}: {outcome:?}",
                case["id"]
            ),
            "Impossible" => assert!(
                matches!(outcome, PhysicalOrderedLifecycleProof::Impossible { .. }),
                "{}: {outcome:?}",
                case["id"]
            ),
            other => panic!("unreviewed mandatory classification {other}"),
        }
    }
}
