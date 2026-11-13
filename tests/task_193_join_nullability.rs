//! TASK-193: equality joins imply NOT NULL only for ordinary SQL TRUE.
//! Null-safe equality, outer null-extension and anti-membership must remain
//! nullable; these boundaries are material to all 13 canonical parsers.

use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, SetMode, ValueDomain,
};

const DIALECTS: &[&str] = &[
    "ansi",
    "bigquery",
    "clickhouse",
    "databricks",
    "duckdb",
    "generic",
    "hive",
    "mssql",
    "mysql",
    "postgresql",
    "redshift",
    "snowflake",
    "sqlite",
];

fn projected_id_domain(sql: &str, dialect_name: &str) -> ValueDomain {
    let dialect = dialect_from_name(dialect_name).expect("registered parser");
    let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
        .unwrap_or_else(|error| panic!("{dialect_name}: SQL parse failed: {error}"));
    let [ProtocolStatement::Query(query)] = protocol.statements() else {
        panic!("{dialect_name}: expected one source-backed query");
    };
    let [column] = query.output().columns() else {
        panic!("{dialect_name}: exactly one projected ID");
    };
    column.domain().clone()
}

fn excludes_null(domain: &ValueDomain) -> bool {
    match domain {
        ValueDomain::Empty => true,
        ValueDomain::Set(set) if set.mode() == SetMode::Exclude => set
            .values()
            .iter()
            .any(|literal| matches!(literal.value(), LiteralValue::Null)),
        ValueDomain::Set(set) if set.mode() == SetMode::Include => !set
            .values()
            .iter()
            .any(|literal| matches!(literal.value(), LiteralValue::Null)),
        _ => false,
    }
}

#[test]
fn ordinary_inner_join_equality_excludes_null_on_the_projected_column() {
    let positive = [
        "SELECT l.id FROM orders l INNER JOIN customers r ON l.id = r.id",
        "SELECT l.id FROM orders l INNER JOIN customers r ON l.id = r.id AND l.amount = r.amount",
        "SELECT a.id FROM orders a JOIN orders b ON a.id = b.id",
        "SELECT a.id FROM orders a JOIN orders b ON a.id = b.id JOIN orders c ON b.id = c.id",
        "SELECT a.id FROM orders a JOIN customers b ON a.id = b.id JOIN products c ON b.id = c.id",
    ];
    for dialect in DIALECTS {
        for sql in positive {
            let domain = projected_id_domain(sql, dialect);
            assert!(
                excludes_null(&domain),
                "{dialect}: ordinary = join's selected ID cannot be SQL NULL: {sql}: {domain:?}"
            );
        }
    }
}

#[test]
fn null_safe_equality_and_outer_join_do_not_falsely_exclude_sql_null() {
    let negative = [
        // NULL amount can match a different row with NULL amount, and l.id
        // itself can be NULL even when the amount comparison is TRUE.
        "SELECT l.id FROM orders l INNER JOIN customers r ON l.amount IS NOT DISTINCT FROM r.amount",
        "SELECT l.id FROM orders l LEFT JOIN customers r ON l.id = r.id",
        "SELECT l.id FROM orders l RIGHT JOIN customers r ON l.id = r.id",
        "SELECT l.id FROM orders l FULL OUTER JOIN customers r ON l.id = r.id",
        "SELECT l.id FROM orders l LEFT ANTI JOIN customers r ON l.id = r.id",
    ];
    for dialect in DIALECTS {
        for sql in negative {
            let domain = projected_id_domain(sql, dialect);
            assert!(
                !excludes_null(&domain),
                "{dialect}: nullable SQL outcome must not be narrowed by an inner = law: {sql}: {domain:?}"
            );
        }
    }
}
