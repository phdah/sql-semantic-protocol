//! TASK-138 public SQL seam for deterministic star projection transforms.

use sql_semantic_protocol::{analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, ValueDomain};

fn output(sql: &str, dialect: &str) -> Vec<(String, ValueDomain)> {
    let parser = dialect_from_name(dialect).expect("registered dialect");
    let document = analyze_sql(sql, dialect, parser.as_ref()).expect("SQL parses and analyzes");
    let Some(ProtocolStatement::Query(query)) = document.statements().first() else {
        panic!("expected query");
    };
    query.output().columns().iter()
        .map(|column| (column.name().to_string(), column.domain().clone()))
        .collect()
}

fn cte(projection: &str) -> String {
    format!("WITH t AS (SELECT 1 AS a, 2 AS b) SELECT {projection} FROM t")
}

#[test]
fn except_removes_named_column_without_losing_surviving_domain() {
    let columns = output(&cte("* EXCEPT (b)"), "bigquery");
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].0, "a");
    assert_eq!(columns[0].1, output(&cte("a"), "bigquery")[0].1);
}

#[test]
fn snowflake_exclude_removes_only_the_named_column() {
    let columns = output(&cte("* EXCLUDE b"), "snowflake");
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].0, "a");
    assert_eq!(columns[0].1, output(&cte("a"), "snowflake")[0].1);
}

#[test]
fn rename_preserves_value_domain_but_changes_output_identity() {
    let columns = output(&cte("* RENAME b AS c"), "snowflake");
    assert_eq!(columns.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), vec!["a", "c"]);
    assert_eq!(columns[1].1, output(&cte("b"), "snowflake")[0].1);
}

#[test]
fn replace_changes_exact_output_domain_at_the_replaced_position() {
    let columns = output(&cte("* REPLACE (9 AS b)"), "bigquery");
    assert_eq!(columns.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    let ValueDomain::Set(ref values) = columns[1].1 else {
        panic!("literal replacement must have a finite domain");
    };
    assert_eq!(values.values().len(), 1);
    assert_eq!(values.values()[0].value(), &LiteralValue::Number("9".to_string()));
}

#[test]
fn unknown_except_column_must_not_silently_expand_the_unmodified_star() {
    let columns = output(&cte("* EXCEPT (missing)"), "bigquery");
    assert!(columns.iter().any(|(_, domain)| matches!(domain, ValueDomain::Unknown(_))));
    assert_ne!(columns.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
}
