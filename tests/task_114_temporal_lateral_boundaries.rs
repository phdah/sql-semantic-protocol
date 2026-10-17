//! TASK-114 red/green public semantic boundary for correlated and temporal joins.
//! These safety regressions do not certify the full TASK-114 constructive matrix.
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, ComparisonOperator, JoinKind, Predicate, ProtocolStatement,
};

fn query(sql: &str, dialect: &str) -> sql_semantic_protocol::QueryStatement {
    let parser = dialect_from_name(dialect).expect("supported dialect");
    let protocol = analyze_sql(sql, dialect, parser.as_ref()).expect("SQL must parse");
    protocol
        .statements()
        .iter()
        .find_map(|statement| match statement {
            ProtocolStatement::Query(query) => Some(query.clone()),
            _ => None,
        })
        .expect("query statement")
}

#[test]
fn asof_ordering_is_not_a_plain_equality_join_and_carries_specific_residual() {
    let sql = "SELECT l.id FROM l ASOF JOIN r MATCH_CONDITION(l.t >= r.t)";
    let result = query(sql, "snowflake");
    assert_eq!(result.joins().len(), 1);
    assert_eq!(result.joins()[0].kind(), JoinKind::Unknown);
    let Some(Predicate::Comparison(eligibility)) = result.joins()[0].condition() else {
        panic!("ASOF eligibility must be present without implying winner selection");
    };
    assert_eq!(eligibility.operator(), ComparisonOperator::Gte);
    assert!(format!("{eligibility:?}").contains("l"));
    assert!(format!("{eligibility:?}").contains("r"));
    assert!(result
        .diagnostics()
        .iter()
        .any(|d| d.code() == "unsupported_asof_nearest_prior"));
}

#[test]
fn cross_and_outer_apply_have_distinct_correlation_diagnostics() {
    for (operator, code) in [
        ("CROSS APPLY", "unsupported_cross_apply"),
        ("OUTER APPLY", "unsupported_outer_apply"),
    ] {
        let sql = format!("SELECT l.id FROM l {operator} (SELECT r.id FROM r WHERE r.t = l.t) q");
        let result = query(&sql, "mssql");
        assert_eq!(result.joins().len(), 1);
        assert_eq!(result.joins()[0].kind(), JoinKind::Unknown);
        assert!(
            result.diagnostics().iter().any(|d| d.code() == code),
            "{operator}"
        );
    }
}

#[test]
fn lateral_derived_table_retains_a_correlated_row_scope_residual() {
    let sql = "SELECT l.id FROM l CROSS JOIN LATERAL (SELECT r.id FROM r WHERE r.t = l.t) q";
    let result = query(sql, "generic");
    assert!(result
        .diagnostics()
        .iter()
        .any(|d| d.code() == "unsupported_lateral_row_scope"));
}
