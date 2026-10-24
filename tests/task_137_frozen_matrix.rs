//! TASK-137 frozen matrix: independently observed source-free DuckDB bags,
//! identities, full output domains, negative membership, and dialect boundaries.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, LiteralValue,
    PhysicalFiniteTableProof, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

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
    value
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), literal(v)))
        .collect()
}

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    result.sort();
    result
}

fn analyze(sql: &str, dialect: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let parser =
        dialect_from_name(dialect).ok_or_else(|| format!("unsupported dialect: {dialect}"))?;
    analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).map_err(|e| e.to_string())
}

fn whole_duckdb_bag(sql: &str, names: &[String]) -> Vec<BTreeMap<String, LiteralValue>> {
    let conn = Connection::open_in_memory().unwrap();
    let mut stmt = conn
        .prepare(sql)
        .unwrap_or_else(|e| panic!("independent SQL {sql}: {e}"));
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
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

#[test]
fn every_frozen_mandatory_cell_proves_complete_duckdb_terminal_and_negative_bags() {
    let freeze = fixture();
    assert_eq!(freeze["version"], 3);
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["77-C137-P", "77-C137-N"])
    );
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 8);
    let mut count = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        if case["classification"] != "mandatory" {
            continue;
        }
        count += 1;
        let bundle =
            analyze(sql, "duckdb").unwrap_or_else(|err| panic!("{id}: parser boundary: {err}"));
        let target = bundle.layers().last().unwrap().id();
        let PhysicalFiniteTableProof::Exact(actual) =
            physical_finite_table_plan(&bundle, target, None)
        else {
            panic!("{id}: missing complete typed proof");
        };
        assert!(
            actual.nodes.iter().all(|node| node.inputs().is_empty()),
            "{id}: synthetic inputs"
        );
        let expected = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(row)
            .collect::<Vec<_>>();
        assert_eq!(
            sorted(&actual.terminal),
            sorted(&expected),
            "{id}: frozen bag"
        );
        let names = case["output_columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let native = whole_duckdb_bag(case["oracle_sql"].as_str().unwrap(), &names);
        assert_eq!(
            sorted(&native),
            sorted(&expected),
            "{id}: independent DuckDB whole bag"
        );
        let ids = case["positive"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            actual.row_ids, ids,
            "{id}: every physical producer identity"
        );
        assert_eq!(
            actual.terminal_domains.len(),
            names.len(),
            "{id}: exact domain arity"
        );
        let typed = case["expected_column_types"].as_array().unwrap();
        assert_eq!(typed.len(), names.len(), "{id}: typed column arity");
        let actual_types = actual
            .terminal_column_types
            .values()
            .map(|ty| ty.as_str())
            .collect::<Vec<_>>();
        let mut expected_types = case["output_columns"]
            .as_array()
            .unwrap()
            .iter()
            .zip(typed.iter())
            .map(|(name, ty)| (name.as_str().unwrap(), ty.as_str().unwrap()))
            .collect::<Vec<_>>();
        expected_types.sort_by_key(|(name, _)| *name);
        assert_eq!(
            actual_types,
            expected_types.iter().map(|(_, ty)| *ty).collect::<Vec<_>>(),
            "{id}: frozen type evidence"
        );
        for name in names {
            let frozen = &case["domains"][&name];
            let domain = &actual.terminal_domains[&name];
            let numbers = frozen["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap().to_string())
                .collect::<Vec<_>>();
            match domain {
                ValueDomain::Empty => assert!(numbers.is_empty(), "{id}/{name}: only empty output"),
                ValueDomain::Set(set) => {
                    assert_eq!(set.mode(), SetMode::Include, "{id}/{name}");
                    let present = set
                        .values()
                        .iter()
                        .filter_map(|v| match v.value() {
                            LiteralValue::Number(n) => Some(n.clone()),
                            LiteralValue::Null => None,
                            _ => panic!("{id}/{name}: invented non-integer value"),
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(present, numbers, "{id}/{name}: precise finite values");
                    if let Some(bound) = frozen["lower"].as_object() {
                        assert_eq!(
                            &present
                                .iter()
                                .map(|n| n.parse::<i128>().unwrap())
                                .min()
                                .unwrap()
                                .to_string(),
                            &bound["value"].as_i64().unwrap().to_string(),
                            "{id}/{name}: inclusive min"
                        );
                        assert_eq!(bound["inclusive"], true);
                    }
                    if let Some(bound) = frozen["upper"].as_object() {
                        assert_eq!(
                            &present
                                .iter()
                                .map(|n| n.parse::<i128>().unwrap())
                                .max()
                                .unwrap()
                                .to_string(),
                            &bound["value"].as_i64().unwrap().to_string(),
                            "{id}/{name}: inclusive max"
                        );
                        assert_eq!(bound["inclusive"], true);
                    }
                }
                other => panic!("{id}/{name}: not the strongest finite domain: {other:?}"),
            }
            assert_eq!(
                domain.admits_null(),
                frozen["nullable"].as_bool(),
                "{id}/{name}: NULL truth"
            );
        }
        let wanted = row(&case["negative_membership"]["row"]);
        assert!(
            matches!(
                physical_finite_table_plan(&bundle, target, Some(&wanted)),
                PhysicalFiniteTableProof::Impossible { .. }
            ),
            "{id}: whole-bag NOT TRUE proof"
        );
        if let Some(positive) = expected.first() {
            assert!(
                matches!(
                    physical_finite_table_plan(&bundle, target, Some(positive)),
                    PhysicalFiniteTableProof::Exact(_)
                ),
                "{id}: positive membership proof"
            );
        }
    }
    assert_eq!(count, 7, "all seven mandatory variants executed");
}

#[test]
fn all_frozen_variant_dialect_cells_are_parser_equivalent_or_explicit_boundaries() {
    use sqlparser::parser::Parser;

    let freeze = fixture();
    let dialects = freeze["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let mut observed = BTreeMap::<&str, usize>::new();
    let mut cells = 0;

    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let expected = case["expected"].as_array();
        let frozen_rows = expected.map(|rows| rows.iter().map(row).collect::<Vec<_>>());
        let canonical = if case["classification"] == "mandatory" {
            let bundle = analyze(sql, "duckdb").unwrap();
            let layer = bundle.layers().last().unwrap().id();
            let PhysicalFiniteTableProof::Exact(reference) =
                physical_finite_table_plan(&bundle, layer, None)
            else {
                panic!("{id}: missing certified DuckDB canonical baseline");
            };
            Some(reference)
        } else {
            None
        };

        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let classifier = case["parser_cells"][name]
                .as_str()
                .unwrap_or_else(|| panic!("{id}/{name}: missing frozen expected classification"));
            cells += 1;
            *observed.entry(classifier).or_default() += 1;
            let parser = dialect_from_name(name).unwrap();
            let parsed = Parser::parse_sql(parser.as_ref(), sql);

            if classifier == "parser_rejected" {
                assert!(
                    parsed.is_err(),
                    "{id}/{name}: frozen parser rejection unexpectedly accepted: {parsed:?}"
                );
                continue;
            }

            let ast = parsed.unwrap_or_else(|error| {
                panic!("{id}/{name}: frozen accepted AST did not parse: {error}")
            });
            // Parser acceptance and semantic normalization are separate gates.
            // An analysis error is never silently recorded as a parser rejection.
            let bundle = analyze(sql, name).unwrap_or_else(|error| {
                panic!("{id}/{name}: accepted SQL failed analysis: {error}")
            });
            let layer = bundle.layers().last().unwrap().id();
            let proof = physical_finite_table_plan(&bundle, layer, None);
            match classifier {
                "exact" => {
                    let PhysicalFiniteTableProof::Exact(exact) = proof else {
                        panic!("{id}/{name}: frozen exact AST has no canonical proof: {proof:?}");
                    };
                    let expected_rows = frozen_rows.as_ref().unwrap();
                    let reference = canonical.as_ref().unwrap();
                    assert_eq!(
                        sorted(&exact.terminal),
                        sorted(expected_rows),
                        "{id}/{name}: full frozen physical bag"
                    );
                    assert_eq!(
                        exact.row_ids, reference.row_ids,
                        "{id}/{name}: row identities"
                    );
                    assert_eq!(
                        exact.terminal_column_types, reference.terminal_column_types,
                        "{id}/{name}: declared type evidence survives empty outputs"
                    );
                    assert_eq!(
                        exact.terminal_domains, reference.terminal_domains,
                        "{id}/{name}: complete typed output domains"
                    );
                    let absent = row(&case["negative_membership"]["row"]);
                    assert!(
                        matches!(
                            physical_finite_table_plan(&bundle, layer, Some(&absent)),
                            PhysicalFiniteTableProof::Impossible { .. }
                        ),
                        "{id}/{name}: negative membership must be Impossible"
                    );
                    if let Some(present) = expected_rows.first() {
                        assert!(
                            matches!(
                                physical_finite_table_plan(&bundle, layer, Some(present)),
                                PhysicalFiniteTableProof::Exact(_)
                            ),
                            "{id}/{name}: positive membership"
                        );
                    }
                }
                "lexical_boundary" => {
                    // SQLite and MSSQL tokenize [1, 2] as a bracket-quoted
                    // identifier, not an array literal. This is materially
                    // different AST semantics, not a missing UNNEST law.
                    assert!(
                        format!("{ast:?}").contains("quote_style: Some('[')"),
                        "{id}/{name}: boundary must retain bracketed identifier evidence"
                    );
                    assert!(
                        matches!(proof, PhysicalFiniteTableProof::Residual { .. }),
                        "{id}/{name}: identifier cannot become literal array rows"
                    );
                }
                "opaque_residual" => assert!(
                    matches!(proof, PhysicalFiniteTableProof::Residual { .. }),
                    "{id}/{name}: absent collection evidence must remain Residual"
                ),
                other => panic!("{id}/{name}: unknown frozen parser classification {other}"),
            }
        }
    }

    assert_eq!(cells, 104);
    assert_eq!(observed.get("exact"), Some(&69));
    assert_eq!(observed.get("parser_rejected"), Some(&18));
    assert_eq!(observed.get("lexical_boundary"), Some(&4));
    assert_eq!(observed.get("opaque_residual"), Some(&13));
}
