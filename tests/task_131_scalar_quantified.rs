//! TASK-131 public finite scalar and quantified-comparison proofs.
//! Native DuckDB supplies independent SQL cardinality and three-valued oracles.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_quantified_comparison, physical_finite_scalar_subquery_plan, AnalysisBundle,
    ComparisonOperator, ConfiguredSqlInput, LiteralValue, PhysicalFiniteQuantifiedProof,
    PhysicalQuantifiedOperands, PhysicalQuantifiedTruth, PhysicalQuantifier,
    PhysicalScalarSubqueryProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};

fn catalog() -> RelationCatalog {
    let source = |name| {
        RelationSchema::new(
            name,
            vec![
                SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").expect("id type"),
                SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").expect("value type"),
            ],
        )
        .expect("source schema")
    };
    RelationCatalog::from_schemas(&[source("outer_rows"), source("inner_rows")]).expect("catalog")
}

fn bundle(sql: &str) -> AnalysisBundle {
    let input = SqlInput::inline(sql);
    let dialect = dialect_from_name("postgresql").expect("dialect");
    let configured = ConfiguredSqlInput::new("task-131", &input, "postgresql", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog()).expect("SQL analysis")
}

fn state(relation: &str, data: &[(&str, Option<i64>)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        data.iter()
            .map(|(id, value)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), LiteralValue::Text((*id).to_string())),
                    (
                        "v".to_string(),
                        value.map_or(LiteralValue::Null, |value| {
                            LiteralValue::Number(value.to_string())
                        }),
                    ),
                ]))
                .expect("row")
            })
            .collect(),
    )
    .expect("source")
}

fn scalar_proof(sql: &str, inner: &[(&str, Option<i64>)]) -> PhysicalScalarSubqueryProof {
    let b = bundle(sql);
    let target = b.layers().last().expect("last layer");
    physical_finite_scalar_subquery_plan(&b, target.id(), &[state("inner_rows", inner)])
}

#[test]
fn scalar_subquery_distinguishes_empty_singleton_null_and_cardinality_error() {
    let sql = "SELECT (SELECT i.v FROM inner_rows i) AS x";
    let PhysicalScalarSubqueryProof::Exact(empty) = scalar_proof(sql, &[]) else {
        panic!("empty scalar bag must produce exact SQL NULL");
    };
    assert_eq!(empty.value, LiteralValue::Null);
    assert_eq!(empty.bounds, None);
    assert!(empty.source_row_ids.is_empty());

    let PhysicalScalarSubqueryProof::Exact(single) = scalar_proof(sql, &[("i1", Some(2))]) else {
        panic!("one input row must produce the typed scalar");
    };
    assert_eq!(single.value, LiteralValue::Number("2".to_string()));
    assert_eq!(single.bounds, Some((2, 2)));
    assert_eq!(single.source_row_ids, ["i1"]);

    let PhysicalScalarSubqueryProof::Exact(null) = scalar_proof(sql, &[("i1", None)]) else {
        panic!("one NULL value must remain SQL NULL");
    };
    assert_eq!(null.value, LiteralValue::Null);
    assert_eq!(null.source_row_ids, ["i1"]);

    let PhysicalScalarSubqueryProof::CardinalityViolation { source_row_ids } =
        scalar_proof(sql, &[("i1", Some(2)), ("i2", Some(2))])
    else {
        panic!("two physical rows must raise scalar cardinality error, even if values agree");
    };
    assert_eq!(source_row_ids, ["i1", "i2"]);
}

#[test]
fn scalar_unknown_filter_and_duplicate_physical_ids_fail_closed() {
    let sql = "SELECT (SELECT i.v FROM inner_rows i WHERE i.v > 2) AS x";
    assert!(matches!(
        scalar_proof(sql, &[("i1", Some(2)), ("i2", Some(3))]),
        PhysicalScalarSubqueryProof::Residual { .. }
    ));
    let sql = "SELECT (SELECT i.v FROM inner_rows i) AS x";
    assert!(matches!(
        scalar_proof(sql, &[("i1", Some(2)), ("i1", Some(3))]),
        PhysicalScalarSubqueryProof::Impossible { .. }
    ));
}

