//! TASK-123 complete two-source bags and independent DuckDB terminal oracles.
mod common;

use common::DIALECTS;
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_join_aggregate_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalAggregateTruth, PhysicalJoinedAggregateProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SetMode,
    SqlInput, ValueDomain,
};
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-123-acceptance-freeze.json"))
        .expect("immutable task-specific SQL and physical source matrix")
}

fn schemas() -> Vec<RelationSchema> {
    ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "k", "x"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            column,
                            if column == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("typed source")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect()
}

fn literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(n) => LiteralValue::Number(n.to_string()),
        Value::String(s) => LiteralValue::Text(s.clone()),
        _ => panic!("invalid frozen source/terminal value: {value}"),
    }
}

fn states(case: &Value) -> Vec<PhysicalWriteState> {
    ["l", "r"]
        .into_iter()
        .map(|name| {
            PhysicalWriteState::new(
                name,
                case["source_bags"][name]
                    .as_array()
                    .expect("complete physical source")
                    .iter()
                    .map(|row| {
                        PhysicalWriteRow::new(
                            row.as_object()
                                .expect("source row")
                                .iter()
                                .map(|(column, value)| (column.clone(), literal(value)))
                                .collect(),
                        )
                        .expect("physical row")
                    })
                    .collect(),
            )
            .expect("closed physical bag")
        })
        .collect()
}

fn normalized(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rows = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn expected(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"]
        .as_array()
        .expect("expected terminal")
        .iter()
        .map(|row| {
            row.as_object()
                .expect("terminal row")
                .iter()
                .map(|(column, value)| (column.clone(), literal(value)))
                .collect()
        })
        .collect()
}

fn parsed(case: &Value, dialect: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let parser = dialect_from_name(dialect).expect("public parser dialect");
    let sql = SqlInput::inline(case["sql"].as_str().expect("SQL"));
    let input = ConfiguredSqlInput::new(
        case["id"].as_str().expect("fixture identity"),
        &sql,
        dialect,
        parser.as_ref(),
    );
    let catalog = RelationCatalog::from_schemas(&schemas()).expect("catalog");
    analyze_configured_inputs_with_catalog(&[input], &catalog).map_err(|e| e.to_string())
}

fn joined_proof(case: &Value, dialect: &str) -> PhysicalJoinedAggregateProof {
    let bundle = parsed(case, dialect).expect("mandatory SQL parses");
    let target = bundle.layers().last().expect("terminal layer").id();
    physical_finite_join_aggregate_plan(&bundle, target, &states(case))
}

fn duckdb_bag(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("independent DuckDB engine");
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, k INTEGER, x INTEGER);
         CREATE TABLE r(id VARCHAR, k INTEGER, x INTEGER)",
    )
    .expect("physical source schema");
    for source in ["l", "r"] {
        let rows = case["source_bags"][source]
            .as_array()
            .expect("closed source");
        for row in rows {
            let number = |column: &str| {
                row[column]
                    .as_i64()
                    .map_or_else(|| "NULL".to_string(), |value| value.to_string())
            };
            db.execute_batch(&format!(
                "INSERT INTO {source} VALUES ('{}', {}, {})",
                row["id"].as_str().expect("stable ID"),
                number("k"),
                number("x")
            ))
            .expect("source insertion");
        }
        let mut statement = db
            .prepare(&format!("SELECT id, k, x FROM {source}"))
            .expect("physical snapshot readback");
        let from_engine = statement
            .query_map([], |row| {
                Ok(BTreeMap::from([
                    (
                        "id".to_string(),
                        LiteralValue::Text(row.get::<_, String>(0)?),
                    ),
                    (
                        "k".to_string(),
                        row.get::<_, Option<i64>>(1)?
                            .map(|n| LiteralValue::Number(n.to_string()))
                            .unwrap_or(LiteralValue::Null),
                    ),
                    (
                        "x".to_string(),
                        row.get::<_, Option<i64>>(2)?
                            .map(|n| LiteralValue::Number(n.to_string()))
                            .unwrap_or(LiteralValue::Null),
                    ),
                ]))
            })
            .expect("query complete source")
            .collect::<Result<Vec<_>, _>>()
            .expect("physical source bag");
        let expected_source = rows
            .iter()
            .map(|row| {
                row.as_object()
                    .expect("row")
                    .iter()
                    .map(|(name, value)| (name.clone(), literal(value)))
                    .collect()
            })
            .collect::<Vec<BTreeMap<String, LiteralValue>>>();
        assert_eq!(
            normalized(&from_engine),
            normalized(&expected_source),
            "entire DuckDB source bag and stable identities"
        );
    }
    let columns = case["terminal_columns"]
        .as_array()
        .expect("projected columns");
    let mut sql = db
        .prepare(case["sql"].as_str().expect("query SQL"))
        .expect("independent SQL oracle");
    sql.query_map([], |row| {
        let mut out = BTreeMap::new();
        for (index, column) in columns.iter().enumerate() {
            let number = row.get::<_, Option<i64>>(index)?;
            out.insert(
                column.as_str().expect("column name").to_string(),
                number
                    .map(|n| LiteralValue::Number(n.to_string()))
                    .unwrap_or(LiteralValue::Null),
            );
        }
        Ok(out)
    })
    .expect("executed DuckDB SQL")
    .collect::<Result<Vec<_>, _>>()
    .expect("complete terminal bag")
}

