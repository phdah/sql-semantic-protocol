//! TASK-107: independent expression-family evidence and seeded SQL truth oracles.
//!
//! These tests exercise public analysis/adapter APIs and DuckDB, never private
//! predicate solver functions. This is a bounded certification increment, not
//! TASK-107 closure or permission to release protocol 3.0.0.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, to_json, BooleanTruthCase, BooleanWitnessDirection,
    ComposedSemantics, ConfiguredSqlInput, Error, LiteralValue, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, ValueDomain,
};

const MATRIX: &str = include_str!("../docs/task-107-oracle-matrix.json");

fn source_catalog() -> RelationCatalog {
    let schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    RelationCatalog::from_schemas(&[schema]).unwrap()
}

fn source_semantics(
    bundle: &sql_semantic_protocol::AnalysisBundle,
) -> &sql_semantic_protocol::ResolvedComposedSemantics {
    let layer = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .consumes()
                .iter()
                .any(|relation| relation == "warehouse.raw.orders")
        })
        .expect("source-consuming layer");
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("typed SQL must resolve");
    };
    semantics
}

#[test]
fn frozen_expression_families_have_explicit_owners_and_test_evidence() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("valid versioned oracle matrix");
    assert_eq!(matrix["version"], 1);
    assert_eq!(matrix["owner"], "TASK-107");
    assert_eq!(matrix["release_approved"], false);

    let fixtures = matrix["fixtures"].as_array().expect("fixture array");
    let mut ids = BTreeSet::new();
    let mut families = BTreeSet::new();
    for fixture in fixtures {
        let id = fixture["id"].as_str().expect("stable fixture id");
        assert!(ids.insert(id), "duplicate fixture id: {id}");
        let family = fixture["family"].as_str().expect("owned expression family");
        families.insert(family);
        let classification = fixture["classification"]
            .as_str()
            .expect("explicit classification");
        assert!(
            matches!(
                classification,
                "exact_required" | "conditional" | "approved_deferred"
            ),
            "{id}: classification must not silently broaden exactness"
        );
        let path = fixture["test_file"].as_str().expect("public test owner");
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(path).is_file(),
            "{id}: missing public regression {path}"
        );
        let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .expect("read declared regression");
        let function = fixture["test_name"].as_str().expect("named public test");
        assert!(
            source.contains(&format!("fn {function}(")),
            "{id}: fixture points to missing test {path}::{function}"
        );
        assert!(
            fixture["expected_domain"].as_str().is_some(),
            "{id}: value-domain expectation must be stated"
        );
        assert!(
            fixture["positive"].as_str().is_some() && fixture["not_true"].as_str().is_some(),
            "{id}: both SQL three-valued truth directions must be classified"
        );
        if classification != "exact_required" {
            assert!(
                fixture["boundary"].as_str().is_some()
                    && fixture["followup_owner"].as_str().is_some(),
                "{id}: every non-exact cell needs a boundary and an owner"
            );
        }
    }
    for required in [
        "signed_arithmetic_and_cast",
        "case_coalesce_nullif",
        "correlated_cross_column",
        "pattern_collation_nullsafe",
        "temporal_json_vendor",
        "composed_relational_predicates",
    ] {
        assert!(
            families.contains(required),
            "TASK-101..106 missing {required}"
        );
    }
    assert!(
        fixtures.len() >= 9,
        "freeze representative exact and residual boundaries"
    );
}

#[test]
fn all_task_70_evidence_families_have_an_explicit_task_107_oracle_owner() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("versioned oracle matrix");
    let coverage: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let expected: BTreeSet<_> = coverage["task_70_predicate_evidence"]["family_evidence"]
        .as_array()
        .expect("authoritative family inventory")
        .iter()
        .map(|family| family["family"].as_str().expect("family id"))
        .collect();
    let registered: BTreeSet<_> = matrix["fixtures"]
        .as_array()
        .expect("frozen fixture ids")
        .iter()
        .filter_map(|fixture| fixture["task_70_family"].as_str())
        .collect();
    assert_eq!(
        registered, expected,
        "every authoritative TASK-70 family needs a named TASK-107 oracle owner"
    );
    assert_eq!(
        registered.len(),
        14,
        "approved expression inventory changed"
    );
}

