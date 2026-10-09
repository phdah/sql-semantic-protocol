//! Closed-world bag transfer laws checked against canonical dialect and DuckDB semantics.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, BagCountProof, BagCountTarget, BagEvidence, BagJoinKeys,
    BagLaw, BagScope, BagSourceIdentity, BagTupleIdentity, CountBounds, JoinKind, ProtocolStatement,
    SetMultiplicityRule,
};

fn exact(n: u64, scope: BagScope) -> BagEvidence {
    let evidence = BagEvidence::new(
        CountBounds::new(n, Some(n)).expect("valid count"),
        scope,
        true,
    );
    if scope == BagScope::CandidateTuple {
        evidence.with_tuple_identity(BagTupleIdentity::new(1))
    } else {
        evidence
    }
}

fn observed(db: &Connection, query: &str) -> u64 {
    let count: i64 = db
        .query_row(query, [], |row| row.get(0))
        .expect("query count");
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
        assert_eq!(
            checked_count(BagLaw::SetTuple(rule).transfer(l, Some(r))),
            5
        );
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
        let actual = observed(
            &db,
            &format!("SELECT COUNT(*) FROM (SELECT k FROM l {sql} SELECT k FROM r) AS bag"),
        );
        assert_eq!(
            checked_count(
                BagLaw::SetTuple(rule).transfer(inputs.0.clone(), Some(inputs.1.clone()))
            ),
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
        observed(
            &db,
            "SELECT COUNT(*) FROM l JOIN r ON l.k = r.k WHERE l.k = 1"
        )
    );
    let nulls = BagLaw::EquiJoin {
        kind: JoinKind::Full,
        keys: BagJoinKeys::NeverMatch,
    }
    .transfer(
        exact(2, BagScope::CompleteRelation),
        Some(exact(1, BagScope::CompleteRelation)),
    );
    assert_eq!(
        checked_count(nulls),
        observed(
            &db,
            "SELECT COUNT(*) FROM l FULL JOIN r ON l.k = r.k WHERE l.k IS NULL AND r.k IS NULL"
        )
    );
}

#[test]
fn negative_counts_and_open_world_are_not_promoted_to_feasible_source_plans() {
    let empty = exact(0, BagScope::CandidateTuple);
    let count = BagLaw::SetTuple(SetMultiplicityRule::Minimum)
        .transfer(exact(3, BagScope::CandidateTuple), Some(empty.clone()));
    assert_eq!(
        count.assess(CountBounds::new(1, Some(1)).expect("valid")),
        BagCountTarget::Impossible
    );
    let open = BagEvidence::new(empty.bounds(), BagScope::CandidateTuple, false);
    assert!(matches!(
        BagLaw::SetTuple(SetMultiplicityRule::Minimum)
            .transfer(exact(3, BagScope::CandidateTuple), Some(open)),
        BagCountProof::Residual {
            reason: "right_open_world"
        }
    ));
}

#[test]
fn duckdb_complete_histograms_prove_positive_duplicate_and_absent_tuple_counts() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(k INTEGER); CREATE TABLE r(k INTEGER);
         INSERT INTO l VALUES (1), (1), (1), (2), (NULL), (NULL);
         INSERT INTO r VALUES (1), (3), (NULL), (NULL), (NULL);",
    )
    .expect("SQL seed");

    for (rule, keyword) in [
        (SetMultiplicityRule::Sum, "UNION ALL"),
        (SetMultiplicityRule::UnionDistinct, "UNION"),
        (SetMultiplicityRule::Minimum, "INTERSECT ALL"),
        (SetMultiplicityRule::IntersectDistinct, "INTERSECT"),
        (SetMultiplicityRule::SaturatingDifference, "EXCEPT ALL"),
        (SetMultiplicityRule::ExceptDistinct, "EXCEPT"),
    ] {
        let mut histogram_sum = 0;
        for (index, literal) in ["1", "2", "3", "NULL"].iter().enumerate() {
            let l_count = observed(
                &db,
                &format!("SELECT COUNT(*) FROM l WHERE k IS NOT DISTINCT FROM {literal}"),
            );
            let r_count = observed(
                &db,
                &format!("SELECT COUNT(*) FROM r WHERE k IS NOT DISTINCT FROM {literal}"),
            );
            let tuple = BagTupleIdentity::new(index as u64);
            let l = exact(l_count, BagScope::CandidateTuple)
                .with_tuple_identity(tuple)
                .with_source(BagSourceIdentity::new("l", "l").expect("physical source"));
            let r = exact(r_count, BagScope::CandidateTuple)
                .with_tuple_identity(tuple)
                .with_source(BagSourceIdentity::new("r", "r").expect("physical source"));
            let expected = checked_count(BagLaw::SetTuple(rule).transfer(l, Some(r)));
            let actual = observed(
                &db,
                &format!(
                    "SELECT COUNT(*) FROM
                     (SELECT k FROM l {keyword} SELECT k FROM r) AS output
                     WHERE k IS NOT DISTINCT FROM {literal}"
                ),
            );
            assert_eq!(expected, actual, "{keyword} candidate {literal}");
            histogram_sum += expected;
        }
        assert_eq!(
            histogram_sum,
            observed(
                &db,
                &format!("SELECT COUNT(*) FROM (SELECT k FROM l {keyword} SELECT k FROM r) AS output"),
            ),
            "{keyword} complete histogram must account for every result tuple"
        );
    }
}

#[test]
fn duckdb_group_rank_and_multirow_write_counts_preserve_cardinality() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE t(id INTEGER, k INTEGER);
         INSERT INTO t VALUES (1, 1), (2, 1), (3, 2), (4, NULL), (5, NULL);",
    ).expect("SQL setup");

    let count = |q: &str| observed(&db, q);
    let grouping = BagLaw::GroupKey;
    for (literal, source_count) in [("1", 2), ("2", 1), ("NULL", 2)] {
        let actual = count(&format!(
            "SELECT COUNT(*) FROM (SELECT k FROM t GROUP BY k) AS result
             WHERE k IS NOT DISTINCT FROM {literal}"
        ));
        assert_eq!(
            checked_count(grouping.transfer(exact(source_count, BagScope::CandidateTuple), None)),
            actual
        );
    }
    let rank = BagLaw::RankedPrefix { limit: 2, strict_total_order: true };
    let actual = count("SELECT COUNT(*) FROM
        (SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS position FROM t)
        WHERE position <= 2");
    assert_eq!(
        checked_count(rank.transfer(exact(5, BagScope::CompleteRelation), None)),
        actual
    );

    db.execute_batch("DELETE FROM t WHERE id IN (2, 4)").expect("delete");
    assert_eq!(
        checked_count(BagLaw::DeleteRows.transfer(
            exact(5, BagScope::CompleteRelation),
            Some(exact(2, BagScope::CompleteRelation))
        )),
        count("SELECT COUNT(*) FROM t")
    );
    db.execute_batch("UPDATE t SET k = 99 WHERE id = 1").expect("update");
    assert_eq!(
        checked_count(BagLaw::UpdateRows.transfer(
            exact(3, BagScope::CompleteRelation),
            Some(exact(1, BagScope::CompleteRelation))
        )),
        count("SELECT COUNT(*) FROM t")
    );
    db.execute_batch("INSERT INTO t VALUES (6, 1), (7, NULL)").expect("append");
    assert_eq!(
        checked_count(BagLaw::AppendRows.transfer(
            exact(3, BagScope::CompleteRelation),
            Some(exact(2, BagScope::CompleteRelation))
        )),
        count("SELECT COUNT(*) FROM t")
    );
}
