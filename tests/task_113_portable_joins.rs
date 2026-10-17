//! TASK-113: portable joins under a complete, independently frozen physical SQL bag.
//! Public semantic analysis and DuckDB execution are the test seams.

use duckdb::{params, Connection};
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ComposedSemantics, ConfiguredSqlInput, JoinKind, JoinWitnessDirection, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessObligation,
};

const MATRIX: &str = include_str!("../docs/task-113-acceptance-freeze.json");

fn analyzed(
    sql: &str,
    dialect: &str,
    complete_schema: bool,
) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            let id = if relation == "l" { "lid" } else { "rid" };
            let mut columns =
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()];
            if complete_schema {
                columns.push(SchemaColumn::from_sql_type(id, "VARCHAR", "postgresql").unwrap());
            }
            RelationSchema::new(relation, columns).unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let parser = dialect_from_name(dialect).unwrap();
    let source = SqlInput::inline(sql);
    let input = ConfiguredSqlInput::new("task-113", &source, dialect, parser.as_ref());
    analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap()
}

#[test]
fn duckdb_executes_every_frozen_join_bag_and_truth_partition() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    assert_eq!(matrix["cases"].as_array().unwrap().len(), 14);
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE l(lid VARCHAR, k INTEGER); CREATE TABLE r(rid VARCHAR, k INTEGER);",
        )
        .unwrap();
        for (relation, entries) in [("l", &case["left"]), ("r", &case["right"])] {
            for entry in entries.as_array().unwrap() {
                db.execute(
                    &format!("INSERT INTO {relation} VALUES (?, ?)"),
                    params![entry[0].as_str().unwrap(), entry[1].as_i64()],
                )
                .unwrap();
            }
        }
        let mut stmt = db.prepare(case["duckdb_sql"].as_str().unwrap()).unwrap();
        let mut actual = stmt
            .query_map([], |row| {
                let lid: String = row.get(0)?;
                let lk: Option<i64> = row.get(1)?;
                let rid: String = row.get(2)?;
                let rk: Option<i64> = row.get(3)?;
                Ok(json!([lid, lk, rid, rk]).to_string())
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        let mut expected = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(
            expected.len() as u64,
            case["expected_output_count"].as_u64().unwrap(),
            "{id}: frozen output count must match the complete bag"
        );
        assert_eq!(
            actual, expected,
            "{id}: full terminal bag, row identity, and NULL"
        );

        let predicate = match case["variant"].as_str().unwrap() {
            "cross" => "TRUE",
            "using" | "natural" => "l.k = r.k",
            "or_on" => "l.k = r.k OR l.k IS NULL",
            "non_equi" => "l.k < r.k",
            "is_not_distinct_from" | "null_safe_equality" => "l.k IS NOT DISTINCT FROM r.k",
            other => panic!("unfrozen variant: {other}"),
        };
        let sql =
            format!("SELECT l.lid, r.rid FROM l CROSS JOIN r WHERE ({predicate}) IS NOT TRUE");
        let mut rejection = db.prepare(&sql).unwrap();
        let mut actual_rejected = rejection
            .query_map([], |row| {
                let a: String = row.get(0)?;
                let b: String = row.get(1)?;
                Ok(json!([a, b]).to_string())
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        let mut expected_rejected = case["not_true_pairs"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>();
        actual_rejected.sort();
        expected_rejected.sort();
        assert_eq!(
            actual_rejected, expected_rejected,
            "{id}: every NOT TRUE pair"
        );

        // A complete join oracle proves both existence and closed-world absence:
        // no physical source pair may disappear from, or occur twice in, the
        // TRUE / IS NOT TRUE partition. Keep row identities, not set counts.
        let mut all_source_pairs = case["left"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|left| {
                case["right"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(move |right| json!([left[0], right[0]]).to_string())
            })
            .collect::<Vec<_>>();
        let mut classified_pairs = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| json!([row[0], row[2]]).to_string())
            .chain(expected_rejected.iter().cloned())
            .collect::<Vec<_>>();
        all_source_pairs.sort();
        classified_pairs.sort();
        assert_eq!(
            classified_pairs, all_source_pairs,
            "{id}: every physical pair must occur exactly once as TRUE or NOT TRUE"
        );

        let values = case["expected"].as_array().unwrap();
        let numeric = values
            .iter()
            .filter_map(|v| v[1].as_i64())
            .collect::<Vec<_>>();
        let domain = &case["output_domain"];
        assert_eq!(
            numeric.iter().min().copied(),
            domain["minimum"].as_i64(),
            "{id}: inclusive minimum"
        );
        assert_eq!(
            numeric.iter().max().copied(),
            domain["maximum"].as_i64(),
            "{id}: inclusive maximum"
        );
        assert_eq!(
            values.iter().any(|row| row[1].is_null()),
            domain["contains_null"],
            "{id}: NULL"
        );
        assert_eq!(values.is_empty(), domain["empty"], "{id}: empty");
        if !numeric.is_empty() {
            assert_eq!(domain["lower_inclusive"], true, "{id}: minimum included");
            assert_eq!(domain["upper_inclusive"], true, "{id}: maximum included");
        }
    }
}

#[test]
fn natural_join_known_source_column_intersection_produces_exact_join_witness() {
    let sql = "SELECT l.k FROM l NATURAL JOIN r";
    let b = analyzed(sql, "generic", true);
    let layer = &b.layers()[0];
    let ComposedSemantics::Resolved(resolved) = layer.composed_semantics() else {
        panic!("NATURAL JOIN with a known k column on both inputs must resolve");
    };
    let [join] = resolved.join_witnesses() else {
        panic!("one join witness required");
    };
    assert_eq!(join.kind(), JoinKind::Inner);
    assert!(matches!(join.qualifying(), JoinWitnessDirection::Exact(_)));
    assert_eq!(join.left().unwrap().column(), "k");
    assert_eq!(join.right().unwrap().column(), "k");
    let physical = physical_joint_source_plan(&b, &[(layer.id(), 1)]);
    let WitnessDirection::Feasible(cases) = physical.outcome() else {
        panic!("same physical join-key assignment required: {physical:?}");
    };
    assert!(cases.iter().any(|case| {
        case.obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 2
    }));
}

#[test]
fn natural_join_unknown_column_intersection_fails_closed() {
    let dialect = sqlparser::dialect::GenericDialect {};
    let analyzed =
        sql_semantic_protocol::analyze_sql("SELECT l.k FROM l NATURAL JOIN r", "generic", &dialect)
            .unwrap();
    let statement = analyzed
        .statements()
        .iter()
        .find_map(|statement| match statement {
            sql_semantic_protocol::ProtocolStatement::Query(query) => Some(query),
            _ => None,
        })
        .unwrap();
    assert!(statement
        .diagnostics()
        .iter()
        .any(|d| d.code() == "unsupported_natural_join_condition"));
}

#[test]
fn natural_join_with_disjoint_known_columns_is_cartesian() {
    let left = RelationSchema::new(
        "l",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let right = RelationSchema::new(
        "r",
        vec![SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[left, right]).unwrap();
    let parser = dialect_from_name("generic").unwrap();
    let sql = SqlInput::inline("SELECT l.k FROM l NATURAL JOIN r");
    let input = ConfiguredSqlInput::new(
        "task-113-empty-intersection",
        &sql,
        "generic",
        parser.as_ref(),
    );
    let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
    let layer = &bundle.layers()[0];
    let query = bundle.inputs()[0]
        .statements()
        .iter()
        .find_map(|statement| match statement {
            sql_semantic_protocol::ProtocolStatement::Query(query) => Some(query),
            _ => None,
        })
        .unwrap();
    assert!(query.diagnostics().is_empty());
    assert_eq!(query.joins()[0].kind(), JoinKind::Cross);
    assert!(query.joins()[0].condition().is_none());
    assert_eq!(layer.consumes().len(), 2);
}

#[test]
fn missing_natural_schema_cannot_be_promoted_to_a_physical_proof() {
    let catalog = RelationCatalog::new(&["l", "r"]).unwrap();
    let parser = dialect_from_name("generic").unwrap();
    let sql = SqlInput::inline("SELECT l.k FROM l NATURAL JOIN r");
    let input = ConfiguredSqlInput::new("task-113-unknown", &sql, "generic", parser.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
    let layer = bundle.layers().last().unwrap().id();
    let physical = physical_joint_source_plan(&bundle, &[(layer, 1)]);
    assert!(
        matches!(physical.outcome(), WitnessDirection::Residual { .. }),
        "unknown NATURAL intersection must remain a typed residual: {physical:?}"
    );
}

#[test]
fn every_conditional_boundary_has_an_owned_pending_release_disposition() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let boundaries = matrix["conditional_boundaries"].as_array().unwrap();
    assert_eq!(boundaries.len(), 3);
    for id in [
        "J113-VENDOR-COLLATION",
        "J113-NATURAL-UNKNOWN-SCHEMA",
        "J113-NULLSAFE-PARSER",
    ] {
        let boundary = boundaries.iter().find(|entry| entry["id"] == id).unwrap();
        assert_eq!(boundary["owner"], "TASK-113");
        assert_eq!(boundary["classification"], "conditional");
        assert_eq!(boundary["disposition"], "pending_release_review");
        assert!(
            boundary["missing_evidence"]
                .as_str()
                .is_some_and(|evidence| !evidence.trim().is_empty()),
            "{id}: missing evidence must be named"
        );
    }
}

#[test]
fn every_portable_frozen_variant_has_a_typed_complete_source_plan() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let variant = case["variant"].as_str().unwrap();
        let dialect = if variant == "null_safe_equality" {
            "mysql"
        } else {
            "generic"
        };
        let bundle = analyzed(case["sql"].as_str().unwrap(), dialect, true);
        let layer = bundle.layers().last().unwrap().id();
        let wanted = case["expected_output_count"].as_u64().unwrap();
        let physical = physical_joint_source_plan(&bundle, &[(layer, wanted)]);
        let WitnessDirection::Feasible(cases) = physical.outcome() else {
            panic!(
                "{id}: expected a canonical, complete, schema-backed physical proof: {physical:?}"
            );
        };
        let expected_uniform = matches!(
            variant,
            "cross" | "or_on" | "non_equi" | "is_not_distinct_from" | "null_safe_equality"
        );
        assert!(
            cases.iter().any(|candidate| {
                let obligations = candidate.obligations();
                let closed = obligations
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count()
                    == 2;
                let matched_law = obligations.iter().any(|obligation| match obligation {
                    WitnessObligation::UniformJoinPopulation {
                        left_row,
                        right_row,
                        left_rows,
                        right_rows,
                        output_rows,
                        closed_world: true,
                        ..
                    } if expected_uniform => {
                        left_row.relation() == "l"
                            && right_row.relation() == "r"
                            && *output_rows == wanted
                            && (left_rows * right_rows >= wanted)
                    }
                    WitnessObligation::JoinPopulation {
                        left_row,
                        right_row,
                        output_rows,
                        closed_world: true,
                        ..
                    } if !expected_uniform => {
                        left_row.relation() == "l"
                            && right_row.relation() == "r"
                            && *output_rows == wanted
                    }
                    _ => false,
                });
                closed && matched_law
            }),
            "{id}: require one complete matched/NOT TRUE physical pair population"
        );
    }
}
