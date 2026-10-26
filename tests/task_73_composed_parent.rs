//! Independent TASK-73 AC #2 witness: reuse the existing TASK-115 exact
//! three-source WHERE/JOIN/projection/GROUP BY/HAVING implementation and frozen
//! physical bags rather than reimplementing that semantic law.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_chain_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteChainProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
        .expect("independently merged multi-join fixture inventory")
}

fn parent() -> Value {
    serde_json::from_str(include_str!("../docs/task-73-acceptance-freeze.json"))
        .expect("TASK-73 parent acceptance freeze")
}

fn scalar(value: &Value) -> LiteralValue {
    if value.is_null() {
        LiteralValue::Null
    } else if let Some(number) = value.as_i64() {
        LiteralValue::Number(number.to_string())
    } else {
        LiteralValue::Text(value.as_str().expect("source text").to_string())
    }
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    ["l", "r", "t"]
        .into_iter()
        .map(|relation| {
            PhysicalWriteState::new(
                relation,
                case["physical_source_bags"][relation]
                    .as_array()
                    .expect("complete physical source")
                    .iter()
                    .map(|row| {
                        PhysicalWriteRow::new(
                            row.as_object()
                                .expect("physical columns")
                                .iter()
                                .map(|(name, value)| (name.clone(), scalar(value)))
                                .collect(),
                        )
                        .expect("typed physical row")
                    })
                    .collect(),
            )
            .expect("closed physical relation")
        })
        .collect()
}

fn catalog(case: &Value) -> RelationCatalog {
    let mut schemas = ["l", "r", "t"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                case["source_column_types"][relation]
                    .as_object()
                    .expect("attested source datatypes")
                    .iter()
                    .map(|(column, datatype)| {
                        SchemaColumn::from_sql_type(
                            column,
                            datatype
                                .as_str()
                                .expect("typed source")
                                .split_whitespace()
                                .next()
                                .expect("SQL type"),
                            "postgresql",
                        )
                        .expect("canonical typed column")
                    })
                    .collect(),
            )
            .expect("typed source relation")
        })
        .collect::<Vec<_>>();
    schemas.push(
        RelationSchema::new(
            "stage",
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                .expect("projected group key")],
        )
        .expect("intermediate projected relation"),
    );
    RelationCatalog::from_schemas(&schemas).expect("complete catalog")
}

fn expected_triplets(case: &Value, key: &str) -> Vec<[String; 3]> {
    let mut items = case[key]
        .as_array()
        .expect("full physical identity bag")
        .iter()
        .map(|value| {
            std::array::from_fn(|index| {
                value[index]
                    .as_str()
                    .expect("frozen physical identity")
                    .to_string()
            })
        })
        .collect::<Vec<_>>();
    items.sort();
    items
}

fn expected_terminal(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["expected_terminal_bag"]
        .as_array()
        .expect("full group output bag")
        .iter()
        .map(|row| {
            row.as_object()
                .expect("typed group row")
                .iter()
                .map(|(column, value)| (column.clone(), scalar(value)))
                .collect()
        })
        .collect()
}

