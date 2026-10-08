mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, ComposedSemantics, ConditionClause,
    ConditionExactnessStatus, ConfiguredSqlInput, LiteralValue, ProtocolStatement, RelationCatalog,
    RelationSchema, ResidualConditionReason, SchemaColumn, SetMode, SqlInput, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect, PostgreSqlDialect};

fn generic(sql: &str) -> sql_semantic_protocol::Protocol {
    analyze_sql(sql, "generic", &GenericDialect {}).expect("SQL should analyze")
}

fn typed(sql: &str, schemas: &[(&str, &[(&str, &str)])]) -> sql_semantic_protocol::ResolvedComposedSemantics {
    let schemas = schemas
        .iter()
        .map(|(relation, columns)| {
            RelationSchema::new(
                *relation,
                columns.iter().map(|(column, data_type)| {
                    SchemaColumn::from_sql_type(*column, data_type, "postgresql")
                        .expect("typed column")
                }).collect(),
            ).expect("typed relation")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("typed catalog");
    let input = SqlInput::inline(sql);
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new("test", &input, "postgresql", &dialect)];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analyze");
    match bundle.layers()[0].composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics.clone(),
        other => panic!("expected resolved semantics, got {other:?}"),
    }
}

#[test]
fn boolean_truth_forms_preserve_three_valued_sql_semantics() {
    let cases = [
        ("flag", true, false, true),
        ("NOT flag", false, false, true),
        ("flag IS TRUE", true, false, true),
        ("flag IS FALSE", false, false, true),
        ("flag IS NOT TRUE", true, true, true),
        ("flag IS NOT FALSE", false, true, true),
    ];
    for (condition, expected, excluded, exact) in cases {
        let sql = format!("SELECT flag FROM t WHERE {condition}");
        let semantics = typed(&sql, &[("t", &[("flag", "BOOLEAN")])]);
        assert_eq!(semantics.condition_exactness().is_exact(), exact, "{condition}");
        assert!(semantics.condition_exactness().residual_conditions().is_empty());
        let domain = semantics.column_domains()
            .iter().find(|value| value.column().name() == "flag").expect("flag domain");
        let ValueDomain::Set(set) = domain.domain() else {
            panic!("{condition}: expected finite domain, got {:?}", domain.domain());
        };
        assert_eq!(
            set.mode(),
            if excluded { SetMode::Exclude } else { SetMode::Include },
            "{condition}"
        );
        assert_eq!(set.values().len(), 1, "{condition}");
        assert_eq!(set.values()[0].value(), &LiteralValue::Boolean(expected));
        assert_eq!(domain.domain().admits_null(), Some(excluded), "{condition}");
    }
}

#[test]
fn shared_boolean_predicates_are_exact_in_every_supported_dialect() {
    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name).expect("supported dialect");
        for condition in ["flag", "NOT flag"] {
            let sql = format!("SELECT flag FROM t WHERE {condition}");
            let bundle = analyze_sql(&sql, dialect_name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{dialect_name}: {condition}: {error}"));
            let Some(ProtocolStatement::Query(query)) = bundle.statements().first() else {
                panic!("expected query");
            };
            assert!(query.condition_exactness().is_exact(), "{dialect_name}: {condition}");
            assert_eq!(query.column_domains()[0].domain().admits_null(), Some(false));
        }
    }
}

#[test]
fn distinct_unsupported_predicates_have_distinct_stable_identities() {
    let bundle = generic("SELECT a FROM t WHERE a + 1 > 3 AND a + 2 > 5");
    let Some(ProtocolStatement::Query(query)) = bundle.statements().first() else {
        panic!("expected query");
    };
    let residuals = query.condition_exactness().residual_conditions();
    assert_eq!(residuals.len(), 2, "{residuals:?}");
    assert!(residuals.iter().all(|r| r.reason() == ResidualConditionReason::ComputedExpression));
    assert!(residuals.iter().all(|r| r.clause() == ConditionClause::Where));
    assert_ne!(residuals[0].identity(), residuals[1].identity());
    assert!(residuals.iter().all(|r| r.identity().starts_with("where:and:")));
}

#[test]
fn typed_failures_have_specific_reasons_in_the_correct_clause() {
    let cases = [
        ("SELECT a FROM t WHERE a = '2024-01-01'", "DATE", ResidualConditionReason::LiteralTypeMismatch),
        ("SELECT a FROM t WHERE a = 1.5", "INTEGER", ResidualConditionReason::LossyCoercion),
        ("SELECT a FROM t WHERE a = 999999", "SMALLINT", ResidualConditionReason::OutOfRangeLiteral),
        ("SELECT a FROM t WHERE a = 'x'", "JSON", ResidualConditionReason::ComparisonSemantics),
    ];
    for (sql, data_type, reason) in cases {
        let semantics = typed(sql, &[("t", &[("a", data_type)])]);
        assert_eq!(semantics.condition_exactness().status(), ConditionExactnessStatus::Residual, "{sql}");
        let residuals = semantics.condition_exactness().residual_conditions();
        assert!(residuals.iter().any(|r| r.reason() == reason && r.clause() == ConditionClause::Where), "{sql}: {residuals:?}");
        assert!(!residuals.iter().any(|r| r.reason() == ResidualConditionReason::ComputedExpression), "{sql}: {residuals:?}");
        assert!(!residuals.iter().any(|r| r.clause() == ConditionClause::RowSetOperator), "{sql}");
    }
}

#[test]
fn unknown_schema_columns_use_their_actual_clause_not_row_set_operator() {
    let where_semantics = typed(
        "SELECT a FROM t WHERE ghost = 1",
        &[("t", &[("a", "INTEGER")])],
    );
    let residuals = where_semantics.condition_exactness().residual_conditions();
    assert_eq!(residuals.len(), 1, "{residuals:?}");
    assert_eq!(residuals[0].reason(), ResidualConditionReason::UnknownSchemaColumn);
    assert_eq!(residuals[0].clause(), ConditionClause::Where);
    assert!(residuals[0].identity().contains("ghost"));

    let join_semantics = typed(
        "SELECT t.a FROM t JOIN u ON t.ghost = u.a",
        &[("t", &[("a", "INTEGER")]), ("u", &[("a", "INTEGER")])],
    );
    let residuals = join_semantics.condition_exactness().residual_conditions();
    assert!(residuals.iter().any(|r| {
        r.reason() == ResidualConditionReason::UnknownSchemaColumn
            && r.clause() == ConditionClause::JoinOn
    }), "{residuals:?}");
    assert!(!residuals.iter().any(|r| r.clause() == ConditionClause::RowSetOperator));

    // An invalid output reference is a schema/lineage diagnostic, not a WHERE condition.
    let projected = typed("SELECT ghost FROM t", &[("t", &[("a", "INTEGER")])]);
    assert!(projected.condition_exactness().residual_conditions().is_empty());
}

#[test]
fn boolean_predicates_compose_through_plain_copy_ctes() {
    let composed = typed(
        "WITH filtered AS (SELECT flag FROM t WHERE flag IS NOT TRUE) SELECT flag FROM filtered",
        &[("t", &[("flag", "BOOLEAN")])],
    );
    assert!(composed.condition_exactness().is_exact(), "{:?}", composed.condition_exactness());
    let domain = composed.column_domains().iter().find(|d| d.column().name() == "flag").expect("flag");
    assert_eq!(domain.domain().admits_null(), Some(true));
}
