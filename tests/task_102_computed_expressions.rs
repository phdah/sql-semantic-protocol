//! TASK-102: guard-refined CASE, COALESCE and NULLIF outcome domains, witnesses and oracles.
mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, to_json, BooleanTruthCase, BooleanWitness,
    BooleanWitnessDirection, ComposedSemantics, ConditionClause, ConfiguredSqlInput, Error,
    Expression, LiteralValue, ProtocolStatement, RelationCatalog, RelationSchema,
    ResidualConditionReason, SchemaColumn, SetMode, SqlInput, ValueDomain,
};
use sqlparser::dialect::{GenericDialect, PostgreSqlDialect};

fn analyzed(sql: &str) -> sql_semantic_protocol::Protocol {
    let dialect = GenericDialect {};
    analyze_sql(sql, "generic", &dialect)
        .unwrap_or_else(|error| panic!("{sql}: expected analysis, got {error}"))
}

/// Catalog-backed t(a INTEGER) projection domain, exercising the same
/// total-arithmetic refinement as production catalog inputs.
fn catalog_projected_domain(sql: &str) -> ValueDomain {
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline(sql);
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "projection",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("{sql}: expected composed output");
    };
    semantics.output().columns()[0].domain().clone()
}

fn first_output_domain(sql: &str) -> ValueDomain {
    let protocol = analyzed(sql);
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("{sql}: expected query statement");
    };
    query.output().columns()[0].domain().clone()
}

fn literal_key(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Null => "NULL".to_string(),
        LiteralValue::Number(number) => number.clone(),
        LiteralValue::Text(text) => format!("'{text}'"),
        other => panic!("unexpected literal in computed domain: {other:?}"),
    }
}

fn assert_value_set(sql: &str, expected: Vec<&str>) {
    let domain = first_output_domain(sql);
    assert_value_set_domain(sql, &domain, expected);
}

fn assert_value_set_domain(sql: &str, domain: &ValueDomain, mut expected: Vec<&str>) {
    let ValueDomain::Set(set) = domain else {
        panic!("{sql}: expected a value set, got {domain:?}");
    };
    assert_eq!(set.mode(), SetMode::Include, "{sql}");
    let mut actual = set
        .values()
        .iter()
        .map(|literal| literal_key(literal.value()))
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "{sql}");
}

fn assert_exactly_null_excluded(sql: &str) {
    let domain = first_output_domain(sql);
    let ValueDomain::Set(set) = &domain else {
        panic!("{sql}: expected a value set, got {domain:?}");
    };
    assert_eq!(set.mode(), SetMode::Exclude, "{sql}: {domain:?}");
    let excluded = set
        .values()
        .iter()
        .map(|literal| literal_key(literal.value()))
        .collect::<Vec<_>>();
    assert_eq!(excluded, vec!["NULL".to_string()], "{sql}");
}

type IntegerBound = Option<(String, bool)>;

fn range_bounds(domain: &ValueDomain) -> Vec<(IntegerBound, IntegerBound)> {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected ranges domain, got {domain:?}");
    };
    let bound = |bound: Option<&sql_semantic_protocol::Bound>| {
        bound.map(|bound| match bound.value().value() {
            LiteralValue::Number(value) => (value.clone(), bound.inclusive()),
            other => panic!("expected numeric bound, got {other:?}"),
        })
    };
    let mut bounds = ranges
        .ranges()
        .iter()
        .map(|range| (bound(range.lower()), bound(range.upper())))
        .collect::<Vec<_>>();
    bounds.sort();
    bounds
}

/// Catalog-backed t(a INTEGER, b BIGINT) witness for one WHERE predicate.
fn witness_for(predicate: &str) -> BooleanWitness {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "BIGINT", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let sql = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
    let dialect = PostgreSqlDialect {};
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "witness",
            &sql,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("{predicate}: resolved source required");
    };
    semantics.boolean_witnesses()[0].witness().clone()
}

