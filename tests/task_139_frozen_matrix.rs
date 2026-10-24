//! TASK-139 task-owned, complete-bag and typed-domain acceptance oracle.
//!
//! Values and expected identities come from the frozen inventory, not from
//! the implementation's row enumerator.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_finite_table_plan, FiniteColumnType, LiteralValue,
    PhysicalFiniteTableProof, SetMode, SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-139-acceptance-freeze.json"))
        .expect("versioned generator matrix")
}

fn value(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        Value::String(s) => LiteralValue::Text(s.clone()),
        other => panic!("unattested value: {other:?}"),
    }
}

fn row(value: &Value) -> BTreeMap<String, LiteralValue> {
    value
        .as_object()
        .expect("complete frozen row")
        .iter()
        .map(|(name, value)| (name.clone(), self::value(value)))
        .collect()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rows = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    rows.sort();
    rows
}

fn native(sql: &str) -> Vec<BTreeMap<String, LiteralValue>> {
    let conn = Connection::open_in_memory().expect("DuckDB");
    let mut stmt = conn.prepare(sql).expect("independent terminal SQL");
    stmt.query_map([], |r| {
        let number = r.get::<_, Option<i64>>(0)?;
        Ok(BTreeMap::from([(
            "v".to_string(),
            number.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        )]))
    })
    .expect("execute terminal oracle")
    .collect::<Result<Vec<_>, _>>()
    .expect("complete bag")
}

#[test]
fn all_mandatory_frozen_source_free_generator_bags_have_exact_domain_and_absence() {
    let frozen = freeze();
    let mut proven = 0;
    for case in frozen["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory" {
            continue;
        }
        let sql = case["sql"].as_str().expect("SQL");
        let dialect = case["canonical_dialect"]
            .as_str()
            .expect("attested parser dialect");
        let parser = dialect_from_name(dialect).expect("supported dialect");
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
        let target = bundle.layers().last().expect("terminal");
        let PhysicalFiniteTableProof::Exact(exact) =
            physical_finite_table_plan(&bundle, target.id(), None)
        else {
            panic!("mandatory generator must be exact: {sql}");
        };
        let expected = case["terminal_rows"]
            .as_array()
            .expect("frozen complete rows")
            .iter()
            .map(row)
            .collect::<Vec<_>>();
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected),
            "{sql}: whole frozen bag"
        );
        assert_eq!(
            bag(&exact.terminal),
            bag(&native(case["oracle_sql"].as_str().expect("DuckDB oracle"))),
            "{sql}: native independent complete terminal oracle"
        );
        let expected_ids = case["producer_row_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            exact.row_ids, expected_ids,
            "{sql}: duplicate producer identities"
        );
        assert!(
            exact.source_rows.is_empty(),
            "source-free literal creates no physical relation"
        );
        assert!(
            exact.source_row_ids.is_empty(),
            "no invented physical input rows"
        );
        assert!(
            exact.contributors.is_empty(),
            "no fictitious physical source instances"
        );
        assert_eq!(
            exact.terminal_column_types["v"],
            FiniteColumnType::Integer,
            "{sql}: authoritative literal element type"
        );
        let ValueDomain::Set(domain) = &exact.terminal_domains["v"] else {
            panic!("{sql}: finite values need a typed set domain");
        };
        assert_eq!(domain.mode(), SetMode::Include);
        let spec = &case["output_domains"]["v"];
        assert_eq!(
            exact.terminal_domains["v"].admits_null(),
            spec["nullable"].as_bool()
        );
        let mut observed = domain
            .values()
            .iter()
            .filter_map(|v| match v.value() {
                LiteralValue::Number(n) => Some(n.parse::<i64>().expect("integer")),
                LiteralValue::Null => None,
                other => panic!("{sql}: incorrect literal type {other:?}"),
            })
            .collect::<Vec<_>>();
        observed.sort_unstable();
        let mut frozen_values = spec["values"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_i64)
            .collect::<Vec<_>>();
        frozen_values.sort_unstable();
        assert_eq!(observed, frozen_values, "{sql}: allowed finite values");
        assert_eq!(observed.first().copied(), spec["lower"]["value"].as_i64());
        assert_eq!(observed.last().copied(), spec["upper"]["value"].as_i64());
        assert_eq!(spec["lower"]["inclusive"], true);
        assert_eq!(spec["upper"]["inclusive"], true);

        let wanted = row(&case["rejected"]);
        assert!(
            matches!(
                physical_finite_table_plan(&bundle, target.id(), Some(&wanted)),
                PhysicalFiniteTableProof::Impossible { .. }
            ),
            "{sql}: closed-world NOT TRUE row"
        );
        let present = expected.first().expect("positive case");
        assert!(
            matches!(
                physical_finite_table_plan(&bundle, target.id(), Some(present)),
                PhysicalFiniteTableProof::Exact(_)
            ),
            "{sql}: present typed row must be Exact"
        );
        proven += 1;
    }
    assert_eq!(
        proven, 3,
        "all mandatory finite-generator cases were checked"
    );
}

