//! TASK-131 source evidence parity: SQL, compiled dbt and ODCS must agree
//! about exact scalar cardinality or three-valued quantified terminal rows.

#[cfg(feature = "odcs")]
#[test]
fn all_frozen_mandatory_scalar_and_quantified_cases_preserve_adapter_parity() {
    use std::collections::BTreeMap;

    use serde_json::{json, Map, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
        physical_finite_quantified_subquery_plan, physical_finite_scalar_subquery_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalQuantifiedScalarProof,
        PhysicalScalarSubqueryProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
        RelationSchema, SchemaColumn, SqlInput,
    };

    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json"))
            .expect("TASK-131 frozen source matrix");
    let physical_name = "warehouse.raw.inner_rows";
    let parser = dialect_from_name("postgresql").unwrap();
    let named = RelationCatalog::new(&[physical_name]).unwrap();
    let contracts = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task131
name: Scalar and quantified input source
version: 1.0.0
status: active
schema:
  - name: inner_rows
    physicalName: warehouse.raw.inner_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: v
        physicalType: INTEGER
"#,
        "postgresql",
        &named,
    )
    .expect("ODCS contract");
    let external = RelationCatalog::from_schemas(contracts.source_schemas()).unwrap();
    let direct = RelationCatalog::from_schemas(&[RelationSchema::new(
        physical_name,
        vec![
            SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap();

    let mut certified = 0;
    for case in frozen["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        if !["scalar", "any", "all"].contains(&kind) {
            continue;
        }
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM inner_rows", "FROM warehouse.raw.inner_rows");
        let input = SqlInput::inline(&sql);
        let direct_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task-131",
                &input,
                "postgresql",
                parser.as_ref(),
            )],
            &direct,
        )
        .unwrap();
        let odcs_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task-131",
                &input,
                "postgresql",
                parser.as_ref(),
            )],
            &external,
        )
        .unwrap();
        let columns = Map::from_iter(
            [("id", "VARCHAR"), ("v", "INTEGER")]
                .into_iter()
                .map(|(name, ty)| (name.to_string(), json!({"name":name,"data_type":ty}))),
        );
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task131": {
                    "unique_id":"model.demo.task131",
                    "resource_type":"model",
                    "relation_name":"warehouse.analytics.task131",
                    "database":"warehouse","schema":"analytics",
                    "original_file_path":"models/task131.sql",
                    "language":"sql","raw_code":sql,"compiled_code":sql,
                    "depends_on":{"nodes":["source.demo.inner_rows"]}
                }
            },
            "sources": {
                "source.demo.inner_rows": {
                    "unique_id":"source.demo.inner_rows", "resource_type":"source",
                    "relation_name":physical_name,"database":"warehouse",
                    "schema":"raw", "name":"inner_rows", "columns":columns
                }
            }
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let compiled =
            analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref()).unwrap();
        let bag = PhysicalWriteState::new(
            physical_name,
            case["inner"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Text(row["id"].as_str().unwrap().to_string()),
                        ),
                        (
                            "v".to_string(),
                            row["v"].as_i64().map_or(LiteralValue::Null, |v| {
                                LiteralValue::Number(v.to_string())
                            }),
                        ),
                    ]))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let proofs = [
            ("direct SQL", direct_bundle),
            ("dbt compiled", compiled),
            ("ODCS contract", odcs_bundle),
        ]
        .into_iter()
        .map(|(adapter, bundle)| {
            let target = bundle.layers().last().expect("one source-backed layer");
            let actual = if kind == "scalar" {
                match physical_finite_scalar_subquery_plan(
                    &bundle,
                    target.id(),
                    std::slice::from_ref(&bag),
                ) {
                    PhysicalScalarSubqueryProof::Exact(exact) => format!(
                        "{:?}",
                        (
                            "exact",
                            exact.source,
                            exact.source_row_ids,
                            exact.value,
                            exact.bounds
                        )
                    ),
                    PhysicalScalarSubqueryProof::CardinalityViolation { source_row_ids } => {
                        format!("{:?}", ("error", source_row_ids))
                    }
                    other => panic!(
                        "{} {adapter}: scalar adapter lost evidence: {other:?}",
                        case["id"]
                    ),
                }
            } else {
                match physical_finite_quantified_subquery_plan(
                    &bundle,
                    target.id(),
                    std::slice::from_ref(&bag),
                ) {
                    PhysicalQuantifiedScalarProof::Exact(exact) => format!(
                        "{:?}",
                        (
                            "exact",
                            exact.source,
                            exact.source_row_ids,
                            exact.truth,
                            exact.terminal
                        )
                    ),
                    other => panic!(
                        "{} {adapter}: quantified adapter lost evidence: {other:?}",
                        case["id"]
                    ),
                }
            };
            actual
        })
        .collect::<Vec<_>>();
        assert_eq!(proofs[0], proofs[1], "{} compiled dbt parity", case["id"]);
        assert_eq!(proofs[0], proofs[2], "{} ODCS parity", case["id"]);
        certified += 3;
    }
    assert_eq!(
        certified, 36,
        "12 complete source cases across three adapters"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn conditional_nested_lateral_and_apply_residuals_preserve_adapter_parity() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
        physical_finite_scalar_subquery_plan, ConfiguredSqlInput,
        PhysicalScalarSubqueryProof, PhysicalWriteRow, PhysicalWriteState,
        LiteralValue, ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn,
        SqlInput,
    };
    use std::collections::BTreeMap;

    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json")).unwrap();
    let physical = ["warehouse.raw.inner_rows", "warehouse.raw.outer_rows", "warehouse.raw.control_rows"];
    let named = RelationCatalog::new(&physical).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task131conditional
