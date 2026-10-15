//! TASK-104: pattern, collation and null-safe expression laws.
mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name, BooleanRowConstraint,
    BooleanTruthCase, BooleanWitnessDirection, ComparisonAssumption, ComposedSemantics,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

fn text_bundle(predicate: &str, attested: bool) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "VARCHAR(8)", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "VARCHAR(8)", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "task104",
        &input,
        "postgresql",
        &dialect,
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    if attested {
        bundle.declare_comparison_assumptions(&[
            ComparisonAssumption::BinaryCollation,
            ComparisonAssumption::NoCharPadding,
        ]);
    }
    bundle
}

fn single_witness(
    bundle: &sql_semantic_protocol::AnalysisBundle,
) -> Option<sql_semantic_protocol::BooleanWitness> {
    let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
        panic!("typed query must resolve");
    };
    assert!(
        composed.boolean_witnesses().len() <= 1,
        "single-filter fixture must emit at most one witness"
    );
    composed
        .boolean_witnesses()
        .first()
        .map(|item| item.witness().clone())
}

fn text_witness(predicate: &str) -> sql_semantic_protocol::BooleanWitness {
    single_witness(&text_bundle(predicate, false)).expect("text predicate must emit a witness")
}

fn attested_text_witness(predicate: &str) -> sql_semantic_protocol::BooleanWitness {
    single_witness(&text_bundle(predicate, true)).expect("text predicate must emit a witness")
}

fn assert_residual(witness: Option<&sql_semantic_protocol::BooleanWitness>, predicate: &str) {
    let Some(witness) = witness else {
        // Predicates without a source-row model (regex, COLLATE, LIKE ANY,
        // PG operator spellings) emit no witness at all: fail-closed.
        return;
    };
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Residual { .. }
        ),
        "{predicate}: must stay residual even when attested, got {:?}",
        witness.qualifying()
    );
    if let BooleanWitnessDirection::Residual { reason } = witness.qualifying() {
        assert!(!reason.is_empty(), "{predicate}: residual needs a reason");
    }
}

// Slice A: wildcard-free LIKE normalizes to attested string equality (see test below).

// Slice C: attested cross-text-column equality is exact in both directions.
#[test]
fn cross_text_column_equality_is_exact_only_when_attested() {
    use sql_semantic_protocol::ComparisonOperator;
    for (predicate, expected) in [
        ("a = b", ComparisonOperator::Eq),
        ("a <> b", ComparisonOperator::Neq),
        ("a IS DISTINCT FROM b", ComparisonOperator::IsDistinctFrom),
        (
            "a IS NOT DISTINCT FROM b",
            ComparisonOperator::IsNotDistinctFrom,
        ),
        ("a <=> b", ComparisonOperator::IsNotDistinctFrom),
    ] {
        let unattested = text_witness(predicate);
        assert!(
            matches!(
                unattested.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: unproven collation must not be exact"
        );
        let witness = attested_text_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            ),
            "{predicate}: certified TRUE, got {:?}",
            witness.qualifying()
        );
        assert!(
            matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}: certified NOT TRUE"
        );
        let BooleanRowConstraint::StringColumnComparison {
            left,
            operator,
            right,
        } = witness.condition()
        else {
            panic!(
                "{predicate}: expected cross-text-column equality, got {:?}",
                witness.condition()
            );
        };
        assert_eq!(left.name(), "a", "{predicate}");
        assert_eq!(right.name(), "b", "{predicate}");
        assert_eq!(*operator, expected, "{predicate}");
    }
    for predicate in ["a < b", "a <= b", "a > b", "a >= b"] {
        let witness = attested_text_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: text ordering has no proven collation order and must stay residual"
        );
    }
}

