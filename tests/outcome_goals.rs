use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, select_targets, to_bundle_json, ConstraintValue, OutcomeGoal,
    OutcomeGoalError, OutcomeGoalStatus, OutputDistribution, OutputValueCount, SqlInput,
};
use sqlparser::dialect::GenericDialect;

fn analyze(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    analyze_inputs(&[SqlInput::inline(sql)], "generic", &GenericDialect {})
        .expect("analyze SQL fixture")
}

fn request(
    bundle: &mut sql_semantic_protocol::AnalysisBundle,
    rows: Option<u64>,
    groups: Option<u64>,
    distributions: Vec<OutputDistribution>,
) {
    let layer = bundle.layers()[0].id().to_string();
    bundle.set_outcome_goals(&[
        OutcomeGoal::new(layer, rows, groups, distributions).expect("valid request")
    ]).expect("attach output goal");
}

fn oracle_count(connection: &Connection, sql: &str) -> u64 {
    connection.query_row(
        &format!("SELECT COUNT(*) FROM ({sql}) AS result"),
        [],
        |row| row.get::<_, i64>(0),
    ).expect("execute SQL") as u64
}

#[test]
fn goal_contract_is_opt_in_and_output_identity_is_stable() {
    let mut bundle = analyze("SELECT id FROM source_data");
    let default_json: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("valid JSON");
    assert!(default_json.get("outcome_goals").is_none());

    request(&mut bundle, Some(0), None, vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
    let result: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("valid JSON");
    assert_eq!(result["outcome_goals"][0]["layer_id"], bundle.layers()[0].id());
    assert_eq!(result["outcome_goals"][0]["assessment"]["status"], "feasible");
    assert_eq!(result["outcome_goals"][0]["requested"]["rows"], 0);

    let connection = Connection::open_in_memory().expect("DuckDB");
    connection.execute_batch("CREATE TABLE source_data(id INTEGER);").expect("create empty source");
    assert_eq!(oracle_count(&connection, "SELECT id FROM source_data"), 0);
}

#[test]
fn singleton_output_bounds_are_proved_and_executed() {
    let mut bundle = analyze("SELECT 1 AS result");
    request(&mut bundle, Some(1), None, vec![]);
    let goal = &bundle.outcome_goals()[0];
    assert_eq!(goal.status(), OutcomeGoalStatus::Feasible);
    assert_eq!((goal.min_rows(), goal.max_rows()), (1, Some(1)));

    request(&mut bundle, Some(0), None, vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);
    let connection = Connection::open_in_memory().expect("DuckDB");
    assert_eq!(oracle_count(&connection, "SELECT 1 AS result"), 1);
}

#[test]
fn global_aggregate_rows_are_not_confused_with_source_rows() {
    let mut bundle = analyze("SELECT COUNT(*) AS total FROM sales");
    request(&mut bundle, Some(1), None, vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
    request(&mut bundle, Some(3), None, vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);

    let connection = Connection::open_in_memory().expect("DuckDB");
    connection.execute_batch("CREATE TABLE sales(id INTEGER);").expect("create source");
    assert_eq!(oracle_count(&connection, "SELECT COUNT(*) AS total FROM sales"), 1);
    connection.execute_batch("INSERT INTO sales VALUES (1),(2),(3);").expect("insert");
    assert_eq!(oracle_count(&connection, "SELECT COUNT(*) AS total FROM sales"), 1);
}

#[test]
fn group_goals_enforce_one_surviving_row_per_ordinary_group() {
    let sql = "SELECT category, COUNT(*) AS n FROM sales GROUP BY category";
    let mut bundle = analyze(sql);
    request(&mut bundle, Some(2), Some(3), vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);
    request(&mut bundle, Some(2), Some(2), vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);

    let connection = Connection::open_in_memory().expect("DuckDB");
    connection.execute_batch(
        "CREATE TABLE sales(category INTEGER);
         INSERT INTO sales VALUES (1),(1),(NULL),(NULL);"
    ).expect("create grouped fixtures");
    assert_eq!(oracle_count(&connection, sql), 2);
}

#[test]
fn distinct_and_null_semantics_reject_duplicate_distribution_values() {
    let sql = "SELECT DISTINCT category FROM sales";
    let mut bundle = analyze(sql);
    let distribution = OutputDistribution::new(
        "category",
        vec![OutputValueCount::new(ConstraintValue::Null, 2)],
    ).expect("valid NULL histogram");
    request(&mut bundle, Some(2), None, vec![distribution]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);

    let connection = Connection::open_in_memory().expect("DuckDB");
    connection.execute_batch(
        "CREATE TABLE sales(category INTEGER);
         INSERT INTO sales VALUES (NULL),(NULL);"
    ).expect("create duplicate null fixture");
    assert_eq!(oracle_count(&connection, sql), 1);
}

#[test]
fn distribution_arithmetic_catches_impossible_totals_and_overflow() {
    let mut bundle = analyze("SELECT v FROM sales");
    let distribution = OutputDistribution::new(
        "v",
        vec![OutputValueCount::new(ConstraintValue::Integer(5), 3)],
    ).expect("valid histogram");
    request(&mut bundle, Some(2), None, vec![distribution]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);

    let distribution = OutputDistribution::new(
        "v",
        vec![
            OutputValueCount::new(ConstraintValue::Integer(5), u64::MAX),
            OutputValueCount::new(ConstraintValue::Integer(6), 1),
        ],
    ).expect("valid histogram");
    request(&mut bundle, Some(2), None, vec![distribution]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);
}

#[test]
fn singleton_literal_histograms_are_proven_and_checked_by_duckdb() {
    let sql = "SELECT 1 AS x";
    let mut bundle = analyze(sql);
    let exact = OutputDistribution::new("x", vec![
        OutputValueCount::new(ConstraintValue::Integer(1), 1)
    ]).expect("histogram");
    request(&mut bundle, Some(1), None, vec![exact]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);

    let impossible = OutputDistribution::new("x", vec![
        OutputValueCount::new(ConstraintValue::Integer(2), 1)
    ]).expect("histogram");
    request(&mut bundle, Some(1), None, vec![impossible]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Unsatisfiable);

    let connection = Connection::open_in_memory().expect("DuckDB");
    assert_eq!(oracle_count(&connection, sql), 1);
    let actual: i64 = connection.query_row(sql, [], |row| row.get(0)).expect("literal value");
    assert_eq!(actual, 1);
}

#[test]
fn null_literal_histogram_is_exact() {
    let mut bundle = analyze("SELECT NULL AS x");
    let histogram = OutputDistribution::new("x", vec![
        OutputValueCount::new(ConstraintValue::Null, 1)
    ]).expect("histogram");
    request(&mut bundle, Some(1), None, vec![histogram]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
}

#[test]
fn complex_join_window_and_group_distribution_require_real_witnesses() {
    let mut bundle = analyze("SELECT a.id FROM a JOIN b ON a.id = b.id");
    request(&mut bundle, Some(3), None, vec![]);
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
}

#[test]
fn input_validation_and_target_selection_preserve_layer_scopes() {
    let duplicate = OutputDistribution::new(
        "x",
        vec![
            OutputValueCount::new(ConstraintValue::Null, 1),
            OutputValueCount::new(ConstraintValue::Null, 2),
        ],
    );
    assert!(matches!(duplicate, Err(OutcomeGoalError::DuplicateValue { .. })));

    let mut bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage AS SELECT id FROM source_data"),
            SqlInput::inline("CREATE TABLE unrelated AS SELECT id FROM other_data"),
        ],
        "generic",
        &GenericDialect {},
    ).expect("bundle");
    let target_id = bundle.layers()[0].id().to_string();
    let other_id = bundle.layers()[1].id().to_string();
    bundle.set_outcome_goals(&[
        OutcomeGoal::new(target_id.clone(), Some(0), None, vec![]).unwrap(),
        OutcomeGoal::new(other_id, Some(0), None, vec![]).unwrap(),
    ]).expect("attach both");
    let selected = select_targets(&bundle, &["stage".to_string()]).expect("target exists");
    assert_eq!(selected.outcome_goals().len(), 1);
    assert_eq!(selected.outcome_goals()[0].goal().layer_id(), target_id);
}