name: Conditional quantified and correlated sources
version: 1.0.0
status: active
schema:
  - name: inner_rows
    physicalName: warehouse.raw.inner_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: v
        physicalType: INTEGER
  - name: outer_rows
    physicalName: warehouse.raw.outer_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: v
        physicalType: INTEGER
  - name: control_rows
    physicalName: warehouse.raw.control_rows
    properties:
      - name: k
        physicalType: INTEGER
"#,
        "postgresql",
        &named,
    )
    .unwrap();
    let odcs = RelationCatalog::from_schemas(contract.source_schemas()).unwrap();
    let direct = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            physical[0],
            vec![
                SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").unwrap(),
            ],
        )
        .unwrap(),
        RelationSchema::new(
            physical[1],
            vec![
                SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").unwrap(),
            ],
        )
        .unwrap(),
        RelationSchema::new(
            physical[2],
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap(),
    ])
    .unwrap();

    let parser = dialect_from_name("postgresql").unwrap();
    let mut checked = 0;
    for case in frozen["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !["75-C131-N6", "75-C131-N7", "75-C131-N8"].contains(&id) {
            continue;
        }
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM inner_rows", "FROM warehouse.raw.inner_rows")
            .replace("FROM outer_rows", "FROM warehouse.raw.outer_rows")
            .replace("FROM control_rows", "FROM warehouse.raw.control_rows");
        let input = SqlInput::inline(&sql);
        let direct_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new("task131-conditional", &input, "postgresql", parser.as_ref())],
            &direct,
        )
        .unwrap();
        let odcs_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new("task131-conditional", &input, "postgresql", parser.as_ref())],
            &odcs,
        )
        .unwrap();
        let manifest = json!({
            "metadata": {
                "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version":"1.11.8",
                "adapter_type":"postgres"
            },
            "nodes": {
                "model.demo.task131conditional": {
                    "unique_id":"model.demo.task131conditional",
                    "resource_type":"model",
                    "relation_name":"warehouse.analytics.task131conditional",
                    "database":"warehouse",
                    "schema":"analytics",
                    "original_file_path":"models/task131conditional.sql",
                    "language":"sql",
                    "raw_code":sql,
                    "compiled_code":sql,
                    "depends_on":{"nodes":["source.demo.inner_rows","source.demo.outer_rows","source.demo.control_rows"]}
                }
            },
            "sources": {
                "source.demo.inner_rows": {
                    "unique_id":"source.demo.inner_rows",
                    "resource_type":"source",
                    "relation_name":physical[0],
                    "database":"warehouse","schema":"raw","name":"inner_rows",
                    "columns":{
                        "id":{"name":"id","data_type":"VARCHAR"},
                        "v":{"name":"v","data_type":"INTEGER"}
                    }
                },
                "source.demo.outer_rows": {
                    "unique_id":"source.demo.outer_rows",
                    "resource_type":"source",
                    "relation_name":physical[1],
                    "database":"warehouse","schema":"raw","name":"outer_rows",
                    "columns":{
                        "id":{"name":"id","data_type":"VARCHAR"},
                        "v":{"name":"v","data_type":"INTEGER"}
                    }
                },
                "source.demo.control_rows": {
                    "unique_id":"source.demo.control_rows",
                    "resource_type":"source",
                    "relation_name":physical[2],
                    "database":"warehouse","schema":"raw","name":"control_rows",
                    "columns":{"k":{"name":"k","data_type":"INTEGER"}}
                }
            }
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let compiled =
            analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref()).unwrap();
        let inner = PhysicalWriteState::new(
            physical[0],
            case["source_bags"]["inner_rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(BTreeMap::from([
                        ("id".to_string(), LiteralValue::Text(row["id"].as_str().unwrap().to_string())),
                        ("v".to_string(), row["v"].as_i64().map_or(
                            LiteralValue::Null,
                            |value| LiteralValue::Number(value.to_string()),
                        )),
                    ]))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();

        for (adapter, bundle) in [
            ("direct", direct_bundle),
            ("dbt", compiled),
            ("odcs", odcs_bundle),
        ] {
            let query = bundle.inputs()[0]
                .statements()
                .iter()
                .find_map(|statement| match statement {
                    ProtocolStatement::Query(query) => Some(query),
                    _ => None,
                })
                .expect("conditional canonical query");
            if id == "75-C131-N6" {
                let target = bundle.layers().last().unwrap();
                assert!(
                    matches!(
                        physical_finite_scalar_subquery_plan(
                            &bundle,
                            target.id(),
                            std::slice::from_ref(&inner),
                        ),
                        PhysicalScalarSubqueryProof::Residual { .. }
                    ),
                    "{id}/{adapter}: nested cardinality is not source-attested"
                );
            } else {
                let diagnostic = if id == "75-C131-N7" {
                    "unsupported_lateral_row_scope"
                } else {
                    "unsupported_cross_apply"
                };
                assert!(
                    query.diagnostics().iter().any(|d| d.code() == diagnostic),
                    "{id}/{adapter}: missing canonical correlation/session boundary"
                );
                assert_eq!(query.joins().len(), 1, "{id}/{adapter}: retained join");
            }
            assert_eq!(case["expected_adapter_cells"][adapter_key(adapter)], "Residual_conditional");
            checked += 1;
        }
    }
    assert_eq!(checked, 9, "every conditional cell across SQL, dbt and ODCS");
}

#[cfg(feature = "odcs")]
fn adapter_key(adapter: &str) -> &str {
    match adapter {
        "direct" => "sql",
        "dbt" => "dbt",
        "odcs" => "odcs",
        _ => unreachable!(),
    }
}