fn quantified(
    outer: &[(&str, Option<i64>)],
    inner: &[(&str, Option<i64>)],
    quantifier: PhysicalQuantifier,
    operator: ComparisonOperator,
) -> PhysicalFiniteQuantifiedProof {
    // The IN analyzer establishes the two independent, typed source schemas.
    // The operator-local quantifier is passed explicitly. This deliberately
    // does not certify SQL ANY/ALL analyzer binding.
    let b = bundle("SELECT o.v FROM outer_rows o WHERE o.v IN (SELECT i.v FROM inner_rows i)");
    physical_finite_quantified_comparison(
        &b,
        &[state("outer_rows", outer), state("inner_rows", inner)],
        PhysicalQuantifiedOperands {
            outer_relation: "outer_rows",
            outer_column: "v",
            inner_relation: "inner_rows",
            inner_column: "v",
        },
        operator,
        quantifier,
    )
}

#[test]
fn any_all_prove_empty_bag_null_dominance_and_duplicates() {
    let cases = [
        (
            PhysicalQuantifier::Any,
            ComparisonOperator::Eq,
            vec![],
            PhysicalQuantifiedTruth::False,
        ),
        (
            PhysicalQuantifier::All,
            ComparisonOperator::Eq,
            vec![],
            PhysicalQuantifiedTruth::True,
        ),
        (
            PhysicalQuantifier::Any,
            ComparisonOperator::Eq,
            vec![("i1", None), ("i2", Some(1)), ("i3", Some(1))],
            PhysicalQuantifiedTruth::True,
        ),
        (
            PhysicalQuantifier::Any,
            ComparisonOperator::Eq,
            vec![("i1", Some(2)), ("i2", None)],
            PhysicalQuantifiedTruth::Unknown,
        ),
        (
            PhysicalQuantifier::All,
            ComparisonOperator::Lt,
            vec![("i1", Some(2)), ("i2", None), ("i3", Some(0))],
            PhysicalQuantifiedTruth::False,
        ),
        (
            PhysicalQuantifier::All,
            ComparisonOperator::Gt,
            vec![("i1", Some(0)), ("i2", None)],
            PhysicalQuantifiedTruth::Unknown,
        ),
    ];
    for (quantifier, operator, inner, expected) in cases {
        let PhysicalFiniteQuantifiedProof::Exact(exact) =
            quantified(&[("o1", Some(1))], &inner, quantifier, operator)
        else {
            panic!("complete integer operator bag must be exact");
        };
        assert_eq!(exact.truth["o1"], expected);
        assert_eq!(exact.candidate_inner["o1"].len(), inner.len());
        assert_eq!(exact.selected.len() + exact.rejected.len(), 1);
        if expected == PhysicalQuantifiedTruth::True {
            assert_eq!(exact.selected, ["o1"]);
            assert_eq!(
                exact.terminal,
                [BTreeMap::from([(
                    "v".to_string(),
                    LiteralValue::Number("1".to_string())
                )])]
            );
            assert_eq!(exact.terminal_domain.bounds(), Some((1, 1)));
        } else {
            assert_eq!(exact.rejected, ["o1"]);
            assert!(exact.terminal.is_empty());
            assert_eq!(exact.terminal_domain.bounds(), None);
        }
    }
}