// Slice D: inventoried variants are exact under certified conditions or
// fail-closed residual. A successful parse never implies engine or
// collation parity.
#[test]
fn inventoried_variants_have_explicit_exact_or_residual_classification() {
    // (predicate, exact when attested with binary collation + no padding)
    const MATRIX: &[(&str, bool)] = &[
        ("a LIKE 'ab%'", true),
        ("a NOT LIKE 'ab%'", true),
        ("a LIKE 'ab'", true),
        ("a NOT LIKE 'ab'", true),
        ("a LIKE 'AB%'", true),
        ("a LIKE 'ab%' ESCAPE '!'", true),
        ("a LIKE 'ab%' ESCAPE 'a'", false),
        ("a LIKE 'ab%' ESCAPE '%'", false),
        ("a LIKE 'ab!%' ESCAPE '!'", false),
        ("a LIKE 'a_%'", false),
        ("a LIKE 'a%b%'", false),
        ("a LIKE '%ab'", false),
        ("a LIKE '_ab'", false),
        ("a LIKE 'a[b]%'", false),
        ("a LIKE 'a\\b%'", false),
        ("a LIKE 'h\u{e9}llo%'", false),
        // Conservative boundary: space and punctuation are not wildcards
        // under binary laws, but the portable law stays ASCII-alphanumeric
        // (mirroring safe prefixes) rather than proving per-dialect
        // tokenization for every punctuation mark.
        ("a LIKE 'a b'", false),
        ("a LIKE 'a-b'", false),
        ("a LIKE 'ab' ESCAPE '%'", true),
        ("a LIKE 'ab' ESCAPE '!'", true),
        ("a ILIKE 'ab%'", false),
        ("a NOT ILIKE 'ab%'", false),
        ("a SIMILAR TO 'ab%'", false),
        ("a NOT SIMILAR TO 'ab%'", false),
        ("a RLIKE 'ab'", false),
        ("a NOT RLIKE 'ab'", false),
        ("a REGEXP 'ab'", false),
        ("a NOT REGEXP 'ab'", false),
        ("a ~ 'ab'", false),
        ("a ~* 'ab'", false),
        ("a !~ 'ab'", false),
        ("a !~* 'ab'", false),
        ("a ~~ 'ab%'", false),
        ("a ~~* 'ab%'", false),
        ("a !~~ 'ab%'", false),
        ("a !~~* 'ab%'", false),
        ("a LIKE ANY ('ab%')", false),
        ("a COLLATE \"C\" = 'ab'", false),
        ("a = b", true),
        ("a <> b", true),
        ("a IS DISTINCT FROM b", true),
        ("a IS NOT DISTINCT FROM b", true),
        ("a <=> b", true),
        ("a <=> 'ab'", true),
        ("a < b", false),
        ("a <= b", false),
        ("a > b", false),
        ("a >= b", false),
        ("a > 'm'", false),
        ("a LIKE 'ab%' AND b LIKE 'ab%'", true),
        ("a = 'ab' OR b = 'ab'", true),
        ("NOT (a LIKE 'ab%')", true),
        ("a LIKE 'ab%' AND a IS NOT NULL", true),
    ];
    for (predicate, exact_when_attested) in MATRIX {
        for name in DIALECTS {
            let dialect = dialect_from_name(name).expect("registered dialect");
            let schema = RelationSchema::new(
                "t",
                vec![
                    SchemaColumn::from_sql_type("a", "VARCHAR(8)", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("b", "VARCHAR(8)", "postgresql").unwrap(),
                ],
            )
            .unwrap();
            let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
            let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
            let configured = [ConfiguredSqlInput::new(
                "task104",
                &input,
                name,
                dialect.as_ref(),
            )];
            let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{name}: {predicate} must parse: {error}"));
            let unattested = single_witness(&bundle);
            assert!(
                unattested.as_ref().is_none_or(|witness| matches!(
                    witness.qualifying(),
                    BooleanWitnessDirection::Residual { .. }
                )),
                "{name}: {predicate}: unproven collation must stay residual, got {:?}",
                unattested.map(|witness| witness.qualifying().clone())
            );
            bundle.declare_comparison_assumptions(&[
                ComparisonAssumption::BinaryCollation,
                ComparisonAssumption::NoCharPadding,
            ]);
            let attested = single_witness(&bundle);
            if *exact_when_attested {
                let witness = attested.expect("{predicate}: certified shape must emit a witness");
                assert!(
                    matches!(
                        witness.qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ),
                    "{name}: {predicate}: certified TRUE expected, got {:?}",
                    witness.qualifying()
                );
                assert!(
                    matches!(
                        witness.rejected(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                    ),
                    "{name}: {predicate}: certified NOT TRUE expected, got {:?}",
                    witness.rejected()
                );
            } else {
                assert_residual(attested.as_ref(), &format!("{name}: {predicate}"));
            }
        }
    }
}

#[test]
fn unparenthesized_null_safe_text_conjunction_is_a_parser_boundary_residual() {
    // sqlparser binds `IS [NOT] DISTINCT FROM b AND ...` as
    // `a IS [NOT] DISTINCT FROM (b AND ...)`, so the unparenthesized form
    // cannot reach the coupled text witness. It must fail closed, exactly
    // like the integer boundary proven in TASK-103.
    for predicate in [
        "a IS DISTINCT FROM b AND a IS NOT NULL",
        "a IS NOT DISTINCT FROM b AND a IS DISTINCT FROM b",
    ] {
        let witness = attested_text_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: parser-boundary form must stay residual"
        );
    }
}

#[test]
fn shared_syntax_has_identical_canonical_shape_across_parser_capable_dialects() {
    const SHARED: &[&str] = &[
        "SELECT a FROM t WHERE a LIKE 'ab%'",
        "SELECT a FROM t WHERE a NOT LIKE 'ab%'",
        "SELECT a FROM t WHERE a LIKE 'ab'",
        "SELECT a FROM t WHERE a ILIKE 'ab%'",
        "SELECT a FROM t WHERE a <=> b",
        "SELECT a FROM t WHERE a = b",
    ];
    for sql in SHARED {
        let mut canonical: Option<String> = None;
        for name in DIALECTS {
            let dialect = dialect_from_name(name).expect("registered dialect");
            let protocol = analyze_sql(sql, name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{name}: shared syntax must parse: {error}"));
            let document: serde_json::Value =
                serde_json::from_str(&sql_semantic_protocol::to_json(&protocol)).unwrap();
            // The injected dialect name is recorded per input; the canonical
            // statement shape (predicates, witnesses, domains) must be
            // identical across parser-capable dialects.
            let statement = document["inputs"][0]["statements"][0].to_string();
            match &canonical {
                None => canonical = Some(statement),
                Some(first) => assert_eq!(
                    &statement, first,
                    "{name}: {sql} must share the canonical protocol shape"
                ),
            }
        }
    }
}

#[test]
fn fixed_width_char_columns_never_claim_binary_laws() {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "CHAR(8)", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "CHAR(8)", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = PostgreSqlDialect {};
    for predicate in ["a = 'ab'", "a LIKE 'ab%'", "a = b", "a LIKE 'ab'"] {
        let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
        let mut bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task104",
                &input,
                "postgresql",
                &dialect,
            )],
            &catalog,
        )
        .unwrap();
        bundle.declare_comparison_assumptions(&[
            ComparisonAssumption::BinaryCollation,
            ComparisonAssumption::NoCharPadding,
        ]);
        let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
            panic!("{predicate}: typed query must resolve");
        };
        let witness = composed.boolean_witnesses()[0].witness().clone();
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: CHAR padding law is unproven and must stay residual"
        );
    }
}