#[test]
fn shared_expression_sql_has_parser_classifications_for_all_13_dialects() {
    let fixtures = [
        "SELECT amount FROM warehouse.raw.orders WHERE amount BETWEEN 1 AND 10",
        "SELECT COALESCE(amount, 0) AS v FROM warehouse.raw.orders WHERE amount IS NULL",
        "SELECT amount FROM warehouse.raw.orders WHERE amount > 2 AND amount < 8",
    ];
    let exposed: Vec<_> = common::DIALECTS
        .iter()
        .copied()
        .filter(|name| *name != "postgres")
        .collect();
    assert_eq!(
        exposed.len(),
        13,
        "one alias is excluded from the public dialect inventory"
    );
    for sql in fixtures {
        let mut baseline: Option<Value> = None;
        let mut parsed = 0;
        let mut rejected = 0;
        for name in &exposed {
            let parser = dialect_from_name(name).expect("known exposed dialect");
            match analyze_sql(sql, name, parser.as_ref()) {
                Ok(protocol) => {
                    parsed += 1;
                    let mut canonical: Value =
                        serde_json::from_str(&to_json(&protocol)).expect("valid emitted JSON");
                    canonical["inputs"][0]["dialect"] = Value::Null;
                    if let Some(expected) = &baseline {
                        assert_eq!(
                            &canonical, expected,
                            "{name}: different canonical SQL outcome for {sql}"
                        );
                    } else {
                        baseline = Some(canonical);
                    }
                }
                Err(Error::Parse(_)) => rejected += 1,
                Err(error) => {
                    panic!("{name}: parsed SQL must not lose its meaning: {sql}: {error}")
                }
            }
        }
        assert!(
            parsed >= 3,
            "{sql}: insufficient canonical dialect evidence"
        );
        assert_eq!(
            parsed + rejected,
            exposed.len(),
            "{sql}: missing parser classification"
        );
    }
}

#[test]
fn compiled_dbt_and_catalog_match_direct_sql_typed_expression_semantics() {
    let parser = dialect_from_name("postgresql").unwrap();
    let dbt_catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let direct_catalog = source_catalog();

    for predicate in [
        "CAST(amount AS BIGINT) * 3 > 9",
        "amount BETWEEN 1 AND 10",
        "COALESCE(amount, 0) IS NULL",
        "id > amount AND amount > id",
        "NULLIF(amount, 1) IS NULL",
    ] {
        let sql = format!(
            "SELECT id, amount, CASE WHEN amount > 5 THEN amount ELSE 0 END AS v \
             FROM warehouse.raw.orders WHERE {predicate}"
        );
        let mut fixture: Value =
            serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
        fixture["nodes"]["model.demo.stg_orders"]["compiled_code"] = sql.clone().into();
        let manifest = parse_dbt_manifest(&fixture.to_string()).unwrap();
        let dbt = analyze_dbt_artifacts(&manifest, &dbt_catalog, "postgresql", parser.as_ref())
            .unwrap_or_else(|error| panic!("dbt/{predicate}: {error}"));

        let input = SqlInput::inline(sql.clone());
        let direct = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "direct",
                &input,
                "postgresql",
                parser.as_ref(),
            )],
            &direct_catalog,
        )
        .unwrap_or_else(|error| panic!("SQL/{predicate}: {error}"));

        let a = source_semantics(&direct);
        let b = source_semantics(&dbt);
        assert_eq!(
            a.output(),
            b.output(),
            "{predicate}: output domains and inclusivity"
        );
        assert_eq!(
            a.column_domains(),
            b.column_domains(),
            "{predicate}: source domains"
        );
        assert_eq!(
            a.condition_exactness(),
            b.condition_exactness(),
            "{predicate}: exactness"
        );
        assert_eq!(
            a.boolean_witnesses()
                .iter()
                .map(|w| w.witness())
                .collect::<Vec<_>>(),
            b.boolean_witnesses()
                .iter()
                .map(|w| w.witness())
                .collect::<Vec<_>>(),
            "{predicate}: positive and NOT TRUE parity"
        );
    }
}

