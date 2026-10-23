//! TASK-137 frozen matrix: independently observed source-free DuckDB bags,
//! identities, full output domains, negative membership, and dialect boundaries.

use std::collections::BTreeMap;
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, LiteralValue,
    PhysicalFiniteTableProof, SetMode, SqlInput, ValueDomain,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../docs/task-137-acceptance-freeze.json"))
        .expect("versioned frozen matrix")
}

fn literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        other => panic!("invalid frozen integer/NULL literal: {other}"),
    }
}

fn row(value: &Value) -> BTreeMap<String, LiteralValue> {
    value.as_object().unwrap().iter()
        .map(|(k, v)| (k.clone(), literal(v)))
        .collect()
}

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    result.sort();
    result
}

fn analyze(sql: &str, dialect: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let parser = dialect_from_name(dialect).ok_or_else(|| format!("unsupported dialect: {dialect}"))?;
    analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).map_err(|e| e.to_string())
}

fn whole_duckdb_bag(sql: &str, names: &[String]) -> Vec<BTreeMap<String, LiteralValue>> {
    let conn = Connection::open_in_memory().unwrap();
    let mut stmt = conn.prepare(sql).unwrap_or_else(|e| panic!("independent SQL {sql}: {e}"));
    stmt.query_map([], |r| {
        let mut values = BTreeMap::new();
        for (i, name) in names.iter().enumerate() {
            values.insert(
                name.clone(),
                r.get::<_, Option<i64>>(i)?
                    .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
            );
        }
        Ok(values)
    }).unwrap().collect::<Result<Vec<_>, _>>().unwrap()
}

#[test]
fn every_frozen_mandatory_cell_proves_complete_duckdb_terminal_and_negative_bags() {
    let freeze = fixture();
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["fixture_ids"], serde_json::json!(["77-C137-P", "77-C137-N"]));
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 8);
    let mut count = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        if case["classification"] != "mandatory" { continue; }
        count += 1;
        let bundle = analyze(sql, "duckdb").unwrap_or_else(|err| panic!("{id}: parser boundary: {err}"));
        let target = bundle.layers().last().unwrap().id();
        let PhysicalFiniteTableProof::Exact(actual) =
            physical_finite_table_plan(&bundle, target, None)
        else { panic!("{id}: missing complete typed proof"); };
        assert!(actual.nodes.iter().all(|node| node.inputs().is_empty()), "{id}: synthetic inputs");
        let expected = case["expected"].as_array().unwrap().iter().map(row).collect::<Vec<_>>();
        assert_eq!(sorted(&actual.terminal), sorted(&expected), "{id}: frozen bag");
        let names = case["output_columns"].as_array().unwrap().iter()
            .map(|name| name.as_str().unwrap().to_string()).collect::<Vec<_>>();
        let native = whole_duckdb_bag(case["oracle_sql"].as_str().unwrap(), &names);
        assert_eq!(sorted(&native), sorted(&expected), "{id}: independent DuckDB whole bag");
        let ids = case["positive"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(actual.row_ids, ids, "{id}: every physical producer identity");
        assert_eq!(actual.terminal_domains.len(), names.len(), "{id}: exact domain arity");
        for name in names {
            let frozen = &case["domains"][&name];
            let domain = &actual.terminal_domains[&name];
            let numbers = frozen["values"].as_array().unwrap()
                .iter().map(|v| v.as_i64().unwrap().to_string()).collect::<Vec<_>>();
            match domain {
                ValueDomain::Empty => assert!(numbers.is_empty(), "{id}/{name}: only empty output"),
                ValueDomain::Set(set) => {
                    assert_eq!(set.mode(), SetMode::Include, "{id}/{name}");
                    let present = set.values().iter().filter_map(|v| {
                        match v.value() {
                            LiteralValue::Number(n) => Some(n.clone()),
                            LiteralValue::Null => None,
                            _ => panic!("{id}/{name}: invented non-integer value")
                        }
                    }).collect::<Vec<_>>();
                    assert_eq!(present, numbers, "{id}/{name}: precise finite values");
                    if let Some(bound) = frozen["lower"].as_object() {
                        assert_eq!(present.first().unwrap(), &bound["value"].as_i64().unwrap().to_string(), "{id}/{name}: inclusive min");
                        assert_eq!(bound["inclusive"], true);
                    }
                    if let Some(bound) = frozen["upper"].as_object() {
                        assert_eq!(present.last().unwrap(), &bound["value"].as_i64().unwrap().to_string(), "{id}/{name}: inclusive max");
                        assert_eq!(bound["inclusive"], true);
                    }
                }
                other => panic!("{id}/{name}: not the strongest finite domain: {other:?}"),
            }
            assert_eq!(domain.admits_null(), frozen["nullable"].as_bool(), "{id}/{name}: NULL truth");
        }
        let wanted = row(&case["negative_membership"]["row"]);
        assert!(matches!(
            physical_finite_table_plan(&bundle, target, Some(&wanted)),
            PhysicalFiniteTableProof::Impossible { .. }
        ), "{id}: whole-bag NOT TRUE proof");
        if let Some(positive) = expected.first() {
            assert!(matches!(
                physical_finite_table_plan(&bundle, target, Some(positive)),
                PhysicalFiniteTableProof::Exact(_)
            ), "{id}: positive membership proof");
        }
    }
    assert_eq!(count, 7, "all seven mandatory variants executed");
}

#[test]
fn all_frozen_variant_dialect_cells_are_parser_equivalent_or_explicit_boundaries() {
    let freeze = fixture();
    let dialects = freeze["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let mut seen = 0;
    let mut parser_boundaries = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            seen += 1;
            let bundle = match analyze(case["sql"].as_str().unwrap(), name) {
                Ok(bundle) => bundle,
                Err(_) => { parser_boundaries += 1; continue; }
            };
            let layer = bundle.layers().last().unwrap().id();
            let proof = physical_finite_table_plan(&bundle, layer, None);
            if case["classification"] == "conditional" {
                assert!(matches!(proof, PhysicalFiniteTableProof::Residual { .. }), "{id}/{name}: opaque collection");
                continue;
            }
            // Where an exposed parser accepts the identical concrete AST law,
            // it must not silently lose the canonical finite relation.
            let PhysicalFiniteTableProof::Exact(proof) = proof else {
                panic!("{id}/{name}: accepted parser form lacks canonical exactness");
            };
            assert_eq!(proof.terminal.len(), case["expected"].as_array().unwrap().len(), "{id}/{name}");
            assert_eq!(proof.terminal_domains.len(), case["domains"].as_object().unwrap().len(), "{id}/{name}");
        }
    }
    assert_eq!(seen, 13 * 8, "all dialect cells explicitly examined");
    assert!(parser_boundaries < seen, "the shared VALUES syntax must be parsed");
}