// Slice B: MySQL null-safe spaceship maps to IS NOT DISTINCT FROM.
#[test]
fn mysql_spaceship_is_null_safe_equality_under_proven_laws() {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = PostgreSqlDialect {};
    for predicate in ["a <=> 2", "a <=> b"] {
        let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
        let bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task104",
                &input,
                "postgresql",
                &dialect,
            )],
            &catalog,
        )
        .unwrap();
        let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
            panic!("{predicate}: typed query must resolve");
        };
        let witness = composed.boolean_witnesses()[0].witness().clone();
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            ),
            "{predicate}: null-safe equality must be exact, got {:?}",
            witness.qualifying()
        );
        assert!(
            matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}"
        );
    }
    let witness = attested_text_witness("a <=> 'ab'");
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "attested text spaceship must be exact, got {:?}",
        witness.qualifying()
    );
    let BooleanRowConstraint::StringEquality { operator, .. } = witness.condition() else {
        panic!("expected string equality, got {:?}", witness.condition());
    };
    assert_eq!(
        *operator,
        sql_semantic_protocol::ComparisonOperator::IsNotDistinctFrom
    );
}

#[test]
fn exact_like_without_wildcard_is_certified_string_equality() {
    for (predicate, negated) in [("a LIKE 'ab'", false), ("a NOT LIKE 'ab'", true)] {
        let unattested = text_witness(predicate);
        assert!(
            matches!(
                unattested.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: unproven collation must not be exact"
        );
        let witness = attested_text_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            ),
            "{predicate}: certified positive witness, got {:?}",
            witness.qualifying()
        );
        assert!(
            matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}: certified negative witness"
        );
        let BooleanRowConstraint::StringEquality {
            column,
            operator,
            literal,
        } = witness.condition()
        else {
            panic!(
                "{predicate}: expected string equality, got {:?}",
                witness.condition()
            );
        };
        assert_eq!(column.name(), "a");
        assert_eq!(literal, "ab");
        assert_eq!(
            *operator == sql_semantic_protocol::ComparisonOperator::Neq,
            negated,
            "{predicate}"
        );
    }
}

