//! TASK-114: independently observe all 13 parser-dialect variants.
//! The classification is intentionally stricter than "SQL parsed".
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, ConfiguredSqlInput, JoinKind,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const MATRIX: &str = include_str!("../docs/task-114-acceptance-freeze.json");

fn observed(variant: &str, dialect: &str, sql: &str) -> &'static str {
    let parser = dialect_from_name(dialect).unwrap();
    let schemas = ["l", "r"]
        .iter()
        .map(|name| {
            RelationSchema::new(
                *name,
                vec![
                    SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("t", "INTEGER", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let source = SqlInput::inline(sql);
    let input = ConfiguredSqlInput::new("task114-dialect", &source, dialect, parser.as_ref());
    let bundle = match analyze_configured_inputs_with_catalog(&[input], &catalog) {
        Ok(bundle) => bundle,
        Err(_) => return "parser_boundary",
    };
    let query = bundle.inputs()[0]
        .statements()
        .iter()
        .find_map(|statement| match statement {
            ProtocolStatement::Query(q) => Some(q),
            _ => None,
        });
    let Some(query) = query else {
        return "parsed_not_query";
    };
    let codes = query
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect::<Vec<_>>();
    match variant {
        "asof_nearest_prior" if codes.contains(&"unsupported_asof_nearest_prior") => {
            "parsed_asof_residual"
        }
        "lateral_derived" if codes.contains(&"unsupported_lateral_row_scope") => {
            "parsed_lateral_residual"
        }
        "cross_apply" if codes.contains(&"unsupported_cross_apply") => {
            "parsed_cross_apply_residual"
        }
        "outer_apply" if codes.contains(&"unsupported_outer_apply") => {
            "parsed_outer_apply_residual"
        }
        "range_inequality" | "time_inequality"
            if query.joins().len() == 1 && query.joins()[0].kind() == JoinKind::Inner =>
        {
            if codes.is_empty() {
                "parser_equivalent"
            } else {
                "parsed_with_residual"
            }
        }
        _ => "parsed_unclassified",
    }
}

#[test]
fn every_frozen_dialect_cell_has_a_reviewed_observed_classification() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let dialects = matrix["dialects"].as_array().unwrap();
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    assert_eq!(cases.len(), 12);
    let mut observations = Vec::new();
    for case in cases {
        let variant = case["variant"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let mut cells = serde_json::Map::new();
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let classification = observed(variant, name, sql);
            cells.insert(name.to_owned(), json!(classification));
        }
        observations.push(json!({"id":case["id"],"classification":cells}));
    }
    // Run this oracle first to collect parser observations. Freeze the literal
    // review once, then assert equality against the versioned matrix.
    for (observed, frozen) in observations.iter().zip(cases) {
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            assert_eq!(
                observed["classification"][name],
                frozen["dialects"][name]["parser"],
                "TASK114_OBSERVATIONS={}\n{} in {name}",
                Value::Array(observations.clone()),
                frozen["id"]
            );
        }
    }
}

#[test]
fn conditional_vendor_laws_have_named_missing_evidence_and_pending_release_reviews() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let boundaries = matrix["conditional_boundaries"].as_array().unwrap();
    assert_eq!(boundaries.len(), 5);
    for id in [
        "J114-ASOF-TIES",
        "J114-TEMPORAL-SESSION",
        "J114-LATERAL-CORRELATION",
        "J114-APPLY-OUTER",
        "J114-APPLY-PARSER",
    ] {
        let boundary = boundaries.iter().find(|item| item["id"] == id).unwrap();
        assert_eq!(boundary["owner"], "TASK-114");
        assert_eq!(boundary["classification"], "conditional");
        assert_eq!(boundary["disposition"], "pending_release_review");
        assert_eq!(boundary["native_engine_certified"], false);
        assert!(boundary["missing_evidence"]
            .as_str()
            .is_some_and(|evidence| !evidence.trim().is_empty()));
    }
}
