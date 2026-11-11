//! TASK-192: compare the same typed predicate source facts through direct
//! SQL DDL, compiled dbt metadata and ODCS where each can represent them.
//! Adapter parity does not independently establish a native vendor law.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, physical_finite_exists_bag_plan, BooleanRowConstraint,
    BooleanTruthCase, BooleanWitnessDirection, ComposedSemantics, ConditionExactnessStatus,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteExistsProof, PhysicalWriteRow,
    PhysicalWriteState, ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const MATRIX: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn direct_catalog(amount_type: &str) -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("public dialect");
    let sql = format!(
        "CREATE TABLE orders (id INTEGER, amount {amount_type}, created_at TIMESTAMP, __physical_row_id VARCHAR);
         CREATE TABLE customers (id INTEGER, name VARCHAR, __physical_row_id VARCHAR)"
    );
    let bundle = analyze_inputs(&[SqlInput::inline(&sql)], "duckdb", dialect.as_ref())
        .expect("direct physical SQL source datatypes");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("direct schemas")
}

fn dbt_source_bundle(
    amount_type: &str,
    sql: &str,
    dialect_name: &str,
) -> sql_semantic_protocol::AnalysisBundle {
    // Fixture dependency edges are explicit physical sources referenced in the
    // original SQL, not all hypothetical sources in the catalog. This tests
    // the compiled model itself, not a convenient substitute query.
    let mut dependencies = Vec::new();
    if sql.contains("orders") {
        dependencies.push("source.task192.orders");
    }
    if sql.contains("customers") {
        dependencies.push("source.task192.customers");
    }
    assert!(
        !dependencies.is_empty(),
        "all 41 source-bearing cases require sources"
    );

    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task192.predicate_probe": {
                "unique_id": "model.task192.predicate_probe",
                "resource_type": "model",
                "name": "predicate_probe",
                "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "predicate_probe",
                "compiled_code": sql,
                "depends_on": {"nodes": dependencies}
            }
        },
        "sources": {
            "source.task192.orders": {
                "unique_id": "source.task192.orders",
                "resource_type": "source",
                "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": amount_type},
                    "created_at": {"name": "created_at", "data_type": "TIMESTAMP"},
                    "__physical_row_id": {"name": "__physical_row_id", "data_type": "VARCHAR"}
                }
            },
            "source.task192.customers": {
                "unique_id": "source.task192.customers",
                "resource_type": "source",
                "name": "customers",
                "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "name": {"name": "name", "data_type": "VARCHAR"},
                    "__physical_row_id": {"name": "__physical_row_id", "data_type": "VARCHAR"}
                }
            }
        }
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt v12 manifest");
    let dialect = dialect_from_name(dialect_name).expect("public parser");
    analyze_dbt_manifest_with_schemas(&parsed, dialect_name, dialect.as_ref())
        .expect("compiled dbt source and query semantics")
}

fn compiled_dbt_domains(
    bundle: &sql_semantic_protocol::AnalysisBundle,
) -> Option<Vec<ValueDomain>> {
    let layer = bundle.layers().last().expect("compiled dbt terminal");
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => Some(
            semantics
                .output()
                .columns()
                .iter()
                .map(|c| c.domain().clone())
                .collect(),
        ),
        _ => None,
    }
}

#[cfg(feature = "odcs")]
fn odcs_catalog(amount_type: &str) -> RelationCatalog {
    let contract = format!(
        "apiVersion: v3.2.0
kind: DataContract
id: task192
name: TASK-192
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: {amount_type}
      - name: created_at
        physicalType: TIMESTAMP
      - name: __physical_row_id
        physicalType: VARCHAR
  - name: customers
    physicalName: customers
    properties:
      - name: id
        physicalType: INTEGER
      - name: name
        physicalType: VARCHAR
      - name: __physical_row_id
        physicalType: VARCHAR
"
    );
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        &contract,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers"]).expect("two named sources"),
    )
    .expect("ODCS source schema evidence");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS schemas")
}