// Slice E: DuckDB oracle, dbt parity, output domains, wire shape.
#[test]
fn duckdb_oracle_matches_certified_pattern_and_text_equality() {
    use duckdb::Connection;
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a VARCHAR, b VARCHAR);
         INSERT INTO t VALUES (NULL, NULL), (NULL, 'ab'), ('ab', NULL),
         ('ab', 'ab'), ('ab', 'ac'), ('ac', 'ab'), ('AB', 'ab'), ('', ''), ('abc', 'abc');",
    )
    .unwrap();
    for predicate in [
        "a LIKE 'ab%'",
        "a NOT LIKE 'ab%'",
        "a LIKE 'ab'",
        "a NOT LIKE 'ab'",
        "a LIKE 'ab%' ESCAPE '!'",
        "a = b",
        "a <> b",
        "a IS DISTINCT FROM b",
        "a IS NOT DISTINCT FROM b",
        "a LIKE 'ab%' AND a = b",
        "a = b OR a LIKE 'ac%'",
        "NOT (a LIKE 'ab%')",
    ] {
        let witness = attested_text_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            ),
            "{predicate}: certified TRUE"
        );
        assert!(
            matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}: certified NOT TRUE"
        );
        let mut statement = db
            .prepare(&format!("SELECT a, b, ({predicate}) FROM t"))
            .unwrap();
        let actual = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<bool>>(2)?,
                ))
            })
            .unwrap();
        for row in actual {
            let (a, b, sql_result) = row.unwrap();
            let computed = text_witness_truth(witness.condition(), a.as_deref(), b.as_deref());
            assert_eq!(
                computed, sql_result,
                "witness differs from DuckDB for {predicate} at ({a:?}, {b:?})"
            );
        }
    }
}

fn text_witness_truth(
    condition: &BooleanRowConstraint,
    a: Option<&str>,
    b: Option<&str>,
) -> Option<bool> {
    fn sql_and(left: Option<bool>, right: Option<bool>) -> Option<bool> {
        match (left, right) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        }
    }
    fn sql_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
        match (left, right) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        }
    }
    match condition {
        BooleanRowConstraint::All(operands) => {
            let mut result = Some(true);
            for operand in operands.iter() {
                result = sql_and(result, text_witness_truth(operand, a, b));
            }
            result
        }
        BooleanRowConstraint::Any(operands) => {
            let mut result = Some(false);
            for operand in operands.iter() {
                result = sql_or(result, text_witness_truth(operand, a, b));
            }
            result
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected column {other}"),
            };
            Some(value.is_none() != *negated)
        }
        BooleanRowConstraint::StringEquality {
            column,
            operator,
            literal,
        } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected column {other}"),
            };
            match operator {
                sql_semantic_protocol::ComparisonOperator::IsDistinctFrom => {
                    Some(value != Some(literal.as_str()))
                }
                sql_semantic_protocol::ComparisonOperator::IsNotDistinctFrom => {
                    Some(value == Some(literal.as_str()))
                }
                sql_semantic_protocol::ComparisonOperator::Eq => value.map(|text| text == literal),
                sql_semantic_protocol::ComparisonOperator::Neq => value.map(|text| text != literal),
                _ => panic!("no text ordering law: {operator:?}"),
            }
        }
        BooleanRowConstraint::StringColumnComparison {
            left,
            operator,
            right,
        } => {
            let get = |column: &sql_semantic_protocol::ColumnRef| match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected column {other}"),
            };
            let (lhs, rhs) = (get(left), get(right));
            match operator {
                sql_semantic_protocol::ComparisonOperator::IsDistinctFrom => Some(lhs != rhs),
                sql_semantic_protocol::ComparisonOperator::IsNotDistinctFrom => Some(lhs == rhs),
                sql_semantic_protocol::ComparisonOperator::Eq => {
                    lhs.zip(rhs).map(|(lhs, rhs)| lhs == rhs)
                }
                sql_semantic_protocol::ComparisonOperator::Neq => {
                    lhs.zip(rhs).map(|(lhs, rhs)| lhs != rhs)
                }
                _ => panic!("no text ordering law: {operator:?}"),
            }
        }
        BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated,
        } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected column {other}"),
            };
            value.map(|text| text.starts_with(prefix) != *negated)
        }
        other => panic!("unsupported differential node: {other:?}"),
    }
}

