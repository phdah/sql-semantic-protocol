//! Physical-source parity against unchanged sql-tdg fixtures.
//!
//! Pinned from phdah/sql-tdg at commit
//! 90ec0e12a2d5cd2c1294556336fc947e176c9db9.
//! These SQL fixture files are byte-for-byte copies, not recreated examples.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_row_count_plan,
    physical_row_count_plan, physical_source_plan, AnalysisBundle, ConfiguredSqlInput,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

const ADVANCED: &str = include_str!("fixtures/sql_tdg/advanced_pipeline.sql");
const BOUNDARY: &str = include_str!("fixtures/sql_tdg/advanced_boundary.sql");
const SETS: &str = include_str!("fixtures/sql_tdg/set_operations.sql");

fn fixture_bundle(sql: &str) -> AnalysisBundle {
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).expect("dialect");
    let columns = [
        (
            "raw_orders",
            &[
                ("order_id", "INTEGER"),
                ("customer_id", "INTEGER"),
                ("amount", "INTEGER"),
            ][..],
        ),
        (
            "raw_customers",
            &[("customer_id", "INTEGER"), ("active", "BOOLEAN")][..],
        ),
        ("raw_a", &[("value", "INTEGER")][..]),
        ("raw_b", &[("value", "INTEGER")][..]),
        ("raw_c", &[("value", "INTEGER")][..]),
    ];
    let schemas = columns
        .iter()
        .map(|(relation, columns)| {
            RelationSchema::new(
                *relation,
                columns
                    .iter()
                    .map(|(name, sql_type)| {
                        SchemaColumn::from_sql_type(*name, sql_type, dialect)
                            .expect("schema column")
                    })
                    .collect(),
            )
            .expect("relation schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let input = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("sql-tdg", &input, dialect, parser.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).expect("fixture analysis")
}

fn layer_id<'a>(bundle: &'a AnalysisBundle, relation: &str) -> &'a str {
    bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|producer| producer.relation_name() == Some(relation))
        })
        .unwrap_or_else(|| panic!("missing fixture output {relation}"))
        .id()
}

#[test]
fn sql_tdg_advanced_pipeline_constructs_joint_terminal_zero_from_raw_sources() {
    let bundle = fixture_bundle(ADVANCED);
    let summary = layer_id(&bundle, "mart_customer_summary");
    let stage = layer_id(&bundle, "stage_orders");
    let plan = physical_source_plan(&bundle, summary);
    assert_eq!(
        plan.sources(),
        &["raw_customers".to_string(), "raw_orders".to_string()]
    );
    assert!(
        matches!(plan.zero_output(), WitnessDirection::Feasible(_)),
        "pinned pipeline must be empty-preserving: {plan:?}; each layer: {:?}",
        bundle
            .layers()
            .iter()
            .map(|layer| (
                layer.id(),
                physical_source_plan(&bundle, layer.id())
                    .zero_output()
                    .clone()
            ))
            .collect::<Vec<_>>()
    );
    let joint = physical_joint_row_count_plan(&bundle, &[(stage, 0), (summary, 0)]);
    let WitnessDirection::Feasible(cases) = joint else {
        panic!("shared sources must prove both real sql-tdg terminals empty: {joint:?}");
    };
    assert_eq!(
        cases[0]
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count(),
        2
    );
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_orders(order_id INTEGER, customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_customers(customer_id INTEGER, active BOOLEAN);",
    )
    .expect("source schemas");
    conn.execute_batch(ADVANCED)
        .expect("committed sql-tdg pipeline");
    for sql in [
        "SELECT COUNT(*) FROM stage_orders",
        "SELECT COUNT(*) FROM stage_customers",
        "SELECT COUNT(*) FROM core_enriched",
        "SELECT COUNT(*) FROM mart_customer_summary",
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("output");
        assert_eq!(count, 0, "{sql}");
    }
}

