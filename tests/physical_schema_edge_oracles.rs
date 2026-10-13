//! SQL oracle checks for portable decimal, temporal, JSON and IEEE754 boundaries.
//!
//! Invalid physical types may be disproved. Warehouse-dependent NaN, JSON and
//! timezone semantics are deliberately residual rather than fabricated.

use duckdb::Connection;
use sql_semantic_protocol::{
    physical_typed_value_plan, DataType, LiteralValue, PhysicalTypeProof, PhysicalTypedValue,
};

fn prove(value: LiteralValue, kind: DataType) -> PhysicalTypeProof {
    physical_typed_value_plan(&PhysicalTypedValue::Scalar(value), &kind)
}

#[test]
fn exact_decimal_precision_and_scale_match_duckdb_storage() {
    let kind = DataType::Decimal {
        precision: Some(38),
        scale: Some(6),
    };
    assert_eq!(
        prove(
            LiteralValue::Number("12345678901234567890.123456".into()),
            kind.clone(),
        ),
        PhysicalTypeProof::Feasible,
    );
    assert!(matches!(
        prove(
            LiteralValue::Number("12345678901234567890.1234567".into()),
            kind.clone()
        ),
        PhysicalTypeProof::Impossible { .. },
    ));
    assert!(matches!(
        prove(
            LiteralValue::Number("99999999999999999999999999999999999999999".into()),
            kind
        ),
        PhysicalTypeProof::Impossible { .. },
    ));

    let db = Connection::open_in_memory().unwrap();
    let value: String = db
        .query_row(
            "SELECT CAST(CAST('12345678901234567890.123456' AS DECIMAL(38,6)) AS VARCHAR)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(value, "12345678901234567890.123456");
}

#[test]
fn date_validation_and_oracle_respect_leap_days() {
    assert_eq!(
        prove(LiteralValue::Text("2024-02-29".into()), DataType::Date),
        PhysicalTypeProof::Feasible,
    );
    assert!(matches!(
        prove(LiteralValue::Text("2023-02-29".into()), DataType::Date),
        PhysicalTypeProof::Impossible { .. }
    ));
    let db = Connection::open_in_memory().unwrap();
    let value: String = db
        .query_row("SELECT CAST(DATE '2024-02-29' AS VARCHAR)", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(value, "2024-02-29");
    assert!(db
        .execute_batch("SELECT CAST('2023-02-29' AS DATE)")
        .is_err());
}

#[test]
fn float_nan_json_and_timezone_require_explicit_engine_semantics() {
    for (value, kind) in [
        (
            LiteralValue::Number("NaN".into()),
            DataType::FloatingPoint { bits: Some(64) },
        ),
        (LiteralValue::Text("{\"k\":1}".into()), DataType::Json),
        (
            LiteralValue::Text("2026-10-13T10:00:00+02:00".into()),
            DataType::Timestamp { precision: Some(6) },
        ),
    ] {
        assert!(matches!(
            prove(value, kind),
            PhysicalTypeProof::Residual { .. }
        ));
    }
    let db = Connection::open_in_memory().unwrap();
    let is_nan: bool = db
        .query_row("SELECT isnan(CAST('NaN' AS DOUBLE))", [], |row| row.get(0))
        .unwrap();
    assert!(is_nan);
    let json: String = db
        .query_row("SELECT CAST(JSON '{\"k\":1}' AS VARCHAR)", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(json.contains("\"k\""));
    // Proving a JSON parser or IEEE754 representation in one engine does not
    // establish the storage, ordering or coercion law for all SQL dialects.
}

#[test]
fn duckdb_decimal_map_key_equality_matches_canonical_nested_proof() {
    let kind = DataType::Map {
        key: Box::new(DataType::Decimal {
            precision: Some(18),
            scale: Some(6),
        }),
        value: Box::new(DataType::Boolean),
    };
    let one = |number: &str| {
        (
            PhysicalTypedValue::Scalar(LiteralValue::Number(number.into())),
            PhysicalTypedValue::Scalar(LiteralValue::Boolean(true)),
        )
    };
    assert!(matches!(
        physical_typed_value_plan(
            &PhysicalTypedValue::Map(vec![one("1.5"), one("01.500000")]),
            &kind
        ),
        PhysicalTypeProof::Impossible { .. }
    ));
    let db = Connection::open_in_memory().unwrap();
    let present: bool = db
        .query_row(
            "SELECT map_extract_value(
                MAP([CAST(1.500000 AS DECIMAL(18,6))], [true]),
                CAST(1.5 AS DECIMAL(18,6)))",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(present);
    assert!(db
        .execute_batch(
            "SELECT MAP(
            [CAST(1.5 AS DECIMAL(18,6)), CAST(1.500000 AS DECIMAL(18,6))],
            [true, false])"
        )
        .is_err());
}