#[test]
fn quantified_null_outer_empty_source_and_unsupported_operator_are_safe() {
    let PhysicalFiniteQuantifiedProof::Exact(exact) = quantified(
        &[("o1", None)],
        &[("i1", Some(2))],
        PhysicalQuantifier::Any,
        ComparisonOperator::Eq,
    ) else {
        panic!("nullable operand");
    };
    assert_eq!(exact.truth["o1"], PhysicalQuantifiedTruth::Unknown);
    let PhysicalFiniteQuantifiedProof::Exact(exact) = quantified(
        &[("o1", None)],
        &[],
        PhysicalQuantifier::All,
        ComparisonOperator::Eq,
    ) else {
        panic!("vacuous ALL");
    };
    assert_eq!(exact.truth["o1"], PhysicalQuantifiedTruth::True);
    assert!(exact.terminal_domain.null_allowed);
    assert_eq!(
        exact.terminal,
        [BTreeMap::from([("v".to_string(), LiteralValue::Null)])]
    );

    assert!(matches!(
        quantified(
            &[("o1", Some(1))],
            &[("i1", Some(2))],
            PhysicalQuantifier::Any,
            ComparisonOperator::IsDistinctFrom,
        ),
        PhysicalFiniteQuantifiedProof::Residual { .. }
    ));
    assert!(matches!(
        quantified(
            &[("o1", Some(1)), ("o1", Some(2))],
            &[("i1", Some(2))],
            PhysicalQuantifier::Any,
            ComparisonOperator::Eq,
        ),
        PhysicalFiniteQuantifiedProof::Impossible { .. }
    ));
}

#[test]
fn duckdb_independently_checks_scalar_cardinality_and_quantifier_truth() {
    let db = Connection::open_in_memory().expect("duckdb");
    db.execute("CREATE TABLE inner_rows (id VARCHAR, v INTEGER)", [])
        .unwrap();
    let scalar_sql = "SELECT (SELECT v FROM inner_rows) AS x";
    let read = |db: &Connection| -> Option<i64> {
        db.query_row(scalar_sql, [], |row| row.get(0))
            .expect("scalar")
    };
    assert_eq!(read(&db), None);
    db.execute("INSERT INTO inner_rows VALUES ('i1', 2)", [])
        .unwrap();
    assert_eq!(read(&db), Some(2));
    db.execute("INSERT INTO inner_rows VALUES ('i2', 2)", [])
        .unwrap();
    assert!(
        db.query_row(scalar_sql, [], |row| row.get::<_, Option<i64>>(0))
            .is_err(),
        "two equal-valued rows are still a cardinality violation"
    );
    db.execute("DELETE FROM inner_rows", []).unwrap();
    for (id, value) in [("i1", Some(1)), ("i2", None), ("i3", Some(1))] {
        db.execute("INSERT INTO inner_rows VALUES (?, ?)", params![id, value])
            .unwrap();
    }
    let classify = |sql: &str| -> String {
        db.query_row(sql, [], |row| row.get(0))
            .expect("three valued SQL")
    };
    assert_eq!(
        classify("SELECT CASE WHEN 1 = ANY (SELECT v FROM inner_rows) THEN 'true' WHEN (1 = ANY (SELECT v FROM inner_rows)) IS NULL THEN 'unknown' ELSE 'false' END"),
        "true"
    );
    assert_eq!(
        classify("SELECT CASE WHEN 2 = ALL (SELECT v FROM inner_rows) THEN 'true' WHEN (2 = ALL (SELECT v FROM inner_rows)) IS NULL THEN 'unknown' ELSE 'false' END"),
        "false"
    );
    assert_eq!(
        classify("SELECT CASE WHEN 2 = SOME (SELECT v FROM inner_rows) THEN 'true' WHEN (2 = SOME (SELECT v FROM inner_rows)) IS NULL THEN 'unknown' ELSE 'false' END"),
        "unknown"
    );
    db.execute("DELETE FROM inner_rows", []).unwrap();
    assert_eq!(
        classify(
            "SELECT CASE WHEN 1 = ANY (SELECT v FROM inner_rows) THEN 'true' ELSE 'false' END"
        ),
        "false"
    );
    assert_eq!(
        classify(
            "SELECT CASE WHEN 1 = ALL (SELECT v FROM inner_rows) THEN 'true' ELSE 'false' END"
        ),
        "true"
    );
}

