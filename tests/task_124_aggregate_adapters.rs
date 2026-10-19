//! TASK-124 cross-child adapter signoff: compiled dbt and ODCS must not
//! introduce different canonical physical outcomes than directly analyzed SQL.

#[cfg(feature = "odcs")]
#[test]
fn every_mandatory_aggregate_case_has_exact_sql_dbt_and_odcs_source_parity() {
    use std::collections::BTreeMap;

    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
        physical_finite_aggregate_bag_plan, physical_finite_join_aggregate_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteAggregateProof,
        PhysicalJoinedAggregateProof, PhysicalWriteRow, PhysicalWriteState,
        RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-124-acceptance-freeze.json"))
            .expect("TASK-124 independent frozen adapter matrix");
    let relation_names = ["warehouse.raw.s", "warehouse.raw.l", "warehouse.raw.r"];
    let schemas = relation_names.iter().map(|name| {
        RelationSchema::new(*name, ["id", "k", "x"].into_iter().map(|column| {
            SchemaColumn::from_sql_type(
                column, if column == "id" { "VARCHAR" } else { "INTEGER" }, "postgresql",
            ).expect("physical source type")
        }).collect()).expect("typed source relation")
    }).collect::<Vec<_>>();
    let direct_catalog = RelationCatalog::from_schemas(&schemas).expect("direct typed source");
    let known = RelationCatalog::new(&relation_names).expect("canonical physical sources");
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task124aggregates
name: Parent aggregate schema parity
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: INTEGER
  - name: l
    physicalName: warehouse.raw.l
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: INTEGER
  - name: r
    physicalName: warehouse.raw.r
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: INTEGER
"#, "postgresql", &known,
    ).expect("authoritative ODCS schema");
    let odcs_catalog =
        RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS canonical catalog");
    let parser = dialect_from_name("postgresql").expect("portable SQL dialect");

    for case in freeze["cases"].as_array().expect("finite aggregate matrix") {
        if case["proof_kind"] != "mandatory" { continue; }
        let fixture = case["id"].as_str().expect("case ID");
        let joined = case["scope"] == "joined";
        let source_names: &[&str] = if joined {
            &relation_names[1..]
        } else {
            &relation_names[..1]
        };
        let sql = if joined {
            case["sql"].as_str().expect("SQL").replace(
                "FROM l INNER JOIN r",
                "FROM warehouse.raw.l AS l INNER JOIN warehouse.raw.r AS r",
            )
        } else {
            case["sql"].as_str().expect("SQL")
                .replace("FROM s", "FROM warehouse.raw.s")
        };
        let input = SqlInput::inline(&sql);
        let config = ConfiguredSqlInput::new(fixture, &input, "postgresql", parser.as_ref());
        let direct = analyze_configured_inputs_with_catalog(&[config], &direct_catalog)
            .unwrap_or_else(|error| panic!("{fixture}: direct SQL: {error}"));
        let config = ConfiguredSqlInput::new(fixture, &input, "postgresql", parser.as_ref());
        let external = analyze_configured_inputs_with_catalog(&[config], &odcs_catalog)
            .unwrap_or_else(|error| panic!("{fixture}: ODCS: {error}"));

        let dependencies = if joined {
            vec!["source.demo.l", "source.demo.r"]
        } else {
            vec!["source.demo.s"]
        };
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task124": {
                    "unique_id": "model.demo.task124",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task124",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task124.sql",
                    "language": "sql", "raw_code": sql, "compiled_code": sql,
                    "depends_on": {"nodes": dependencies}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s", "resource_type": "source",
                    "relation_name": "warehouse.raw.s", "database": "warehouse",
                    "schema": "raw", "name": "s",
                    "columns": {
                        "id": {"name":"id", "data_type":"VARCHAR"},
                        "k": {"name":"k", "data_type":"INTEGER"},
                        "x": {"name":"x", "data_type":"INTEGER"}
                    }
                },
                "source.demo.l": {
                    "unique_id": "source.demo.l", "resource_type": "source",
                    "relation_name": "warehouse.raw.l", "database": "warehouse",
                    "schema": "raw", "name": "l",
                    "columns": {
                        "id": {"name":"id", "data_type":"VARCHAR"},
                        "k": {"name":"k", "data_type":"INTEGER"},
                        "x": {"name":"x", "data_type":"INTEGER"}
                    }
                },
                "source.demo.r": {
                    "unique_id": "source.demo.r", "resource_type": "source",
                    "relation_name": "warehouse.raw.r", "database": "warehouse",
                    "schema": "raw", "name": "r",
                    "columns": {
                        "id": {"name":"id", "data_type":"VARCHAR"},
                        "k": {"name":"k", "data_type":"INTEGER"},
                        "x": {"name":"x", "data_type":"INTEGER"}
                    }
                }
            }
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt metadata");
        let dbt = analyze_dbt_manifest_with_schemas(&manifest, "postgresql", parser.as_ref())
            .unwrap_or_else(|error| panic!("{fixture}: compiled dbt: {error}"));
        let states = source_names.iter().enumerate().map(|(index, name)| {
            let short = if joined { if index == 0 { "l" } else { "r" } } else { "s" };
            let rows = if joined { &case["source_bags"][short] } else { &case["source_bag"] };
            PhysicalWriteState::new(
                *name,
                rows.as_array().expect("complete physical source").iter().map(|row| {
                    PhysicalWriteRow::new(
                        row.as_object().expect("physical source row").iter().map(|(column, v)| {
                            let literal = if v.is_null() {
                                LiteralValue::Null
                            } else if let Some(n) = v.as_i64() {
                                LiteralValue::Number(n.to_string())
                            } else {
                                LiteralValue::Text(v.as_str().expect("row identity").to_string())
                            };
                            (column.clone(), literal)
                        }).collect::<BTreeMap<_, _>>()
                    ).expect("typed source row")
                }).collect(),
            ).expect("source population")
        }).collect::<Vec<_>>();
        let mut proofs = Vec::new();
        for (adapter, bundle) in [("direct", &direct), ("dbt", &dbt), ("ODCS", &external)] {
            let layer = bundle.layers().iter()
                .find(|layer| layer.consumes().iter().any(|name| name == source_names[0]))
                .unwrap_or_else(|| panic!("{fixture}:{adapter}: no canonical physical source"));
            let evidence = if joined {
                let proof = physical_finite_join_aggregate_plan(bundle, layer.id(), &states);
                let PhysicalJoinedAggregateProof::Exact(exact) = proof else {
                    panic!("{fixture}:{adapter}: joined mandatory not exact: {proof:?}");
                };
                (format!("{:?}", exact.selected), format!("{:?}", exact.rejected),
                    format!("{:?}", exact.groups), format!("{:?}", exact.terminal),
                    format!("{:?}", exact.terminal_domains))
            } else {
                let proof = physical_finite_aggregate_bag_plan(bundle, layer.id(), &states);
                let PhysicalFiniteAggregateProof::Exact(exact) = proof else {
                    panic!("{fixture}:{adapter}: aggregate mandatory not exact: {proof:?}");
                };
                assert_eq!(exact.source, source_names[0], "{fixture}:{adapter}: source identity");
                (String::new(), String::new(), format!("{:?}", exact.groups),
                    format!("{:?}", exact.terminal), format!("{:?}", exact.terminal_domains))
            };
            proofs.push(evidence);
        }
        assert_eq!(proofs.len(), 3, "{fixture}: every adapter executed");
        assert_eq!(proofs[0], proofs[1], "{fixture}: compiled dbt canonical parity");
        assert_eq!(proofs[0], proofs[2], "{fixture}: ODCS canonical parity");
    }
}
