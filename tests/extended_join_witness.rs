//! DuckDB-backed regression tests for typed extended-join source witnesses.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sqlparser::parser::Parser;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, AnalysisBundle,
    ComposedSemantics, ConfiguredSqlInput, JoinSide, JoinWitness, JoinWitnessDirection,
    JoinWitnessShape, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn schema(name: &str) -> RelationSchema {
    RelationSchema::new(
        name,
        ["id", "k"]
            .into_iter()
            .map(|column| {
                SchemaColumn::from_sql_type(column, "BIGINT", "postgresql").expect("integer type")
            })
            .collect(),
    )
    .expect("source schema")
}

fn analyze(sql: &str, dialect: &str) -> AnalysisBundle {
    let catalog = RelationCatalog::from_schemas(&[schema("l"), schema("r"), schema("orders")])
        .expect("catalog");
    let parser = dialect_from_name(dialect).expect("dialect");
    let sql_input = SqlInput::inline(sql);
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "join-witness",
            &sql_input,
            dialect,
            parser.as_ref(),
        )],
        &catalog,
    )
    .unwrap_or_else(|error| panic!("analysis for {dialect} {sql}: {error}"))
}

/// Exercise each exposed dialect when its parser accepts a syntax variant.
fn analyze_if_parsed(sql: &str, dialect: &str) -> Option<AnalysisBundle> {
    let parser = dialect_from_name(dialect).expect("documented dialect");
    match Parser::parse_sql(parser.as_ref(), sql) {
        Ok(_) => Some(analyze(sql, dialect)),
        Err(error) => {
            eprintln!("{dialect} rejects join syntax at the parser boundary: {error}");
            None
        }
    }
}

fn witnesses(bundle: &AnalysisBundle) -> &[JoinWitness] {
    match bundle
        .layers()
        .last()
        .expect("query layer")
        .composed_semantics()
    {
        ComposedSemantics::Resolved(semantics) => semantics.join_witnesses(),
        other => panic!("unresolved composed semantics: {other:?}"),
    }
}

fn shapes(direction: &JoinWitnessDirection) -> Vec<JoinWitnessShape> {
    match direction {
        JoinWitnessDirection::Exact(cases) => cases.iter().map(|case| case.shape()).collect(),
        other => panic!("expected exact direction, got {other:?}"),
    }
}

fn oracle() -> Connection {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l (id BIGINT, k BIGINT);
         CREATE TABLE r (id BIGINT, k BIGINT);
         INSERT INTO l VALUES (1,1),(2,2),(3,NULL),(4,3),(5,3);
         INSERT INTO r VALUES (11,1),(12,1),(13,4),(14,NULL);",
    )
    .expect("seed source relations");
    db
}

fn count(db: &Connection, from: &str) -> i64 {
    db.query_row(&format!("SELECT COUNT(*) FROM {from}"), [], |row| {
        row.get(0)
    })
    .expect("DuckDB witness count")
}

