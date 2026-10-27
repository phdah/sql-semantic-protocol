//! Parent TASK-74: independent frozen acceptance and native cross-operator regression.
//! Child cases remain authoritative; this guard never regenerates their expected bags.

use duckdb::Connection;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const CHILDREN: [(&str, &str); 5] = [
    (
        "TASK-125",
        include_str!("../docs/task-125-acceptance-freeze.json"),
    ),
    (
        "TASK-126",
        include_str!("../docs/task-126-acceptance-freeze.json"),
    ),
    (
        "TASK-189",
        include_str!("../docs/task-189-acceptance-freeze.json"),
    ),
    (
        "TASK-127",
        include_str!("../docs/task-127-acceptance-freeze.json"),
    ),
    (
        "TASK-128",
        include_str!("../docs/task-128-acceptance-freeze.json"),
    ),
];

fn parent() -> Value {
    serde_json::from_str(include_str!("../docs/task-74-acceptance-freeze.json"))
        .expect("versioned TASK-74 parent inventory")
}

fn values(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .expect("frozen array")
        .iter()
        .map(|item| item.as_str().expect("frozen string").to_owned())
        .collect()
}

fn exact(task: &str, case: &Value) -> bool {
    match task {
        "TASK-125" => case["exact"] == true,
        "TASK-127" => case["proof"] == "mandatory_exact",
        "TASK-128" => case["classification"] == "exact_required" && case["proof"] == "Exact",
        _ => case["proof"] == "Exact",
    }
}

#[test]
fn parent_inventory_covers_every_child_case_dialect_adapter_and_original_criterion() {
    let freeze = parent();
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["task"], "TASK-74");
    assert_eq!(freeze["release_approved"], false);
    let dialects = values(&freeze["required_dialects"]);
    let adapters = values(&freeze["required_adapters"]);
    assert_eq!(dialects.len(), 13);
    assert_eq!(
        adapters,
        values(&json!(["direct_sql", "dbt_compiled", "odcs_schema"]))
    );

    let mut mandatory = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    let mut fixtures = BTreeSet::new();

    for (task, text) in CHILDREN {
        let child: Value = serde_json::from_str(text).expect("child acceptance freeze");
        assert_eq!(child["owner"], task);
        assert_eq!(child["parent"], "TASK-74");
        let record = &freeze["source_of_truth"][task];
        assert_eq!(
            record["freeze"].as_str().expect("child path"),
            format!("docs/task-{}-acceptance-freeze.json", &task[5..])
        );
        assert_eq!(record["version"], child["version"]);
        let child_dialects = child.get("required_dialects").unwrap_or(&child["dialects"]);
        assert_eq!(
            values(child_dialects),
            dialects,
            "{task} parser dialect coverage"
        );
        let child_adapters = child.get("required_adapters").unwrap_or(&child["adapters"]);
        let normalized = values(child_adapters)
            .into_iter()
            .map(|name| {
                if name == "compiled_dbt_manifest_catalog" {
                    "dbt_compiled".to_owned()
                } else {
                    name
                }
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(normalized, adapters, "{task} canonical adapter parity");
        let child_fixtures = values(&child["fixture_ids"]);
        assert_eq!(values(&record["fixture_ids"]), child_fixtures);
        fixtures.extend(child_fixtures);

        let mut task_mandatory = BTreeSet::new();
        let mut task_conditional = BTreeSet::new();
        let cases = child["cases"].as_array().expect("frozen cases");
        let numeric = child.get("numeric_oracle_cases").and_then(Value::as_array);
        for case in cases.iter().chain(numeric.into_iter().flatten()) {
            let case_id = case["id"].as_str().expect("stable fixture variant");
            assert!(!case["sql"].as_str().unwrap_or("").is_empty(), "{case_id}");
            if let Some(parser) = case
                .get("parser_status_by_dialect")
                .or_else(|| case.get("dialect_cells"))
                .or_else(|| child["parser_status_matrix"].get(case_id))
            {
                let names = parser
                    .as_object()
                    .expect("per-dialect parser classifications")
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                assert_eq!(names, dialects, "{case_id} missing parser cells");
            }
            if exact(task, case) {
                assert!(
                    case.get("full_terminal_bag")
                        .or_else(|| case.get("expected_full_terminal_bag"))
                        .is_some_and(Value::is_array),
                    "{case_id} has no closed terminal oracle"
                );
                assert!(
                    case.get("typed_terminal_domains")
                        .is_some_and(|domain| domain.is_array() || domain.is_object()),
                    "{case_id} has no typed outcome domain"
                );
                assert!(
                    task_mandatory.insert(case_id.to_owned()),
                    "duplicate {case_id}"
                );
                assert!(mandatory.insert(case_id.to_owned()), "duplicate {case_id}");
            } else {
                assert!(
                    task_conditional.insert(case_id.to_owned()),
                    "duplicate {case_id}"
                );
                assert!(
                    conditional.insert(case_id.to_owned()),
                    "duplicate {case_id}"
                );
            }
        }
        assert_eq!(
            task_mandatory,
            values(&record["mandatory_case_ids"]),
            "{task}"
        );
        assert_eq!(
            task_conditional,
            values(&record["conditional_case_ids"]),
            "{task}"
        );
        assert!(
            child
                .get("mandatory_unverified")
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty)),
            "{task} has unverified mandatory cases"
        );
    }

    assert_eq!(mandatory, values(&freeze["mandatory_case_ids"]));
    assert_eq!(conditional, values(&freeze["conditional_case_ids"]));
    assert_eq!(fixtures, values(&freeze["fixture_ids"]));
    assert!(mandatory.is_disjoint(&conditional));
    assert_eq!(mandatory.len(), 34);
    assert_eq!(conditional.len(), 22);

    let criteria = freeze["original_acceptance_criteria"]
        .as_array()
        .expect("original parent AC mapping");
    assert_eq!(criteria.len(), 6);
    for (index, criterion) in criteria.iter().enumerate() {
        assert_eq!(criterion["id"], index + 1);
        let exact_refs = values(&criterion["mandatory_case_ids"]);
        assert!(!exact_refs.is_empty());
        assert!(
            exact_refs.is_subset(&mandatory),
            "AC {} refers to missing mandatory evidence",
            index + 1
        );
        assert!(
            values(&criterion["conditional_case_ids"]).is_subset(&conditional),
            "AC {} promotes an unreviewed conditional",
            index + 1
        );
    }
}