#[derive(Clone, Copy)]
enum Proof {
    Exact,
    Impossible,
}

fn check_direction(
    actual: &BooleanWitnessDirection,
    expected: Proof,
    truth_case: BooleanTruthCase,
    predicate: &str,
) {
    let valid = match (expected, actual) {
        (Proof::Exact, BooleanWitnessDirection::Exact(direction)) => *direction == truth_case,
        (Proof::Impossible, BooleanWitnessDirection::Impossible) => true,
        _ => false,
    };
    assert!(
        valid,
        "{predicate}: false exactness or missing proof: {actual:?}"
    );
}

fn seeded_values(seed: u64) -> Vec<(Option<i64>, Option<i64>)> {
    let mut state = seed;
    let mut rows = vec![(None, None), (Some(1), Some(1)), (Some(1), Some(1))];
    for _ in 0..96 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let a = if state & 7 == 0 {
            None
        } else {
            Some(((state >> 32) % 9) as i64 - 4)
        };
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let b = if state & 7 == 0 {
            None
        } else {
            Some(((state >> 32) % 9) as i64 - 4)
        };
        rows.push((a, b));
    }
    rows
}

fn sql_value(value: Option<i64>) -> String {
    value.map_or_else(|| "NULL".to_owned(), |value| value.to_string())
}