#[test]
fn frozen_quantifiers_are_bound_to_sql_not_a_caller_operator() {
    use serde_json::Value;
    use sql_semantic_protocol::{
        physical_finite_quantified_subquery_plan, PhysicalQuantifiedScalarProof,
    };
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json"))
            .expect("fixture contract");
    let db = Connection::open_in_memory().expect("duckdb");
    db.execute("CREATE TABLE inner_rows (id VARCHAR, v INTEGER)", [])
        .unwrap();
    let mut proved = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if !["any", "all"].contains(&case["kind"].as_str().unwrap_or_default()) {
            continue;
        }
        let sql = case["sql"].as_str().unwrap();
        let b = bundle(sql);
        let target = b.layers().last().expect("target");
        let snapshot = state(
            "inner_rows",
            &case["inner"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| (row["id"].as_str().unwrap(), row["v"].as_i64()))
                .collect::<Vec<_>>(),
        );
        db.execute("DELETE FROM inner_rows", []).unwrap();
        for row in case["inner"].as_array().unwrap() {
            db.execute(
                "INSERT INTO inner_rows VALUES (?, ?)",
                params![row["id"].as_str().unwrap(), row["v"].as_i64()],
            )
            .unwrap();
        }
        let PhysicalQuantifiedScalarProof::Exact(actual) =
            physical_finite_quantified_subquery_plan(&b, target.id(), &[snapshot])
        else {
            panic!("{} canonical quantified proof missing", case["id"]);
        };
        let expected = match case["expect"]["value"].as_str().unwrap() {
            "true" => PhysicalQuantifiedTruth::True,
            "false" => PhysicalQuantifiedTruth::False,
            "unknown" => PhysicalQuantifiedTruth::Unknown,
            v => panic!("unexpected truth {v}"),
        };
        assert_eq!(actual.truth, expected, "{} protocol truth", case["id"]);
        assert_eq!(
            actual.source_row_ids.len(),
            case["inner"].as_array().unwrap().len()
        );
        assert_eq!(actual.terminal.len(), 1);
        let wanted = match expected {
            PhysicalQuantifiedTruth::True => Some(true),
            PhysicalQuantifiedTruth::False => Some(false),
            PhysicalQuantifiedTruth::Unknown => None,
        };
        let native: Option<bool> = db.query_row(sql, [], |row| row.get(0)).unwrap();
        assert_eq!(native, wanted, "{} native SQL outcome", case["id"]);
        assert_eq!(
            actual.terminal[0].values().next(),
            Some(&wanted.map_or(LiteralValue::Null, LiteralValue::Boolean))
        );
        proved += 1;
    }
    assert_eq!(proved, 8, "all frozen ANY/SOME/ALL obligations");
}

#[test]
fn sql_bound_quantifiers_reject_unproved_nested_filters_and_bad_ids() {
    use sql_semantic_protocol::{
        physical_finite_quantified_subquery_plan, PhysicalQuantifiedScalarProof,
    };
    let sql = "SELECT 2 = ANY (SELECT v FROM inner_rows WHERE v > 1) AS x";
    let b = bundle(sql);
    let id = b.layers().last().unwrap().id().to_string();
    assert!(matches!(
        physical_finite_quantified_subquery_plan(
            &b,
            &id,
            &[state("inner_rows", &[("i1", Some(2))])]
        ),
        PhysicalQuantifiedScalarProof::Residual { .. }
    ));
    let b = bundle("SELECT 2 = ANY (SELECT v FROM inner_rows) AS x");
    let id = b.layers().last().unwrap().id().to_string();
    assert!(matches!(
        physical_finite_quantified_subquery_plan(
            &b,
            &id,
            &[state("inner_rows", &[("i1", Some(2)), ("i1", Some(3))])]
        ),
        PhysicalQuantifiedScalarProof::Impossible { .. }
    ));
}

