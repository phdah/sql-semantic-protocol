//! TASK-134 mandatory frozen acceptance matrix at public analysis and physical proof seams.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    physical_finite_page_membership_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-134-acceptance-freeze.json")).unwrap()
}
fn scalar(value: &Value) -> LiteralValue {
    value
        .as_i64()
        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
}
fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap()
}
fn analyze(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task134",
        &input,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap()
}
fn source(case: &Value) -> PhysicalWriteState {
    let rows = case["source_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), scalar(value))])).unwrap()
        })
        .collect();
    PhysicalWriteState::new("s", rows).unwrap()
}
fn str_bag(values: impl IntoIterator<Item = LiteralValue>) -> Vec<String> {
    let mut bag = values
        .into_iter()
        .map(|v| format!("{v:?}"))
        .collect::<Vec<_>>();
    bag.sort();
    bag
}
fn oracle(case: &Value) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER)").unwrap();
    for value in case["source_bag"].as_array().unwrap() {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({})",
            value.as_i64().map_or("NULL".to_string(), |v| v.to_string())
        ))
        .unwrap();
    }
    let native_source = db
        .prepare("SELECT k FROM s")
        .unwrap()
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        str_bag(
            native_source
                .into_iter()
                .map(|v| { v.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())) })
        ),
        str_bag(case["source_bag"].as_array().unwrap().iter().map(scalar))
    );
    let sql = case["sql"].as_str().unwrap();
    let oracle_sql = match case["id"].as_str().unwrap() {
        "76-C134-P-with-ties" | "76-C134-N-absent" =>
            "SELECT k FROM (SELECT k, RANK() OVER (ORDER BY k DESC) AS rank_value FROM s) WHERE rank_value <= 2 ORDER BY k DESC",
        "76-C134-P-explicit-nulls" =>
            "SELECT k FROM (SELECT k, RANK() OVER (ORDER BY k NULLS FIRST) AS rank_value FROM s) WHERE rank_value <= 1",
        "76-C134-N-with-ties-zero" =>
            "SELECT k FROM s WHERE FALSE",
        _ => sql,
    };
    let native = db
        .prepare(oracle_sql)
        .unwrap()
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        str_bag(
            native
                .into_iter()
                .map(|v| { v.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())) })
        ),
        str_bag(case["terminal_bag"].as_array().unwrap().iter().map(scalar)),
        "independent whole-terminal DuckDB bag for {}",
        case["id"]
    );
}
fn check_domain(case: &Value, observed: &ValueDomain) {
    let expected = &case["typed_output_domain"]["k"];
    assert_eq!(observed.admits_null(), expected["nullable"].as_bool());
    if expected["kind"] == "empty" {
        assert!(matches!(observed, ValueDomain::Empty), "{observed:?}");
        return;
    }
    let ValueDomain::Set(inclusion) = observed else {
        panic!("full terminal domain must be a finite inclusion set: {observed:?}");
    };
    assert_eq!(inclusion.mode(), SetMode::Include);
    assert_eq!(
        str_bag(inclusion.values().iter().map(|v| v.value().clone())),
        str_bag(case["terminal_bag"].as_array().unwrap().iter().map(scalar))
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let finite = case["terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_i64)
        .collect::<Vec<_>>();
    if let Some(minimum) = finite.iter().min() {
        assert_eq!(expected["lower"]["value"].as_i64(), Some(*minimum));
        assert_eq!(expected["lower"]["inclusive"], true);
    } else {
        assert!(expected["lower"].is_null());
    }
    if let Some(maximum) = finite.iter().max() {
        assert_eq!(expected["upper"]["value"].as_i64(), Some(*maximum));
        assert_eq!(expected["upper"]["inclusive"], true);
    } else {
        assert!(expected["upper"].is_null());
    }
}
#[test]
fn all_six_mandatory_cases_certify_full_bags_domains_and_rejections() {
    let freeze = freeze();
    assert_eq!(freeze["version"], 1);
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(
        cases
            .iter()
            .filter(|c| c["classification"] == "mandatory")
            .count(),
        6
    );
    for case in cases.iter().filter(|c| c["classification"] == "mandatory") {
        oracle(case);
        let sql = case["sql"].as_str().unwrap();
        let bundle = analyze(sql, "duckdb");
        let target = bundle.layers().last().unwrap().id();
        let src = source(case);
        let proof = physical_finite_page_bag_plan(&bundle, target, std::slice::from_ref(&src));
        let PhysicalFinitePageProof::Exact(exact) = &proof else {
            panic!("{} must have full exact proof: {proof:?}", case["id"]);
        };
        let actual = exact
            .terminal
            .iter()
            .map(|row| row.get("k").unwrap().clone());
        assert_eq!(
            str_bag(actual),
            str_bag(case["terminal_bag"].as_array().unwrap().iter().map(scalar)),
            "{} terminal multiplicity",
            case["id"]
        );
        let mut selected = exact
            .rows
            .iter()
            .filter(|r| r.survives)
            .map(|r| r.source_row_id.clone())
            .collect::<Vec<_>>();
        let mut expected_selected = case["selected_source_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        selected.sort();
        expected_selected.sort();
        assert_eq!(
            selected, expected_selected,
            "{} positive source identity",
            case["id"]
        );
        let mut rejected = exact
            .rows
            .iter()
            .filter(|r| !r.survives)
            .map(|r| r.source_row_id.clone())
            .collect::<Vec<_>>();
        let mut expected_rejected = case["rejected_source_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        rejected.sort();
        expected_rejected.sort();
        assert_eq!(
            rejected, expected_rejected,
            "{} NOT TRUE physical row identity",
            case["id"]
        );
        check_domain(case, &exact.terminal_domains["k"]);
        let missing = BTreeMap::from([("k".to_string(), LiteralValue::Number("999999".into()))]);
        assert!(matches!(
            physical_finite_page_membership_plan(&bundle, target, &[src], &missing),
            PhysicalFinitePageProof::Impossible { .. }
        ));
    }
}
#[test]
fn all_conditional_cases_have_frozen_owner_and_nonexact_evidence() {
    let freeze = freeze();
    let cases = freeze["cases"].as_array().unwrap();
    let conditional = cases
        .iter()
        .filter(|c| c["classification"] == "conditional")
        .collect::<Vec<_>>();
    assert_eq!(conditional.len(), 3);
    for case in conditional {
        assert_eq!(case["owner"], "TASK-134");
        assert_eq!(case["proof"], "Residual");
        assert!(!case["release_disposition"].as_str().unwrap().is_empty());
    }
}
#[test]
fn every_shared_distinct_parser_path_has_an_exact_canonical_bag() {
    const DIALECTS: [&str; 13] = [
        "generic",
        "mysql",
        "postgresql",
        "hive",
        "sqlite",
        "snowflake",
        "redshift",
        "mssql",
        "clickhouse",
        "bigquery",
        "ansi",
        "duckdb",
        "databricks",
    ];
    let case = freeze()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "76-C134-P-distinct")
        .unwrap()
        .clone();
    let sql = case["sql"].as_str().unwrap();
    for name in DIALECTS {
        let bundle = analyze(sql, name);
        let target = bundle.layers().last().unwrap().id();
        let proof = physical_finite_page_bag_plan(&bundle, target, &[source(&case)]);
        assert!(
            matches!(proof, PhysicalFinitePageProof::Exact(_)),
            "{name}: {proof:?}"
        );
    }
}

#[test]
fn every_frozen_parser_path_records_canonical_exact_or_explicit_boundary() {
    use sqlparser::parser::Parser;
    const DIALECTS: [&str; 13] = [
        "generic",
        "mysql",
        "postgresql",
        "hive",
        "sqlite",
        "snowflake",
        "redshift",
        "mssql",
        "clickhouse",
        "bigquery",
        "ansi",
        "duckdb",
        "databricks",
    ];
    let inventory = freeze();
    let mut checked = 0;
    let mut observed = BTreeMap::<String, BTreeMap<String, String>>::new();
    for case in inventory["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).unwrap();
            let parsed = Parser::parse_sql(parser.as_ref(), sql);
            match parsed {
                Err(error) => {
                    assert!(!error.to_string().is_empty());
                    observed
                        .entry(id.to_string())
                        .or_default()
                        .insert(dialect.to_string(), "parser_boundary".to_string());
                }
                Ok(_) => {
                    let input = SqlInput::inline(sql);
                    let configured = [ConfiguredSqlInput::new(
                        id,
                        &input,
                        dialect,
                        parser.as_ref(),
                    )];
                    let analysis = analyze_configured_inputs_with_catalog(&configured, &catalog());
                    let classification = match analysis {
                        Err(reason) => {
                            println!("TASK134_PARSER {id} {dialect} analysis_boundary: {reason}");
                            "analysis_boundary"
                        }
                        Ok(bundle) => {
                            let target = bundle.layers().last().unwrap().id();
                            let proof =
                                physical_finite_page_bag_plan(&bundle, target, &[source(case)]);
                            match proof {
                                PhysicalFinitePageProof::Exact(_) => "exact",
                                PhysicalFinitePageProof::Impossible { .. } => "impossible",
                                PhysicalFinitePageProof::Residual { .. } => "residual",
                            }
                        }
                    };
                    observed
                        .entry(id.to_string())
                        .or_default()
                        .insert(dialect.to_string(), classification.to_string());
                    if case["classification"] == "mandatory" {
                        assert_eq!(classification, "exact", "{id}/{dialect}");
                    } else {
                        assert_ne!(
                            classification, "exact",
                            "{id}/{dialect}: conditional boundary"
                        );
                    }
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 117, "nine frozen variants in all 13 dialects");
    let frozen = inventory["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let id = case["id"].as_str().unwrap().to_string();
            let cells = case["dialect_cells"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, status)| (key.clone(), status.as_str().unwrap().to_string()))
                .collect::<BTreeMap<_, _>>();
            (id, cells)
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observed, frozen, "frozen 117-cell parser dispositions must match executed canonical proof; observed matrix printed on mismatch");
}
