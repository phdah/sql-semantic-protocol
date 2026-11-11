//! TASK-192 source-independent counterexamples: native predicates may select
//! NULL, negative, unbounded, and duplicate projected IDs. A finite observed
//! terminal bag is not an allowed value domain for arbitrary future inputs.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name,
    ComposedSemantics, ConfiguredSqlInput, ProtocolStatement, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");

struct Counterexample {
    id: &'static str,
    setup: &'static str,
    relation: &'static str,
    predicate: &'static str,
    column: &'static str,
    datatype: &'static str,
}

fn original<'a>(registry: &'a Value, id: &str) -> &'a str {
    registry["cases"]
        .as_array()
        .expect("original named SQLs")
        .iter()
        .find(|case| case["id"] == id)
        .expect("fixed original SQL")["sql"]
        .as_str()
        .expect("SQL text")
}

fn row_ids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut statement = db.prepare(sql).expect("physical DuckDB source");
    let mut result: Vec<_> = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("native rowid query")
        .map(|row| row.expect("physical identity"))
        .collect();
    result.sort();
    result
}

fn id_bag(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut statement = db.prepare(sql).expect("original SQL");
    let mut result: Vec<_> = statement
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .expect("native original result")
        .map(|row| row.expect("nullable output"))
        .collect();
    result.sort();
    result
}

#[test]
fn nullable_duplicate_projected_ids_cannot_gain_false_bounds_from_source_predicates() {
    let registry: Value = serde_json::from_str(FREEZE).expect("original source SQL");
    let samples = [
        Counterexample {
            id: "88-C177-S020",
            setup: "CREATE TABLE customers (id BIGINT, name VARCHAR);
                    INSERT INTO customers VALUES
                    (999, 'Alice'), (-100, 'A_foo'), (NULL, 'ABC'),
                    (999, 'Bob'), (8, NULL), (999, 'A_foo');",
            relation: "customers",
            predicate: "name LIKE 'A%'",
            column: "name",
            datatype: "VARCHAR",
        },
        Counterexample {
            id: "88-C177-S034",
            setup: "CREATE TABLE orders (id BIGINT, created_at TIMESTAMP);
                    INSERT INTO orders VALUES
                    (999, '2020-01-02 00:00:00'), (-100, '2020-01-03 00:00:00'),
                    (NULL, '2020-01-02 00:00:00'), (999, '2020-01-01 00:00:00'),
                    (8, NULL), (999, '2020-01-02 00:00:00');",
            relation: "orders",
            predicate: "created_at >= TIMESTAMP '2020-01-01 00:00:00' + INTERVAL '1' DAY",
            column: "created_at",
            datatype: "TIMESTAMP",
        },
        Counterexample {
            id: "88-C177-S040",
            setup: "CREATE TABLE orders (id BIGINT, amount DOUBLE);
                    INSERT INTO orders VALUES
                    (999, CAST('NaN' AS DOUBLE)), (-100, CAST('NaN' AS DOUBLE)),
                    (NULL, CAST('NaN' AS DOUBLE)), (999, 0.0), (8, NULL),
                    (999, CAST('NaN' AS DOUBLE));",
            relation: "orders",
            predicate: "amount = 'NaN'",
            column: "amount",
            datatype: "DOUBLE",
        },
    ];

    for sample in samples {
        let sql = original(&registry, sample.id);
        assert!(
            sql.ends_with(&format!(" WHERE {}", sample.predicate)),
            "{}: counterexample must use the original predicate",
            sample.id
        );
        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(sample.setup).expect("independent fixed source");
        let entire = vec![None, Some(-100), Some(8), Some(999), Some(999), Some(999)];
        assert_eq!(
            id_bag(&db, &format!("SELECT id FROM {}", sample.relation)),
            entire,
            "{}: complete physical source including SQL NULL and duplicate IDs",
            sample.id
        );
        assert_eq!(
            id_bag(&db, sql),
            vec![None, Some(-100), Some(999), Some(999)],
            "{}: full selected terminal, not only set membership",
            sample.id
        );
        for (suffix, expected) in [
            ("IS TRUE", vec![0, 1, 2, 5]),
            ("IS FALSE", vec![3]),
            ("IS UNKNOWN", vec![4]),
            ("IS NOT TRUE", vec![3, 4]),
        ] {
            let query = format!(
                "SELECT rowid FROM {} WHERE ({}) {suffix}",
                sample.relation, sample.predicate
            );
            assert_eq!(
                row_ids(&db, &query),
                expected,
                "{}: complete {suffix} physical row occurrence partition",
                sample.id
            );
        }
        for dialect_name in registry["dialects"].as_array().expect("13 exposed dialects") {
            let dialect_name = dialect_name.as_str().expect("dialect");
            let dialect = dialect_from_name(dialect_name).expect("supported parser");
            let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
                .expect("original SQL must parse");
            let [ProtocolStatement::Query(query)] = protocol.statements() else {
                panic!("{}: missing query", sample.id);
            };
            assert!(
                matches!(
                    query.output().columns()[0].domain(),
                    ValueDomain::Unbounded | ValueDomain::Unknown(_)
                ),
                "{}/{}: observed terminal cannot constrain arbitrary projected IDs",
                sample.id,
                dialect_name
            );

            let source = RelationSchema::new(
                sample.relation,
                vec![
                    SchemaColumn::from_sql_type("id", "BIGINT", dialect_name)
                        .expect("signed identity"),
                    SchemaColumn::from_sql_type(sample.column, sample.datatype, dialect_name)
                        .expect("physical source type"),
                ],
            )
            .expect("typed source");
            let catalog = RelationCatalog::from_schemas(&[source])
                .expect("independent physical type facts");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "task192-counterexample",
                &input,
                dialect_name,
                dialect.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .expect("typed public analyzer");
            let layer = bundle.layers().last().expect("one output");
            let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
                panic!("{}/{}: source-complete query did not resolve", sample.id, dialect_name);
            };
            assert!(
                matches!(
                    semantics.output().columns()[0].domain(),
                    ValueDomain::Unbounded | ValueDomain::Unknown(_)
                ),
                "{}/{}: independently typed source must not invent finite projected ID",
                sample.id,
                dialect_name
            );
        }
    }
}
