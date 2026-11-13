//! TASK-194: representable direct SQL, compiled dbt and ODCS source facts
//! must feed the same canonical typed set output on all 13 parser dialects.
//! A catalog declaration is not a vendor execution or finite-source proof.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput, LiteralValue,
    RelationCatalog, SetMode, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-194-acceptance-freeze.json");

fn fixture() -> Value {
    serde_json::from_str(FREEZE).expect("TASK-194 acceptance contract")
}

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("supported SQL dialect");
    let source = SqlInput::inline(
        "CREATE TABLE orders(id INTEGER, amount INTEGER);
         CREATE TABLE customers(id INTEGER, amount INTEGER)",
    );
    let bundle = analyze_inputs(&[source], "duckdb", dialect.as_ref())
        .expect("authoritative direct SQL column types");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("typed DDL source catalog")
}

fn dbt_catalog(sql: &str) -> RelationCatalog {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task194.set_result": {
                "unique_id": "model.task194.set_result",
                "resource_type": "model", "name": "set_result", "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "task194_set_result", "compiled_code": sql,
                "depends_on": {
                    "nodes": ["source.task194.orders", "source.task194.customers"]
                }
            }
        },
        "sources": {
            "source.task194.orders": {
                "unique_id": "source.task194.orders",
                "resource_type": "source", "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task194.customers": {
                "unique_id": "source.task194.customers",
                "resource_type": "source", "name": "customers",
                "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            }
        }
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt source model");
    let dialect = dialect_from_name("duckdb").expect("source dialect");
    let analyzed = analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref())
        .expect("compiled model with authoritative source metadata");
    RelationCatalog::from_schemas(analyzed.source_schemas()).expect("dbt schema evidence")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: task194
name: TASK-194 source schema
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
"#;
    let catalog = RelationCatalog::new(&["orders", "customers"]).expect("physical identities");
    let parsed = sql_semantic_protocol::parse_odcs_yaml(yaml, "duckdb", &catalog)
        .expect("ODCS authoritative source metadata");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("typed ODCS source catalog")
}

fn domains(catalog: &RelationCatalog, sql: &str, name: &str) -> Vec<ValueDomain> {
    let dialect = dialect_from_name(name).expect("public parser");
    let source = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task194-set-parity",
        &source,
        name,
        dialect.as_ref(),
    )];
    let analyzed = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("source-backed original set SQL");
    let ComposedSemantics::Resolved(semantics) = analyzed
        .layers()
        .last()
        .expect("set terminal")
        .composed_semantics()
    else {
        panic!("{name}: representable typed set output must not silently disappear");
    };
    semantics
        .output()
        .columns()
        .iter()
        .map(|column| column.domain().clone())
        .collect()
}

fn admits(domain: &ValueDomain, value: &Value) -> bool {
    match domain {
        ValueDomain::Unbounded | ValueDomain::Unknown(_) => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(set) => {
            let value = if value.is_null() {
                LiteralValue::Null
            } else if let Some(number) = value.as_i64() {
                LiteralValue::Number(number.to_string())
            } else {
                return false;
            };
            let found = set.values().iter().any(|item| item.value() == &value);
            match set.mode() {
                SetMode::Include => found,
                SetMode::Exclude => !found,
            }
        }
        ValueDomain::Ranges(ranges) => {
            let Some(actual) = value.as_i64().map(i128::from) else {
                return false;
            };
            ranges.ranges().iter().any(|range| {
                let lower = range.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(min) = raw.parse::<i128>() else {
                        return false;
                    };
                    actual > min || (actual == min && bound.inclusive())
                });
                let upper = range.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    let Ok(max) = raw.parse::<i128>() else {
                        return false;
                    };
                    actual < max || (actual == max && bound.inclusive())
                });
                lower && upper
            })
        }
        // A new domain kind must be evaluated explicitly, never auto-certified.
        _ => false,
    }
}

#[test]
fn twenty_original_set_queries_preserve_direct_and_compiled_dbt_domain_parity() {
    let fixture = fixture();
    let direct = direct_catalog();
    let mut checked = 0;
    let mut verified_occurrences = 0;
    for case in fixture["cases"].as_array().expect("22 SQL cases") {
        let Some(sql) = case["native_duckdb_sql"].as_str() else {
            continue;
        };
        let id = case["id"].as_str().expect("original case");
        let dbt = dbt_catalog(sql);
        for dialect in fixture["dialects"].as_array().expect("13 exposed parsers") {
            let name = dialect.as_str().expect("dialect");
            let expected = domains(&direct, sql, name);
            assert_eq!(
                expected.len(),
                case["expected_output_columns"]
                    .as_u64()
                    .expect("expected arity") as usize,
                "{id}/{name}: all positional output domains"
            );
            assert_eq!(
                domains(&dbt, sql, name),
                expected,
                "{id}/{name}: compiled dbt must preserve the same source facts"
            );
            for tuple in case["expected_terminal_bag"]
                .as_array()
                .expect("native bag")
            {
                for (domain, value) in expected.iter().zip(tuple.as_array().expect("tuple")) {
                    assert!(
                        admits(domain, value),
                        "{id}/{name}: native literal {value} excluded by {domain:?}"
                    );
                    verified_occurrences += 1;
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 260, "20 native cases x 13 parsers");
    assert!(verified_occurrences > 1000);
}

#[cfg(feature = "odcs")]
#[test]
fn twenty_original_set_queries_preserve_odcs_and_direct_typed_domains() {
    let fixture = fixture();
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    let mut checked = 0;
    for case in fixture["cases"].as_array().expect("22 cases") {
        let Some(sql) = case["native_duckdb_sql"].as_str() else {
            continue;
        };
        for name in fixture["dialects"].as_array().expect("13 parsers") {
            let name = name.as_str().expect("dialect");
            assert_eq!(
                domains(&odcs, sql, name),
                domains(&direct, sql, name),
                "{}/{}: equivalent typed evidence source cannot change canonical outcome",
                case["id"],
                name
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 260);
}
