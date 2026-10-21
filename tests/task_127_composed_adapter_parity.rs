//! TASK-127: joined and grouped QUALIFY proofs must be adapter-independent.
//! No adapter's parsed syntax alone is evidence of physical executability.
#[cfg(feature = "odcs")]
#[test]
fn joined_and_grouped_upstream_proofs_survive_sql_dbt_and_odcs() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_window_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };
    use std::collections::BTreeMap;

    let fixture: Value =
        serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json")).unwrap();
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).unwrap();
    let mut visited = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !matches!(id, "74-C127-P4" | "74-C127-P5") {
            continue;
        }
        visited += 1;
        let schemas = case["source_schema"].as_object().unwrap();
        let names = schemas.keys()
            .map(|name| (name.to_owned(),format!("warehouse.raw.{name}")))
            .collect::<Vec<_>>();
        let all_relations = names.iter().map(|(_,name)|name.as_str()).collect::<Vec<_>>();
        let named = RelationCatalog::new(&all_relations).unwrap();
        let authoritative = RelationCatalog::from_schemas(&names.iter()
            .map(|(short,relation)| RelationSchema::new(
                relation,
                schemas[short].as_object().unwrap().iter()
                    .map(|(name,kind)|SchemaColumn::from_sql_type(
                        name,kind.as_str().unwrap(),dialect
                    ).unwrap()).collect(),
            ).unwrap()).collect::<Vec<_>>()).unwrap();
        let mut contract = String::from(
            "apiVersion: v3.2.0\nkind: DataContract\nid: task127upstream\nname: QUALIFY upstream\nversion: 1.0.0\nstatus: active\nschema:\n",
        );
        for (short,relation) in &names {
            contract.push_str(&format!(
                "  - name: {short}\n    physicalName: {relation}\n    properties:\n"
            ));
            for (column,kind) in schemas[short].as_object().unwrap() {
                contract.push_str(&format!(
                    "      - name: {column}\n        physicalType: {}\n",
                    kind.as_str().unwrap(),
                ));
            }
        }
        let odcs = parse_odcs_yaml(&contract,dialect,&named).unwrap();
        let external = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
        let sql = case["sql"].as_str().unwrap();
        let compiled = if id=="74-C127-P4" {
            sql.replace(
                "FROM l JOIN r",
                "FROM warehouse.raw.l AS l JOIN warehouse.raw.r AS r",
            )
        } else {
            sql.replace("FROM s GROUP BY k","FROM warehouse.raw.s GROUP BY k")
        };
        let input = SqlInput::inline(&compiled);
        let direct = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(id,&input,dialect,parser.as_ref())],
            &authoritative,
        ).unwrap();
        let odcs_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(id,&input,dialect,parser.as_ref())],
            &external,
        ).unwrap();
        let sources = names.iter().map(|(short,relation)| {
            let columns = schemas[short].as_object().unwrap().iter()
                .map(|(name,kind)| (name.clone(),json!({
                    "name": name, "data_type": kind.as_str().unwrap()
                }))).collect::<serde_json::Map<String,Value>>();
            (
                format!("source.demo.{short}"),
                json!({
                    "unique_id": format!("source.demo.{short}"),
                    "resource_type": "source", "relation_name": relation,
                    "database": "warehouse", "schema": "raw", "name": short,
                    "columns": columns
                }),
            )
        }).collect::<serde_json::Map<String,Value>>();
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task127_upstream": {
                    "unique_id": "model.demo.task127_upstream",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task127_upstream",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task127_upstream.sql",
                    "language": "sql", "raw_code": compiled, "compiled_code": compiled,
                    "depends_on": {
                        "nodes": names.iter()
                            .map(|(short,_)|format!("source.demo.{short}"))
                            .collect::<Vec<_>>()
                    }
                }
            },
            "sources": sources
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(
            &manifest,dialect,parser.as_ref()
        ).unwrap();
        let states = names.iter().map(|(short,relation)| {
            PhysicalWriteState::new(relation,case["physical_source_bags"][short]
                .as_array().unwrap().iter().map(|row| {
                    PhysicalWriteRow::new(row.as_object().unwrap().iter()
                        .map(|(name,value)| (
                            name.clone(),
                            if value.is_null() {LiteralValue::Null}
                            else {LiteralValue::Number(value.to_string())},
                        )).collect::<BTreeMap<_,_>>()).unwrap()
                }).collect()).unwrap()
        }).collect::<Vec<_>>();
        let mut outcomes = Vec::new();
        for (adapter,bundle) in [
            ("direct_sql",direct),("dbt_compiled",dbt),("odcs_schema",odcs_bundle)
        ] {
            let target = bundle.layers().iter()
                .find(|layer| layer.consumes().iter()
                    .any(|source| all_relations.contains(&source.as_str())))
                .unwrap_or_else(||panic!("{id}/{adapter}: missing physical producer"));
            let proof = physical_finite_window_bag_plan(&bundle,target.id(),&states);
            let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                panic!("{id}/{adapter}: missing complete composed proof {proof:?}");
            };
            assert_eq!(exact.physical_sources.len(),names.len(),"{id}/{adapter}");
            assert_eq!(
                exact.source_row_ids.len(),
                exact.terminal_row_ids.len()+exact.rejected_row_ids.len(),
                "{id}/{adapter}: joined/grouped identities not partitioned",
            );
            let mut actual = exact.terminal.iter().map(|row|format!("{row:?}")).collect::<Vec<_>>();
            let mut expected = case["expected_full_terminal_bag"].as_array().unwrap().iter()
                .map(|row| {
                    format!("{:?}",row.as_object().unwrap().iter().map(|(name,value)| (
                        name.clone(),LiteralValue::Number(value.to_string())
                    )).collect::<BTreeMap<_,_>>())
                }).collect::<Vec<_>>();
            actual.sort();
            expected.sort();
            assert_eq!(actual,expected,"{id}/{adapter}: wrong terminal bag");
            outcomes.push(format!("{:?}",(
                &exact.physical_sources,
                &exact.physical_source_row_ids,
                &exact.source_row_ids,
                &exact.terminal_row_ids,
                &exact.rejected_row_ids,
                &exact.rejected_upstream_row_ids,
                &exact.producer_contributors,
                &exact.terminal,
                &exact.terminal_domains,
            )));
        }
        assert_eq!(outcomes[0],outcomes[1],"{id}: dbt composition differs");
        assert_eq!(outcomes[0],outcomes[2],"{id}: ODCS composition differs");
    }
    assert_eq!(visited,2,"both mandatory composed cases must be tested");
}