fn assert_exact_both_directions(predicate: &str) {
    let proof = witness_for(predicate);
    assert!(
        matches!(
            proof.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "{predicate}: qualifying {:?}",
        proof.qualifying()
    );
    assert!(
        matches!(
            proof.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{predicate}: rejected {:?}",
        proof.rejected()
    );
}

fn assert_computed_expression_residual(sql: &str) {
    let protocol = analyzed(sql);
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("{sql}: expected query statement");
    };
    assert!(
        query
            .condition_exactness()
            .residual_conditions()
            .iter()
            .any(
                |residual| residual.reason() == ResidualConditionReason::ComputedExpression
                    && residual.clause() == ConditionClause::Where
            ),
        "{sql}: expected an explicit computed-expression residual"
    );
}

fn assert_residual(predicate: &str) {
    let proof = witness_for(predicate);
    assert!(
        matches!(proof.qualifying(), BooleanWitnessDirection::Residual { .. }),
        "{predicate}: expected residual qualifying, got {:?}",
        proof.qualifying()
    );
}

fn assert_impossible_true(predicate: &str) {
    let proof = witness_for(predicate);
    assert!(
        matches!(proof.qualifying(), BooleanWitnessDirection::Impossible),
        "{predicate}: expected impossible qualifying, got {:?}",
        proof.qualifying()
    );
    assert!(
        matches!(
            proof.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{predicate}: rejected {:?}",
        proof.rejected()
    );
}

#[test]
fn null_free_case_is_null_is_proven_impossible() {
    // The `a > 1` arm yields a non-NULL `a` and the ELSE arm is the literal 0,
    // so the CASE is never NULL and `IS NULL` can never be TRUE.
    assert_impossible_true("CASE WHEN a > 1 THEN a ELSE 0 END IS NULL");
}

#[test]
fn coalesce_with_null_free_fallback_proves_impossible_and_trivially_true() {
    // A non-NULL literal fallback can never be NULL: IS NULL can never be
    // TRUE, and IS NOT NULL holds on every row.
    assert_impossible_true("COALESCE(a, 0) IS NULL");
    let proof = witness_for("COALESCE(a, 0) IS NOT NULL");
    assert!(
        matches!(
            proof.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "COALESCE(a, 0) IS NOT NULL qualifying: {:?}",
        proof.qualifying()
    );
    assert!(
        matches!(proof.rejected(), BooleanWitnessDirection::Impossible),
        "COALESCE(a, 0) IS NOT NULL rejected: {:?}",
        proof.rejected()
    );
}

#[test]
fn column_coalesce_and_column_nullif_null_tests_stay_exact() {
    // COALESCE of two nullable columns is TRUE for IS NULL exactly when both
    // are NULL. NULLIF of two nullable columns is TRUE exactly when the left
    // is NULL or the two columns are equal; both remain exact.
    assert_exact_both_directions("COALESCE(a, b) IS NULL");
    assert_exact_both_directions("NULLIF(a, b) IS NULL");
}

#[test]
fn negative_literal_nullif_operand_keeps_exact_two_valued_regions() {
    // The witness layer handles signed literal operands, so a negative
    // literal boundary stays exact rather than degrading to residual.
    assert_exact_both_directions("NULLIF(a, -5) IS NULL");
}

#[test]
fn coalesce_unions_argument_domains_and_excludes_null_only_when_proven() {
    assert_value_set("SELECT COALESCE(NULL, 5) AS v FROM t", vec!["5"]);
    assert_value_set("SELECT COALESCE(NULL, NULL) AS v FROM t", vec!["NULL"]);
    assert_value_set(
        "SELECT COALESCE(a, 0) AS v FROM t WHERE a IN (1, 3)",
        vec!["1", "3"],
    );
    // An IS NULL filter proves the first argument is NULL, so the coalesce
    // result is exactly the fallback value.
    assert_value_set(
        "SELECT COALESCE(a, 0) AS v FROM t WHERE a IS NULL",
        vec!["0"],
    );
    assert_exactly_null_excluded("SELECT COALESCE(a, 0) AS v FROM t");
}

#[test]
fn coalesce_short_circuits_null_possibility_per_argument() {
    // The first argument can never be NULL on a surviving row, so the
    // coalesce result can never be NULL, but the value bound stays unbounded.
    let sql = "SELECT COALESCE(a, b) AS v FROM t WHERE a IN (1, 3)";
    assert_exactly_null_excluded(sql);
}

#[test]
fn nullif_domain_is_exact_for_finite_source_sets() {
    assert_value_set(
        "SELECT NULLIF(a, 5) AS v FROM t WHERE a IN (3, 5)",
        vec!["3", "NULL"],
    );
    assert_value_set(
        "SELECT NULLIF(a, 5) AS v FROM t WHERE a IN (1, 3)",
        vec!["1", "3"],
    );
    assert_value_set("SELECT NULLIF(a, 5) AS v FROM t WHERE a = 5", vec!["NULL"]);
    // b can also be 5's non-equal sibling, so an a equal to a shared value
    // can still pass through unchanged; only NULL is proven possible.
    assert_value_set(
        "SELECT NULLIF(a, b) AS v FROM t WHERE a IN (1, 3) AND b IN (3, 5)",
        vec!["1", "3", "NULL"],
    );
    assert_value_set(
        "SELECT NULLIF(a, NULL) AS v FROM t WHERE a IN (3, 5)",
        vec!["3", "5"],
    );
    assert_value_set(
        "SELECT NULLIF(a, 5) AS v FROM t WHERE a IS NULL",
        vec!["NULL"],
    );
}

#[test]
fn nullif_range_filters_keep_exact_bounded_domains() {
    let sql = "SELECT NULLIF(a, 5) AS v FROM t WHERE a BETWEEN 1 AND 3";
    let domain = first_output_domain(sql);
    assert_eq!(
        range_bounds(&domain),
        vec![(Some(("1".to_string(), true)), Some(("3".to_string(), true)))],
        "{sql}"
    );
}

#[test]
fn searched_case_guard_refines_column_arm_domains() {
    let sql = "SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS v FROM t";
    assert_eq!(
        range_bounds(&first_output_domain(sql)),
        vec![
            (Some(("0".to_string(), true)), Some(("0".to_string(), true))),
            (Some(("5".to_string(), true)), None),
        ],
        "{sql}"
    );

    let filtered = "SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS v FROM t WHERE a BETWEEN 0 AND 10";
    assert_eq!(
        range_bounds(&first_output_domain(filtered)),
        vec![
            (Some(("0".to_string(), true)), Some(("0".to_string(), true))),
            (
                Some(("5".to_string(), true)),
                Some(("10".to_string(), true))
            ),
        ],
        "{filtered}"
    );
}

#[test]
fn searched_case_without_else_adds_null_and_drops_guard_excluded_values() {
    let sql = "SELECT CASE WHEN a >= 5 THEN a END AS v FROM t WHERE a IN (3, 7)";
    assert_value_set(sql, vec!["7", "NULL"]);
}

#[test]
fn simple_case_guard_refines_column_arm_domains() {
    let sql = "SELECT CASE a WHEN 7 THEN a ELSE 0 END AS v FROM t WHERE a IN (3, 7)";
    assert_value_set(sql, vec!["0", "7"]);
}

#[test]
fn else_arm_never_claims_a_null_free_domain_from_a_three_valued_guard() {
    // The ELSE arm must keep admitting NULL rows (a > 50 is UNKNOWN for a
    // IS NULL) and must not fall back to an explicit residual.
    let sql = "SELECT CASE WHEN a > 50 THEN 1 ELSE a END AS v FROM t";
    assert!(
        matches!(first_output_domain(sql), ValueDomain::Unbounded),
        "{sql}: expected an unbounded else arm, got {:?}",
        first_output_domain(sql)
    );
}

#[test]
fn unreachable_case_branch_contributes_no_domain() {
    let sql = "SELECT CASE WHEN a = 7 THEN a WHEN a = 7 THEN 0 ELSE a END AS v
               FROM t WHERE a IN (3, 7)";
    assert_value_set(sql, vec!["3", "7"]);
}

#[test]
fn contradictory_case_guard_contributes_no_domain() {
    // a > 0 AND a IS NULL is contradictory once the WHERE filter proves a is
    // non-NULL, so the THEN arm is unreachable and only ELSE survives.
    let sql = "SELECT CASE WHEN a > 0 AND a IS NULL THEN b ELSE 7 END AS v
               FROM t WHERE a IN (3, 7)";
    assert_value_set(sql, vec!["7"]);
}

#[test]
fn computed_case_arms_evaluate_through_catalog_total_arithmetic() {
    // A CASE arm is evaluated only for source values selecting that arm.
    let sql = "SELECT CASE WHEN a > 1 THEN a + 10 ELSE 0 END AS v
               FROM t WHERE a IN (1, 3)";
    assert_value_set_domain(sql, &catalog_projected_domain(sql), vec!["0", "13"]);

    let implicit = "SELECT CASE WHEN a > 1 THEN a + 10 END AS v
                    FROM t WHERE a IN (1, 3)";
    assert_value_set_domain(
        implicit,
        &catalog_projected_domain(implicit),
        vec!["13", "NULL"],
    );

    let simple = "SELECT CASE a WHEN 1 THEN a + 10 ELSE 0 END AS v
                  FROM t WHERE a IN (1, 3)";
    assert_value_set_domain(simple, &catalog_projected_domain(simple), vec!["0", "11"]);
}

#[test]
fn underivable_case_guard_keeps_the_unfiltered_arm_domain() {
    let sql = "SELECT CASE WHEN ABS(a) > 10 THEN a ELSE b END AS v FROM t";
    assert!(
        matches!(first_output_domain(sql), ValueDomain::Unbounded),
        "{sql}: expected unbounded domain, got {:?}",
        first_output_domain(sql)
    );
}

#[test]
fn computed_alias_references_are_rename_invariant_and_canonical() {
    let direct = first_output_domain("SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS v FROM t");
    for alias in ["x", "y", "v"] {
        let sql = format!("SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS {alias} FROM t");
        assert_eq!(first_output_domain(&sql), direct, "{sql}");
    }

    // A projection over a derived computed column keeps canonical reference
    // identity: the output stays a plain column whose lineage reaches the
    // physical source, never a leaked parser AST node.
    let protocol =
        analyzed("SELECT v FROM (SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS v FROM t) AS sub");
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("expected query statement");
    };
    let column = &query.output().columns()[0];
    assert_eq!(column.name(), "v");
    // The derived projection inlines the canonical computed expression (a
    // CASE or column reference), never a leaked parser AST node.
    assert!(
        !matches!(
            column.expression(),
            Expression::Unknown(_) | Expression::Unsupported(_)
        ),
        "unexpected derived expression: {:?}",
        column.expression()
    );
    let lineage = column.lineage();
    assert_eq!(lineage.len(), 1, "canonical lineage: {lineage:?}");
    assert_eq!(lineage[0].relation(), "t");
    assert_eq!(lineage[0].column(), "a");

    let json = to_json(&analyzed(
        "SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS v FROM t",
    ));
    let text = serde_json::to_string(&serde_json::from_str::<Value>(&json).expect("valid json"))
        .expect("serializable");
    assert!(text.contains("COALESCE") || text.contains("CASE") || text.contains("case"));
    assert!(
        !text.contains("sqlparser"),
        "AST types must not leak: {text}"
    );
}

#[test]
fn output_alias_shadowing_a_source_column_keeps_direct_refinement() {
    let sql = "SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS a FROM t WHERE a IN (3, 7)";
    assert_value_set(sql, vec!["0", "7"]);
}

#[test]
fn alias_shifted_case_guards_fall_back_to_unbounded() {
    // The guard alternatives resolve to physical source references, while
    // the arm result resolves through the derived relation. Neither lookup
    // matches, so no refinement fires and the CASE keeps an explicit
    // unbounded domain instead of claiming a narrowed one.
    let sql = "SELECT CASE WHEN v >= 5 THEN v ELSE 0 END AS v
               FROM (SELECT a AS v FROM t WHERE a IN (3, 7)) AS sub";
    assert!(
        matches!(first_output_domain(sql), ValueDomain::Unbounded),
        "{sql}: expected an unbounded fallback, got {:?}",
        first_output_domain(sql)
    );
}

#[test]
fn numeric_boundaries_survive_computed_domains() {
    let boundary = "SELECT COALESCE(NULL, 2147483647) AS v FROM t";
    assert_value_set(boundary, vec!["2147483647"]);

    let nullif = "SELECT NULLIF(a, 2147483647) AS v FROM t WHERE a IN (3, 2147483647)";
    assert_value_set(nullif, vec!["3", "NULL"]);

    let wide = "SELECT COALESCE(a, 9223372036854775807) AS v FROM t WHERE a = 0";
    assert_value_set(wide, vec!["0", "9223372036854775807"]);
}

#[test]
fn opaque_scalar_functions_keep_an_explicit_unknown_domain() {
    let sql = "SELECT MY_UDF(a) AS v FROM t";
    let ValueDomain::Unknown(reason) = first_output_domain(sql) else {
        panic!(
            "{sql}: expected an explicit unknown domain, got {:?}",
            first_output_domain(sql)
        );
    };
    assert!(
        reason.reason().contains("scalar-function"),
        "{sql}: unexpected reason {:?}",
        reason.reason()
    );

    let nested = "SELECT COALESCE(a, MY_UDF(b)) AS v FROM t WHERE a IN (1, 3)";
    assert!(
        matches!(first_output_domain(nested), ValueDomain::Unknown(_)),
        "{nested}: an unprovable argument must stay explicit, got {:?}",
        first_output_domain(nested)
    );
}

#[test]
fn parser_capable_dialects_share_canonical_computed_domains() {
    for sql in [
        "SELECT CASE WHEN a >= 5 THEN a ELSE 0 END AS v FROM t",
        "SELECT CASE a WHEN 5 THEN a ELSE 0 END AS v FROM t",
        "SELECT COALESCE(a, 0) AS v FROM t",
        "SELECT NULLIF(a, 5) AS v FROM t",
    ] {
        let mut reference: Option<ValueDomain> = None;
        for name in common::DIALECTS {
            let dialect = dialect_from_name(name).expect("registered dialect");
            match analyze_sql(sql, name, dialect.as_ref()) {
                Ok(protocol) => {
                    let Some(ProtocolStatement::Query(query)) = protocol.statements().first()
                    else {
                        panic!("{name}: expected query");
                    };
                    let domain = query.output().columns()[0].domain().clone();
                    match &reference {
                        None => reference = Some(domain),
                        Some(reference) => assert_eq!(&domain, reference, "{name}: {sql}"),
                    }
                }
                Err(Error::Parse(_)) => {
                    // Parser rejection is a documented boundary, not a semantic proof.
                }
                Err(error) => panic!("{name}: parsed computed expression must analyze: {error}"),
            }
        }
        assert!(reference.is_some(), "{sql}: no dialect accepted the query");
    }
}

#[test]
fn null_branch_case_is_null_has_exact_two_valued_regions() {
    assert_exact_both_directions("CASE WHEN a > 0 THEN a ELSE NULL END IS NULL");
}

#[test]
fn null_branch_case_is_not_null_has_exact_two_valued_regions() {
    assert_exact_both_directions("CASE WHEN a > 0 THEN a ELSE NULL END IS NOT NULL");
}

#[test]
fn implicit_else_case_is_null_treats_fallthrough_as_null() {
    assert_exact_both_directions("CASE WHEN a > 0 THEN a END IS NULL");
}

#[test]
fn nullable_or_guard_case_falls_through_on_unknown() {
    assert_exact_both_directions("CASE WHEN a > 0 OR b IS NULL THEN a ELSE NULL END IS NULL");
}

#[test]
fn nested_not_and_guard_case_stays_exact() {
    assert_exact_both_directions(
        "CASE WHEN NOT (a > 0 AND b IS NULL) THEN a ELSE NULL END IS NULL",
    );
}

#[test]
fn nullif_literal_is_null_has_exact_two_valued_regions() {
    assert_exact_both_directions("NULLIF(a, 5) IS NULL");
}

#[test]
fn simple_case_null_tests_have_exact_two_valued_regions() {
    // Simple CASE uses three-valued equality: a NULL operand never matches,
    // so it falls through to ELSE exactly like an UNKNOWN searched guard.
    assert_exact_both_directions("CASE a WHEN 1 THEN b ELSE NULL END IS NULL");
    assert_exact_both_directions("CASE a WHEN 1 THEN NULL ELSE b END IS NOT NULL");
}

#[test]
fn non_literal_simple_case_operands_stay_residual() {
    // Operands and WHEN values outside column-plus-signed-literal cannot be
    // reduced to a source constraint and stay explicit residuals.
    assert_residual("CASE a + 1 WHEN 1 THEN b ELSE NULL END IS NULL");
    assert_residual("CASE a WHEN b THEN b ELSE NULL END IS NULL");
}

#[test]
fn opaque_guard_case_residualizes() {
    assert_residual("CASE WHEN a > MY_UDF(b) THEN a ELSE NULL END IS NULL");
}

#[test]
fn comparisons_over_computed_values_stay_explicitly_residual() {
    // CASE, COALESCE and NULLIF comparisons with certified integer
    // operands are reversible without reconstructing SQL in consumers.
    for predicate in [
        "CASE WHEN a > 1 THEN a ELSE 0 END = 5",
        "NOT (CASE WHEN a > 1 THEN a ELSE 0 END = 5)",
        "COALESCE(a, 0) > 5",
        "COALESCE(a, 0) = 0",
        "NULLIF(a, 5) = 3",
        "NOT (NULLIF(a, 5) = 3)",
        "NULLIF(a, 5) = 3 OR b = 7",
    ] {
        assert_exact_both_directions(predicate);
    }
}

#[test]
fn computed_alias_predicates_through_derived_tables_stay_explicitly_residual() {
    // Computed subquery output domains do not propagate outward, so a NULL
    // test over an aliased computed column cannot anchor source rows yet;
    // the filter stays an explicit computed-expression residual.
    assert_computed_expression_residual(
        "SELECT v FROM (SELECT CASE WHEN a > 1 THEN b ELSE NULL END AS v FROM t) AS sub WHERE v IS NULL",
    );
}

#[test]
fn duplicated_output_names_resolve_positionally() {
    let sql = "SELECT a AS v, b AS v FROM t WHERE a IN (1, 3)";
    let protocol = analyzed(sql);
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("{sql}: expected query statement");
    };
    let domains = query
        .output()
        .columns()
        .iter()
        .map(|column| {
            assert_eq!(column.name(), "v", "{sql}: duplicated alias");
            column.domain().clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(domains.len(), 2, "{sql}: both projections survive");
    assert_value_set_domain(sql, &domains[0], vec!["1", "3"]);
    assert!(
        matches!(domains[1], ValueDomain::Unbounded),
        "{sql}: unfiltered duplicate stays unbounded, got {:?}",
        domains[1]
    );
}

/// Distinct non-NULL values plus NULL presence from a DuckDB projection.
fn duckdb_distinct(db: &Connection, sql: &str) -> (Vec<String>, bool) {
    let mut statement = db.prepare(sql).unwrap();
    let rows = statement
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap();
    let mut values = std::collections::BTreeSet::new();
    let mut has_null = false;
    for row in rows {
        match row.unwrap() {
            None => has_null = true,
            Some(value) => {
                values.insert(value.to_string());
            }
        }
    }
    (values.into_iter().collect(), has_null)
}

fn assert_domain_sound(sql: &str, domain: &ValueDomain, values: &[String], has_null: bool) {
    // Every oracle fixture is designed for an exact set domain; anything else
    // means the fixture no longer proves what it claims.
    let ValueDomain::Set(set) = domain else {
        panic!("{sql}: oracle fixture must yield an exact set domain, got {domain:?}");
    };
    match set.mode() {
        SetMode::Include => {
            let mut expected = set
                .values()
                .iter()
                .filter_map(|literal| match literal.value() {
                    LiteralValue::Number(number) => Some(number.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            expected.sort();
            let expected_null = set
                .values()
                .iter()
                .any(|literal| matches!(literal.value(), LiteralValue::Null));
            assert_eq!(values, expected, "{sql}: non-NULL domain must match");
            assert_eq!(has_null, expected_null, "{sql}: NULL domain must match");
        }
        SetMode::Exclude => {
            if set
                .values()
                .iter()
                .any(|literal| matches!(literal.value(), LiteralValue::Null))
            {
                assert!(!has_null, "{sql}: NULL excluded but DuckDB produced NULL");
            }
        }
    }
}

#[test]
fn duckdb_computed_value_oracles_match_claimed_domains() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (3, NULL), (5, 5), (7, 7), (NULL, NULL);",
    )
    .unwrap();
    for (sql, oracle) in [
        (
            "SELECT COALESCE(NULL, 5) AS v FROM t",
            "SELECT COALESCE(NULL, 5) FROM t",
        ),
        (
            "SELECT COALESCE(a, 0) AS v FROM t WHERE a IS NULL",
            "SELECT COALESCE(a, 0) FROM t WHERE a IS NULL",
        ),
        (
            "SELECT NULLIF(a, 5) AS v FROM t WHERE a IN (3, 5)",
            "SELECT NULLIF(a, 5) FROM t WHERE a IN (3, 5)",
        ),
        (
            "SELECT CASE WHEN a > 5 THEN a ELSE 0 END AS v FROM t WHERE a IN (3, 7)",
            "SELECT CASE WHEN a > 5 THEN a ELSE 0 END FROM t WHERE a IN (3, 7)",
        ),
    ] {
        let domain = first_output_domain(sql);
        let (values, has_null) = duckdb_distinct(&db, oracle);
        assert_domain_sound(sql, &domain, &values, has_null);
    }
}

#[test]
fn direct_sql_and_dbt_catalog_share_computed_output_domains() {
    let dialect = PostgreSqlDialect {};
    let dbt_catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();

    let computed_domain = |bundle: &sql_semantic_protocol::AnalysisBundle| -> ValueDomain {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| layer.consumes().iter().any(|s| s == "warehouse.raw.orders"))
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
            panic!("resolved layer expected");
        };
        semantics
            .output()
            .columns()
            .iter()
            .find(|column| column.name() == "v")
            .expect("computed column v")
            .domain()
            .clone()
    };

    // (compiled SQL, expected exact value set): a guard-refined CASE and a
    // NULL-free COALESCE fallback.
    for (sql, expected) in [
        (
            "SELECT id, CASE WHEN amount > 5 THEN amount ELSE 0 END AS v
             FROM warehouse.raw.orders WHERE amount IN (3, 7)",
            vec!["0", "7"],
        ),
        (
            "SELECT id, COALESCE(amount, 0) AS v
             FROM warehouse.raw.orders WHERE amount IN (3, 7)",
            vec!["0", "3", "7"],
        ),
    ] {
        let mut manifest: Value =
            serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
        manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = sql.into();
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let from_dbt =
            analyze_dbt_artifacts(&manifest, &dbt_catalog, "postgresql", &dialect).unwrap();

        let physical = RelationSchema::new(
            "warehouse.raw.orders",
            vec![
                SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
            ],
        )
        .unwrap();
        let direct_catalog = RelationCatalog::from_schemas(&[physical]).unwrap();
        let source = SqlInput::inline(sql);
        let from_sql = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "direct",
                &source,
                "postgresql",
                &dialect,
            )],
            &direct_catalog,
        )
        .unwrap();

        let dbt_domain = computed_domain(&from_dbt);
        let direct_domain = computed_domain(&from_sql);
        assert_eq!(
            dbt_domain, direct_domain,
            "dbt and direct domains must agree: {sql}"
        );
        assert_value_set_domain(sql, &dbt_domain, expected);
    }
}