#[test]
fn impossible_text_conjunctions_yield_empty_output_domains() {
    use sql_semantic_protocol::ValueDomain;
    for predicate in [
        "a = b AND a IS NULL",
        "a <> a",
        "a IS DISTINCT FROM a",
        "a <> a OR b <> b",
        "(a IS DISTINCT FROM a) OR (b IS DISTINCT FROM b)",
        "a = b AND a <> b",
        "(a IS NOT DISTINCT FROM b) AND (a IS DISTINCT FROM b)",
    ] {
        let bundle = text_bundle(predicate, true);
        let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
            panic!("{predicate}: typed query must resolve");
        };
        let witness = composed.boolean_witnesses()[0].witness().clone();
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
            "{predicate}: provably impossible TRUE, got {:?}",
            witness.qualifying()
        );
        assert!(
            matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}"
        );
        assert!(
            matches!(composed.output().columns()[0].domain(), ValueDomain::Empty),
            "{predicate}: impossible filter must yield an empty output domain"
        );
    }
}

#[test]
fn transitive_text_equalities_share_prefix_candidates() {
    // If only adjacent text comparisons share candidates, a = b AND b = c
    // cannot find c's required LIKE prefix on a even though the row exists.
    let schema = RelationSchema::new(
        "t",
        ["a", "b", "c"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "VARCHAR(8)", "postgresql").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input =
        SqlInput::inline("SELECT a FROM t WHERE a = b AND b = c AND c LIKE 'xy%'".to_string());
    let dialect = PostgreSqlDialect {};
    let mut bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "transitive_text",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    bundle.declare_comparison_assumptions(&[
        ComparisonAssumption::BinaryCollation,
        ComparisonAssumption::NoCharPadding,
    ]);
    let witness = single_witness(&bundle).expect("attested text witness");
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "transitive equality must share prefix candidates: {:?}",
        witness.qualifying()
    );
    assert!(
        matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "a mismatch or NULL rejects the chain"
    );
}

#[test]
fn null_pattern_and_unsafe_escape_fail_closed_without_witness_claims() {
    for predicate in [
        "a LIKE NULL",
        "a NOT LIKE NULL",
        "NULL LIKE 'ab%'",
        "a LIKE 'ab%' ESCAPE NULL",
    ] {
        let unattested = single_witness(&text_bundle(predicate, false));
        assert!(
            unattested.as_ref().is_none_or(|witness| matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            )),
            "{predicate}: NULL pattern operands must never be exact"
        );
        let attested = single_witness(&text_bundle(predicate, true));
        assert_residual(attested.as_ref(), predicate);
    }
}

