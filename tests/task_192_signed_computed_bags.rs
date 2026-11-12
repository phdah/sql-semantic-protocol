//! TASK-192 original S027/S032: source-complete, width-safe arithmetic proof.
//! Unchanged SQL across 13 parser dialects. Independent DuckDB terminal
//! partitions are frozen in TASK-177, not derived from this evaluator.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_signed_computed_bag_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalSignedComputedProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const ORACLES: &str = include_str!("../docs/task-177-predicate-oracles.json");

fn catalog(ty: &str) -> RelationCatalog {
    let column = |name, kind| {
        SchemaColumn::from_sql_type(name, kind, "duckdb").expect("typed source declaration")
    };
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "orders",
        vec![column("id", "INTEGER"), column("amount", ty)],
    )
    .expect("source schema")])
    .expect("source catalog")
}

fn row(id: i64, amount: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "amount".to_string(),
            amount.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
        ),
    ]))
    .expect("complete physical source row")
}

fn original_source() -> PhysicalWriteState {
    let rows = [
        (-1, Some(-5)),
        (0, Some(0)),
        (1, Some(10)),
        (2, Some(10)),
        (3, Some(19)),
        (4, Some(20)),
        (5, None),
        (6, Some(5)),
        (7, Some(0)),
        (8, Some(1)),
        (9, Some(20)),
    ]
    .into_iter()
    .map(|(id, amount)| row(id, amount))
    .collect();
    PhysicalWriteState::new("orders", rows).expect("complete source population")
}

fn original_sql<'a>(freeze: &'a Value, id: &str) -> &'a str {
    freeze["cases"]
        .as_array()
        .expect("43 original SQL")
        .iter()
        .find(|case| case["id"] == id)
        .expect("case")["sql"]
        .as_str()
        .expect("unchanged original SQL")
}

fn prove(
    sql: &str,
    dialect_name: &str,
    catalog: &RelationCatalog,
    source: &PhysicalWriteState,
) -> PhysicalSignedComputedProof {
    let dialect = dialect_from_name(dialect_name).expect("public parser");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-computed-physical",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("original SQL canonical analysis");
    let target = bundle.layers().last().expect("query target");
    physical_finite_signed_computed_bag_plan(&bundle, target.id(), source)
}

fn projected_values(proof: &sql_semantic_protocol::PhysicalSignedComputedExact) -> Vec<i64> {
    proof
        .terminal
        .iter()
        .map(|row| {
            let LiteralValue::Number(value) = row.get("id").expect("output id") else {
                panic!("original terminal must contain integer id");
            };
            value.parse::<i64>().expect("source id integer")
        })
        .collect()
}

