//! TASK-77 independently reconciles merged child proof evidence at the public API.
//! Child freezes own the exact SQL and full-bag oracles; this test guards the
//! complete union and re-executes representative laws against native DuckDB.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, LiteralValue,
    PhysicalFiniteTableProof, SqlInput, ValueDomain,
};

const CHILDREN: [(&str, &str); 4] = [
    (
        "TASK-137",
        include_str!("../docs/task-137-acceptance-freeze.json"),
    ),
    (
        "TASK-138",
        include_str!("../docs/task-138-acceptance-freeze.json"),
    ),
    (
        "TASK-139",
        include_str!("../docs/task-139-acceptance-freeze.json"),
    ),
    (
        "TASK-140",
        include_str!("../docs/task-140-acceptance-freeze.json"),
    ),
];

fn parent() -> Value {
    serde_json::from_str(include_str!("../docs/task-77-acceptance-freeze.json"))
        .expect("parent acceptance freeze")
}

fn ids(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .expect("ID array")
        .iter()
        .map(|item| item.as_str().expect("ID").to_owned())
        .collect()
}

fn adapter(name: &str) -> &str {
    match name {
        "compiled_dbt" => "dbt_compiled_sql_catalog",
        "odcs" => "odcs_equivalent_schema",
        other => other,
    }
}

fn cells(value: &Value, field: &str) -> BTreeSet<String> {
    if let Some(map) = value.as_object() {
        return map
            .keys()
            .map(|id| {
                if field == "dialect" {
                    id.clone()
                } else {
                    adapter(id).to_owned()
                }
            })
            .collect();
    }
    value
        .as_array()
        .expect("cells")
        .iter()
        .map(|row| {
            let name = row[field].as_str().expect("named cell");
            if field == "dialect" {
                name.to_owned()
            } else {
                adapter(name).to_owned()
            }
        })
        .collect()
}

fn case_list<'a>(child: &'a Value, name: &str) -> &'a Vec<Value> {
    let field = if name == "TASK-140" {
        "variants"
    } else {
        "cases"
    };
    child[field].as_array().expect("finite child inventory")
}

fn domain(case: &Value) -> Option<&Value> {
    case.get("domains").or_else(|| case.get("output_domains"))
}

fn full_terminal(case: &Value) -> Option<&Value> {
    [
        "expected",
        "expected_terminal_rows",
        "terminal_rows",
        "terminal",
    ]
    .iter()
    .find_map(|key| case.get(*key))
}