#[test]
fn sql_tdg_boundary_fixture_retains_join_provenance_and_zero_rejection() {
    let bundle = fixture_bundle(BOUNDARY);
    let enriched = layer_id(&bundle, "core_enriched");
    let plan = physical_source_plan(&bundle, enriched);
    assert!(matches!(plan.zero_output(), WitnessDirection::Feasible(_)));
    assert!(matches!(
        physical_row_count_plan(&bundle, enriched, 1),
        WitnessDirection::Residual { .. }
    ));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_orders(order_id INTEGER, customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_customers(customer_id INTEGER, active BOOLEAN);",
    )
    .expect("source schemas");
    conn.execute_batch(BOUNDARY)
        .expect("committed sql-tdg boundary");
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM core_enriched", [], |row| row.get(0))
        .expect("enriched count");
    assert_eq!(rows, 0);
}

#[test]
fn sql_tdg_set_fixtures_prove_empty_closed_world_and_respect_multiplicities() {
    let bundle = fixture_bundle(SETS);
    for target in [
        "union_all_result",
        "union_result",
        "intersect_result",
        "except_result",
        "distinct_result",
        "limited_result",
    ] {
        let id = layer_id(&bundle, target);
        let proof = physical_row_count_plan(&bundle, id, 0);
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| layer.id() == id)
            .expect("layer");
        let wire: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&bundle))
                .expect("canonical JSON");
        if target == "union_all_result" {
            let branches = wire["inputs"][0]["statements"][3]["set_operation"]["membership"]
                ["branches"]
                .as_array()
                .expect("typed canonical set branches");
            assert_eq!(branches.len(), 2);
            assert!(branches.iter().all(|branch| {
                branch["empty_input_preserving"] == serde_json::Value::Bool(true)
            }));
        }
        let branch_details = bundle
            .inputs()
            .iter()
            .find(|input| input.id() == layer.input_id())
            .and_then(|input| input.statements().get(layer.statement_index()))
            .and_then(|statement| match statement {
                ProtocolStatement::Query(query) => query.set_operation(),
                _ => None,
            })
            .map(|operation| {
                operation
                    .branches()
                    .iter()
                    .map(|branch| {
                        (
                            branch.identity().to_string(),
                            branch
                                .sources()
                                .iter()
                                .map(|source| source.name().to_string())
                                .collect::<Vec<_>>(),
                            branch.witness_boundary().is_some(),
                            branch.predicates().having_predicate().is_some(),
                            branch.condition_exactness().is_exact(),
                            branch
                                .output()
                                .columns()
                                .iter()
                                .map(|column| format!("{:?}", column.expression()))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            });
        let shape = bundle
            .inputs()
            .iter()
            .find(|input| input.id() == layer.input_id())
            .and_then(|input| input.statements().get(layer.statement_index()))
            .and_then(|statement| match statement {
                ProtocolStatement::Query(query) => Some((
                    query.sources().len(),
                    query.aggregation().map(|aggregation| (aggregation.distinct(), aggregation.group_by().is_some())),
                    query.diagnostics().iter().map(|diagnostic| diagnostic.code()).collect::<Vec<_>>(),
                    query.output().columns().iter().map(|column| format!("{:?}", column.expression())).collect::<Vec<_>>(),
                )),
                _ => None,
            });
        assert!(
            matches!(proof, WitnessDirection::Feasible(_)),
            "pinned sql-tdg set fixture {target} must preserve empty inputs: {proof:?}; source plan: {:?}; branch details: {branch_details:?}; shape: {shape:?}",
            physical_source_plan(&bundle, id)
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_a(value INTEGER);
         CREATE TABLE raw_b(value INTEGER);
         CREATE TABLE raw_c(value INTEGER);",
    )
    .expect("source schemas");
    conn.execute_batch(SETS)
        .expect("committed sql-tdg set pipeline");
    for target in [
        "union_all_result",
        "union_result",
        "intersect_result",
        "except_result",
        "distinct_result",
        "limited_result",
    ] {
        let sql = format!("SELECT COUNT(*) FROM {target}");
        let rows: i64 = conn
            .query_row(&sql, [], |row| row.get(0))
            .expect("set output count");
        assert_eq!(rows, 0, "{target}");
    }
}

#[test]
fn pinned_sql_tdg_advanced_pipeline_oracle_exercises_positive_and_rejected_rows() {
    let bundle = fixture_bundle(ADVANCED);
    let summary = layer_id(&bundle, "mart_customer_summary");
    assert!(
        matches!(
            physical_row_count_plan(&bundle, summary, 1),
            WitnessDirection::Residual { .. }
        ),
        "a positive joined/grouped construction is not yet supported"
    );
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_orders(order_id INTEGER, customer_id INTEGER, amount INTEGER);
         CREATE TABLE raw_customers(customer_id INTEGER, active BOOLEAN);
         INSERT INTO raw_customers VALUES (1, true), (2, false);
         INSERT INTO raw_orders VALUES
            (11, 1, 100), (12, 2, 100), (13, 1, 90), (14, 999, 100);",
    )
    .expect("controlled source data");
    conn.execute_batch(ADVANCED)
        .expect("committed sql-tdg advanced SQL");
    let counts = [
        ("stage_orders", 3_i64),
        ("stage_customers", 1),
        ("core_enriched", 1),
        ("mart_customer_summary", 1),
    ];
    for (table, expected) in counts {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = conn.query_row(&sql, [], |row| row.get(0)).expect("count");
        assert_eq!(count, expected, "{table}");
    }
    let (category, orders, maximum): (String, i64, i64) = conn
        .query_row(
            "SELECT amount_bucket, order_count, max_amount FROM mart_customer_summary",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("summarized row");
    assert_eq!((category.as_str(), orders, maximum), ("high", 1, 100));
}

#[test]
fn pinned_sql_tdg_set_oracles_preserve_duplicate_and_distinct_multiplicity() {
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE raw_a(value INTEGER);
         CREATE TABLE raw_b(value INTEGER);
         CREATE TABLE raw_c(value INTEGER);
         INSERT INTO raw_a VALUES (10), (10), (NULL);
         INSERT INTO raw_b VALUES (20);
         INSERT INTO raw_c VALUES (30);",
    )
    .expect("controlled duplicate and NULL inputs");
    conn.execute_batch(SETS).expect("committed sql-tdg set SQL");
    for (target, expected) in [
        ("set_a", 2_i64),
        ("set_b", 1),
        ("set_c", 1),
        ("union_all_result", 2),
        ("union_result", 1),
        ("intersect_result", 1),
        ("except_result", 1),
        ("distinct_result", 1),
        ("limited_result", 1),
    ] {
        let sql = format!("SELECT COUNT(*) FROM {target}");
        let rows: i64 = conn
            .query_row(&sql, [], |row| row.get(0))
            .expect("set cardinality");
        assert_eq!(rows, expected, "{target}");
    }
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM union_all_result WHERE marker = 2",
            [],
            |row| row.get(0),
        )
        .expect("limited rejection");
    assert_eq!(count, 0);
}

#[test]
fn source_free_set_leaf_cannot_be_erased_by_emptying_the_other_source() {
    let bundle = fixture_bundle("SELECT 1 AS marker UNION ALL SELECT value AS marker FROM raw_a");
    let target = bundle.layers()[0].id();
    assert!(matches!(
        physical_source_plan(&bundle, target).zero_output(),
        WitnessDirection::Residual { .. }
    ));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE raw_a(value INTEGER)")
        .expect("empty source schema");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT 1 AS marker UNION ALL SELECT value AS marker FROM raw_a)",
            [],
            |row| row.get(0),
        )
        .expect("set count");
    assert_eq!(count, 1);
}
