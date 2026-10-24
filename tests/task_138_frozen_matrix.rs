//! Frozen TASK-138 exact-bag, typed-domain and negative-outcome regression.
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, FiniteColumnType, LiteralValue,
    PhysicalFiniteTableProof, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;
fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-138-acceptance-freeze.json")).expect("frozen")
}
fn literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        Value::String(s) => LiteralValue::Text(s.clone()),
        other => panic!("unsupported typed frozen value: {other}"),
    }
}
fn row(value: &Value) -> BTreeMap<String, LiteralValue> {
    value
        .as_object()
        .expect("row")
        .iter()
        .map(|(key, value)| (key.clone(), literal(value)))
        .collect()
}
fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    result.sort();
    result
}
fn case_dialect(variant: &str) -> &str {
    match variant {
        "star_except" | "star_except_missing" | "star_replace" => "bigquery",
        "star_rename" => "snowflake",
        "star_exclude" | "pivot_static" | "unpivot_exclude_null" | "unpivot_include_null" => {
            "duckdb"
        }
        "pivot_dynamic" => "snowflake",
        "pivot_opaque" => "duckdb",
        _ => panic!("unreviewed variant: {variant}"),
    }
}
#[test]
fn frozen_mandatory_complete_bags_null_domains_and_absence_agree() {
    let frozen = frozen();
    assert_eq!(frozen["version"], 5);
    let mut checked = 0;
    for case in frozen["cases"].as_array().expect("cases") {
        if case["classification"] != "mandatory" {
            continue;
        }
        let sql = case["sql"].as_str().expect("SQL");
        let dialect = case_dialect(case["variant"].as_str().expect("variant"));
        let parser = dialect_from_name(dialect).expect("dialect");
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            .unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        let target = bundle.layers().last().expect("target");
        let proof = physical_finite_table_plan(&bundle, target.id(), None);
        assert_eq!(
            case["expected_proof"], "Exact and Impossible for negative tuple",
            "{sql}: mandatory case must be exact"
        );
        let PhysicalFiniteTableProof::Exact(actual) = proof else {
            panic!("{sql}: no exact physical proof: {proof:?}");
        };
        let expected = case["expected_terminal_rows"]
            .as_array()
            .expect("typed fixture")
            .iter()
            .map(row)
            .collect::<Vec<_>>();
        assert_eq!(
            sorted(&actual.terminal),
            sorted(&expected),
            "{sql}: complete bag"
        );
        assert_eq!(
            actual.row_ids,
            case["expected_terminal_row_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            "{sql}: row identity"
        );
        assert_eq!(
            actual.source_row_ids,
            case["source_row_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            "{sql}: original source identities"
        );
        let actual_source = actual
            .source_rows
            .iter()
            .map(|row| format!("{row:?}"))
            .collect::<Vec<_>>();
        let expected_source = case["source_rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|values| {
                let cols = case["source_columns"].as_array().unwrap();
                let values = values.as_array().unwrap();
                cols.iter()
                    .zip(values)
                    .map(|(name, value)| (name.as_str().unwrap().to_string(), literal(value)))
                    .collect::<BTreeMap<_, _>>()
            })
            .map(|row| format!("{row:?}"))
            .collect::<Vec<_>>();
        assert_eq!(
            actual_source, expected_source,
            "{sql}: complete physical VALUES source"
        );
        let expected_contributors = case["expected_contributors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|indices| {
                indices
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual.contributors, expected_contributors,
            "{sql}: source identity witnesses for every output row"
        );
        for (column, spec) in case["output_domains"].as_object().expect("typed domains") {
            let value = actual.terminal_domains.get(column).expect("outcome column");
            let ValueDomain::Set(set) = value else {
                panic!("{column}: expected exact finite domain, got {value:?}");
            };
            assert_eq!(set.mode(), SetMode::Include);
            let claimed_type = match spec["type"].as_str().expect("frozen scalar type") {
                "integer" => FiniteColumnType::Integer,
                "text" => FiniteColumnType::Text,
                other => panic!("{column}: unreviewed typed domain {other}"),
            };
            assert_eq!(
                actual.terminal_column_types.get(column),
                Some(&claimed_type),
                "{column}: attested output type"
            );
            if claimed_type == FiniteColumnType::Integer {
                let numeric = set
                    .values()
                    .iter()
                    .filter_map(|v| match v.value() {
                        LiteralValue::Number(n) => Some(n.parse::<i128>().expect("integer")),
                        LiteralValue::Null => None,
                        value => panic!("{column}: unexpected noninteger {value:?}"),
                    })
                    .collect::<Vec<_>>();
                let lower = spec["lower"]["value"].as_i64().map(i128::from);
                let upper = spec["upper"]["value"].as_i64().map(i128::from);
                assert_eq!(
                    numeric.iter().copied().min(),
                    lower,
                    "{column}: lower domain bound"
                );
                assert_eq!(
                    numeric.iter().copied().max(),
                    upper,
                    "{column}: upper domain bound"
                );
                if lower.is_some() {
                    assert_eq!(
                        spec["lower"]["inclusive"], true,
                        "{column}: lower bound inclusive"
                    );
                    assert_eq!(
                        spec["upper"]["inclusive"], true,
                        "{column}: upper bound inclusive"
                    );
                }
            } else {
                assert!(
                    spec["lower"].is_null() && spec["upper"].is_null(),
                    "{column}: unproven text ordering must have no numeric bound"
                );
            }

            let mut observed = set
                .values()
                .iter()
                .filter(|v| !matches!(v.value(), LiteralValue::Null))
                .map(|v| format!("{:?}", v.value()))
                .collect::<Vec<_>>();
            observed.sort();
            let mut frozen_values = spec["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| format!("{:?}", literal(v)))
                .collect::<Vec<_>>();
            frozen_values.sort();
            assert_eq!(observed, frozen_values, "{column}: complete finite domain");
            assert_eq!(
                value.admits_null(),
                spec["nullable"].as_bool(),
                "{column}: NULL"
            );
        }
        let absent = row(&case["negative_membership"]);
        assert!(
            matches!(
                physical_finite_table_plan(&bundle, target.id(), Some(&absent)),
                PhysicalFiniteTableProof::Impossible { .. }
            ),
            "{sql}: absent tuple is impossible"
        );
        checked += 1;
    }
    assert_eq!(checked, 7, "all seven mandatory exact variants checked");
}
#[test]
fn named_dialects_are_all_probed_without_inferred_native_execution() {
    let data = frozen();
    let dialects = data["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let mut seen = 0;
    for case in data["cases"].as_array().unwrap() {
        let sql = case["sql"].as_str().unwrap();
        for name in dialects {
            let dialect = name.as_str().unwrap();
            let parser = dialect_from_name(dialect).expect("canonical parser name");
            // SQL parser rejection is a distinct conditional boundary. If it
            // parses, only the canonical protocol's finite proof is assessed;
            // native vendor/session execution is never inferred from parsing.
            let analysis = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref());
            let observed = match &analysis {
                Err(_) => "parser_boundary",
                Ok(bundle) => {
                    let layer = bundle.layers().last().expect("layer");
                    match physical_finite_table_plan(bundle, layer.id(), None) {
                        PhysicalFiniteTableProof::Exact(_) => "exact",
                        PhysicalFiniteTableProof::Impossible { .. } => "impossible",
                        PhysicalFiniteTableProof::Residual { .. } => "residual",
                    }
                }
            };
            let stored = case["dialect_matrix"]
                .as_array()
                .expect("13 frozen dialect cells")
                .iter()
                .find(|cell| cell["dialect"] == dialect)
                .expect("named dialect");
            assert_eq!(stored["classification"], observed, "{dialect}: {sql}");

            if let Ok(bundle) = analysis {
                let layer = bundle.layers().last().expect("layer");
                let proof = physical_finite_table_plan(&bundle, layer.id(), None);
                if case["classification"] == "mandatory"
                    && case["expected_proof"] == "Exact and Impossible for negative tuple"
                {
                    assert!(
                        matches!(proof, PhysicalFiniteTableProof::Exact(_)),
                        "{dialect}: parsed mandatory {sql} but no exact source/terminal proof: {proof:?}"
                    );
                } else {
                    assert!(
                        matches!(proof, PhysicalFiniteTableProof::Residual { .. }),
                        "{dialect}: invalid or conditional variant must fail closed: {proof:?}"
                    );
                }
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 130);
}

#[test]
fn every_conditional_has_an_explicit_reviewed_fail_closed_release_disposition() {
    let inventory = frozen();
    let mut seen = 0;
    for case in inventory["cases"].as_array().unwrap() {
        if case["classification"] != "conditional" {
            continue;
        }
        let id = case["id"].as_str().expect("fixture id");
        assert_eq!(
            case["release_disposition"]["owner"], "TASK-138",
            "{id}: owner"
        );
        assert_eq!(
            case["release_disposition"]["status"], "reviewed_conditional_residual",
            "{id}: reviewer disposition"
        );
        assert_eq!(
            case["required_complete_physical_proof"], false,
            "{id}: no invalid mandatory Residual"
        );
        assert!(
            case["release_disposition"]["reason"]
                .as_str()
                .unwrap()
                .len()
                > 35
        );
        let dialect = case_dialect(case["variant"].as_str().unwrap());
        let sql = case["sql"].as_str().unwrap();
        let parser = dialect_from_name(dialect).unwrap();
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            .expect("conditional SQL parses");
        let layer = bundle.layers().last().expect("layer");
        assert!(
            matches!(
                physical_finite_table_plan(&bundle, layer.id(), None),
                PhysicalFiniteTableProof::Residual { .. }
            ),
            "{id}: no invented exact row bag"
        );
        seen += 1;
    }
    assert_eq!(
        seen, 3,
        "all three reviewable residual families explicitly classified"
    );
}
