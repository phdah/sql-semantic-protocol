//! TASK-178: independent source/adapter metadata parity for the shared
//! portable read families. Adapter provenance does not attest execution.

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput, LiteralValue,
    ProducerExecution, RelationCatalog, SetMode, SqlInput, ValueDomain,
};

const DIALECTS: &[&str] = &[
    "ansi",
    "bigquery",
    "clickhouse",
    "databricks",
    "duckdb",
    "generic",
    "hive",
    "mssql",
    "mysql",
    "postgresql",
    "redshift",
    "snowflake",
    "sqlite",
];

const CASES: &[(&str, &str)] = &[
    ("predicate", "SELECT amount FROM orders WHERE amount >= 10"),
    (
        "join",
        "SELECT o.id FROM orders o INNER JOIN customers c ON o.id = c.id",
    ),
    (
        "set",
        "SELECT id FROM orders UNION ALL SELECT id FROM customers",
    ),
    (
        "window",
        "SELECT ROW_NUMBER() OVER (ORDER BY id) AS rn FROM orders",
    ),
    ("pagination", "SELECT id FROM orders ORDER BY id LIMIT 2"),
];

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("exposed parser");
    let ddl = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id INTEGER, amount INTEGER);
             CREATE TABLE customers (id INTEGER, amount INTEGER);\n             CREATE TABLE products (id INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct DDL source metadata");
    RelationCatalog::from_schemas(ddl.source_schemas()).expect("complete direct source catalog")
}

fn dbt_catalog(sql: &str) -> RelationCatalog {
    // dbt depends_on enumerates the physical relations actually read by
    // compiled SQL. Inventing an unrelated source edge must be rejected.
    let mut dependencies = vec!["source.task178.orders"];
    if sql.contains("customers") {
        dependencies.push("source.task178.customers");
    }
    if sql.contains("products") {
        dependencies.push("source.task178.products");
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task178.read": {
                "unique_id": "model.task178.read", "resource_type": "model",
                "name": "read", "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "task178_read", "compiled_code": sql,
                "depends_on": {
                    "nodes": dependencies
                }
            }
        },
        "sources": {
            "source.task178.orders": {
                "unique_id": "source.task178.orders", "resource_type": "source",
                "name": "orders", "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task178.customers": {
                "unique_id": "source.task178.customers", "resource_type": "source",
                "name": "customers", "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task178.products": {
                "unique_id": "source.task178.products", "resource_type": "source",
                "name": "products", "relation_name": "products",
                "columns": {"id": {"name": "id", "data_type": "INTEGER"}}
            }
        }
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt v12");
    let dialect = dialect_from_name("duckdb").expect("dbt compile dialect");
    let analyzed = analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref())
        .expect("dbt compiled read model");
    assert!(
        analyzed
            .producer_evidence()
            .iter()
            .all(|producer| producer.execution() == ProducerExecution::Unattested),
        "dbt materialization metadata must not be promoted to physical execution"
    );
    RelationCatalog::from_schemas(analyzed.source_schemas())
        .expect("dbt source types, not guessed SQL output")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let contract = "apiVersion: v3.2.0
kind: DataContract
id: task178
name: TASK-178 adapter parity
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: customers
    physicalName: customers
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: products
    physicalName: products
    properties:
      - name: id
        physicalType: INTEGER
";
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        contract,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers", "products"])
            .expect("named physical relations"),
    )
    .expect("contract source schema");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("contract schema")
}

fn output_domains(catalog: &RelationCatalog, name: &str, sql: &str) -> Vec<ValueDomain> {
    let dialect = dialect_from_name(name).expect("one of 13 dialects");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task178-adapter-read",
        &input,
        name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("source-typed analysis");
    let ComposedSemantics::Resolved(semantics) = bundle
        .layers()
        .last()
        .expect("one terminal")
        .composed_semantics()
    else {
        panic!("{name}: parser supported shared SQL but not its typed output");
    };
    semantics
        .output()
        .columns()
        .iter()
        .map(|column| column.domain().clone())
        .collect()
}

