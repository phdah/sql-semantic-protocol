//! TASK-114: execute the immutable P/N bags against an independent DuckDB engine.
//! SQL execution is necessary but not sufficient for canonical constructive signoff.
use duckdb::Connection;
use serde_json::Value;

const MATRIX: &str = include_str!("../docs/task-114-acceptance-freeze.json");

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn typed_literal(value: &Value, is_timestamp: bool) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::String(value) if is_timestamp => {
            format!("CAST({} AS TIMESTAMP)", quote(value))
        }
        Value::String(value) => quote(value),
        other => other.to_string(),
    }
}

fn actual_rows(db: &Connection, sql: &str, width: usize) -> Vec<String> {
    let mut statement = db.prepare(sql).unwrap();
    let mut rows = statement.query([]).unwrap();
    let mut result = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let values = (0..width)
            .map(|index| {
                row.get::<usize, Option<String>>(index)
                    .unwrap()
                    .map_or(Value::Null, Value::String)
            })
            .collect::<Vec<_>>();
        result.push(Value::Array(values).to_string());
    }
    result.sort();
    result
}

fn expected_rows(rows: &Value) -> Vec<String> {
    let mut rows = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            Value::Array(
                row.as_array()
                    .unwrap()
                    .iter()
                    .map(|value| {
                        if value.is_null() {
                            Value::Null
                        } else {
                            Value::String(
                                value
                                    .as_str()
                                    .map(ToString::to_string)
                                    .unwrap_or_else(|| value.to_string()),
                            )
                        }
                    })
                    .collect(),
            )
            .to_string()
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn every_frozen_task_114_positive_and_not_true_bag_executes_independently() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let variant = case["variant"].as_str().unwrap();
        let timestamp = case["columns"]["t"] == "TIMESTAMP";
        let column_type = if timestamp { "TIMESTAMP" } else { "INTEGER" };
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!(
            "CREATE TABLE l(id VARCHAR NOT NULL,t {column_type}); CREATE TABLE r(id VARCHAR NOT NULL,t {column_type});"
        ))
        .unwrap();
        for name in ["l", "r"] {
            let mut values = Vec::new();
            for record in case[if name == "l" { "left" } else { "right" }]
                .as_array()
                .unwrap()
            {
                let row = record.as_array().unwrap();
                values.push(format!(
                    "({}, {})",
                    quote(row[0].as_str().unwrap()),
                    typed_literal(&row[1], timestamp)
                ));
            }
            if !values.is_empty() {
                db.execute_batch(&format!("INSERT INTO {name} VALUES {}", values.join(",")))
                    .unwrap();
            }
            let source_sql = format!("SELECT id,CAST(t AS VARCHAR) FROM {name}");
            let frozen_source = &case[if name == "l" { "left" } else { "right" }];
            assert_eq!(
                actual_rows(&db, &source_sql, 2),
                expected_rows(frozen_source),
                "{id}: entire closed physical {name} bag"
            );
            let bounds = &case["typed_bounds"][if name == "l" { "left_t" } else { "right_t" }];
            let text = bounds.as_str().unwrap().replace('T', " ");
            let sql = format!("SELECT CAST(MIN(t) AS VARCHAR),CAST(MAX(t) AS VARCHAR) FROM {name}");
            let actual = actual_rows(&db, &sql, 2);
            let extrema = text
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                vec![Value::Array(
                    extrema
                        .iter()
                        .map(|s| Value::String((*s).to_string()))
                        .collect()
                )
                .to_string()],
                "{id}: inclusive typed source bounds"
            );
        }

        let cols = "l.id,CAST(l.t AS VARCHAR),r.id,CAST(r.t AS VARCHAR)";
        let (selected, rejected) = match variant {
            "asof_nearest_prior" => (
                format!("SELECT {cols} FROM l ASOF JOIN r ON l.t >= r.t"),
                "WITH winners AS (SELECT l.id lid, r.id rid FROM l ASOF JOIN r ON l.t >= r.t) SELECT l.id,r.id FROM l CROSS JOIN r LEFT JOIN winners w ON w.lid=l.id AND w.rid=r.id WHERE w.lid IS NULL".to_string(),
            ),
            "range_inequality" | "time_inequality" => (
                format!("SELECT {cols} FROM l JOIN r ON l.t > r.t"),
                "SELECT l.id,r.id FROM l CROSS JOIN r WHERE (l.t > r.t) IS NOT TRUE".to_string(),
            ),
            "lateral_derived" | "cross_apply" => (
                "SELECT l.id,q.id FROM l CROSS JOIN LATERAL (SELECT r.id,r.t FROM r WHERE r.t = l.t) q".to_string(),
                "SELECT l.id,r.id FROM l CROSS JOIN r WHERE (l.t = r.t) IS NOT TRUE".to_string(),
            ),
            "outer_apply" => (
                "SELECT l.id,q.id FROM l LEFT JOIN LATERAL (SELECT r.id,r.t FROM r WHERE r.t = l.t) q ON TRUE".to_string(),
                "SELECT l.id,r.id FROM l CROSS JOIN r WHERE (l.t = r.t) IS NOT TRUE".to_string(),
            ),
            other => panic!("unknown frozen TASK-114 variant: {other}"),
        };
        let width = case["expected"]
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .as_array()
            .unwrap()
            .len();
        assert_eq!(
            actual_rows(&db, &selected, width),
            expected_rows(&case["expected"]),
            "{id}: complete output bag, multiplicity and identities"
        );
        assert_eq!(
            actual_rows(&db, &rejected, 2),
            expected_rows(&case["not_true"]),
            "{id}: non-selected pairs, FALSE/UNKNOWN and ASOF eligible-but-nonnearest"
        );
        // Except for the OUTER APPLY synthetic NULL row, every output pair
        // and non-selected pair must partition the complete physical Cartesian bag.
        if variant != "outer_apply" {
            let pairs = "SELECT l.id,r.id FROM l CROSS JOIN r";
            let selected_pairs = match variant {
                "asof_nearest_prior" => "SELECT l.id,r.id FROM l ASOF JOIN r ON l.t >= r.t",
                "range_inequality" | "time_inequality" => "SELECT l.id,r.id FROM l JOIN r ON l.t > r.t",
                _ => "SELECT l.id,q.id FROM l CROSS JOIN LATERAL (SELECT r.id FROM r WHERE r.t=l.t) q",
            };
            let mut observed = actual_rows(&db, selected_pairs, 2);
            observed.extend(actual_rows(&db, &rejected, 2));
            observed.sort();
            assert_eq!(
                observed,
                actual_rows(&db, pairs, 2),
                "{id}: physical pair partition"
            );
        }
    }
}