#[test]
fn left_right_and_full_preserve_unmatched_null_and_duplicate_rows() {
    let db = oracle();
    for (kind, sql, expected_count, expected_qualifying, expected_rejected, extension) in [
        (
            "left",
            "l LEFT JOIN r ON l.k = r.k",
            6,
            vec![JoinWitnessShape::Matched, JoinWitnessShape::LeftUnmatched],
            vec![JoinWitnessShape::RightUnmatched],
            Some(JoinSide::Right),
        ),
        (
            "right",
            "l RIGHT JOIN r ON l.k = r.k",
            4,
            vec![JoinWitnessShape::Matched, JoinWitnessShape::RightUnmatched],
            vec![JoinWitnessShape::LeftUnmatched],
            Some(JoinSide::Left),
        ),
        (
            "full",
            "l FULL JOIN r ON l.k = r.k",
            8,
            vec![
                JoinWitnessShape::Matched,
                JoinWitnessShape::LeftUnmatched,
                JoinWitnessShape::RightUnmatched,
            ],
            vec![],
            None,
        ),
    ] {
        assert_eq!(count(&db, sql), expected_count, "{kind}");
        for &dialect in DIALECTS {
            let query = format!("SELECT l.id FROM {sql}");
            let bundle = analyze_if_parsed(&query, dialect)
                .unwrap_or_else(|| panic!("{dialect} must parse standard {kind} JOIN syntax"));
            let [witness] = witnesses(&bundle) else {
                panic!("expected one {kind} witness in {dialect}");
            };
            assert_eq!(shapes(witness.qualifying()), expected_qualifying);
            if kind == "full" {
                assert_eq!(witness.rejected(), &JoinWitnessDirection::Impossible);
            } else {
                assert_eq!(shapes(witness.rejected()), expected_rejected);
                let JoinWitnessDirection::Exact(cases) = witness.qualifying() else {
                    panic!("qualifying must be exact");
                };
                assert_eq!(cases[1].null_extended_side(), extension);
            }
            let JoinWitnessDirection::Exact(cases) = witness.qualifying() else {
                panic!("matching obligation");
            };
            assert_eq!(cases[0].shape().min_matches(), 1);
            assert_eq!(cases[0].shape().max_matches(), None);
            let json: serde_json::Value =
                serde_json::from_str(&to_bundle_json(&bundle)).expect("valid emitted JSON");
            assert!(json["layers"][0]["composed_semantics"]["join_witnesses"].is_array());
        }
    }
}

#[test]
fn semi_and_anti_witnesses_are_duplicate_insensitive() {
    let db = oracle();
    for (sql, expected_count, qualifying, rejected) in [
        (
            "l SEMI JOIN r ON l.k = r.k",
            1,
            JoinWitnessShape::Matched,
            JoinWitnessShape::LeftUnmatched,
        ),
        (
            "l ANTI JOIN r ON l.k = r.k",
            4,
            JoinWitnessShape::LeftUnmatched,
            JoinWitnessShape::Matched,
        ),
    ] {
        assert_eq!(count(&db, sql), expected_count);
        let query = format!("SELECT l.id FROM {sql}");
        for &dialect in DIALECTS {
            let Some(bundle) = analyze_if_parsed(&query, dialect) else {
                continue;
            };
            let [witness] = witnesses(&bundle) else {
                panic!("one semi/anti witness in {dialect}")
            };
            assert_eq!(shapes(witness.qualifying()), vec![qualifying], "{dialect}");
            assert_eq!(shapes(witness.rejected()), vec![rejected], "{dialect}");
        }
    }
}

#[test]
fn inequality_matches_and_nulls_follow_three_valued_on_logic() {
    let db = oracle();
    assert_eq!(count(&db, "l LEFT JOIN r ON l.k < r.k"), 5);
    let query = "SELECT l.id FROM l LEFT JOIN r ON l.k < r.k";
    for &dialect in DIALECTS {
        let bundle = analyze_if_parsed(query, dialect)
            .unwrap_or_else(|| panic!("{dialect} must parse standard inequality JOIN syntax"));
        let [witness] = witnesses(&bundle) else {
            panic!("inequality witness in {dialect}")
        };
        assert_eq!(
            witness.comparison(),
            Some(sql_semantic_protocol::ComparisonOperator::Lt),
            "{dialect}"
        );
        assert_eq!(
            shapes(witness.qualifying()),
            vec![JoinWitnessShape::Matched, JoinWitnessShape::LeftUnmatched],
            "{dialect}"
        );
    }
}

#[test]
fn self_join_preserves_distinct_physical_source_instances() {
    let db = oracle();
    assert_eq!(count(&db, "l a LEFT JOIN l b ON a.k = b.k"), 7);
    let query = "SELECT a.id FROM l a LEFT JOIN l b ON a.k = b.k";
    for &dialect in DIALECTS {
        let bundle = analyze_if_parsed(query, dialect)
            .unwrap_or_else(|| panic!("{dialect} must parse standard self JOIN syntax"));
        let [witness] = witnesses(&bundle) else {
            panic!("self-join witness in {dialect}")
        };
        let left = witness.left().expect("left endpoint");
        let right = witness.right().expect("right endpoint");
        assert_eq!(left.relation(), "l", "{dialect}");
        assert_eq!(right.relation(), "l", "{dialect}");
        assert_eq!(left.relation_instance(), "a", "{dialect}");
        assert_eq!(right.relation_instance(), "b", "{dialect}");
        assert_ne!(left, right, "{dialect}");
    }
}