#[test]
fn original_s027_s032_13_dialects_have_exact_full_physical_partition_and_terminal() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original case matrix");
    let oracles: Value = serde_json::from_str(ORACLES).expect("independent native goldens");
    let population = original_source();
    let catalog = catalog("BIGINT");
    for id in ["88-C177-S027", "88-C177-S032"] {
        let sql = original_sql(&freeze, id);
        let oracle = oracles["cases"]
            .as_array()
            .expect("source goldens")
            .iter()
            .find(|case| case["id"] == id)
            .expect("fixed native golden");
        let selected: Vec<i64> =
            serde_json::from_value(oracle["expected_selected_ids"].clone()).expect("native TRUE");
        let false_ids: Vec<i64> =
            serde_json::from_value(oracle["expected_false_ids"].clone()).expect("native FALSE");
        let unknown_ids: Vec<i64> =
            serde_json::from_value(oracle["expected_unknown_ids"].clone()).expect("native UNKNOWN");
        for name in freeze["dialects"]
            .as_array()
            .expect("all 13 public dialects")
        {
            let name = name.as_str().expect("dialect");
            let proof = prove(sql, name, &catalog, &population);
            let PhysicalSignedComputedProof::Exact(exact) = proof else {
                panic!("{id}/{name}: complete signed safe source needs Exact: {proof:?}");
            };
            assert_eq!(exact.source, "orders", "{id}/{name}: physical source");
            assert_eq!(
                projected_values(&exact),
                selected,
                "{id}/{name}: full native TRUE bag"
            );
            let rejected = exact
                .rejected_ordinals
                .iter()
                .map(|index| {
                    let LiteralValue::Number(value) = population.rows()[*index]
                        .values()
                        .get("id")
                        .expect("source id")
                    else {
                        panic!("source id must be integer");
                    };
                    value.parse::<i64>().expect("id")
                })
                .collect::<Vec<_>>();
            let unknown = exact
                .unknown_ordinals
                .iter()
                .map(|index| {
                    let LiteralValue::Number(value) = population.rows()[*index]
                        .values()
                        .get("id")
                        .expect("source id")
                    else {
                        panic!("source id must be integer");
                    };
                    value.parse::<i64>().expect("id")
                })
                .collect::<Vec<_>>();
            let expected_rejected = population
                .rows()
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    let LiteralValue::Number(id) = row.values().get("id").expect("id") else {
                        panic!("id is integer")
                    };
                    let id = id.parse::<i64>().expect("id");
                    false_ids.contains(&id) || unknown_ids.contains(&id)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            assert_eq!(
                exact.rejected_ordinals, expected_rejected,
                "{id}/{name}: all NOT TRUE ordinals"
            );
            assert_eq!(rejected.len(), false_ids.len() + unknown_ids.len());
            assert_eq!(
                unknown, unknown_ids,
                "{id}/{name}: SQL UNKNOWN from source NULL"
            );
            let mut domain = selected.iter().map(|id| *id as i128).collect::<Vec<_>>();
            domain.sort_unstable();
            domain.dedup();
            assert_eq!(
                exact.terminal_domain.values(),
                domain,
                "{id}/{name}: full typed set"
            );
            assert!(
                !exact.terminal_domain.null_allowed(),
                "{id}/{name}: native ids nonNULL"
            );
            assert_eq!(
                exact.terminal_domain.bounds(),
                domain.first().zip(domain.last()).map(|(a, b)| (*a, *b)),
                "{id}/{name}: inclusive typed source bag bounds"
            );
        }
    }
}

#[test]
fn signed_arithmetic_overflow_is_a_named_residual_not_an_invented_vendor_law() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original case matrix");
    let catalog = catalog("BIGINT");
    for (id, amount) in [("88-C177-S027", i64::MAX), ("88-C177-S032", i64::MIN)] {
        let source = PhysicalWriteState::new("orders", vec![row(1, Some(amount)), row(2, None)])
            .expect("extremal complete bag");
        let sql = original_sql(&freeze, id);
        for dialect in freeze["dialects"]
            .as_array()
            .expect("all 13 parser dialects")
        {
            let dialect = dialect.as_str().expect("dialect");
            let proof = prove(sql, dialect, &catalog, &source);
            assert!(
                matches!(proof, PhysicalSignedComputedProof::Residual { ref reason }
                    if reason.contains("overflow source width")),
                "{id}/{dialect}: source-width overflow must fail closed: {proof:?}"
            );
        }
    }
}

#[test]
fn narrow_typed_source_integer_and_duplicate_rows_preserve_exact_bag() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let sql = original_sql(&freeze, "88-C177-S032");
    let source = PhysicalWriteState::new(
        "orders",
        vec![
            row(1, Some(-5)),
            row(1, Some(-5)),
            row(2, None),
            row(3, Some(0)),
        ],
    )
    .expect("duplicate source-valued rows");
    let proof = prove(sql, "duckdb", &catalog("SMALLINT"), &source);
    let PhysicalSignedComputedProof::Exact(exact) = proof else {
        panic!("safe signed16 ABS and duplicated values must be Exact: {proof:?}");
    };
    assert_eq!(exact.selected_ordinals, [0, 1]);
    assert_eq!(exact.rejected_ordinals, [2, 3]);
    assert_eq!(exact.unknown_ordinals, [2]);
    assert_eq!(projected_values(&exact), [1, 1]);
    assert_eq!(exact.terminal_domain.values(), [1]);
    assert_eq!(exact.terminal_domain.bounds(), Some((1, 1)));
}

#[test]
fn missing_or_non_signed_width_does_not_make_arithmetic_portable() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let source = original_source();
    for id in ["88-C177-S027", "88-C177-S032"] {
        let sql = original_sql(&freeze, id);
        let proof = prove(sql, "duckdb", &catalog("DOUBLE"), &source);
        assert!(
            !matches!(proof, PhysicalSignedComputedProof::Exact(_)),
            "{id}: floating coercion/NaN must not borrow signed arithmetic exactness"
        );
    }
}
