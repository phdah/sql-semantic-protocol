//! TASK-131 public finite scalar and quantified-comparison proofs.
//! Native DuckDB supplies independent SQL cardinality and three-valued oracles.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_quantified_comparison, physical_finite_scalar_subquery_plan,
    AnalysisBundle, ComparisonOperator, ConfiguredSqlInput, LiteralValue,
    PhysicalFiniteQuantifiedProof, PhysicalQuantifiedTruth, PhysicalQuantifier,
    PhysicalScalarSubqueryProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
    RelationCatalog::from_schemas(&[source("outer_rows"), source("inner_rows")])
        .expect("catalog")
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

fn scalar_proof(
    sql: &str,
    inner: &[(&str, Option<i64>)],
) -> PhysicalScalarSubqueryProof {
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

    let PhysicalScalarSubqueryProof::Exact(single) =
        scalar_proof(sql, &[("i1", Some(2))])
    else {
        panic!("one input row must produce the typed scalar");
    };
    assert_eq!(single.value, LiteralValue::Number("2".to_string()));
    assert_eq!(single.bounds, Some((2, 2)));
    assert_eq!(single.source_row_ids, ["i1"]);

    let PhysicalScalarSubqueryProof::Exact(null) =
        scalar_proof(sql, &[("i1", None)])
    else {
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
        "outer_rows",
        "v",
        "inner_rows",
        "v",
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
                [BTreeMap::from([("v".to_string(), LiteralValue::Number("1".to_string()))])]
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
    assert_eq!(exact.terminal, [BTreeMap::from([("v".to_string(), LiteralValue::Null)])]);

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
    db.execute("CREATE TABLE inner_rows (id VARCHAR, v INTEGER)", []).unwrap();
    let scalar_sql = "SELECT (SELECT v FROM inner_rows) AS x";
    let read = |db: &Connection| -> Option<i64> {
        db.query_row(scalar_sql, [], |row| row.get(0)).expect("scalar")
    };
    assert_eq!(read(&db), None);
    db.execute("INSERT INTO inner_rows VALUES ('i1', 2)", []).unwrap();
    assert_eq!(read(&db), Some(2));
    db.execute("INSERT INTO inner_rows VALUES ('i2', 2)", []).unwrap();
    assert!(
        db.query_row(scalar_sql, [], |row| row.get::<_, Option<i64>>(0)).is_err(),
        "two equal-valued rows are still a cardinality violation"
    );
    db.execute("DELETE FROM inner_rows", []).unwrap();
    for (id, value) in [("i1", Some(1)), ("i2", None), ("i3", Some(1))] {
        db.execute(
            "INSERT INTO inner_rows VALUES (?, ?)",
            params![id, value],
        )
        .unwrap();
    }
    let classify = |sql: &str| -> String {
        db.query_row(sql, [], |row| row.get(0)).expect("three valued SQL")
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
    assert_eq!(classify("SELECT CASE WHEN 1 = ANY (SELECT v FROM inner_rows) THEN 'true' ELSE 'false' END"), "false");
    assert_eq!(classify("SELECT CASE WHEN 1 = ALL (SELECT v FROM inner_rows) THEN 'true' ELSE 'false' END"), "true");
}