#[test]
fn exact_static_dialect_matrix_is_independently_observed_for_all_frozen_cases() {
    let frozen = freeze();
    let mut checked = 0;
    for case in frozen["cases"].as_array().unwrap() {
        let sql = case["sql"].as_str().expect("SQL");
        let cells = case["dialect_matrix"]
            .as_array()
            .expect("13 dialect dispositions");
        assert_eq!(cells.len(), 13);
        for cell in cells {
            let dialect = cell["dialect"].as_str().unwrap();
            let parser = dialect_from_name(dialect).expect("project dialect");
            let observed = match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            {
                Err(_) => "parser_boundary",
                Ok(bundle) => match bundle.layers().last() {
                    None => "residual",
                    Some(layer) => match physical_finite_table_plan(&bundle, layer.id(), None) {
                        PhysicalFiniteTableProof::Exact(_) => "exact",
                        PhysicalFiniteTableProof::Residual { .. }
                        | PhysicalFiniteTableProof::Impossible { .. } => "residual",
                    },
                },
            };
            assert_eq!(cell["classification"], observed, "{dialect}: {sql}");
            assert_eq!(
                cell["certified"], true,
                "{dialect}: no unverified parser cell"
            );
            // A parser-accepted opaque generator never inherits a finite row bag.
            if case["classification"] == "conditional" {
                assert_ne!(
                    observed, "exact",
                    "{dialect}: vendor/opaque law is unproved"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 91);
}

#[test]
fn all_conditional_vendor_boundaries_have_reviewed_owner_and_no_fake_exactness() {
    let frozen = freeze();
    let mut reviewed = 0;
    for case in frozen["cases"].as_array().unwrap() {
        if case["classification"] != "conditional" {
            continue;
        }
        assert_eq!(case["release_disposition"]["owner"], "TASK-139");
        assert_eq!(
            case["release_disposition"]["status"],
            "reviewed_conditional_residual"
        );
        assert!(
            case["release_disposition"]["reason"]
                .as_str()
                .unwrap()
                .len()
                >= 30
        );
        assert_eq!(case["release_disposition"]["native_runtime"], "uncertified");
        reviewed += 1;
    }
    assert_eq!(reviewed, 4);
}

#[test]
fn complete_literal_array_inputs_are_independently_executed_as_duckdb_source_oracles() {
    for case in freeze()["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory" {
            continue;
        }
        let sql = case["source_oracle_sql"].as_str().expect("source SQL");
        let expected = case["source_oracle_expected"].as_array().unwrap();
        let db = Connection::open_in_memory().expect("DuckDB source");
        let mut statement = db.prepare(sql).expect("source literal SQL");
        let observed = statement
            .query_row([], |row| {
                (0..expected.len())
                    .map(|i| row.get::<_, Option<i64>>(i))
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("one authoritative literal-array row");
        let expected = expected
            .iter()
            .map(|v| v.as_i64())
            .collect::<Vec<_>>();
        assert_eq!(observed, expected, "{sql}: all array slots including NULL");
    }
}
