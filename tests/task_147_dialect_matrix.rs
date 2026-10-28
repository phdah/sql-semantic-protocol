//! TASK-147: parse boundary and canonical form are deliberately distinct.

use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput, WriteKind,
};

const DIALECTS: [&str; 13] = [
    "generic", "mysql", "postgresql", "hive", "sqlite", "snowflake",
    "redshift", "mssql", "clickhouse", "bigquery", "ansi", "duckdb", "databricks",
];

#[test]
fn portable_conflict_syntax_is_canonical_for_every_parser_that_accepts_it() {
    let sql = "INSERT INTO target(id,score) VALUES (1,2)
        ON CONFLICT(id) DO NOTHING RETURNING id";
    let mut supported = 0;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) else {
            // A dialect parser rejecting the syntax is a parser boundary,
            // not an inference of native engine support.
            continue;
        };
        let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
            panic!("parsed shared syntax must not lose its canonical law: {dialect}");
        };
        let write = query.write().expect("normalized INSERT operation");
        assert_eq!(write.kind(), WriteKind::Append, "{dialect}");
        assert_eq!(write.insert_conflict().unwrap().columns(), &["id"], "{dialect}");
        assert_eq!(write.returning_columns(), &["id"], "{dialect}");
        supported += 1;
    }
    assert!(supported >= 2, "DuckDB and PostgreSQL syntax must parse");
}

#[test]
fn unsupported_conflict_operators_never_become_plain_append() {
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for sql in [
            "INSERT INTO target(id,score) VALUES (1,2) ON CONFLICT DO NOTHING",
            "INSERT INTO target(id,score) VALUES (1,2) ON CONFLICT(id) DO UPDATE SET score=3",
        ] {
            let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) else {
                continue;
            };
            assert!(
                matches!(bundle.inputs()[0].statements()[0], ProtocolStatement::Unsupported(_)),
                "{dialect}: {sql}"
            );
        }
    }
}