#[test]
fn all_four_merged_child_inventories_preserve_every_exact_and_conditional_obligation() {
    let freeze = parent();
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["task"], "TASK-77");
    assert_eq!(freeze["release_approved"], false);
    if freeze["parent_signed_off"] == true {
        assert!(freeze["mandatory_unverified"].as_array().is_some_and(Vec::is_empty));
    } else {
        assert_eq!(
            freeze["mandatory_unverified"],
            serde_json::json!(["77-PARENT-DERIVED-P"])
        );
    }

    let dialects = ids(&freeze["required_dialects"]);
    let adapters = ids(&freeze["required_adapters"]);
    assert_eq!(dialects.len(), 13);
    assert_eq!(adapters.len(), 3);

    let mut mandatory = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    let mut fixtures = BTreeSet::new();

    for (name, content) in CHILDREN {
        let child: Value = serde_json::from_str(content).expect("versioned child");
        assert_eq!(child["parent"], "TASK-77", "{name}");
        assert_eq!(ids(&child["dialects"]), dialects, "{name}: dialect drift");
        let child_adapter = child
            .get("adapters")
            .or_else(|| child.get("adapter_sources"))
            .expect("child adapter inventory");
        assert_eq!(
            ids(child_adapter)
                .into_iter()
                .map(|s| adapter(&s).to_owned())
                .collect::<BTreeSet<_>>(),
            adapters,
            "{name}: adapter drift"
        );
        let reference = &freeze["source_of_truth"][name];
        assert_eq!(
            reference["version"], child["version"],
            "{name}: version drift"
        );
        assert_eq!(ids(&reference["fixture_ids"]), ids(&child["fixture_ids"]));
        fixtures.extend(ids(&child["fixture_ids"]));

        let exact = ids(&reference["mandatory_case_ids"]);
        let residual = ids(&reference["conditional_case_ids"]);
        assert!(exact.is_disjoint(&residual));
        let mut seen = BTreeSet::new();
        for case in case_list(&child, name) {
            let id = case["id"].as_str().expect("case identity");
            assert!(seen.insert(id.to_owned()), "{name}: duplicate case {id}");
            assert!(
                !case["sql"].as_str().unwrap_or("").is_empty(),
                "{id}: no SQL"
            );
            let parsers = case
                .get("parser_cells")
                .or_else(|| case.get("dialect_matrix"))
                .expect("full parser evidence");
            assert_eq!(cells(parsers, "dialect"), dialects, "{id}: parser matrix");
            let sources = case
                .get("adapter_cells")
                .or_else(|| case.get("adapter_matrix"))
                .expect("full adapter evidence");
            let adapter_field = if sources.is_array()
                && sources
                    .as_array()
                    .unwrap()
                    .first()
                    .is_some_and(|cell| cell.get("source").is_some())
            {
                "source"
            } else {
                "adapter"
            };
            assert_eq!(
                cells(sources, adapter_field),
                adapters,
                "{id}: adapter matrix"
            );

            let exact_case = case
                .get("classification")
                .or_else(|| case.get("kind"))
                .and_then(Value::as_str)
                == Some("mandatory");
            assert_eq!(exact_case, exact.contains(id), "{id}: classification drift");

            if exact_case {
                let complete_source = case
                    .get("source")
                    .or_else(|| case.get("source_rows"))
                    .is_some_and(Value::is_array);
                let complete_terminal = full_terminal(case).is_some_and(Value::is_array);
                if !complete_source || !complete_terminal {
                    let target_id = freeze["domain_cross_references"][id]
                        .as_str()
                        .expect("only reviewed TASK-140 source/terminal references");
                    assert_eq!(
                        name, "TASK-140",
                        "{id}: only canonical peer bags may be reused"
                    );
                    assert_eq!(case["source"].as_str(), Some(target_id));
                    assert_eq!(case["terminal"].as_str(), Some(target_id));
                    let target = case_list(&child, name)
                        .iter()
                        .find(|peer| peer["id"] == target_id)
                        .expect("referenced peer must remain in frozen child inventory");
                    assert!(
                        target["source"].is_array(),
                        "{id}: referenced source not complete"
                    );
                    assert!(
                        target["terminal"].is_array(),
                        "{id}: referenced terminal not complete"
                    );
                    assert!(
                        domain(target).is_some_and(Value::is_object),
                        "{id}: referenced source has no typed domain"
                    );
                }
                if !domain(case).is_some_and(Value::is_object) {
                    assert!(
                        freeze["domain_cross_references"][id].is_string(),
                        "{id}: missing typed domain witness"
                    );
                }
                assert!(
                    case.get("positive")
                        .or_else(|| case.get("positive_membership"))
                        .or_else(|| case.get("membership_positive"))
                        .or_else(|| case.get("source_row_ids"))
                        .is_some(),
                    "{id}: missing positive physical evidence"
                );
                assert!(
                    case.get("rejected")
                        .or_else(|| case.get("negative_membership"))
                        .or_else(|| case.get("not_true"))
                        .is_some(),
                    "{id}: missing NOT TRUE or impossible witness"
                );
                assert!(mandatory.insert(id.to_owned()), "duplicate mandatory {id}");
            } else {
                assert!(
                    residual.contains(id),
                    "{id}: undeclared conditional boundary"
                );
                let owner = case
                    .get("owner")
                    .or_else(|| case.get("reviewed_conditional_owner"))
                    .or_else(|| case.get("parser_review").and_then(|v| v.get("owner")))
                    .or_else(|| case.get("release_disposition").and_then(|v| v.get("owner")));
                assert_eq!(
                    owner.and_then(Value::as_str),
                    Some(name),
                    "{id}: conditional boundary owner mismatch"
                );
                if name == "TASK-137" {
                    assert!(case["reason"].as_str().is_some_and(|v| !v.is_empty()));
                } else {
                    let disposition = &case["release_disposition"];
                    assert_eq!(disposition["owner"], name);
                    assert!(disposition["reason"]
                        .as_str()
                        .is_some_and(|v| !v.is_empty()));
                    assert!(disposition["status"]
                        .as_str()
                        .is_some_and(|v| !v.is_empty()));
                }
                assert!(
                    conditional.insert(id.to_owned()),
                    "duplicate conditional {id}"
                );
            }
        }
        assert_eq!(
            seen,
            exact.union(&residual).cloned().collect(),
            "{name}: unclassified case"
        );
    }

    assert_eq!(fixtures, ids(&freeze["fixture_ids"]));
    assert_eq!(mandatory.len(), 24, "child-owned mandatory matrix drift");
    assert_eq!(conditional.len(), 10, "child-owned conditional matrix drift");

    // The parent owns newly identified original-AC obligations. Child freezes
    // remain immutable, and new parent cases cannot be passed by checklist.
    let extensions = &freeze["parent_extensions"];
    let new_exact = extensions["mandatory"].as_array().expect("parent exact cases");
    let new_conditional = extensions["conditional"].as_array().expect("parent residuals");
    assert_eq!(new_exact.len(), 1);
    assert_eq!(new_conditional.len(), 1);
    let derived = &new_exact[0];
    assert_eq!(derived["id"], "77-PARENT-DERIVED-P");
    assert_eq!(derived["owner"], "TASK-77");
    assert_eq!(derived["classification"], "mandatory");
    assert!(derived["sql"].as_str().is_some_and(|sql| !sql.is_empty()));
    assert!(derived["source_bag"].as_array().is_some_and(|bag| bag.len() == 3));
    assert!(derived["terminal_bag"].as_array().is_some_and(|bag| bag.len() == 3));
    assert_eq!(ids(&derived["source_row_ids"]), ids(&serde_json::json!([
        "unnest:1", "unnest:2", "unnest:3"
    ])));
    assert_eq!(derived["terminal_domains"]["v"]["nullable"], true);
    assert_eq!(derived["terminal_domains"]["v"]["lower"]["inclusive"], true);
    assert_eq!(derived["terminal_domains"]["v"]["upper"]["inclusive"], true);
    assert_eq!(derived["positive_proof"], "Exact");
    assert_eq!(derived["negative_membership"]["proof"], "Impossible");
    assert_eq!(derived["oracle_test"], "tests/task_77_derived_subquery.rs");

    let lateral = &new_conditional[0];
    assert_eq!(lateral["id"], "77-PARENT-LATERAL-R");
    assert_eq!(lateral["owner"], "TASK-77");
    assert_eq!(lateral["classification"], "conditional");
    assert_eq!(lateral["positive_proof"], "Residual");
    assert_eq!(lateral["negative_proof"], "Residual");
    assert_eq!(lateral["release_disposition"]["owner"], "TASK-77");
    assert!(lateral["release_disposition"]["reason"]
        .as_str().is_some_and(|reason| !reason.is_empty()));
    assert!(lateral["reason"].as_str().is_some_and(|reason| !reason.is_empty()));

    let mut required_exact = mandatory;
    let mut required_conditional = conditional;
    assert!(required_exact.insert(derived["id"].as_str().unwrap().to_owned()));
    assert!(required_conditional.insert(lateral["id"].as_str().unwrap().to_owned()));
    assert_eq!(required_exact, ids(&freeze["mandatory_case_ids"]));
    assert_eq!(required_conditional, ids(&freeze["conditional_case_ids"]));
    assert!(required_exact.is_disjoint(&required_conditional));
    assert_eq!(required_exact.len(), 25);
    assert_eq!(required_conditional.len(), 11);
    assert_eq!(freeze["total_mandatory_exact_cases"], 25);
    assert_eq!(freeze["total_reviewed_conditional_cases"], 11);

    let criteria = freeze["original_acceptance_criteria"].as_array().unwrap();
    assert_eq!(criteria.len(), 6);
    let mut covered_mandatory = BTreeSet::new();
    let mut covered_conditional = BTreeSet::new();
    for (index, ac) in criteria.iter().enumerate() {
        assert_eq!(ac["id"], index + 1);
        let exact = ids(&ac["mandatory_case_ids"]);
        let residual = ids(&ac["conditional_case_ids"]);
        assert!(!exact.is_empty(), "AC {} has no mandatory evidence", index + 1);
        assert!(exact.is_subset(&required_exact), "AC {} invents proof", index + 1);
        assert!(
            residual.is_subset(&required_conditional),
            "AC {} invents boundary", index + 1
        );
        assert!(ac["evidence"].as_array().is_some_and(|list| !list.is_empty()));
        covered_mandatory.extend(exact);
        covered_conditional.extend(residual);
    }
    for (ac, id) in [
        (0, "77-C137-P-values"),
        (0, "77-C138-P-rename"),
        (0, "77-C138-P-pivot"),
        (0, "77-PARENT-DERIVED-P"),
        (1, "77-C137-P-ordinality"),
        (1, "77-C137-N-null-array"),
        (1, "77-C138-N-unpivot"),
        (3, "77-C140-P-GROUP"),
        (3, "77-C140-P-JOIN"),
        (3, "77-PARENT-DERIVED-P"),
    ] {
        assert!(
            ids(&criteria[ac]["mandatory_case_ids"]).contains(id),
            "AC {} lacks required {}", ac + 1, id
        );
    }
    assert!(ids(&criteria[0]["conditional_case_ids"]).contains("77-C139-N-json-table"));
    assert!(ids(&criteria[0]["conditional_case_ids"]).contains("77-PARENT-LATERAL-R"));
    assert!(ids(&criteria[2]["conditional_case_ids"]).contains("77-C139-N-table-udf"));
    assert_eq!(covered_mandatory, required_exact);
    assert_eq!(covered_conditional, required_conditional);
}