fn duckdb_output(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("real DuckDB");
    for relation in ["l", "r", "t"] {
        let columns = case["source_column_types"][relation]
            .as_object()
            .expect("source declarations");
        let declaration = columns
            .iter()
            .map(|(name, datatype)| format!("{name} {}", datatype.as_str().expect("type")))
            .collect::<Vec<_>>()
            .join(", ");
        db.execute_batch(&format!("CREATE TABLE {relation} ({declaration})"))
            .expect("create physical source");
        for row in case["physical_source_bags"][relation]
            .as_array()
            .expect("physical source rows")
        {
            let names = columns.keys().cloned().collect::<Vec<_>>();
            let values = names
                .iter()
                .map(|name| {
                    let value = &row[name];
                    if value.is_null() {
                        "NULL".to_string()
                    } else if let Some(number) = value.as_i64() {
                        number.to_string()
                    } else {
                        format!(
                            "'{}'",
                            value
                                .as_str()
                                .expect("physical identity")
                                .replace('\'', "''")
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            db.execute_batch(&format!(
                "INSERT INTO {relation} ({}) VALUES ({values})",
                names.join(", ")
            ))
            .expect("insert complete physical source");
        }
    }

    let statements = case["sql"]
        .as_str()
        .expect("frozen two-stage SQL")
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 2, "projection then grouped terminal");
    db.execute_batch(statements[0])
        .expect("materialized three-source projection");
    let mut query = db.prepare(statements[1]).expect("grouped terminal");
    let rows = query
        .query_map([], |row| {
            Ok(BTreeMap::from([
                (
                    "k".to_string(),
                    LiteralValue::Number(row.get::<_, i64>(0)?.to_string()),
                ),
                (
                    "n".to_string(),
                    LiteralValue::Number(row.get::<_, i64>(1)?.to_string()),
                ),
            ]))
        })
        .expect("query full terminal output");
    rows.collect::<Result<Vec<_>, _>>()
        .expect("DuckDB full terminal bag")
}

#[test]
fn original_ac2_exact_three_source_projected_group_oracles() {
    let parent = parent();
    let evidence = &parent["upstream_composition"];
    assert_eq!(evidence["owner"], "TASK-115");
    assert_eq!(
        evidence["source_of_truth"],
        "docs/task-115-acceptance-freeze.json"
    );
    assert_eq!(evidence["mandatory_dialect_cells"], 26);
    assert_eq!(evidence["mandatory_adapter_cells"], 6);
    assert_eq!(evidence["native_engine"], "duckdb");
    assert_eq!(evidence["native_vendor_engines_certified"], false);
    let frozen = frozen();
    let dialects: BTreeSet<&str> = frozen["dialects"]
        .as_array()
        .expect("all parser dialects")
        .iter()
        .map(|value| value.as_str().expect("dialect"))
        .collect();
    let parent_dialects: BTreeSet<&str> = parent["portable_gate"]["required_dialects"]
        .as_array()
        .expect("parent dialects")
        .iter()
        .map(|value| value.as_str().expect("dialect"))
        .collect();
    assert_eq!(dialects, parent_dialects);
    assert_eq!(dialects.len(), 13);
    assert_eq!(
        evidence["mandatory_case_ids"],
        json!(["71-C115-P-GROUP", "71-C115-N-GROUP"])
    );

    for case_id in ["71-C115-P-GROUP", "71-C115-N-GROUP"] {
        let case = frozen["cases"]
            .as_array()
            .expect("frozen child cases")
            .iter()
            .find(|case| case["id"] == case_id)
            .expect("parent-pinned exact multi-join case");
        assert_eq!(case["variant"], "downstream_group_count");
        assert_eq!(
            case["acceptance_status"],
            "verified_exact_canonical_and_independent_duckdb_bags"
        );
        let adapter_matrix = case["adapter_matrix"]
            .as_array()
            .expect("source-independent adapter cells");
        assert_eq!(adapter_matrix.len(), 3);
        for adapter in adapter_matrix {
            assert_eq!(
                adapter["canonical_population_status"],
                "verified_with_explicit_physical_attestation"
            );
            assert_eq!(adapter["owner"], "TASK-115");
        }

        let expected = expected_terminal(case);
        let expected_selected = expected_triplets(case, "selected_physical_triplets");
        let expected_rejected = expected_triplets(case, "rejected_not_true_triplets");
        let statements = case["sql"]
            .as_str()
            .expect("bounded two-stage SQL")
            .split(';')
            .map(str::trim)
            .filter(|sql| !sql.is_empty())
            .map(SqlInput::inline)
            .collect::<Vec<_>>();
        assert_eq!(statements.len(), 2, "must contain a projected producer");
        assert!(case["sql"].as_str().expect("SQL").contains("JOIN t"));
        let ids = ["task73-stage", "task73-group"];
        for dialect in &dialects {
            let parser = dialect_from_name(dialect).expect("exposed SQL dialect");
            let configured = statements
                .iter()
                .zip(ids)
                .map(|(input, id)| ConfiguredSqlInput::new(id, input, dialect, parser.as_ref()))
                .collect::<Vec<_>>();
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog(case))
                .unwrap_or_else(|error| panic!("{dialect}/{case_id}: {error}"));
            let target = bundle.layers().last().expect("terminal layer").id();
            let proof = physical_finite_chain_bag_plan(&bundle, target, &snapshots(case));
            let PhysicalFiniteChainProof::Exact(exact) = proof else {
                panic!("{dialect}/{case_id}: chain/group proof is not exact: {proof:?}");
            };
            let mut selected = exact.selected.clone();
            let mut rejected = exact.rejected.clone();
            selected.sort();
            rejected.sort();
            assert_eq!(
                selected, expected_selected,
                "{case_id}/{dialect}: TRUE triples"
            );
            assert_eq!(
                rejected, expected_rejected,
                "{case_id}/{dialect}: NOT TRUE triples"
            );
            assert_eq!(
                exact.terminal, expected,
                "{case_id}/{dialect}: complete output"
            );
            let mut expected_counts = BTreeMap::<i128, usize>::new();
            for source in &expected_selected {
                let key = case["physical_source_bags"]["l"]
                    .as_array()
                    .expect("left rows")
                    .iter()
                    .find(|row| row["id"].as_str() == Some(source[0].as_str()))
                    .expect("stable physical key")["k"]
                    .as_i64()
                    .expect("selected integer key");
                *expected_counts.entry(i128::from(key)).or_default() += 1;
            }
            for (key, identities) in &exact.contributions {
                assert_eq!(
                    identities.len(),
                    *expected_counts.get(key).expect("group contributors"),
                    "{case_id}/{dialect}: source-backed duplicate multiplicity"
                );
            }
            assert_eq!(exact.contributions.len(), expected_counts.len());
            for (name, domain) in &exact.terminal_domains {
                let frozen_domain = &case["terminal_domains"][name];
                assert_eq!(
                    domain.null_allowed(),
                    frozen_domain["null_allowed"].as_bool().expect("NULL law")
                );
                let allowed = frozen_domain["allowed"]
                    .as_array()
                    .expect("allowed values")
                    .iter()
                    .map(|value| i128::from(value.as_i64().expect("integer")))
                    .collect::<Vec<_>>();
                assert_eq!(domain.allowed(), allowed.as_slice());
                let bounds = match (
                    frozen_domain["lower"]["value"].as_i64(),
                    frozen_domain["upper"]["value"].as_i64(),
                ) {
                    (Some(lower), Some(upper)) => {
                        assert_eq!(frozen_domain["lower"]["inclusive"], true);
                        assert_eq!(frozen_domain["upper"]["inclusive"], true);
                        Some((i128::from(lower), i128::from(upper)))
                    }
                    _ => None,
                };
                assert_eq!(
                    domain.bounds(),
                    bounds,
                    "{case_id}/{dialect}: inclusive bounds"
                );
            }
        }
        assert_eq!(
            duckdb_output(case),
            expected,
            "{case_id}: independent executable DuckDB terminal bag"
        );
    }
}
