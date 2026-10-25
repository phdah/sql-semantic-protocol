//! Executable v3 TASK-133 acceptance contract: full closed-world source and
//! terminal oracles, exact typed domains, membership and every dialect cell.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    physical_finite_page_membership_plan, to_bundle_json, ConfiguredSqlInput, LiteralValue,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SetMode, SqlInput, ValueDomain,
};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-133-acceptance-freeze.json")).unwrap()
}

fn scalar(value: &Value) -> LiteralValue {
    value
        .as_i64()
        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
}

fn source(case: &Value) -> PhysicalWriteState {
    let rows = case["source_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), scalar(n))])).unwrap())
        .collect();
    PhysicalWriteState::new("s", rows).unwrap()
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap()
}

fn analyze(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let configured = [ConfiguredSqlInput::new(
        case["id"].as_str().unwrap(),
        &sql,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap()
}

fn expected_terminal(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let output = case["output_column"].as_str().unwrap();
    case["terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| BTreeMap::from([(output.to_string(), scalar(value))]))
        .collect()
}

fn strings(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn sorted(values: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut bag = values
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    bag.sort();
    bag
}

fn observed_ids(
    proof: &sql_semantic_protocol::PhysicalFinitePageExact,
    survive: bool,
) -> Vec<String> {
    proof
        .rows
        .iter()
        .filter(|row| row.survives == survive)
        .map(|row| row.source_row_id.clone())
        .collect()
}

fn exact_domain_matches(case: &Value, domain: &ValueDomain) {
    let frozen = &case["typed_output_domain"][case["output_column"].as_str().unwrap()];
    assert_eq!(
        domain.admits_null(),
        frozen["nullable"].as_bool(),
        "NULL law"
    );
    let expected = frozen["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(scalar)
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>();
    if expected.is_empty() {
        assert!(matches!(domain, ValueDomain::Empty), "{domain:?}");
        assert!(frozen["lower"].is_null() && frozen["upper"].is_null());
        return;
    }
    let ValueDomain::Set(values) = domain else {
        panic!("finite page domain is not strongest exact set: {domain:?}");
    };
    assert_eq!(values.mode(), SetMode::Include);
    let mut observed = values
        .values()
        .iter()
        .map(|literal| format!("{:?}", literal.value()))
        .collect::<Vec<_>>();
    let mut expected = expected;
    observed.sort();
    expected.sort();
    assert_eq!(observed, expected, "typed complete terminal values");

    let nonnull = frozen["values"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_i64)
        .collect::<Vec<_>>();
    for (endpoint, expect) in [
        ("lower", nonnull.iter().min().copied()),
        ("upper", nonnull.iter().max().copied()),
    ] {
        if let Some(value) = expect {
            assert_eq!(
                frozen[endpoint]["value"].as_i64(),
                Some(value),
                "{endpoint}"
            );
            assert_eq!(frozen[endpoint]["inclusive"], true, "{endpoint}");
        } else {
            assert!(frozen[endpoint].is_null(), "{endpoint}");
        }
    }
}

fn native_source_terminal(case: &Value) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER)").unwrap();
    for n in case["source_bag"].as_array().unwrap() {
        let literal = n.as_i64().map_or("NULL".to_string(), |v| v.to_string());
        db.execute_batch(&format!("INSERT INTO s VALUES ({literal})"))
            .unwrap();
    }
    let physical = db
        .prepare("SELECT k FROM s")
        .unwrap()
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected_source = case["source_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::as_i64)
        .collect::<Vec<_>>();
    let mut physical_sorted = physical;
    let mut expected_sorted = expected_source;
    physical_sorted.sort();
    expected_sorted.sort();
    assert_eq!(
        physical_sorted, expected_sorted,
        "complete DuckDB source bag"
    );
    let oracle_sql = case["oracle_sql"].as_str().unwrap();
    let terminal = db
        .prepare(oracle_sql)
        .unwrap_or_else(|e| panic!("{oracle_sql}: {e}"))
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected_terminal = case["terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::as_i64)
        .collect::<Vec<_>>();
    assert_eq!(
        terminal, expected_terminal,
        "independent ordered DuckDB full terminal"
    );
}

#[test]
fn mandatory_cases_have_independent_complete_duckdb_and_membership_oracles() {
    let inventory = freeze();
    assert_eq!(inventory["version"], 3);
    let mut mandatory = 0;
    let mut conditional = 0;
    for case in inventory["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if case["status"] != "mandatory" {
            conditional += 1;
            continue;
        }
        mandatory += 1;
        native_source_terminal(case);
        let bundle = analyze(
            case,
            if id.ends_with("-top") {
                "mssql"
            } else {
                "duckdb"
            },
        );
        let target = bundle.layers().last().unwrap().id();
        let state = source(case);
        let proof = physical_finite_page_bag_plan(&bundle, target, std::slice::from_ref(&state));
        let PhysicalFinitePageProof::Exact(actual) = proof else {
            panic!("{id}: expected total ordering proof: {proof:?}");
        };
        assert_eq!(actual.source, case["source_relation"].as_str().unwrap());
        assert_eq!(
            actual.rows.len(),
            state.rows().len(),
            "{id}: every input identity"
        );
        assert_eq!(
            actual
                .rows
                .iter()
                .map(|r| r.source_row_id.clone())
                .collect::<Vec<_>>(),
            strings(&case["source_row_ids"]),
            "{id}: stable source identities"
        );
        assert_eq!(
            observed_ids(&actual, true),
            strings(&case["selected_source_ids"]),
            "{id}: TRUE surviving identities"
        );
        assert_eq!(
            observed_ids(&actual, false),
            strings(&case["rejected_source_ids"]),
            "{id}: NOT TRUE rejected identities"
        );
        let expected = expected_terminal(case);
        assert_eq!(
            sorted(&actual.terminal),
            sorted(&expected),
            "{id}: entire bag"
        );
        assert_eq!(
            actual.terminal.len(),
            expected.len(),
            "{id}: multiplicities"
        );
        assert_eq!(actual.terminal_domains.len(), 1, "{id}: typed arity");
        exact_domain_matches(
            case,
            actual
                .terminal_domains
                .get(case["output_column"].as_str().unwrap())
                .unwrap(),
        );
        let negative = BTreeMap::from([(
            case["output_column"].as_str().unwrap().to_string(),
            scalar(&case["negative_membership"][case["output_column"].as_str().unwrap()]),
        )]);
        assert!(
            matches!(
                physical_finite_page_membership_plan(
                    &bundle,
                    target,
                    std::slice::from_ref(&state),
                    &negative
                ),
                PhysicalFinitePageProof::Impossible { .. }
            ),
            "{id}: requested row missing from closed terminal must be Impossible"
        );
        if let Some(positive) = expected.first() {
            assert!(
                matches!(
                    physical_finite_page_membership_plan(
                        &bundle,
                        target,
                        std::slice::from_ref(&state),
                        positive
                    ),
                    PhysicalFinitePageProof::Exact(_)
                ),
                "{id}: requested survivor must be Exact"
            );
        }
        let emitted: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
        assert_eq!(
            emitted["protocol_version"],
            env!("CARGO_PKG_VERSION"),
            "{id}: active protocol identity"
        );
        let shape = &emitted["inputs"][0]["statements"][0]["row_shaping"];
        assert_eq!(
            shape["keys"][0]["position"], 0,
            "{id}: canonical sort position"
        );
        assert!(shape["limit"].is_u64() && shape["offset"].is_u64(), "{id}");
    }
    assert_eq!(mandatory, 9, "all exact named variants frozen and executed");
    assert_eq!(
        conditional, 4,
        "all explicitly bounded Residual laws frozen"
    );
}

#[test]
fn every_frozen_parser_cell_matches_canonical_physical_classification() {
    let inventory = freeze();
    let dialects = inventory["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let mut mandatory_cells = 0;
    let mut conditional_cells = 0;
    for case in inventory["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let state = source(case);
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let classifier = case["parser_cells"][name].as_str().unwrap();
            let parser = dialect_from_name(name).unwrap();
            assert!(
                Parser::parse_sql(parser.as_ref(), sql).is_ok(),
                "{id}/{name}: registered canonical parser cell must accept SQL"
            );
            let bundle = analyze(case, name);
            let target = bundle.layers().last().unwrap().id();
            let proof =
                physical_finite_page_bag_plan(&bundle, target, std::slice::from_ref(&state));
            match classifier {
                "exact" => {
                    mandatory_cells += 1;
                    let PhysicalFinitePageProof::Exact(exact) = proof else {
                        panic!("{id}/{name}: missing Exact: {proof:?}");
                    };
                    assert_eq!(
                        sorted(&exact.terminal),
                        sorted(&expected_terminal(case)),
                        "{id}/{name}: full equivalent terminal"
                    );
                    assert_eq!(
                        observed_ids(&exact, true),
                        strings(&case["selected_source_ids"]),
                        "{id}/{name}: same physical identities"
                    );
                    exact_domain_matches(
                        case,
                        &exact.terminal_domains[case["output_column"].as_str().unwrap()],
                    );
                }
                "residual" => {
                    conditional_cells += 1;
                    assert!(
                        matches!(proof, PhysicalFinitePageProof::Residual { .. }),
                        "{id}/{name}: conditional native/session law overclaimed: {proof:?}"
                    );
                    assert!(
                        case["owner"].is_string() && case["disposition"].is_string(),
                        "{id}/{name}: conditional case has no owner/disposition"
                    );
                }
                other => panic!("{id}/{name}: non-frozen parser classification {other}"),
            }
        }
    }
    assert_eq!(mandatory_cells, 117, "9 mandatory x 13 dialect paths");
    assert_eq!(conditional_cells, 52, "4 conditional x 13 dialect paths");
    assert_eq!(mandatory_cells + conditional_cells, 169);
}

#[test]
fn schema_and_golden_shape_include_direct_row_selection_contract() {
    let schema: Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json")).unwrap();
    assert!(
        schema["$defs"]["queryStatement"]["properties"]["row_shaping"].is_object(),
        "optional canonical direct page shape must be part of active protocol"
    );
    let case = freeze()["cases"][0].clone();
    let bundle = analyze(&case, "duckdb");
    let document: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let expected = serde_json::json!({
        "keys": [{"position": 0, "descending": false, "nulls_first": null}],
        "offset": 1,
        "limit": 2
    });
    assert_eq!(
        document["inputs"][0]["statements"][0]["row_shaping"],
        expected
    );
}