#[test]
fn all_five_portable_read_families_have_identical_direct_and_dbt_domains_in_13_dialects() {
    let direct = direct_catalog();
    for (family, sql) in CASES {
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            let expected = output_domains(&direct, name, sql);
            let from_dbt = output_domains(&dbt, name, sql);
            assert!(
                !expected.is_empty(),
                "{family}/{name}: missing typed output columns"
            );
            assert_eq!(
                from_dbt, expected,
                "{family}/{name}: compiled dbt source types changed canonical output domains"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn all_five_portable_read_families_preserve_odcs_source_type_parity() {
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    for (family, sql) in CASES {
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            let expected = output_domains(&direct, name, sql);
            assert_eq!(
                output_domains(&dbt, name, sql),
                expected,
                "{family}/{name}: compiled dbt metadata changed output domains"
            );
            assert_eq!(
                output_domains(&odcs, name, sql),
                expected,
                "{family}/{name}: ODCS source datatype changed output domains"
            );
        }
    }
}

/// Cross-adapter replay of the independently frozen 25-case physical predicate
/// suite. The original native SQL, NULL/FALSE/TRUE source partitions and exact
/// bounds are checked by task_177_predicate_oracles; this checks that equivalent
/// typed source evidence is not lost at the direct/dbt/ODCS ingestion boundary.
#[test]
fn all_25_frozen_predicate_variants_preserve_direct_and_dbt_typed_domains() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-predicate-oracles.json"))
            .expect("independent original physical predicate oracle registry");
    assert_eq!(fixture["case_count"], 25);
    let direct = direct_catalog();
    for case in fixture["cases"]
        .as_array()
        .expect("25 named original variants")
    {
        let id = case["id"].as_str().expect("stable native sample");
        let sql = case["sql"].as_str().expect("original SQL");
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            let expected = output_domains(&direct, name, sql);
            assert_eq!(
                output_domains(&dbt, name, sql),
                expected,
                "{id}/{name}: compiled dbt typed facts must preserve all output domain claims"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn all_25_frozen_predicate_variants_preserve_odcs_typed_domains() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-predicate-oracles.json"))
            .expect("independent original physical predicate oracle registry");
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    for case in fixture["cases"].as_array().expect("25 original variants") {
        let id = case["id"].as_str().expect("stable source sample");
        let sql = case["sql"].as_str().expect("original SQL");
        for name in DIALECTS {
            assert_eq!(
                output_domains(&odcs, name, sql),
                output_domains(&direct, name, sql),
                "{id}/{name}: contract source facts must preserve full typed output domains"
            );
        }
    }
}

/// Every frozen native join example has a pinned complete source and terminal
/// bag in task_177_relational_bag_oracles. Verify that the *same* output-domain
/// evidence survives direct DDL, dbt source metadata, and ODCS schemas.
#[test]
fn all_21_frozen_native_join_variants_preserve_direct_and_dbt_domains() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("independently pinned native join registry");
    let ids = registry["groups"]["joins"]
        .as_array()
        .expect("21 join oracles");
    assert_eq!(ids.len(), 21);
    let cases = registry["cases"].as_array().expect("complete native cases");
    let direct = direct_catalog();
    for id in ids {
        let id = id.as_str().expect("stable join identity");
        let case = cases
            .iter()
            .find(|item| item["id"] == id)
            .expect("native terminal oracle");
        let sql = case["sql"].as_str().expect("registered SQL");
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            let expected = output_domains(&direct, name, sql);
            assert_eq!(
                output_domains(&dbt, name, sql),
                expected,
                "{id}/{name}: compiled dbt source typed claims differ from SQL DDL"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn all_21_frozen_native_join_variants_preserve_odcs_domains() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("native join registry");
    let ids = registry["groups"]["joins"]
        .as_array()
        .expect("21 join cases");
    let cases = registry["cases"].as_array().expect("source originals");
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    for id in ids {
        let id = id.as_str().expect("stable join case");
        let sql = cases
            .iter()
            .find(|item| item["id"] == id)
            .expect("native SQL")["sql"]
            .as_str()
            .expect("original SQL");
        for name in DIALECTS {
            assert_eq!(
                output_domains(&odcs, name, sql),
                output_domains(&direct, name, sql),
                "{id}/{name}: ODCS and direct source facts differ"
            );
        }
    }
}

/// Window and pagination transformations compose against the same typed
/// physical sources regardless of adapter. The independently frozen original
/// DuckDB source/output bags remain owned by task_177_relational_bag_oracles.
#[test]
fn all_14_frozen_window_and_pagination_variants_have_direct_dbt_parity() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("native source/terminal bag registry");
    let cases = registry["cases"].as_array().expect("named native SQL");
    let direct = direct_catalog();
    let mut checked = 0;
    for group in ["windows", "pagination"] {
        for id in registry["groups"][group].as_array().expect("named group") {
            let id = id.as_str().expect("stable native case");
            let sql = cases
                .iter()
                .find(|item| item["id"] == id)
                .expect("native oracle")["sql"]
                .as_str()
                .expect("original SQL");
            let dbt = dbt_catalog(sql);
            for name in DIALECTS {
                assert_eq!(
                    output_domains(&dbt, name, sql),
                    output_domains(&direct, name, sql),
                    "{group}/{id}/{name}: dbt typing drifted from direct SQL"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 14);
}

#[cfg(feature = "odcs")]
#[test]
fn all_14_frozen_window_and_pagination_variants_have_odcs_parity() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("native SQL registry");
    let cases = registry["cases"].as_array().expect("original SQL");
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    let mut checked = 0;
    for group in ["windows", "pagination"] {
        for id in registry["groups"][group].as_array().expect("named group") {
            let id = id.as_str().expect("native case");
            let sql = cases
                .iter()
                .find(|item| item["id"] == id)
                .expect("native oracle")["sql"]
                .as_str()
                .expect("original SQL");
            for name in DIALECTS {
                assert_eq!(
                    output_domains(&odcs, name, sql),
                    output_domains(&direct, name, sql),
                    "{group}/{id}/{name}: contract typing drifted from direct SQL"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 14);
}

/// The public API reports S083's BY NAME output alignment as an explicit
/// unsupported boundary; it must not be counted as a typed parity success.
/// Other frozen native set examples have complete literal DuckDB bag goldens
/// in task_177_relational_bag_oracles.
#[test]
fn all_18_representable_native_set_variants_have_direct_dbt_parity() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("native set bag registry");
    let cases = registry["cases"].as_array().expect("source originals");
    let ids = registry["groups"]["sets"]
        .as_array()
        .expect("19 native set cases");
    assert_eq!(ids.len(), 19);
    let direct = direct_catalog();
    let mut certified_parser_shapes = 0;
    let mut named_boundaries = 0;
    for id in ids {
        let id = id.as_str().expect("stable sample");
        let case = cases
            .iter()
            .find(|item| item["id"] == id)
            .expect("literal terminal oracle");
        if case["canonical_output_boundary"] == "name_alignment_output_unsupported" {
            assert_eq!(id, "88-C177-S083");
            named_boundaries += 1;
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            assert_eq!(
                output_domains(&dbt, name, sql),
                output_domains(&direct, name, sql),
                "{id}/{name}: dbt evidence changed positional or multiset output domain"
            );
        }
        certified_parser_shapes += 1;
    }
    assert_eq!(certified_parser_shapes, 18);
    assert_eq!(named_boundaries, 1);
}

#[cfg(feature = "odcs")]
#[test]
fn all_18_representable_native_set_variants_have_odcs_parity() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("native set bag registry");
    let cases = registry["cases"].as_array().expect("source SQL");
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    let mut checked = 0;
    for id in registry["groups"]["sets"]
        .as_array()
        .expect("named variants")
    {
        let id = id.as_str().expect("sample ID");
        let case = cases
            .iter()
            .find(|item| item["id"] == id)
            .expect("native oracle");
        if id == "88-C177-S083" {
            assert_eq!(
                case["canonical_output_boundary"],
                "name_alignment_output_unsupported"
            );
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        for name in DIALECTS {
            assert_eq!(
                output_domains(&odcs, name, sql),
                output_domains(&direct, name, sql),
                "{id}/{name}: ODCS evidence changed output domains"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 18);
}

/// A pinned native value must satisfy every precise typed claim in the public
/// protocol. Unknown/Unbounded states assert no membership promise, while
/// Empty, included/excluded sets and interval inclusivity are executable
/// obligations. Do not infer untyped NULL or scalar coercion evidence.
fn typed_domain_contains_native_integer(domain: &ValueDomain, literal: &serde_json::Value) -> bool {
    let native = literal.as_i64();
    match domain {
        ValueDomain::Unknown(_) | ValueDomain::Unbounded => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(set) => {
            let value = match native {
                Some(value) => LiteralValue::Number(value.to_string()),
                None if literal.is_null() => LiteralValue::Null,
                None => return false,
            };
            let present = set.values().iter().any(|entry| entry.value() == &value);
            match set.mode() {
                SetMode::Include => present,
                SetMode::Exclude => !present,
            }
        }
        ValueDomain::Ranges(ranges) => {
            let Some(value) = native.map(i128::from) else {
                return false;
            };
            ranges.ranges().iter().any(|range| {
                let below_upper = range.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(upper) = raw.parse::<i128>() else {
                        return false;
                    };
                    value < upper || (bound.inclusive() && value == upper)
                });
                let above_lower = range.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(lower) = raw.parse::<i128>() else {
                        return false;
                    };
                    value > lower || (bound.inclusive() && value == lower)
                });
                above_lower && below_upper
            })
        }
        // ValueDomain is non-exhaustive for external consumers. An unknown
        // future domain law cannot be accepted as a sound membership proof.
        _ => false,
    }
}

#[test]
fn all_54_portable_native_relational_terminals_respect_each_dialect_typed_domain() {
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-177-relational-bag-oracles.json"))
            .expect("literal whole-bag DuckDB terminals");
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-178-full-variant-inventory.json"))
            .expect("frozen dialect classifications");
    let cases = registry["cases"].as_array().expect("named native oracles");
    let direct = direct_catalog();
    let mut evaluated = 0;
    let mut explicit_boundaries = 0;
    for group in ["joins", "sets", "windows", "pagination"] {
        for id in registry["groups"][group]
            .as_array()
            .expect("named original samples")
        {
            let id = id.as_str().expect("source identity");
            let native = cases
                .iter()
                .find(|case| case["id"] == id)
                .expect("native golden");
            if native["canonical_output_boundary"] == "name_alignment_output_unsupported" {
                assert_eq!(id, "88-C177-S083");
                explicit_boundaries += 1;
                continue;
            }
            let sql = native["sql"].as_str().expect("exact original SQL");
            let original = inventory["cases"]
                .as_array()
                .expect("task variant matrix")
                .iter()
                .find(|case| case["id"] == id)
                .expect("same mandatory variant");
            assert_eq!(original["sql"], sql);
            assert_eq!(original["full_document_classification"], "CCCCCCCCCCCCC");
            let arity = native["columns"].as_u64().expect("native terminal arity") as usize;
            let bag = native["expected_terminal_bag"]
                .as_array()
                .expect("complete native bag");
            for name in DIALECTS {
                let domains = output_domains(&direct, name, sql);
                assert_eq!(
                    domains.len(),
                    arity,
                    "{id}/{name}: public output arity differs from physical native terminal"
                );
                for row in bag {
                    let values = row.as_array().expect("literal native row");
                    assert_eq!(values.len(), arity, "{id}: frozen physical arity mismatch");
                    for (column, (domain, value)) in domains.iter().zip(values).enumerate() {
                        assert!(
                            typed_domain_contains_native_integer(domain, value),
                            "{id}/{name}: output column {column} physically emits {value} outside its typed domain {domain:?}"
                        );
                    }
                }
            }
            evaluated += 1;
        }
    }
    assert_eq!(evaluated, 53);
    assert_eq!(explicit_boundaries, 1);
}