fn configured_bundle(
    catalog: &RelationCatalog,
    dialect_name: &str,
    sql: &str,
) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name(dialect_name).expect("public SQL dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-adapter-evidence",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("source-typed public analyzer")
}

fn composed_truth_signature(
    bundle: &sql_semantic_protocol::AnalysisBundle,
) -> (
    ConditionExactnessStatus,
    Vec<(
        BooleanRowConstraint,
        BooleanWitnessDirection,
        BooleanWitnessDirection,
    )>,
) {
    let layer = bundle.layers().last().expect("original terminal query");
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("original source-complete query must resolve");
    };
    (
        semantics.condition_exactness().status(),
        semantics
            .boolean_witnesses()
            .iter()
            .map(|item| {
                let witness = item.witness();
                (
                    witness.condition().clone(),
                    witness.qualifying().clone(),
                    witness.rejected().clone(),
                )
            })
            .collect(),
    )
}

fn domains(catalog: &RelationCatalog, dialect_name: &str, sql: &str) -> Option<Vec<ValueDomain>> {
    let bundle = configured_bundle(catalog, dialect_name, sql);
    let layer = bundle.layers().last().expect("one query layer");
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => Some(
            semantics
                .output()
                .columns()
                .iter()
                .map(|c| c.domain().clone())
                .collect(),
        ),
        _ => None,
    }
}

fn cases() -> Value {
    serde_json::from_str(MATRIX).expect("task-owned versioned matrix")
}

