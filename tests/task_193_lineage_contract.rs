//! TASK-193: independent case-owned relational kind, source identity, and
//! projection lineage obligations across all 13 exposed parser dialects.
//! Full-document equivalence alone cannot prove these facts.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, ConfiguredSqlInput,
    JoinKind, ProtocolStatement, RelationCatalog, SqlInput,
};

const MATRIX: &str = include_str!("../docs/task-193-source-instance-matrix-v2.json");
const FREEZE: &str = include_str!("../docs/task-193-acceptance-freeze.json");

fn join_kind(kind: JoinKind) -> &'static str {
    match kind {
        JoinKind::Inner => "inner",
        JoinKind::Left => "left",
        JoinKind::Right => "right",
        JoinKind::Full => "full",
        JoinKind::Cross => "cross",
        JoinKind::LeftSemi => "left_semi",
        JoinKind::RightSemi => "right_semi",
        JoinKind::LeftAnti => "left_anti",
        JoinKind::RightAnti => "right_anti",
        JoinKind::Unknown => "unknown",
    }
}

fn complete_source_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("project dialect");
    let ddl = SqlInput::inline(
        "CREATE TABLE orders(id INTEGER, amount INTEGER);
         CREATE TABLE customers(id INTEGER, amount INTEGER);
         CREATE TABLE products(id INTEGER)",
    );
    let schemas = analyze_inputs(&[ddl], "duckdb", dialect.as_ref())
        .expect("authoritative complete physical schemas");
    RelationCatalog::from_schemas(schemas.source_schemas()).expect("typed physical relations")
}

#[test]
fn all_312_join_cells_preserve_expected_join_kinds_sources_and_lineage() {
    let expected: Value = serde_json::from_str(MATRIX).expect("manual original SQL obligations");
    let freeze: Value = serde_json::from_str(FREEZE).expect("immutable v1 acceptance");
    assert_eq!(expected["case_count"], 24);
    let mut checked = BTreeSet::new();
    let catalog = complete_source_catalog();
    for case in expected["rows"].as_array().expect("24 SQL variants") {
        let id = case["case_id"].as_str().expect("fixture identity");
        let original = freeze["cases"]
            .as_array()
            .expect("original 24 SQL")
            .iter()
            .find(|original| original["id"] == id)
            .expect("original SQL variant");
        assert_eq!(case["original_sql"], original["sql"]);
        let sql = case["original_sql"].as_str().expect("literal original SQL");
        let want_joins: Vec<String> =
            serde_json::from_value(case["expected_public_join_kinds"].clone())
                .expect("manually expected SQL join operator sequence");
        let want_deps: BTreeSet<String> =
            serde_json::from_value(case["expected_physical_dependencies"].clone())
                .expect("manually expected physical source identities");
        let want_lineage = case["expected_projection_lineage"]
            .as_array()
            .expect("manually expected projection lineage");
        let rule = case["projection_lineage_rule"]
            .as_str()
            .expect("lineage rule");
        for dialect in freeze["dialects"].as_array().expect("13 public parsers") {
            let dialect_name = dialect.as_str().expect("dialect");
            let identity = format!("{id}/{dialect_name}");
            assert!(
                checked.insert(identity.clone()),
                "{identity}: repeated cell"
            );
            let parser = dialect_from_name(dialect_name).expect("project dialect");
            let input = SqlInput::inline(sql);
            let configured =
                [ConfiguredSqlInput::new(&identity, &input, dialect_name, parser.as_ref())];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|e| panic!("{identity}: typed original SQL: {e}"));
            let [ProtocolStatement::Query(query)] = bundle.inputs()[0].statements() else {
                panic!("{identity}: query cannot disappear");
            };
            let actual_joins = query
                .joins()
                .iter()
                .map(|join| join_kind(join.kind()).to_string())
                .collect::<Vec<_>>();
            assert_eq!(
                actual_joins, want_joins,
                "{identity}: independently specified relational join operators"
            );
            let actual_deps = query
                .dependencies()
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            assert_eq!(
                actual_deps, want_deps,
                "{identity}: exact shared physical sources"
            );
            let output = query.output().columns();
            assert_eq!(output.len(), want_lineage.len(), "{identity}: output arity");
            for (column, expected) in output.iter().zip(want_lineage) {
                let allowed = expected
                    .as_array()
                    .expect("lineage alternatives")
                    .iter()
                    .map(|v| v.as_str().expect("physical relation.column"))
                    .collect::<BTreeSet<_>>();
                let actual = column
                    .lineage()
                    .iter()
                    .map(|source| format!("{}.{}", source.relation(), source.column()))
                    .collect::<BTreeSet<_>>();
                if rule == "exact" {
                    assert_eq!(
                        actual,
                        allowed.iter().map(|s| s.to_string()).collect(),
                        "{identity}: exact physical output lineage"
                    );
                } else {
                    assert!(
                        !actual.is_empty()
                        && actual.iter().all(|v| allowed.contains(v.as_str())),
                        "{identity}: USING/NATURAL lineage cannot be absent or invented: {actual:?}"
                    );
                }
            }
        }
    }
    assert_eq!(checked.len(), 312);
}