fn canonical_bag(rows: &[Value]) -> Vec<String> {
    let mut values = rows.iter().map(Value::to_string).collect::<Vec<_>>();
    values.sort();
    values
}

fn read_bag(db: &Connection, sql: &str) -> Vec<Value> {
    let mut statement = db.prepare(sql).expect("native DuckDB query");
    statement
        .query_map([], |row| {
            let mut result = Map::new();
            for col in 0..row.as_ref().column_count() {
                let key = row.as_ref().column_name(col)?.to_owned();
                let value = row.get::<_, Option<i64>>(col)?;
                result.insert(key, value.map_or(Value::Null, |n| json!(n)));
            }
            Ok(Value::Object(result))
        })
        .expect("native DuckDB query execution")
        .collect::<Result<Vec<_>, _>>()
        .expect("complete native result")
}

fn run_complete_native_case(case: &Value) -> Vec<Value> {
    let db = Connection::open_in_memory().expect("native DuckDB");
    for (table, schema) in case["source_schema"].as_object().expect("source schemas") {
        let columns = schema.as_object().expect("typed column schema");
        let definitions = columns
            .iter()
            .map(|(col, kind)| format!("{col} {}", kind.as_str().expect("column type")))
            .collect::<Vec<_>>()
            .join(", ");
        db.execute_batch(&format!("CREATE TABLE {table} ({definitions})"))
            .expect("typed table");
        let col_names = columns.keys().cloned().collect::<Vec<_>>();
        let names = col_names.join(", ");
        let bag = case["physical_source_bags"][table]
            .as_array()
            .expect("closed physical source");
        for row in bag {
            let cells = col_names
                .iter()
                .map(|col| row[col.as_str()].to_string())
                .collect::<Vec<_>>()
                .join(", ");
            db.execute_batch(&format!("INSERT INTO {table} ({names}) VALUES ({cells})"))
                .expect("insert attested physical row");
        }
        let actual = read_bag(&db, &format!("SELECT {names} FROM {table}"));
        assert_eq!(
            canonical_bag(&actual),
            canonical_bag(bag),
            "{table} source must be read back from native DuckDB"
        );
    }

    let actual = read_bag(&db, case["sql"].as_str().expect("SQL"));
    let expected = case["expected_full_terminal_bag"]
        .as_array()
        .expect("closed terminal bag");
    assert_eq!(
        canonical_bag(&actual),
        canonical_bag(expected),
        "{} complete terminal bag",
        case["id"]
    );
    for (column, domain) in case["typed_terminal_domains"]
        .as_object()
        .expect("typed domains")
    {
        let mut numeric = expected
            .iter()
            .filter_map(|row| row[column.as_str()].as_i64())
            .collect::<Vec<_>>();
        assert!(!numeric.is_empty(), "exact outcome domain for {column}");
        numeric.sort_unstable();
        assert_eq!(
            domain["inclusive_bounds"],
            json!([numeric[0], numeric[numeric.len() - 1]]),
            "{} {column} typed inclusive bounds",
            case["id"]
        );
        assert_eq!(
            domain["nullable"],
            expected.iter().any(|row| row[column.as_str()].is_null()),
            "{} {column} nullability",
            case["id"]
        );
    }
    actual
}