#[test]
fn every_frozen_mandatory_case_checks_all_thirteen_parser_dialects_and_duckdb() {
    use serde_json::Value;
    use sql_semantic_protocol::{
        physical_finite_quantified_subquery_plan, PhysicalQuantifiedScalarProof,
    };
    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json")).unwrap();
    let dialects = frozen["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let db = Connection::open_in_memory().unwrap();
    db.execute("CREATE TABLE inner_rows (id VARCHAR, v INTEGER)", [])
        .unwrap();
    let mut dialect_cells = 0;
    let mut duckdb_cases = 0;
    for case in frozen["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        if !["scalar", "any", "all"].contains(&kind) {
            continue;
        }
        let rows = case["inner"].as_array().unwrap();
        let values = rows
            .iter()
            .map(|row| (row["id"].as_str().unwrap(), row["v"].as_i64()))
            .collect::<Vec<_>>();
        let physical = state("inner_rows", &values);
        let sql = case["sql"].as_str().unwrap();
        db.execute("DELETE FROM inner_rows", []).unwrap();
        for row in rows {
            db.execute(
                "INSERT INTO inner_rows VALUES (?, ?)",
                params![row["id"].as_str().unwrap(), row["v"].as_i64()],
            )
            .unwrap();
        }
        let mut native_source = db
            .prepare("SELECT id,v FROM inner_rows ORDER BY id")
            .unwrap();
        let source_readback = native_source
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
            })
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();
        let mut expected_source = values
            .iter()
            .map(|(id, v)| ((*id).to_string(), *v))
            .collect::<Vec<_>>();
        expected_source.sort();
        assert_eq!(
            source_readback, expected_source,
            "{} independent complete physical source bag",
            case["id"]
        );
        assert_eq!(
            case["source_row_ids"].as_array().unwrap().len(),
            values.len()
        );
        assert_eq!(
            case["terminal_oracle"]["type"],
            if kind == "scalar" && case["expect"]["type"] == "cardinality_error" {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(
                    if kind == "scalar" {
                        "SIGNED_INTEGER"
                    } else {
                        "BOOLEAN"
                    }
                    .to_string(),
                )
            }
        );
        let want = &case["expect"];
        if kind == "scalar" {
            let native = db.query_row(sql, [], |row| row.get::<_, Option<i64>>(0));
            if want["type"] == "cardinality_error" {
                assert!(native.is_err(), "{} native cardinality error", case["id"]);
            } else {
                assert_eq!(
                    native.unwrap(),
                    want["value"].as_i64(),
                    "{} native scalar terminal bag",
                    case["id"]
                );
            }
        } else {
            let expected = match want["value"].as_str().unwrap() {
                "true" => Some(true),
                "false" => Some(false),
                "unknown" => None,
                other => panic!("invalid frozen truth {other}"),
            };
            let native: Option<bool> = db.query_row(sql, [], |row| row.get(0)).unwrap();
            assert_eq!(native, expected, "{} native truth", case["id"]);
        }
        duckdb_cases += 1;
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let parser = dialect_from_name(name).unwrap();
            let input = SqlInput::inline(sql);
            let configured =
                ConfiguredSqlInput::new("task-131-matrix", &input, name, parser.as_ref());
            let b = analyze_configured_inputs_with_catalog(&[configured], &catalog())
                .unwrap_or_else(|e| {
                    panic!("{} {name}: mandatory SQL parser failed: {e:?}", case["id"])
                });
            let id = b.layers().last().expect("semantic layer").id().to_string();
            if kind == "scalar" {
                let proof =
                    physical_finite_scalar_subquery_plan(&b, &id, std::slice::from_ref(&physical));
                match (want["type"].as_str().unwrap(), proof) {
                    ("exact", PhysicalScalarSubqueryProof::Exact(result)) => {
                        let value = want["value"]
                            .as_i64()
                            .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()));
                        assert_eq!(result.value, value, "{} {name} scalar", case["id"]);
                        assert_eq!(
                            result.source_row_ids,
                            values.iter().map(|v| v.0.to_string()).collect::<Vec<_>>()
                        );
                        assert_eq!(
                            result.bounds,
                            want["value"].as_i64().map(|v| (v as i128, v as i128))
                        );
                    }
                    (
                        "cardinality_error",
                        PhysicalScalarSubqueryProof::CardinalityViolation { source_row_ids },
                    ) => assert_eq!(
                        source_row_ids,
                        values.iter().map(|v| v.0.to_string()).collect::<Vec<_>>()
                    ),
                    (expect, other) => {
                        panic!("{} {name}: expected {expect}, got {other:?}", case["id"])
                    }
                };
            } else {
                let proof = physical_finite_quantified_subquery_plan(
                    &b,
                    &id,
                    std::slice::from_ref(&physical),
                );
                let PhysicalQuantifiedScalarProof::Exact(exact) = proof else {
                    panic!(
                        "{} {name}: required bound quantified proof missing: {proof:?}",
                        case["id"]
                    );
                };
                let expected = match want["value"].as_str().unwrap() {
                    "true" => PhysicalQuantifiedTruth::True,
                    "false" => PhysicalQuantifiedTruth::False,
                    "unknown" => PhysicalQuantifiedTruth::Unknown,
                    _ => unreachable!(),
                };
                assert_eq!(
                    exact.truth, expected,
                    "{} {name} three-valued truth",
                    case["id"]
                );
                assert_eq!(
                    exact.source_row_ids,
                    values.iter().map(|v| v.0.to_string()).collect::<Vec<_>>()
                );
                assert_eq!(exact.terminal.len(), 1);
                let terminal = match expected {
                    PhysicalQuantifiedTruth::True => LiteralValue::Boolean(true),
                    PhysicalQuantifiedTruth::False => LiteralValue::Boolean(false),
                    PhysicalQuantifiedTruth::Unknown => LiteralValue::Null,
                };
                assert_eq!(exact.terminal[0].values().next(), Some(&terminal));
            }
            dialect_cells += 1;
        }
    }
    assert_eq!(duckdb_cases, 12, "all 12 mandatory frozen cases");
    assert_eq!(dialect_cells, 156, "12 cases across 13 dialect parsers");
}