#[test]
fn string_column_comparison_has_a_schema_backed_wire_shape() {
    use sql_semantic_protocol::to_bundle_json;
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json")).unwrap();
    let variants = schema["$defs"]["booleanConstraint"]["oneOf"]
        .as_array()
        .expect("boolean constraint union");
    let entry = variants
        .iter()
        .find(|variant| variant["properties"]["kind"]["const"] == "string_column_comparison")
        .expect("string column comparison in active schema");
    assert_eq!(
        entry["required"],
        serde_json::json!(["kind", "left", "operator", "right"])
    );
    let operators = entry["properties"]["operator"]["enum"]
        .as_array()
        .expect("certified text equality operators");
    for operator in ["eq", "neq", "is_distinct_from", "is_not_distinct_from"] {
        assert!(operators.iter().any(|item| item == operator), "{operator}");
    }
    let bundle = text_bundle("a = b", true);
    let output: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let constraint = &output["inputs"][0]["statements"][0]["boolean_witness"]["condition"];
    assert_eq!(constraint["kind"], "string_column_comparison");
    assert_eq!(constraint["operator"], "eq");
    assert_eq!(constraint["left"]["name"], "a");
    assert_eq!(constraint["right"]["name"], "b");
    let declared = output["declared_comparison_assumptions"]
        .as_array()
        .expect("declared comparison assumptions");
    for setting in ["binary_collation", "no_char_padding"] {
        assert!(
            declared.iter().any(|item| item == setting),
            "{setting}: bundle must record the declared comparison settings"
        );
    }
    // A literal-backed text comparison additionally records the certified
    // settings on the statement's row-condition exactness contract
    // (variable-length strings require binary collation; the bundle-level
    // declaration above also carries the no-padding attestation the witness
    // licensing checks).
    let bundle = text_bundle("a = 'ab'", true);
    let output: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let assumptions = output["inputs"][0]["statements"][0]["condition_exactness"]
        ["comparison_assumptions"]
        .as_array()
        .expect("comparison assumptions array");
    assert!(
        assumptions
            .iter()
            .any(|item| item["name"] == "binary_collation"
                && item["declared"] == serde_json::Value::Bool(true)),
        "output must record the certified binary collation setting"
    );
}

#[test]
fn dbt_compiled_pattern_sql_matches_direct_analysis() {
    use sql_semantic_protocol::{analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest};
    // The shared fixture source has no text columns, so dbt parity covers
    // the null-safe integer spaceship law plus a mixed Boolean coupling.
    // Pattern/attestation parity is covered by the cross-dialect test above.
    const SQL: &str =
        "SELECT id, amount FROM warehouse.raw.orders WHERE id <=> amount OR amount IS NULL";
    let mut manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();
    let schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &SqlInput::inline(SQL),
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let witness = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|relation| relation == "warehouse.raw.orders")
            })
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(ref semantics) = layer.composed_semantics() else {
            panic!("resolved semantics required");
        };
        semantics
            .boolean_witnesses()
            .first()
            .expect("witness")
            .witness()
            .clone()
    };
    // The null-safe integer law needs no comparison attestations, so both
    // sides agree on exact TRUE and NOT TRUE rather than on a residual.
    let from_dbt = witness(&dbt);
    let from_sql = witness(&direct);
    assert_eq!(from_dbt.condition(), from_sql.condition());
    assert_eq!(from_dbt.qualifying(), from_sql.qualifying());
    assert_eq!(from_dbt.rejected(), from_sql.rejected());
    assert!(
        matches!(
            from_dbt.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "dbt null-safe equality must be exact"
    );

    // A pattern over a computed expression has no source-row model on either
    // path: dbt and direct analysis share the same fail-closed residual.
    const RESIDUAL_SQL: &str =
        "SELECT id, amount FROM warehouse.raw.orders WHERE CAST(id AS VARCHAR) LIKE 'ab%'";
    let mut manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = RESIDUAL_SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();
    let schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &SqlInput::inline(RESIDUAL_SQL),
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let from_dbt = witness(&dbt);
    let from_sql = witness(&direct);
    assert_eq!(from_dbt.condition(), from_sql.condition());
    assert_eq!(from_dbt.qualifying(), from_sql.qualifying());
    assert_eq!(from_dbt.rejected(), from_sql.rejected());
    assert!(
        matches!(
            from_dbt.qualifying(),
            BooleanWitnessDirection::Residual { .. }
        ),
        "computed pattern operands must stay residual on both paths"
    );
}