fn plan(
    sql: &str,
    dialect: &str,
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalFiniteTableProof {
    let parser = dialect_from_name(dialect).expect("project dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
        .expect("public parser and analyzer");
    let layer = bundle.layers().last().expect("terminal layer");
    physical_finite_table_plan(&bundle, layer.id(), wanted)
}

fn number(value: i64) -> LiteralValue {
    LiteralValue::Number(value.to_string())
}

#[test]
fn grouped_fanout_preserves_exact_native_bag_and_impossible_membership() {
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST([1,1,2]) AS t(v) GROUP BY v";
    let db = Connection::open_in_memory().unwrap();
    let mut stmt = db.prepare(sql).unwrap();
    let mut native = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    native.sort();
    assert_eq!(native, vec![(1, 2), (2, 1)]);

    let PhysicalFiniteTableProof::Exact(exact) = plan(sql, "duckdb", None) else {
        panic!("finite group has complete source and terminal bags");
    };
    assert_eq!(exact.source_row_ids, ["unnest:1", "unnest:2", "unnest:3"]);
    let mut outcome = exact
        .terminal
        .iter()
        .map(|row| match (&row["v"], &row["n"]) {
            (LiteralValue::Number(v), LiteralValue::Number(n)) => {
                (v.parse::<i64>().unwrap(), n.parse::<i64>().unwrap())
            }
            _ => panic!("typed integer group expected"),
        })
        .collect::<Vec<_>>();
    outcome.sort();
    assert_eq!(outcome, native);
    assert_eq!(exact.terminal_domains["v"].admits_null(), Some(false));
    assert_eq!(exact.terminal_domains["n"].admits_null(), Some(false));

    let absent = BTreeMap::from([("v".to_owned(), number(3)), ("n".to_owned(), number(1))]);
    assert!(matches!(
        plan(sql, "duckdb", Some(&absent)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}

#[test]
fn star_projection_preserves_native_null_and_duplicate_physical_rows() {
    let source = "(VALUES (1,10,20), (1,NULL,30), (2,10,NULL)) AS t(id,a,b)";
    let sql = format!("SELECT * EXCLUDE b FROM {source}");
    let db = Connection::open_in_memory().unwrap();
    let reference = format!("SELECT id, a FROM {source}");
    let mut stmt = db.prepare(&reference).unwrap();
    let mut native = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    native.sort();
    assert_eq!(native, vec![(1, None), (1, Some(10)), (2, Some(10))]);

    let PhysicalFiniteTableProof::Exact(proof) = plan(&sql, "duckdb", None) else {
        panic!("finite star EXCLUDE has exact proof");
    };
    assert_eq!(proof.row_ids.len(), 3);
    let mut outcome = proof
        .terminal
        .iter()
        .map(|row| {
            let LiteralValue::Number(id) = &row["id"] else {
                panic!("integer id expected")
            };
            let value = match &row["a"] {
                LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
                LiteralValue::Null => None,
                _ => panic!("nullable integer expected"),
            };
            (id.parse::<i64>().unwrap(), value)
        })
        .collect::<Vec<_>>();
    outcome.sort();
    assert_eq!(outcome, native);
    assert_eq!(proof.terminal_domains["a"].admits_null(), Some(true));
    assert!(!matches!(
        proof.terminal_domains["id"],
        ValueDomain::Unknown(_)
    ));
}

#[test]
fn opaque_source_does_not_become_a_false_exactness_claim() {
    assert!(matches!(
        plan(
            "SELECT v, COUNT(*) AS n FROM UNNEST(opaque_array) AS t(v) GROUP BY v",
            "duckdb",
            None
        ),
        PhysicalFiniteTableProof::Residual { .. }
    ));
}