#[test]
fn source_free_outer_limit_and_unproved_outer_shape_do_not_claim_exactness() {
    use sql_semantic_protocol::{
        physical_finite_quantified_subquery_plan, PhysicalQuantifiedScalarProof,
    };
    for sql in [
        "SELECT (SELECT v FROM inner_rows) AS x LIMIT 0",
        "SELECT (SELECT v FROM inner_rows) AS x LIMIT 1",
    ] {
        assert!(
            matches!(
                scalar_proof(sql, &[("i1", Some(2))]),
                PhysicalScalarSubqueryProof::Residual { .. }
            ),
            "{sql}"
        );
    }
    for sql in [
        "SELECT 1 = ANY (SELECT v FROM inner_rows) AS x LIMIT 0",
        "SELECT 1 = ANY (SELECT v FROM inner_rows) AS x LIMIT 1",
    ] {
        let b = bundle(sql);
        let id = b.layers().last().unwrap().id().to_string();
        assert!(
            matches!(
                physical_finite_quantified_subquery_plan(
                    &b,
                    &id,
                    &[state("inner_rows", &[("i1", Some(1))])]
                ),
                PhysicalQuantifiedScalarProof::Residual { .. }
            ),
            "{sql}"
        );
    }
}

#[test]
fn conditional_nested_lateral_and_apply_cases_have_explicit_parser_dispositions() {
    use serde_json::Value;
    use sqlparser::parser::Parser;
    use std::collections::BTreeMap;

    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json")).unwrap();
    let dialects = frozen["dialects"].as_array().unwrap();
    let mut results = BTreeMap::<String, BTreeMap<String, String>>::new();
    for case in frozen["cases"].as_array().unwrap() {
        if case["expected_proof"] != "Residual" {
            continue;
        }
        let id = case["id"].as_str().unwrap().to_string();
        assert_eq!(case["expect"]["owner"], "TASK-131");
        let mut statuses = BTreeMap::new();
        for name in dialects {
            let dialect = name.as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            let observed =
                if Parser::parse_sql(parser.as_ref(), case["sql"].as_str().unwrap()).is_ok() {
                    "parsed"
                } else {
                    "parser_boundary"
                };
            statuses.insert(dialect.to_string(), observed.to_string());
        }
        results.insert(id, statuses);
    }
    // The pinned conditional parser matrix must be reviewed and checked
    // against actual sqlparser paths, never inferred from vendor support.
    let expected = &frozen["conditional_parser_status_by_case"];
    assert_eq!(
        serde_json::to_value(&results).unwrap(),
        *expected,
        "record and review all 39 conditional parser dispositions"
    );
}

