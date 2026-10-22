//! TASK-128: executed dbt Core manifest/catalog, complete native source, and terminal oracles.
//! Run through make dbt-e2e. The isolated model subgraph has one attested physical source.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, physical_finite_window_bag_plan,
    LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow, PhysicalWriteState, SetMode,
    ValueDomain,
};
use sqlparser::dialect::dialect_from_str;

fn literal(value: &Value) -> LiteralValue {
    value.as_i64().map_or(LiteralValue::Null, |number| {
        LiteralValue::Number(number.to_string())
    })
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    result.sort();
    result
}

#[test]
#[ignore = "requires executed dbt Core artifacts; run make dbt-e2e"]
fn compiled_dbt_ntile_and_groups_match_native_complete_source_and_terminal_bags() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = root.join("tests/fixtures/dbt_core_project/target");
    let manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("manifest.json")).unwrap()).unwrap();
    let catalog_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("catalog.json")).unwrap()).unwrap();
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-128-acceptance-freeze.json")).unwrap();
    let db_path = std::env::var("DBT_E2E_DATABASE").unwrap();
    let db = Connection::open(db_path).unwrap();

    for (model_name, case_id, output) in [
        ("task128_ntile", "74-C128-P4", "tile"),
        ("task128_groups", "74-C128-P3", "total"),
    ] {
        let model_id = format!("model.sql_semantic_protocol_e2e.{model_name}");
        let source_id = "source.sql_semantic_protocol_e2e.raw.task128_window_source";
        let model = &manifest_json["nodes"][&model_id];
        let source = &manifest_json["sources"][source_id];
        assert_eq!(model["resource_type"], "model", "{model_name}");
        assert_eq!(source["name"], "task128_window_source");
        assert_eq!(source["columns"]["x"]["data_type"], "INTEGER");
        let compiled = model["compiled_code"].as_str().unwrap();
        assert!(!compiled.contains("{{"));
        assert!(compiled.to_ascii_lowercase().contains("over ("));
        let source_relation = source["relation_name"].as_str().unwrap();
        let output_relation = model["relation_name"].as_str().unwrap();

        let mut isolated_manifest = manifest_json.clone();
        isolated_manifest["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == &model_id);
        isolated_manifest["sources"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == source_id);
        let mut isolated_catalog = catalog_json.clone();
        isolated_catalog["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == &model_id);
        isolated_catalog["sources"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == source_id);
        let manifest = parse_dbt_manifest(&isolated_manifest.to_string()).unwrap();
        let catalog = parse_dbt_catalog(&isolated_catalog.to_string()).unwrap();
        let dialect = dialect_from_str(manifest.adapter_type()).unwrap();
        let bundle = analyze_dbt_artifacts(
            &manifest,
            &catalog,
            manifest.adapter_type(),
            dialect.as_ref(),
        )
        .unwrap();
        let source_name = bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation().contains("task128_window_source"))
            .expect("compiled dbt catalog source schema")
            .relation();
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| layer.consumes().iter().any(|s| s == source_name))
            .expect("compiled model must consume authoritative raw source");

        let case = freeze["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == case_id)
            .unwrap();
        let mut read = db
            .prepare(&format!(
                "SELECT id, k, x FROM {source_relation} ORDER BY id"
            ))
            .unwrap();
        let physical = read
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let expected_source = case["source_bag"].as_array().unwrap();
        assert_eq!(physical.len(), expected_source.len());
        let mut rows = Vec::new();
        for ((id, k, x), expected) in physical.iter().zip(expected_source) {
            assert_eq!(*id, expected["id"].as_i64().unwrap(), "{model_name}");
            assert_eq!(*k, expected["k"].as_i64().unwrap(), "{model_name}");
            assert_eq!(*x, expected["x"].as_i64(), "{model_name}");
            rows.push(
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".into(), LiteralValue::Number(id.to_string())),
                    ("k".into(), LiteralValue::Number(k.to_string())),
                    (
                        "x".into(),
                        x.map_or(LiteralValue::Null, |x| LiteralValue::Number(x.to_string())),
                    ),
                ]))
                .unwrap(),
            );
        }
        let source_bag = PhysicalWriteState::new(source_name, rows).unwrap();
        let proof = physical_finite_window_bag_plan(&bundle, layer.id(), &[source_bag]);
        let PhysicalFiniteWindowProof::Exact(exact) = proof else {
            panic!("{model_name}: real dbt compiled SQL lost physical proof: {proof:?}");
        };
        let expected_rows = case["full_terminal_bag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                BTreeMap::from([
                    ("id".into(), literal(&row["id"])),
                    (output.to_string(), literal(&row[output])),
                ])
            })
            .collect::<Vec<_>>();
        assert_eq!(
            bag(&exact.terminal),
            bag(&expected_rows),
            "{model_name}: protocol output"
        );

        let mut native = db
            .prepare(&format!(
                "SELECT id, {output} FROM {output_relation} ORDER BY id"
            ))
            .unwrap();
        let actual_rows = native
            .query_map([], |row| {
                Ok(BTreeMap::from([
                    (
                        "id".into(),
                        LiteralValue::Number(row.get::<_, i64>(0)?.to_string()),
                    ),
                    (
                        output.to_string(),
                        row.get::<_, Option<i64>>(1)?
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                ]))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            bag(&actual_rows),
            bag(&expected_rows),
            "{model_name}: native warehouse output"
        );
        let expected_ids = |field: &str| {
            case[field]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    format!(
                        "{source_name}#{}",
                        row.as_str().unwrap().split('#').next_back().unwrap()
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            exact.terminal_row_ids,
            expected_ids("terminal_row_ids"),
            "{model_name}"
        );
        assert_eq!(
            exact.rejected_row_ids,
            expected_ids("not_true_row_ids"),
            "{model_name}"
        );
        assert_eq!(exact.source_row_ids.len(), expected_source.len());

        for specified in case["typed_terminal_domains"].as_array().unwrap() {
            let name = specified["column"].as_str().unwrap();
            let Some(ValueDomain::Set(domain)) = exact.terminal_domains.get(name) else {
                panic!("{model_name}/{name}: missing finite domain")
            };
            assert_eq!(domain.mode(), SetMode::Include);
            let values = domain
                .values()
                .iter()
                .map(|v| format!("{:?}", v.value()))
                .collect::<BTreeSet<_>>();
            let expected = specified["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| format!("{:?}", literal(v)))
                .collect::<BTreeSet<_>>();
            assert_eq!(values, expected, "{model_name}/{name}: inclusive domain");
        }
    }
}

#[test]
#[ignore = "requires executed dbt Core artifacts; run make dbt-e2e"]
fn existing_ranked_orders_model_has_independent_full_native_source_and_rank_oracles() {
    // The existing ref('enriched_orders') model composes joined upstream physical
    // relations. This checks its native complete producer bag and terminal row
    // outcomes without pretending a single-source physical proof of that join.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(root.join("tests/fixtures/dbt_core_project/target/manifest.json"))
            .unwrap(),
    ).unwrap();
    let ranked = &manifest["nodes"]["model.sql_semantic_protocol_e2e.ranked_orders"];
    let enriched = &manifest["nodes"]["model.sql_semantic_protocol_e2e.enriched_orders"];
    let ranked_sql = ranked["compiled_code"].as_str().unwrap().to_ascii_lowercase();
    assert!(ranked_sql.contains("row_number() over"));
    assert!(ranked_sql.contains("sum(amount) over"));
    assert!(ranked_sql.contains("qualify rn <= 2"));
    let src = enriched["relation_name"].as_str().unwrap();
    let dst = ranked["relation_name"].as_str().unwrap();
    let db = Connection::open(std::env::var("DBT_E2E_DATABASE").unwrap()).unwrap();
    let rows = |sql: String| {
        let mut statement = db.prepare(&sql).unwrap();
        statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        }).unwrap().collect::<Result<Vec<_>, _>>().unwrap()
    };
    let source = rows(format!(
        "SELECT order_id,customer_id,amount,status,CAST(created_at AS VARCHAR),region
         FROM {src} ORDER BY order_id"
    ));
    let expected = vec![
        (2,10,20,"paid".to_string(),"2026-01-02 10:00:00".to_string(),"north".to_string()),
        (3,10,60,"pending".to_string(),"2026-01-03 10:00:00".to_string(),"north".to_string()),
        (4,20,80,"paid".to_string(),"2026-01-01 11:00:00".to_string(),"south".to_string()),
    ];
    assert_eq!(source, expected, "all joined and filtered upstream rows");
    let mut query = db.prepare(&format!(
        "SELECT order_id,customer_id,amount,status,CAST(created_at AS VARCHAR),region,rn,rolling_amount
         FROM {dst} ORDER BY order_id"
    )).unwrap();
    let terminal = query.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?, row.get::<_, i64>(7)?,
        ))
    }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    let expected_terminal = expected.into_iter().zip([(1,20),(2,80),(1,80)])
        .map(|(s,(rank,sum))| (s.0,s.1,s.2,s.3,s.4,s.5,rank,sum))
        .collect::<Vec<_>>();
    assert_eq!(terminal, expected_terminal, "every original ranked_orders.sql row and window value");
}