#[test]
fn every_representable_original_predicate_keeps_dbt_and_direct_typed_domains() {
    let matrix = cases();
    let dialects = matrix["dialects"].as_array().expect("13 dialects");
    let integer = direct_catalog("INTEGER");
    let float = direct_catalog("DOUBLE");
    let mut verified = 0;
    for case in matrix["cases"].as_array().expect("43 original cases") {
        let id = case["id"].as_str().expect("named case");
        if id == "88-C177-S033" || id == "88-C177-S179" {
            // The original scalar inputs have no physical table facts for dbt
            // or ODCS to attest; this is an explicit not-applicable boundary.
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        let floating = matches!(id, "88-C177-S040" | "88-C177-S041");
        let (direct, amount_type) = if floating {
            (&float, "DOUBLE")
        } else {
            (&integer, "INTEGER")
        };
        for name in dialects {
            let name = name.as_str().expect("dialect");
            let expected = domains(direct, name, sql);
            assert!(
                expected.as_ref().is_some_and(|c| !c.is_empty()),
                "{id}/{name}: missing typed output"
            );
            let dbt = dbt_source_bundle(amount_type, sql, name);
            let actual = compiled_dbt_domains(&dbt);
            assert_eq!(
                actual, expected,
                "{id}/{name}: original compiled dbt SQL parity"
            );
            let sourced = RelationCatalog::from_schemas(dbt.source_schemas())
                .expect("dbt exported typed source facts");
            assert_eq!(
                domains(&sourced, name, sql),
                expected,
                "{id}/{name}: dbt source facts changed when exported to a catalog"
            );
            assert!(
                dbt.producer_evidence()
                    .iter()
                    .all(|producer| producer.execution() == ProducerExecution::Unattested),
                "{id}/{name}: compiled dbt metadata is not native execution"
            );
            verified += 1;
        }
    }
    assert_eq!(verified, 41 * 13);
}

#[cfg(feature = "odcs")]
#[test]
fn every_representable_original_predicate_keeps_odcs_and_direct_typed_domains() {
    let matrix = cases();
    let dialects = matrix["dialects"].as_array().expect("13 dialects");
    let direct_integer = direct_catalog("INTEGER");
    let direct_float = direct_catalog("DOUBLE");
    let odcs_integer = odcs_catalog("INTEGER");
    let odcs_float = odcs_catalog("DOUBLE");
    let mut verified = 0;
    for case in matrix["cases"].as_array().expect("43 original cases") {
        let id = case["id"].as_str().expect("original ID");
        if id == "88-C177-S033" || id == "88-C177-S179" {
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        let floating = matches!(id, "88-C177-S040" | "88-C177-S041");
        let (direct, odcs) = if floating {
            (&direct_float, &odcs_float)
        } else {
            (&direct_integer, &odcs_integer)
        };
        for name in dialects {
            let name = name.as_str().expect("dialect");
            let expected = domains(direct, name, sql);
            let actual = domains(odcs, name, sql);
            assert!(
                expected.as_ref().is_some_and(|c| !c.is_empty()),
                "{id}/{name}: missing source domain"
            );
            assert_eq!(actual, expected, "{id}/{name}: ODCS source-fact parity");
            verified += 1;
        }
    }
    assert_eq!(verified, 41 * 13);
}

#[test]
fn nullable_three_valued_witnesses_agree_across_direct_and_compiled_dbt() {
    let matrix = cases();
    let source = direct_catalog("INTEGER");
    for id in [
        "88-C177-S010",
        "88-C177-S011",
        "88-C177-S012",
        "88-C177-S016",
        "88-C177-S017",
    ] {
        let sql = matrix["cases"]
            .as_array()
            .expect("original SQL cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original nullable predicate")["sql"]
            .as_str()
            .expect("original text");
        for name in matrix["dialects"].as_array().expect("13 parsers") {
            let name = name.as_str().expect("dialect name");
            let direct = configured_bundle(&source, name, sql);
            let dbt = dbt_source_bundle("INTEGER", sql, name);
            let expected = composed_truth_signature(&direct);
            let actual = composed_truth_signature(&dbt);
            assert_eq!(
                actual, expected,
                "{id}/{name}: dbt compiled query changed the complete typed boolean truth contract"
            );
            assert!(
                expected.1.iter().any(|(_, positive, negative)| {
                    matches!(
                        positive,
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ) && matches!(
                        negative,
                        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                    )
                }),
                "{id}/{name}: complete source TRUE and NOT TRUE witness missing"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn nullable_three_valued_witnesses_agree_across_odcs_and_direct() {
    let matrix = cases();
    let direct = direct_catalog("INTEGER");
    let odcs = odcs_catalog("INTEGER");
    for id in [
        "88-C177-S010",
        "88-C177-S011",
        "88-C177-S012",
        "88-C177-S016",
        "88-C177-S017",
    ] {
        let sql = matrix["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original SQL")["sql"]
            .as_str()
            .expect("unchanged SQL");
        for name in matrix["dialects"].as_array().expect("13 dialects") {
            let name = name.as_str().expect("dialect");
            assert_eq!(
                composed_truth_signature(&configured_bundle(&odcs, name, sql)),
                composed_truth_signature(&configured_bundle(&direct, name, sql)),
                "{id}/{name}: ODCS source evidence changed nullable TRUE/NOT TRUE proof"
            );
        }
    }
}

fn original_s013_complete_physical_snapshots() -> Vec<PhysicalWriteState> {
    fn row(identity: &str, entries: &[(&str, LiteralValue)]) -> PhysicalWriteRow {
        let mut values = BTreeMap::from([(
            "__physical_row_id".to_string(),
            LiteralValue::Text(identity.to_string()),
        )]);
        for (key, value) in entries {
            values.insert((*key).to_string(), value.clone());
        }
        PhysicalWriteRow::new(values).expect("complete typed source occurrence")
    }
    let integer = |value: Option<i64>| {
        value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
    };
    let outer = [
        (Some(-1), Some(10)),
        (Some(0), Some(10)),
        (Some(1), Some(10)),
        (Some(1), None),
        (Some(2), Some(19)),
        (Some(3), Some(20)),
        (None, Some(30)),
    ]
    .into_iter()
    .enumerate()
    .map(|(n, (id, amount))| {
        row(
            &format!("o{n}"),
            &[
                ("id", integer(id)),
                ("amount", integer(amount)),
                ("created_at", LiteralValue::Null),
            ],
        )
    })
    .collect::<Vec<_>>();
    let inner = [
        (Some(1), "one"),
        (Some(1), "duplicate"),
        (Some(3), "three"),
        (None, "null"),
    ]
    .into_iter()
    .enumerate()
    .map(|(n, (id, name))| {
        row(
            &format!("c{n}"),
            &[
                ("id", integer(id)),
                ("name", LiteralValue::Text(name.to_string())),
            ],
        )
    })
    .collect::<Vec<_>>();
    vec![
        PhysicalWriteState::new("orders", outer).expect("orders bag"),
        PhysicalWriteState::new("customers", inner).expect("customers bag"),
    ]
}

#[test]
fn original_s013_closed_world_semijoin_is_equal_across_representable_adapter_sources() {
    let matrix = cases();
    let sql = matrix["cases"]
        .as_array()
        .expect("43 original SQL")
        .iter()
        .find(|case| case["id"] == "88-C177-S013")
        .expect("original correlated EXISTS")["sql"]
        .as_str()
        .expect("unaltered SQL");
    let physical = original_s013_complete_physical_snapshots();
    let direct_catalog = direct_catalog("INTEGER");
    #[cfg(feature = "odcs")]
    let odcs_catalog = odcs_catalog("INTEGER");
    let mut comparisons = 0;

    for dialect in matrix["dialects"].as_array().expect("13 public parsers") {
        let name = dialect.as_str().expect("dialect");
        let direct = configured_bundle(&direct_catalog, name, sql);
        let dbt = dbt_source_bundle("INTEGER", sql, name);
        let mut bundles = vec![("direct", direct), ("dbt_compiled", dbt)];
        #[cfg(feature = "odcs")]
        bundles.push(("odcs", configured_bundle(&odcs_catalog, name, sql)));

        for (adapter, bundle) in bundles {
            let layer = bundle.layers().last().expect("original S013 query layer");
            let proof = physical_finite_exists_bag_plan(&bundle, layer.id(), &physical);
            let PhysicalFiniteExistsProof::Exact(exact) = proof else {
                panic!("S013/{name}/{adapter}: incomplete source-bag proof: {proof:?}");
            };
            assert_eq!(exact.selected, ["o2", "o3", "o5"]);
            assert_eq!(exact.rejected, ["o0", "o1", "o4", "o6"]);
            assert_eq!(
                exact.matching_inner["o2"],
                ["c0".to_string(), "c1".to_string()]
            );
            assert_eq!(exact.matching_inner["o6"], Vec::<String>::new());
            assert_eq!(
                exact
                    .terminal
                    .iter()
                    .map(|r| r.get("id"))
                    .collect::<Vec<_>>(),
                vec![
                    Some(&LiteralValue::Number("1".to_string())),
                    Some(&LiteralValue::Number("1".to_string())),
                    Some(&LiteralValue::Number("3".to_string())),
                ],
                "S013/{name}/{adapter}: duplicate-preserving terminal"
            );
            let domain = exact
                .terminal_domains
                .get("id")
                .expect("terminal id domain");
            assert_eq!(domain.values(), &[1, 3]);
            assert_eq!(domain.bounds(), Some((1, 3)));
            assert!(!domain.null_allowed());
            assert!(
                bundle
                    .producer_evidence()
                    .iter()
                    .all(|evidence| evidence.execution() == ProducerExecution::Unattested),
                "S013/{name}/{adapter}: no adapter claims native vendor execution"
            );
            comparisons += 1;
        }
    }
    #[cfg(feature = "odcs")]
    assert_eq!(comparisons, 39);
    #[cfg(not(feature = "odcs"))]
    assert_eq!(comparisons, 26);
}