#[test]
fn conditional_duckdb_oracles_do_not_upgrade_uncertified_nested_or_lateral_proofs() {
    use sql_semantic_protocol::{
        physical_finite_quantified_subquery_plan, PhysicalQuantifiedScalarProof,
    };
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE inner_rows (id VARCHAR, v INTEGER);
         CREATE TABLE outer_rows (id VARCHAR, v INTEGER);
         CREATE TABLE control_rows (k INTEGER);
         INSERT INTO inner_rows VALUES ('i1',2),('i2',2);
         INSERT INTO outer_rows VALUES ('o1',2),('o2',3);
         INSERT INTO control_rows VALUES (2);",
    )
    .unwrap();
    // A nested scalar filter is not an identity-preserving direct projection:
    // here its two matching source rows cause a runtime cardinality failure.
    let nested_sql =
        "SELECT (SELECT v FROM inner_rows WHERE v = (SELECT k FROM control_rows)) AS x";
    assert!(db
        .query_row(nested_sql, [], |r| r.get::<_, Option<i64>>(0))
        .is_err());
    let b = bundle(nested_sql);
    let id = b.layers().last().unwrap().id().to_string();
    assert!(matches!(
        physical_finite_scalar_subquery_plan(
            &b,
            &id,
            &[state("inner_rows", &[("i1", Some(2)), ("i2", Some(2))])]
        ),
        PhysicalScalarSubqueryProof::Residual { .. }
    ));
    // Correlated LATERAL is executable under native DuckDB, but a single
    // globally evaluated nested source would invent the per-outer-row bag.
    let lateral_sql = "SELECT o.id FROM outer_rows o CROSS JOIN LATERAL (SELECT v FROM inner_rows i WHERE i.v = o.v) s";
    let mut stmt = db.prepare(lateral_sql).unwrap();
    let mut ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    ids.sort();
    assert_eq!(ids, ["o1".to_string(), "o1".to_string()]);
    let b = bundle(lateral_sql);
    let id = b.layers().last().unwrap().id().to_string();
    assert!(matches!(
        physical_finite_quantified_subquery_plan(
            &b,
            &id,
            &[state("inner_rows", &[("i1", Some(2)), ("i2", Some(2))])]
        ),
        PhysicalQuantifiedScalarProof::Residual { .. }
    ));
}

#[test]
fn canonical_quantifiers_round_trip_to_the_public_json_predicate_contract() {
    use sql_semantic_protocol::{analyze_sql, to_json};
    for (sql, quantifier, operator) in [
        (
            "SELECT 1 = ANY (SELECT v FROM inner_rows) AS x",
            "any",
            "eq",
        ),
        (
            "SELECT 2 = SOME (SELECT v FROM inner_rows) AS x",
            "any",
            "eq",
        ),
        (
            "SELECT 5 < ALL (SELECT v FROM inner_rows) AS x",
            "all",
            "lt",
        ),
    ] {
        let parser = dialect_from_name("duckdb").unwrap();
        let protocol = analyze_sql(sql, "duckdb", parser.as_ref()).expect("SQL normalization");
        let emitted: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
        let projection =
            &emitted["inputs"][0]["statements"][0]["output"]["columns"][0]["expression"];
        assert_eq!(projection["kind"], "boolean_predicate", "{sql}");
        let predicate = &projection["predicate"];
        assert_eq!(predicate["kind"], "quantified_subquery", "{sql}");
        assert_eq!(predicate["quantifier"], quantifier, "{sql}");
        assert_eq!(predicate["operator"], operator, "{sql}");
        assert_eq!(
            predicate["subquery"]["dependencies"][0], "inner_rows",
            "{sql}"
        );
        assert!(
            predicate["operand"].is_object(),
            "{sql}: canonical typed operand missing"
        );
    }
}

