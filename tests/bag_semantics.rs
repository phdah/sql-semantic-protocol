//! Closed-world bag transfer laws checked against canonical dialect and DuckDB semantics.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, BagCountProof, BagCountTarget, BagEvidence, BagJoinKeys,
    BagLaw, BagScope, CountBounds, JoinKind, ProtocolStatement, SetMultiplicityRule,
};

fn exact(n: u64, scope: BagScope) -> BagEvidence {
    BagEvidence::new(CountBounds::new(n, Some(n)).expect("valid count"), scope, true)
}

fn observed(db: &Connection, query: &str) -> u64 {
    let count: i64 = db.query_row(query, [], |row| row.get(0)).expect("query count");
    u64::try_from(count).expect("nonnegative SQL row count")
}

fn checked_count(proof: BagCountProof) -> u64 {
    let BagCountProof::Bounds(bounds) = proof else {
        panic!("expected complete count proof: {proof:?}");
    };
    assert_eq!(bounds.maximum(), Some(bounds.minimum()));
    bounds.minimum()
}

#[test]
fn all_dialects_expose_the_same_typed_set_duplicate_law() {
    for &name in DIALECTS {
        let dialect = dialect_from_name(name).expect("known dialect");
        let output = analyze_sql(
            "SELECT k FROM l UNION ALL SELECT k FROM r",
            name,
            dialect.as_ref(),
        )
        .expect("shared syntax parses");
        let ProtocolStatement::Query(query) = &output.statements()[0] else {
            panic!("query statement for {name}");
        };
        let operation = query.set_operation().expect("typed set semantics");
        let rule = operation.multiplicity_rule().expect("positional set law");
        assert_eq!(rule, SetMultiplicityRule::Sum, "{name}");
        let l = exact(3, BagScope::CandidateTuple);
        let r = exact(2, BagScope::CandidateTuple);
        assert_eq!(checked_count(BagLaw::SetTuple(rule).transfer(l, Some(r))), 5);
    }
}

#[test]
fn duckdb_duplicate_and_null_tuple_counts_obey_all_six_set_laws() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(k INTEGER); CREATE TABLE r(k INTEGER);
         INSERT INTO l VALUES (NULL), (NULL), (NULL);
         INSERT INTO r VALUES (NULL), (NULL);",
    )
    .expect("seed SQL");
    let inputs = (
        exact(3, BagScope::CandidateTuple),
        exact(2, BagScope::CandidateTuple),
    );
    for (rule, sql) in [
        (SetMultiplicityRule::Sum, "UNION ALL"),
        (SetMultiplicityRule::UnionDistinct, "UNION"),
        (SetMultiplicityRule::Minimum, "INTERSECT ALL"),
        (SetMultiplicityRule::IntersectDistinct, "INTERSECT"),
        (SetMultiplicityRule::SaturatingDifference, "EXCEPT ALL"),
        (SetMultiplicityRule::ExceptDistinct, "EXCEPT"),
    ] {
        let actual = observed(&db, &format!(
            "SELECT COUNT(*) FROM (SELECT k FROM l {sql} SELECT k FROM r) AS bag"
        ));
        assert_eq!(
            checked_count(BagLaw::SetTuple(rule).transfer(inputs.0, Some(inputs.1))),
            actual,
            "{sql}"
        );
    }
}

#[test]
fn duckdb_join_duplicate_and_sql_null_nonmatches_are_counted_separately() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(k INTEGER); CREATE TABLE r(k INTEGER);
         INSERT INTO l VALUES (1), (1), (1), (NULL), (NULL);
         INSERT INTO r VALUES (1), (1), (NULL);",
    )
    .expect("seed SQL");
    let left = exact(3, BagScope::CompleteRelation);
    let right = exact(2, BagScope::CompleteRelation);
    let matched = BagLaw::EquiJoin {
        kind: JoinKind::Inner,
        keys: BagJoinKeys::EqualNonNull,
    }
    .transfer(left, Some(right));
    assert_eq!(
        checked_count(matched),
        observed(&db, "SELECT COUNT(*) FROM l JOIN r ON l.k = r.k WHERE l.k = 1")
    );
    let nulls = BagLaw::EquiJoin {
        kind: JoinKind::Full,
        keys: BagJoinKeys::NeverMatch,
    }
    .transfer(exact(2, BagScope::CompleteRelation), Some(exact(1, BagScope::CompleteRelation)));
    assert_eq!(
        checked_count(nulls),
        observed(&db, "SELECT COUNT(*) FROM l FULL JOIN r ON l.k = r.k WHERE l.k IS NULL AND r.k IS NULL")
    );
}

#[test]
fn negative_counts_and_open_world_are_not_promoted_to_feasible_source_plans() {
    let empty = exact(0, BagScope::CandidateTuple);
    let count = BagLaw::SetTuple(SetMultiplicityRule::Minimum).transfer(
        exact(3, BagScope::CandidateTuple), Some(empty),
    );
    assert_eq!(count.assess(CountBounds::new(1, Some(1)).expect("valid")), BagCountTarget::Impossible);
    let open = BagEvidence::new(empty.bounds(), BagScope::CandidateTuple, false);
    assert!(matches!(
        BagLaw::SetTuple(SetMultiplicityRule::Minimum)
            .transfer(exact(3, BagScope::CandidateTuple), Some(open)),
        BagCountProof::Residual { reason: "right_open_world" }
    ));
}