#[test]
fn joined_and_grouped_qualify_reexecute_complete_native_source_and_terminal_bags() {
    let child: Value =
        serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json"))
            .expect("frozen cross-operator children");
    let required = values(&parent()["cross_operator_native_cases"]);
    assert_eq!(required, values(&json!(["74-C127-P4", "74-C127-P5"])));

    for case in child["cases"].as_array().expect("TASK-127 cases") {
        let id = case["id"].as_str().expect("case id");
        if required.contains(id) {
            assert_eq!(case["proof"], "mandatory_exact");
            let actual = run_complete_native_case(case);
            assert!(!actual.is_empty(), "{id} must have a positive witness");
        }
    }
}

#[test]
fn projected_outer_rank_predicate_has_exact_true_and_not_true_native_bags() {
    use sql_semantic_protocol::{ProtocolStatement, WindowWitnessDirection};
    use sqlparser::dialect::GenericDialect;

    let oracle = &parent()["projected_window_oracle"];
    assert_eq!(
        values(&oracle["case_ids"]),
        values(&json!(["74-PARENT-PROJECTED-P", "74-PARENT-PROJECTED-N"]))
    );
    let sql = oracle["positive_sql"].as_str().expect("projected rank SQL");
    let protocol = sql_semantic_protocol::analyze_sql(sql, "generic", &GenericDialect {})
        .expect("public parser-independent SQL analysis");
    let witness = match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query
            .window_witness()
            .expect("projected ROW_NUMBER witness"),
        other => panic!("projected query missing: {other:?}"),
    };
    assert!(matches!(
        witness.qualifying(),
        WindowWitnessDirection::Exact(case)
            if case.min_preceding() == 0 && case.max_preceding() == Some(1)
    ));
    assert!(matches!(
        witness.rejected(),
        WindowWitnessDirection::Exact(case)
            if case.min_preceding() == 2 && case.max_preceding().is_none()
    ));

    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch("CREATE TABLE events(account_id INTEGER, score INTEGER)")
        .expect("physical source schema");
    for row in oracle["full_source_bag"]
        .as_array()
        .expect("full source bag")
    {
        db.execute_batch(&format!(
            "INSERT INTO events VALUES ({}, {})",
            row["account_id"], row["score"]
        ))
        .expect("native source row");
    }
    let physical = read_bag(&db, "SELECT account_id, score FROM events");
    assert_eq!(
        canonical_bag(&physical),
        canonical_bag(oracle["full_source_bag"].as_array().expect("source bag"))
    );
    let actual_positive = read_bag(&db, sql);
    assert_eq!(
        canonical_bag(&actual_positive),
        canonical_bag(
            oracle["expected_positive_terminal_bag"]
                .as_array()
                .expect("exact positive terminal bag")
        )
    );
    let rejected = read_bag(&db, oracle["negative_sql"].as_str().expect("NOT TRUE SQL"));
    assert_eq!(
        canonical_bag(&rejected),
        canonical_bag(
            oracle["expected_not_true_physical_bag"]
                .as_array()
                .expect("exact NOT TRUE bag")
        )
    );
    let positive_ranks = actual_positive
        .iter()
        .filter_map(|row| row["rn"].as_i64())
        .collect::<BTreeSet<_>>();
    assert_eq!(positive_ranks, [1, 2].into_iter().collect());
    let domain = &oracle["terminal_domain"]["rn"];
    assert_eq!(domain["lower"], 1);
    assert_eq!(domain["upper"], 2);
    assert_eq!(domain["nullable"], false);
    assert_eq!(domain["lower_inclusive"], true);
    assert_eq!(domain["upper_inclusive"], true);
}