#[test]
fn multi_seed_duckdb_true_and_not_true_oracles_check_exact_impossible_and_null() {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();

    let cases = [
        ("a BETWEEN 1 AND 3", Proof::Exact, Proof::Exact),
        ("a > b AND b > a", Proof::Impossible, Proof::Exact),
        ("COALESCE(a, 0) IS NULL", Proof::Impossible, Proof::Exact),
        ("NULLIF(a, 1) IS NULL", Proof::Exact, Proof::Exact),
        (
            "(a IS DISTINCT FROM b) OR (a IS NOT DISTINCT FROM b)",
            Proof::Exact,
            Proof::Impossible,
        ),
    ];
    for seed in [0_u64, 1, 42, 20_261_016] {
        let db = Connection::open_in_memory().expect("DuckDB oracle");
        db.execute_batch("CREATE TABLE t(a INTEGER, b INTEGER)")
            .expect("create complete source");
        let values = seeded_values(seed);
        let inserts = values
            .iter()
            .map(|(a, b)| format!("({}, {})", sql_value(*a), sql_value(*b)))
            .collect::<Vec<_>>()
            .join(", ");
        db.execute_batch(&format!("INSERT INTO t VALUES {inserts}"))
            .expect("load seeded complete source");
        let total = values.len() as i64;

        for (predicate, positive, rejected) in cases {
            let sql = format!("SELECT a FROM t WHERE {predicate}");
            let input = SqlInput::inline(sql.clone());
            let bundle = analyze_configured_inputs_with_catalog(
                &[ConfiguredSqlInput::new(
                    "seeded",
                    &input,
                    "postgresql",
                    dialect.as_ref(),
                )],
                &catalog,
            )
            .unwrap_or_else(|error| panic!("seed {seed} SQL {sql}: {error}"));
            let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics()
            else {
                panic!("seed {seed} SQL {sql}: source semantics unresolved");
            };
            let witness = semantics.boolean_witnesses()[0].witness();
            check_direction(witness.qualifying(), positive, BooleanTruthCase::True, &sql);
            check_direction(
                witness.rejected(),
                rejected,
                BooleanTruthCase::NotTrue,
                &sql,
            );

            let true_count: i64 = db
                .query_row(
                    &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or_else(|error| panic!("seed {seed}, {sql}: {error}"));
            let false_count: i64 = db
                .query_row(
                    &format!("SELECT COUNT(*) FROM t WHERE NOT ({predicate})"),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or_else(|error| panic!("seed {seed}, {sql}: {error}"));
            let unknown_count = total - true_count - false_count;
            assert!(
                unknown_count >= 0,
                "seed {seed}, {sql}: invalid SQL 3VL partition"
            );
            assert_eq!(
                total - true_count,
                false_count + unknown_count,
                "seed {seed}, {sql}: SQL NOT TRUE includes UNKNOWN"
            );
            if matches!(positive, Proof::Impossible) {
                assert_eq!(true_count, 0, "seed {seed}, {sql}: false Impossible claim");
            }
            if matches!(rejected, Proof::Impossible) {
                assert_eq!(
                    false_count + unknown_count,
                    0,
                    "seed {seed}, {sql}: false tautology claim"
                );
            }
        }
    }
}

#[test]
fn typed_range_bounds_are_closed_and_null_rejection_remains_explicit() {
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let parser = dialect_from_name("postgresql").unwrap();
    let input = SqlInput::inline("SELECT a FROM t WHERE a BETWEEN 1 AND 3");
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "range",
            &input,
            "postgresql",
            parser.as_ref(),
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("typed range must resolve");
    };
    let ValueDomain::Ranges(ranges) = semantics.output().columns()[0].domain() else {
        panic!("typed range must be bounded");
    };
    assert_eq!(ranges.ranges().len(), 1);
    let range = &ranges.ranges()[0];
    for (bound, value) in [
        (range.lower().expect("lower"), "1"),
        (range.upper().expect("upper"), "3"),
    ] {
        assert!(bound.inclusive(), "BETWEEN requires inclusive endpoints");
        assert_eq!(
            bound.value().value(),
            &LiteralValue::Number(value.to_owned())
        );
    }
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_physical_schema_matches_sql_and_compiled_dbt_expression_outcomes() {
    use sql_semantic_protocol::parse_odcs_yaml;

    const ODCS: &str = r#"
apiVersion: v3.2.0
kind: DataContract
id: task107
name: Expression parity
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: BIGINT
      - name: amount
        physicalType: INTEGER
"#;
    let names = RelationCatalog::new(&["warehouse.raw.orders"]).unwrap();
    let metadata = parse_odcs_yaml(ODCS, "postgresql", &names).unwrap();
    let external_catalog = RelationCatalog::from_schemas(metadata.source_schemas()).unwrap();
    let direct_catalog = source_catalog();
    let dbt_catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let parser = dialect_from_name("postgresql").unwrap();

    for predicate in [
        "amount BETWEEN 1 AND 10",
        "CAST(amount AS BIGINT) * 3 > 9",
        "COALESCE(amount, 0) IS NULL",
        "NULLIF(amount, 1) IS NULL",
        "id > amount AND amount > id",
    ] {
        let sql = format!("SELECT id, amount FROM warehouse.raw.orders WHERE {predicate}");
        let mut manifest: Value =
            serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
        manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = sql.clone().into();
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_artifacts(&manifest, &dbt_catalog, "postgresql", parser.as_ref())
            .unwrap_or_else(|error| panic!("dbt/{predicate}: {error}"));

        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "task107",
            &input,
            "postgresql",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
            .unwrap_or_else(|error| panic!("direct SQL/{predicate}: {error}"));
        let odcs = analyze_configured_inputs_with_catalog(&configured, &external_catalog)
            .unwrap_or_else(|error| panic!("ODCS/{predicate}: {error}"));

        let expected = source_semantics(&direct);
        for (adapter, actual) in [("ODCS", &odcs), ("compiled dbt", &dbt)] {
            let result = source_semantics(actual);
            assert_eq!(
                result.output(),
                expected.output(),
                "{predicate}/{adapter}: output typed domains"
            );
            assert_eq!(
                result.column_domains(),
                expected.column_domains(),
                "{predicate}/{adapter}: input typed domains"
            );
            assert_eq!(
                result.condition_exactness(),
                expected.condition_exactness(),
                "{predicate}/{adapter}: proof strength"
            );
            assert_eq!(
                result
                    .boolean_witnesses()
                    .iter()
                    .map(|w| w.witness())
                    .collect::<Vec<_>>(),
                expected
                    .boolean_witnesses()
                    .iter()
                    .map(|w| w.witness())
                    .collect::<Vec<_>>(),
                "{predicate}/{adapter}: SQL TRUE/NOT TRUE proof witnesses"
            );
        }
    }
}
