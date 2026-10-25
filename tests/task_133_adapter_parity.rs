//! TASK-133: public canonical parser classification and three-adapter physical parity.

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFinitePageProof, PhysicalWriteRow,
    PhysicalWriteState, ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

const DIALECTS: &[&str] = &[
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

fn source(name: &str) -> PhysicalWriteState {
    let rows = [1, 2, 3, 4]
        .into_iter()
        .map(|n| {
            PhysicalWriteRow::new(BTreeMap::from([(
                "k".to_string(),
                LiteralValue::Number(n.to_string()),
            )]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new(name, rows).unwrap()
}

fn catalog(name: &str) -> RelationCatalog {
    let schema = RelationSchema::new(
        name,
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    RelationCatalog::from_schemas(&[schema]).unwrap()
}

fn verify_physical(
    label: &str,
    bundle: &sql_semantic_protocol::AnalysisBundle,
    state: &PhysicalWriteState,
) -> String {
    let layer = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .consumes()
                .iter()
                .any(|input| input == state.relation())
        })
        .unwrap_or_else(|| panic!("{label}: no canonical source layer"));
    let proof = physical_finite_page_bag_plan(bundle, layer.id(), std::slice::from_ref(state));
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("{label}: canonical pagination proof must be exact: {proof:?}");
    };
    assert_eq!(exact.source, state.relation(), "{label}");
    let selected_ids = exact
        .rows
        .iter()
        .filter(|row| row.survives)
        .map(|row| row.source_row_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        selected_ids,
        vec![
            format!("{}#1", state.relation()),
            format!("{}#2", state.relation()),
        ],
        "{label}"
    );
    assert_eq!(exact.terminal.len(), 2, "{label}");
    let selected_values = exact
        .terminal
        .iter()
        .map(|row| row.get("k").cloned())
        .collect::<Vec<_>>();
    assert_eq!(
        selected_values,
        vec![
            Some(LiteralValue::Number("2".to_string())),
            Some(LiteralValue::Number("3".to_string())),
        ],
        "{label}"
    );
    format!("{:?}", (exact.rows, exact.terminal, exact.terminal_domains))
}

#[test]
fn parser_equivalent_direct_sql_has_the_same_complete_physical_bag() {
    let sql = "SELECT k FROM s ORDER BY k LIMIT 2 OFFSET 1";
    let catalog = catalog("s");
    let state = source("s");
    let mut classified = 0;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        match Parser::parse_sql(parser.as_ref(), sql) {
            Ok(_) => {
                let input = SqlInput::inline(sql);
                let configured = [ConfiguredSqlInput::new(
                    "task133",
                    &input,
                    dialect,
                    parser.as_ref(),
                )];
                let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
                let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
                    panic!("{dialect}: direct SQL was not represented by a query");
                };
                let shape = query
                    .row_shaping()
                    .unwrap_or_else(|| panic!("{dialect}: lost typed row-selection law"));
                assert_eq!(shape.offset, 1, "{dialect}");
                assert_eq!(shape.limit, 2, "{dialect}");
                assert_eq!(shape.keys.len(), 1, "{dialect}");
                verify_physical(dialect, &bundle, &state);
                classified += 1;
            }
            Err(error) => {
                // A dialect-specific parser boundary is never native execution
                // certification and never silently promoted to Exact.
                assert!(!error.to_string().is_empty(), "{dialect}");
            }
        }
    }
    assert_eq!(DIALECTS.len(), 13);
    assert_eq!(
        classified, 13,
        "all shared LIMIT forms must be parser-equivalent"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn all_frozen_sql_dbt_odcs_cells_have_identical_bags_or_typed_residuals() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_dbt_manifest_with_schemas, parse_dbt_manifest, parse_odcs_yaml,
    };

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-133-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 3);
    let relation = "warehouse.raw.s";
    let schema = catalog(relation);
    let known = RelationCatalog::new(&[relation]).unwrap();
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task133paginated
name: Pagination source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: k
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    let external_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let mut certified_cells = 0;
    let mut mandatory = 0;
    let mut conditional = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let dialect = if id.ends_with("-top") {
            "mssql"
        } else {
            "postgresql"
        };
        let parser = dialect_from_name(dialect).unwrap();
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        assert!(Parser::parse_sql(parser.as_ref(), &sql).is_ok(), "{id}");
        let sql_input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            id,
            &sql_input,
            dialect,
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &schema).unwrap();
        let external =
            analyze_configured_inputs_with_catalog(&configured, &external_catalog).unwrap();

        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task133": {
                    "unique_id": "model.demo.task133",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task133",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task133.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s",
                    "resource_type": "source",
                    "relation_name": relation,
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "s",
                    "columns": {
                        "k": {"name": "k", "data_type": "INTEGER"}
                    }
                }
            }
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(&manifest, dialect, parser.as_ref()).unwrap();

        let state = PhysicalWriteState::new(
            relation,
            case["source_bag"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    PhysicalWriteRow::new(BTreeMap::from([(
                        "k".to_string(),
                        value
                            .as_i64()
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    )]))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();

        let mut outcomes = Vec::new();
        for (adapter, bundle) in [
            ("direct_sql", direct),
            ("dbt_compiled", dbt),
            ("odcs", external),
        ] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|s| s == relation))
                .unwrap_or_else(|| panic!("{id}/{adapter}: no canonical physical source"));
            let proof =
                physical_finite_page_bag_plan(&bundle, layer.id(), std::slice::from_ref(&state));
            match case["adapter_cells"][adapter].as_str().unwrap() {
                "canonical_exact_required" => {
                    let PhysicalFinitePageProof::Exact(exact) = proof else {
                        panic!("{id}/{adapter}: missing exact adapter proof: {proof:?}");
                    };
                    assert_eq!(exact.source, relation, "{id}/{adapter}");
                    let rows = exact
                        .terminal
                        .iter()
                        .map(|row| row.get(case["output_column"].as_str().unwrap()).unwrap())
                        .map(|value| match value {
                            LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
                            LiteralValue::Null => None,
                            other => panic!("{id}/{adapter}: unsupported terminal {other:?}"),
                        })
                        .collect::<Vec<_>>();
                    let wanted = case["terminal_bag"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(Value::as_i64)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        rows, wanted,
                        "{id}/{adapter}: complete terminal order and NULL"
                    );
                    outcomes.push(format!(
                        "{:?}",
                        (exact.rows, exact.terminal, exact.terminal_domains)
                    ));
                    mandatory += 1;
                }
                "typed_residual_required" => {
                    assert!(
                        matches!(proof, PhysicalFinitePageProof::Residual { .. }),
                        "{id}/{adapter}: conditional MUST fail closed: {proof:?}"
                    );
                    assert!(case["owner"].is_string() && case["disposition"].is_string());
                    conditional += 1;
                }
                other => panic!("{id}/{adapter}: unfrozen adapter classification {other}"),
            }
            certified_cells += 1;
        }
        if case["status"] == "mandatory" {
            assert_eq!(outcomes.len(), 3, "{id}: all adapters must be exact");
            assert_eq!(outcomes[0], outcomes[1], "{id}: direct/dbt parity");
            assert_eq!(outcomes[0], outcomes[2], "{id}: direct/ODCS parity");
        }
    }
    assert_eq!(mandatory, 27, "9 mandatory cases x 3 adapter paths");
    assert_eq!(conditional, 12, "4 conditional cases x 3 adapter paths");
    assert_eq!(
        certified_cells, 39,
        "every registered TASK-133 adapter cell"
    );
}