#[test]
fn repeated_join_tree_and_complex_on_remain_residual() {
    for sql in [
        "SELECT l.id FROM l JOIN r ON l.k = r.k AND l.id <> r.id",
        "SELECT l.id FROM l JOIN r ON l.k = r.k OR l.id = r.id",
        "SELECT a.id FROM l a JOIN r ON a.k = r.k JOIN l b ON b.k = r.k",
    ] {
        let bundle = analyze(sql, "duckdb");
        assert!(witnesses(&bundle)
            .iter()
            .all(|witness| matches!(witness.qualifying(), JoinWitnessDirection::Residual { .. })));
    }
}

#[test]
fn composed_witness_survives_named_upstream_layers() {
    let sql = "CREATE TABLE stage AS SELECT id, k FROM l;
        CREATE TABLE mart AS SELECT stage.id FROM stage LEFT JOIN r ON stage.k = r.k";
    let bundle = analyze(sql, "duckdb");
    let last = witnesses(&bundle);
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].left().expect("physical left").relation(), "l");
    assert_eq!(last[0].right().expect("physical right").relation(), "r");
}

#[test]
fn filtered_upstream_rows_do_not_become_exact_physical_join_witnesses() {
    let db = oracle();
    db.execute_batch("CREATE TABLE stage AS SELECT id, k FROM r WHERE id > 12")
        .expect("filtered producer");
    assert_eq!(count(&db, "l LEFT JOIN r ON l.k = r.k"), 6);
    assert_eq!(count(&db, "l LEFT JOIN stage ON l.k = stage.k"), 5);

    let sql = "CREATE TABLE stage AS SELECT id, k FROM r WHERE id > 12;
        CREATE TABLE mart AS SELECT l.id FROM l LEFT JOIN stage ON l.k = stage.k";
    let bundle = analyze(sql, "duckdb");
    let [witness] = witnesses(&bundle) else {
        panic!("filtered upstream join witness");
    };
    assert!(matches!(
        witness.qualifying(),
        JoinWitnessDirection::Residual { reason }
            if reason == "upstream_row_membership_not_preserved"
    ));
    assert!(matches!(witness.rejected(), JoinWitnessDirection::Residual { .. }));
}

#[test]
fn exact_physical_join_witness_survives_multiple_plain_copy_producers() {
    let sql = "CREATE TABLE stage0 AS SELECT id, k FROM l;
        CREATE TABLE stage1 AS SELECT id, k FROM stage0;
        CREATE TABLE mart AS SELECT stage1.id FROM stage1 LEFT JOIN r ON stage1.k = r.k";
    let bundle = analyze(sql, "duckdb");
    let [witness] = witnesses(&bundle) else {
        panic!("plain-copy join witness");
    };
    assert_eq!(witness.left().expect("physical left").relation(), "l");
    assert_eq!(witness.right().expect("physical right").relation(), "r");
    assert!(matches!(witness.qualifying(), JoinWitnessDirection::Exact(_)));
}

#[test]
fn upstream_row_shaping_cannot_be_flattened_into_exact_source_obligations() {
    for sql in [
        "CREATE TABLE stage AS SELECT DISTINCT k FROM r;
         CREATE TABLE mart AS SELECT l.id FROM l LEFT JOIN stage ON l.k = stage.k",
        "CREATE TABLE stage AS SELECT id, k FROM r LIMIT 1;
         CREATE TABLE mart AS SELECT l.id FROM l LEFT JOIN stage ON l.k = stage.k",
    ] {
        let bundle = analyze(sql, "duckdb");
        let [witness] = witnesses(&bundle) else {
            panic!("row-shaping upstream witness");
        };
        assert!(matches!(witness.qualifying(), JoinWitnessDirection::Residual { .. }));
    }
}