fn pair_set(value: &Value) -> Vec<[String; 2]> {
    value
        .as_array()
        .expect("complete pairs")
        .iter()
        .map(|pair| {
            [
                pair[0].as_str().expect("left identity").to_string(),
                pair[1].as_str().expect("right identity").to_string(),
            ]
        })
        .collect()
}

#[test]
fn joined_grouped_having_and_empty_global_bags_match_independent_duckdb() {
    let inventory = freeze();
    for case in inventory["cases"].as_array().expect("cases") {
        if case["kind"] != "mandatory" {
            continue;
        }
        let id = case["id"].as_str().expect("case identity");
        let result = joined_proof(case, "generic");
        let PhysicalJoinedAggregateProof::Exact(exact) = result else {
            panic!("{id}: exact proof required: {result:?}");
        };
        assert_eq!(
            exact.selected,
            pair_set(&case["full_selected_pairs"]),
            "{id}: all selected physical pairs"
        );
        assert_eq!(
            exact.rejected,
            pair_set(&case["full_rejected_pairs"]),
            "{id}: all NOT TRUE physical pairs"
        );
        assert_eq!(
            normalized(&exact.terminal),
            normalized(&expected(case)),
            "{id}"
        );
        assert_eq!(
            normalized(&duckdb_bag(case)),
            normalized(&expected(case)),
            "{id}: independent DuckDB terminal"
        );
        assert_eq!(
            exact.groups.len(),
            case["groups"].as_array().expect("complete groups").len(),
            "{id}: all surviving and rejected group identities"
        );
        for (actual, group) in exact
            .groups
            .iter()
            .zip(case["groups"].as_array().expect("groups"))
        {
            let expected_truth = match group["having"].as_str() {
                Some("TRUE") => Some(PhysicalAggregateTruth::True),
                Some("FALSE") => Some(PhysicalAggregateTruth::False),
                Some("UNKNOWN") => Some(PhysicalAggregateTruth::Unknown),
                None => None,
                truth => panic!("unexpected frozen group truth: {truth:?}"),
            };
            assert_eq!(actual.having_truth, expected_truth);
            let ids = group["source_row_ids"]
                .as_array()
                .expect("group membership")
                .iter()
                .map(|v| v.as_str().expect("row id").to_string())
                .collect::<Vec<_>>();
            assert_eq!(actual.source_row_ids, ids, "{id}: group source identities");
            let expected_contributions = group["aggregate_row_ids"]
                .as_object()
                .expect("contributors")
                .iter()
                .map(|(name, ids)| {
                    (
                        name.clone(),
                        ids.as_array()
                            .expect("contributor identities")
                            .iter()
                            .map(|v| v.as_str().expect("pair").to_string())
                            .collect(),
                    )
                })
                .collect::<BTreeMap<String, Vec<String>>>();
            assert_eq!(actual.aggregate_row_ids, expected_contributions);
        }
        for expected_domain in case["typed_terminal_domains"].as_array().expect("domains") {
            let name = expected_domain["column"].as_str().expect("column");
            let domain = exact
                .terminal_domains
                .get(name)
                .expect("column outcome domain");
            let wanted = expected_domain["values"]
                .as_array()
                .expect("finite expected values")
                .iter()
                .map(|value| format!("{:?}", literal(value)))
                .collect::<Vec<_>>();
            if wanted.is_empty() {
                assert!(
                    matches!(domain, ValueDomain::Empty),
                    "{id}:{name}: empty set"
                );
            } else {
                let ValueDomain::Set(set) = domain else {
                    panic!("{id}:{name}: not an exact finite set: {domain:?}")
                };
                assert_eq!(set.mode(), SetMode::Include);
                let mut got = set
                    .values()
                    .iter()
                    .map(|v| format!("{:?}", v.value()))
                    .collect::<Vec<_>>();
                let mut wanted = wanted;
                got.sort();
                wanted.sort();
                assert_eq!(got, wanted, "{id}:{name}: exact included values");
                let has_null = set
                    .values()
                    .iter()
                    .any(|value| value.value() == &LiteralValue::Null);
                assert_eq!(
                    has_null,
                    expected_domain["contains_null"].as_bool().expect("NULL membership"),
                    "{id}:{name}: exact NULL membership"
                );
                let numbers = set
                    .values()
                    .iter()
                    .filter_map(|value| match value.value() {
                        LiteralValue::Number(value) => Some(value.parse::<i64>().expect("typed integer")),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if !numbers.is_empty() {
                    let bounds = &expected_domain["inclusive_bounds"];
                    assert_eq!(numbers.iter().min().copied(), bounds["lower"].as_i64());
                    assert_eq!(numbers.iter().max().copied(), bounds["upper"].as_i64());
                    assert_eq!(bounds["lower_inclusive"], true);
                    assert_eq!(bounds["upper_inclusive"], true);
                }
            }
        }
    }
}

#[test]
fn every_public_dialect_classifies_the_frozen_joined_sql() {
    let inventory = freeze();
    for dialect in DIALECTS {
        for case in inventory["cases"].as_array().expect("cases") {
            if case["kind"] == "mandatory" {
                let proof = joined_proof(case, dialect);
                assert!(
                    matches!(proof, PhysicalJoinedAggregateProof::Exact(_)),
                    "{dialect}:{}: cannot weaken portable mandatory bag: {proof:?}",
                    case["id"]
                );
            } else {
                match parsed(case, dialect) {
                    Ok(bundle) => {
                        let layer = bundle.layers().last().expect("target").id();
                        let result =
                            physical_finite_join_aggregate_plan(&bundle, layer, &states(case));
                        assert!(
                            matches!(result, PhysicalJoinedAggregateProof::Residual { .. }),
                            "{dialect}:{}: extension cannot silently become exact: {result:?}",
                            case["id"]
                        );
                    }
                    Err(reason) => {
                        eprintln!("{dialect}:{}: parser boundary: {reason}", case["id"]);
                    }
                }
            }
        }
    }
}

#[test]
fn rejected_null_join_pairs_and_unsupported_scalars_never_become_false_proofs() {
    let inventory = freeze();
    let positive = &inventory["cases"][0];
    let exact = joined_proof(positive, "generic");
    let PhysicalJoinedAggregateProof::Exact(exact) = exact else {
        panic!("complete nullable join bag must be exact");
    };
    assert!(
        exact
            .rejected
            .iter()
            .all(|pair| !exact.selected.contains(pair)),
        "SQL NULL equality and WHERE UNKNOWN cannot be treated as TRUE"
    );
    let mut invalid = positive.clone();
    invalid["sql"] = Value::String(
        "SELECT l.k AS k, COUNT(*) AS row_count FROM l JOIN r ON l.k=r.k WHERE ABS(l.x)>0 GROUP BY l.k"
            .to_string(),
    );
    if let Ok(bundle) = parsed(&invalid, "generic") {
        let target = bundle.layers().last().expect("target").id();
        assert!(
            matches!(
                physical_finite_join_aggregate_plan(&bundle, target, &states(positive)),
                PhysicalJoinedAggregateProof::Residual { .. }
            ),
            "unsupported computed WHERE must fail closed"
        );
    } // A parser error is an equally conservative unsupported boundary.
}

#[test]
fn empty_join_never_proves_comparison_without_numeric_execution_law() {
    let case = &freeze()["cases"][2];
    let mut invalid = case.clone();
    invalid["sql"] = Value::String(
        "SELECT COUNT(*) AS row_count FROM l INNER JOIN r ON l.k = r.k WHERE l.x = TRUE"
            .to_string(),
    );
    if let Ok(bundle) = parsed(&invalid, "generic") {
        let target = bundle.layers().last().expect("target").id();
        assert!(
            matches!(
                physical_finite_join_aggregate_plan(&bundle, target, &states(case)),
                PhysicalJoinedAggregateProof::Residual { .. }
            ),
            "empty physical source does not certify integer/Boolean coercion"
        );
    }
}