#[test]
fn case_vacuous_then_arm_keeps_true_rows_in_witness() {
    assert_impossible_true("CASE WHEN a IS NULL THEN 0 ELSE a END IS NULL");
    let witness = witness_for("CASE WHEN a IS NULL THEN 0 ELSE a END IS NOT NULL");
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Impossible
    ));
}

#[test]
fn coalesce_stops_after_a_guaranteed_non_null_argument() {
    assert_value_set("SELECT COALESCE(1, 2) AS v FROM t", vec!["1"]);
    assert_value_set(
        "SELECT COALESCE(a, 0) AS v FROM t WHERE a IN (1, 3)",
        vec!["1", "3"],
    );
}

#[test]
fn nullif_numeric_literals_compare_by_sql_value() {
    assert_value_set("SELECT NULLIF(1, 1.0) AS v FROM t", vec!["NULL"]);
    assert_value_set("SELECT NULLIF(1, 2.0) AS v FROM t", vec!["1"]);
}

#[test]
fn implicit_else_is_excluded_when_where_proves_every_guard_true() {
    assert_value_set(
        "SELECT CASE WHEN a > 0 THEN a END AS v FROM t WHERE a IN (1, 3)",
        vec!["1", "3"],
    );
}

#[test]
fn computed_integer_comparisons_preserve_true_and_unknown_regions() {
    for predicate in [
        "COALESCE(a, 0) > 5",
        "COALESCE(a, 0) = 0",
        "NOT (COALESCE(a, 0) > 5)",
        "NULLIF(a, 5) = 3",
        "NOT (NULLIF(a, 5) = 3)",
        "NULLIF(a, 5) = 3 AND b > 0",
        "NULLIF(a, 5) = 3 OR b IS NULL",
    ] {
        assert_exact_both_directions(predicate);
    }
}

#[test]
fn computed_case_comparisons_preserve_selection_and_unknown() {
    for predicate in [
        "CASE WHEN a > 1 THEN a ELSE 0 END = 5",
        "NOT (CASE WHEN a > 1 THEN a ELSE 0 END = 5)",
        "CASE WHEN a > 1 THEN NULL ELSE a END = 5",
        "NOT (CASE WHEN a > 1 THEN NULL ELSE a END = 5)",
        "CASE a WHEN 1 THEN a ELSE 0 END = 1",
        "CASE WHEN a IS NULL THEN 0 ELSE a END = 0",
    ] {
        assert_exact_both_directions(predicate);
    }
}