#[test]
fn frozen_acceptance_matrix_is_complete_and_has_no_unreviewed_cells() {
    use serde_json::{json, Value};
    use std::collections::BTreeSet;
    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json")).unwrap();
    assert_eq!(frozen["version"], 4);
    assert_eq!(frozen["owner"], "TASK-131");
    assert_eq!(frozen["parent"], "TASK-75");
    assert_eq!(frozen["fixture_ids"], json!(["75-C131-P", "75-C131-N"]));
    let dialects = frozen["dialects"].as_array().unwrap();
    let adapters = frozen["required_adapters"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    assert_eq!(adapters, json!(["sql", "dbt", "odcs"]).as_array().unwrap());
    let mut ids = BTreeSet::new();
    let mut mandatory = 0;
    let mut conditional = 0;
    let mut parser_cells = 0;
    let mut adapter_cells = 0;
    for case in frozen["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id));
        assert!(case["sql"].as_str().is_some_and(|sql| !sql.is_empty()));
        assert!(case["canonical_source_identities"]
            .as_array()
            .is_some_and(|sources| !sources.is_empty()));
        assert!(case["canonical_column_identities"]
            .as_object()
            .is_some_and(|columns| !columns.is_empty()));
        let sources = case["source_bags"].as_object().unwrap();
        assert_eq!(
            sources["inner_rows"], case["inner"],
            "{id}: complete source snapshot"
        );
        assert_eq!(
            case["parser_status_by_dialect"].as_object().unwrap().len(),
            13
        );
        assert_eq!(
            case["native_execution_disposition_by_dialect"]
                .as_object()
                .unwrap()
                .len(),
            13
        );
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            assert_eq!(
                case["parser_status_by_dialect"][name], "parsed",
                "{id}/{name}"
            );
            let engine = case["native_execution_disposition_by_dialect"][name]
                .as_str()
                .unwrap();
            if name != "duckdb" {
                assert!(
                    !engine.contains("executed_in_ci"),
                    "{id}/{name}: no native vendor oracle"
                );
            }
            parser_cells += 1;
        }
        for adapter in adapters {
            let name = adapter.as_str().unwrap();
            let cell = case["expected_adapter_cells"][name].as_str().unwrap();
            assert!(
                cell.contains("required") || cell == "Residual_conditional",
                "{id}/{name}: unreviewed adapter"
            );
            adapter_cells += 1;
        }
        match case["expected_proof"].as_str().unwrap() {
            "Exact" | "CardinalityViolation" => {
                mandatory += 1;
                let row_ids = case["source_row_ids"].as_array().unwrap();
                assert_eq!(row_ids.len(), case["inner"].as_array().unwrap().len());
                let oracle = &case["terminal_oracle"];
                if case["expected_proof"] == "CardinalityViolation" {
                    assert_eq!(oracle["status"], "runtime_cardinality_error");
                    assert!(oracle["rows"].is_null());
                } else {
                    assert_eq!(oracle["status"], "exact");
                    assert_eq!(oracle["rows"].as_array().unwrap().len(), 1);
                    assert_eq!(oracle["null_allowed"], oracle["rows"][0].is_null());
                    if oracle["type"] == "SIGNED_INTEGER" {
                        let value = case["expect"]["value"].as_i64();
                        assert_eq!(
                            oracle["inclusive_bounds"],
                            value.map_or(Value::Null, |v| json!([v, v]))
                        );
                    } else {
                        assert_eq!(oracle["type"], "BOOLEAN");
                    }
                }
            }
            "Residual" => {
                conditional += 1;
                assert_eq!(case["expect"]["owner"], "TASK-131");
                assert!(case["release_disposition"]
                    .as_str()
                    .unwrap()
                    .contains("TASK-131 reviewed conditional"));
                assert!(case["review_evidence"]["parser"].as_str().is_some());
            }
            other => panic!("unreviewed mandatory/conditional case {id}: {other}"),
        }
    }
    assert_eq!((mandatory, conditional), (12, 3));
    assert_eq!(parser_cells, 195);
    assert_eq!(adapter_cells, 45);
}
