use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, dialect_from_name,
    CaseSourceDomains, ColumnDomain, ComparisonOperator, ComposedSemantics,
    ConditionExactnessStatus, ConfiguredSqlInput, Expression, Join, LiteralValue, Predicate,
    Protocol, ProtocolStatement, QueryStatement, RelationCatalog, RelationSchema,
    ResolvedComposedSemantics, SchemaColumn, SetMode, SqlInput, ValueDomain,
};

type ColumnIdentity<'a> = (&'a str, &'a str);
type ColumnEquality<'a> = (ColumnIdentity<'a>, ColumnIdentity<'a>);

#[derive(Debug, Clone, Copy)]
struct SourceRow {
    row_id: i64,
    a: Option<i64>,
    b: Option<i64>,
    c: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct LeftRow {
    row_id: i64,
    a: Option<i64>,
    x: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct RightRow {
    row_id: i64,
    b: Option<i64>,
    y: Option<i64>,
}

fn duckdb_connection() -> Connection {
    let connection = Connection::open_in_memory().expect("DuckDB in-memory connection");
    connection
        .execute_batch(
            "
            CREATE TABLE predicate_rows (
                row_id BIGINT NOT NULL,
                a BIGINT,
                b BIGINT,
                c BIGINT
            );
            CREATE TABLE left_rows (
                row_id BIGINT NOT NULL,
                a BIGINT,
                x BIGINT
            );
            CREATE TABLE right_rows (
                row_id BIGINT NOT NULL,
                b BIGINT,
                y BIGINT
            );
            ",
        )
        .expect("create oracle tables");

    let predicate_values = source_rows()
        .into_iter()
        .map(|row| {
            format!(
                "({}, {}, {}, {})",
                row.row_id,
                sql_integer(row.a),
                sql_integer(row.b),
                sql_integer(row.c)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let left_values = left_rows()
        .into_iter()
        .map(|row| {
            format!(
                "({}, {}, {})",
                row.row_id,
                sql_integer(row.a),
                sql_integer(row.x)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let right_values = right_rows()
        .into_iter()
        .map(|row| {
            format!(
                "({}, {}, {})",
                row.row_id,
                sql_integer(row.b),
                sql_integer(row.y)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    connection
        .execute_batch(&format!(
            "INSERT INTO predicate_rows VALUES {predicate_values};
             INSERT INTO left_rows VALUES {left_values};
             INSERT INTO right_rows VALUES {right_values};"
        ))
        .expect("populate oracle tables");

    connection
}

fn sql_integer(value: Option<i64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

fn source_rows() -> Vec<SourceRow> {
    let values = [None, Some(-2), Some(-1), Some(0), Some(1), Some(2)];
    let mut rows = Vec::with_capacity(values.len().pow(3));
    let mut row_id = 1_i64;

    for a in values {
        for b in values {
            for c in values {
                rows.push(SourceRow { row_id, a, b, c });
                row_id += 1;
            }
        }
    }

    rows
}

fn left_rows() -> Vec<LeftRow> {
    vec![
        LeftRow {
            row_id: 1,
            a: Some(-1),
            x: Some(1),
        },
        LeftRow {
            row_id: 2,
            a: Some(1),
            x: Some(1),
        },
        LeftRow {
            row_id: 3,
            a: Some(2),
            x: Some(2),
        },
        LeftRow {
            row_id: 4,
            a: None,
            x: Some(2),
        },
        LeftRow {
            row_id: 5,
            a: Some(3),
            x: None,
        },
    ]
}

fn right_rows() -> Vec<RightRow> {
    vec![
        RightRow {
            row_id: 11,
            b: Some(1),
            y: Some(1),
        },
        RightRow {
            row_id: 12,
            b: Some(3),
            y: Some(1),
        },
        RightRow {
            row_id: 13,
            b: Some(2),
            y: Some(2),
        },
        RightRow {
            row_id: 14,
            b: None,
            y: Some(2),
        },
        RightRow {
            row_id: 15,
            b: Some(0),
            y: None,
        },
    ]
}

fn analyze_duckdb(sql: &str) -> Protocol {
    let dialect = dialect_from_name("duckdb").expect("DuckDB dialect");
    analyze_sql(sql, "duckdb", dialect.as_ref())
        .unwrap_or_else(|error| panic!("DuckDB SQL should analyze: {sql}\n{error}"))
}

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

fn row_ids(connection: &Connection, sql: &str) -> BTreeSet<i64> {
    let mut statement = connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("prepare oracle query: {sql}\n{error}"));
    statement
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|error| panic!("execute oracle query: {sql}\n{error}"))
        .map(|row| row.expect("read oracle row id"))
        .collect()
}

fn row_pairs(connection: &Connection, sql: &str) -> BTreeSet<(i64, i64)> {
    let mut statement = connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("prepare oracle query: {sql}\n{error}"));
    statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap_or_else(|error| panic!("execute oracle query: {sql}\n{error}"))
        .map(|row| row.expect("read oracle row pair"))
        .collect()
}

fn source_value(row: &SourceRow, column: &str) -> Option<i64> {
    match column {
        "row_id" => Some(row.row_id),
        "a" => row.a,
        "b" => row.b,
        "c" => row.c,
        other => panic!("unexpected source column {other}"),
    }
}

fn left_value(row: &LeftRow, column: &str) -> Option<i64> {
    match column {
        "row_id" => Some(row.row_id),
        "a" => row.a,
        "x" => row.x,
        other => panic!("unexpected left column {other}"),
    }
}

fn right_value(row: &RightRow, column: &str) -> Option<i64> {
    match column {
        "row_id" => Some(row.row_id),
        "b" => row.b,
        "y" => row.y,
        other => panic!("unexpected right column {other}"),
    }
}

fn literal_integer(value: &sql_semantic_protocol::LiteralExpression) -> Option<i64> {
    match value.value() {
        LiteralValue::Number(value) => Some(
            value
                .parse::<i64>()
                .unwrap_or_else(|error| panic!("expected integer literal {value}: {error}")),
        ),
        LiteralValue::Null => None,
        other => panic!("expected integer or NULL literal, got {other:?}"),
    }
}

fn domain_admits_integer(domain: &ValueDomain, value: Option<i64>) -> bool {
    let Some(value) = value else {
        return domain
            .admits_null()
            .expect("exact conformance checks require known NULL membership");
    };

    match domain {
        ValueDomain::Unbounded => true,
        ValueDomain::Empty => false,
        ValueDomain::Unknown(unknown) => {
            panic!(
                "exact conformance check received unknown domain: {}",
                unknown.reason()
            )
        }
        ValueDomain::Set(set) => {
            let contains = set
                .values()
                .iter()
                .filter_map(literal_integer)
                .any(|candidate| candidate == value);
            match set.mode() {
                SetMode::Include => contains,
                SetMode::Exclude => !contains,
            }
        }
        ValueDomain::Ranges(ranges) => ranges.ranges().iter().any(|range| {
            let lower = range.lower().is_none_or(|bound| {
                let candidate = literal_integer(bound.value()).expect("range bound is not NULL");
                if bound.inclusive() {
                    value >= candidate
                } else {
                    value > candidate
                }
            });
            let upper = range.upper().is_none_or(|bound| {
                let candidate = literal_integer(bound.value()).expect("range bound is not NULL");
                if bound.inclusive() {
                    value <= candidate
                } else {
                    value < candidate
                }
            });
            lower && upper
        }),
        _ => panic!("new value-domain variant needs conformance support"),
    }
}

fn output_domain_admits_integer(domain: &ValueDomain, value: Option<i64>) -> bool {
    match domain {
        ValueDomain::Unknown(_) | ValueDomain::Unbounded => true,
        _ => domain_admits_integer(domain, value),
    }
}

fn source_row_matches_domains(row: &SourceRow, domains: &[ColumnDomain]) -> bool {
    domains.iter().all(|domain| {
        let relation = domain.column().relation();
        assert!(
            relation.is_none() || relation == Some("predicate_rows"),
            "unexpected relation in direct source domain: {relation:?}"
        );
        domain_admits_integer(domain.domain(), source_value(row, domain.column().name()))
    })
}

fn predicted_row_ids(domains: &[ColumnDomain]) -> BTreeSet<i64> {
    source_rows()
        .into_iter()
        .filter(|row| source_row_matches_domains(row, domains))
        .map(|row| row.row_id)
        .collect()
}

fn assert_exact_matches_oracle(connection: &Connection, predicate: &str) {
    let sql = format!("SELECT row_id FROM predicate_rows WHERE {predicate} ORDER BY row_id");
    let protocol = analyze_duckdb(&sql);
    let query = first_query(&protocol);

    assert_eq!(
        query.condition_exactness().status(),
        ConditionExactnessStatus::Exact,
        "protocol unexpectedly marked exact fixture residual: {sql}\n{:?}",
        query.condition_exactness().residual_conditions()
    );
    assert_eq!(
        predicted_row_ids(query.column_domains()),
        row_ids(connection, &sql),
        "protocol domains disagree with DuckDB for {sql}"
    );
}

fn column_pair(predicate: &Predicate) -> Option<ColumnEquality<'_>> {
    let Predicate::Comparison(comparison) = predicate else {
        return None;
    };
    if comparison.operator() != ComparisonOperator::Eq {
        return None;
    }
    let Expression::Column(left) = comparison.left() else {
        return None;
    };
    let Expression::Column(right) = comparison.right() else {
        return None;
    };
    Some((
        (
            left.relation().expect("join column has relation"),
            left.name(),
        ),
        (
            right.relation().expect("join column has relation"),
            right.name(),
        ),
    ))
}

fn collect_join_equalities<'a>(predicate: &'a Predicate, equalities: &mut Vec<ColumnEquality<'a>>) {
    if let Some(equality) = column_pair(predicate) {
        equalities.push(equality);
        return;
    }

    if let Predicate::And(and) = predicate {
        for operand in and.operands() {
            collect_join_equalities(operand, equalities);
        }
    }
}

fn join_equalities(joins: &[Join]) -> Vec<ColumnEquality<'_>> {
    let mut equalities = Vec::new();
    for join in joins {
        if let Some(condition) = join.condition() {
            collect_join_equalities(condition, &mut equalities);
        }
    }
    equalities
}

fn joined_value(left: &LeftRow, right: &RightRow, relation: &str, column: &str) -> Option<i64> {
    match relation {
        "left_rows" => left_value(left, column),
        "right_rows" => right_value(right, column),
        other => panic!("unexpected joined relation {other}"),
    }
}

fn pair_matches_domains(left: &LeftRow, right: &RightRow, domains: &[ColumnDomain]) -> bool {
    domains.iter().all(|domain| {
        let relation = domain
            .column()
            .relation()
            .expect("join source domain should have a relation");
        domain_admits_integer(
            domain.domain(),
            joined_value(left, right, relation, domain.column().name()),
        )
    })
}

fn pair_matches_equalities(
    left: &LeftRow,
    right: &RightRow,
    equalities: &[ColumnEquality<'_>],
) -> bool {
    equalities.iter().all(
        |((left_relation, left_column), (right_relation, right_column))| {
            let left_value = joined_value(left, right, left_relation, left_column);
            let right_value = joined_value(left, right, right_relation, right_column);
            match (left_value, right_value) {
                (Some(left_value), Some(right_value)) => left_value == right_value,
                _ => false,
            }
        },
    )
}

fn expression_integer(expression: &Expression) -> i64 {
    let Expression::Literal(literal) = expression else {
        panic!("CASE conformance fixture expects literal branch results, got {expression:?}");
    };
    literal_integer(literal).expect("CASE branch result is not NULL")
}

fn alternative_matches(row: &SourceRow, domains: &[ColumnDomain]) -> bool {
    source_row_matches_domains(row, domains)
}

fn source_domains_match(row: &SourceRow, domains: &CaseSourceDomains) -> bool {
    match domains {
        CaseSourceDomains::Reachable { alternatives } => alternatives
            .iter()
            .any(|alternative| alternative_matches(row, alternative.column_domains())),
        CaseSourceDomains::Unreachable => false,
        CaseSourceDomains::Unknown(unknown) => {
            panic!(
                "CASE conformance fixture has unknown source domains: {}",
                unknown.reason()
            )
        }
        _ => panic!("new CASE source-domain variant needs conformance support"),
    }
}

fn query_optional_i64_by_row_id(connection: &Connection, sql: &str) -> BTreeMap<i64, Option<i64>> {
    let mut statement = connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("prepare oracle query: {sql}\n{error}"));
    statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .unwrap_or_else(|error| panic!("execute oracle query: {sql}\n{error}"))
        .map(|row| row.expect("read oracle output"))
        .collect()
}

fn query_optional_i64(connection: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut statement = connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("prepare oracle query: {sql}\n{error}"));
    statement
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap_or_else(|error| panic!("execute oracle query: {sql}\n{error}"))
        .map(|row| row.expect("read oracle output"))
        .collect()
}

#[test]
fn allow_listed_predicates_match_duckdb_exactly() {
    let connection = duckdb_connection();

    for predicate in [
        "a = 1",
        "a <> 1",
        "a < 1",
        "a <= 1",
        "a > 0",
        "a >= 0",
        "a BETWEEN 0 AND 1",
        "a NOT BETWEEN 0 AND 1",
        "a IN (0, 1, 2)",
        "a NOT IN (0, 1, 2)",
        "a IS NULL",
        "a IS NOT NULL",
        "a IS DISTINCT FROM 1",
        "a IS NOT DISTINCT FROM 1",
        "a >= 0 AND a <= 1",
        "a = 0 OR a = 2",
    ] {
        assert_exact_matches_oracle(&connection, predicate);
    }
}

#[test]
fn residual_predicates_are_never_presented_as_exact() {
    for predicate in [
        "a = 1 OR b = 2",
        "(a = 1 AND b = 2) OR (a = 2 AND b = 1)",
        "NOT (a + 1 > 2)",
        "a + 1 > 2",
        "a = b",
        "a IN (SELECT b FROM predicate_rows)",
    ] {
        let sql = format!("SELECT row_id FROM predicate_rows WHERE {predicate}");
        let protocol = analyze_duckdb(&sql);
        let exactness = first_query(&protocol).condition_exactness();
        assert_eq!(
            exactness.status(),
            ConditionExactnessStatus::Residual,
            "protocol over-claimed exactness for {sql}"
        );
        assert!(
            !exactness.residual_conditions().is_empty(),
            "residual query did not explain its residual condition: {sql}"
        );
    }
}

#[test]
fn exact_join_domains_and_equalities_match_duckdb() {
    let connection = duckdb_connection();
    let sql = "
        SELECT left_rows.row_id, right_rows.row_id
        FROM left_rows
        JOIN right_rows
          ON left_rows.x = right_rows.y
         AND left_rows.a > 0
         AND right_rows.b <= 2
        ORDER BY left_rows.row_id, right_rows.row_id
    ";
    let protocol = analyze_duckdb(sql);
    let query = first_query(&protocol);
    assert!(query.condition_exactness().is_exact());

    let equalities = join_equalities(query.joins());
    assert_eq!(equalities.len(), 1, "expected one join equality");

    let mut predicted = BTreeSet::new();
    let mut violated_domain = false;
    let mut violated_equality = false;
    for left in left_rows() {
        for right in right_rows() {
            let domains = pair_matches_domains(&left, &right, query.column_domains());
            let equality = pair_matches_equalities(&left, &right, &equalities);
            if domains && equality {
                predicted.insert((left.row_id, right.row_id));
            }
            violated_domain |= !domains && equality;
            violated_equality |= domains && !equality;
        }
    }

    assert!(
        violated_domain,
        "fixture must exercise a one-domain violation"
    );
    assert!(
        violated_equality,
        "fixture must exercise an equality violation"
    );
    assert_eq!(predicted, row_pairs(&connection, sql));
}

#[test]
fn output_domains_are_sound_against_duckdb_results() {
    let connection = duckdb_connection();

    for sql in [
        "SELECT a FROM predicate_rows WHERE a BETWEEN -1 AND 2",
        "SELECT a + 1 AS value FROM predicate_rows WHERE a = 1",
        "SELECT CASE WHEN a < 0 THEN 10 WHEN a = 0 THEN 20 ELSE 30 END AS value FROM predicate_rows",
    ] {
        let protocol = analyze_duckdb(sql);
        let domain = first_query(&protocol).output().columns()[0].domain();
        for value in query_optional_i64(&connection, sql) {
            assert!(
                output_domain_admits_integer(domain, value),
                "DuckDB produced {value:?} outside protocol output domain {domain:?} for {sql}"
            );
        }
    }
}

#[test]
fn case_branch_source_domains_select_the_engine_branch() {
    let connection = duckdb_connection();
    let sql = "
        SELECT row_id,
               CASE
                 WHEN a < 0 THEN 10
                 WHEN a = 0 OR a = 1 THEN 20
                 ELSE 30
               END AS bucket
        FROM predicate_rows
        ORDER BY row_id
    ";
    let protocol = analyze_duckdb(sql);
    let query = first_query(&protocol);
    let Expression::Case(case_expression) = query.output().columns()[1].expression() else {
        panic!("expected CASE output expression");
    };
    let actual = query_optional_i64_by_row_id(&connection, sql);

    for row in source_rows() {
        let actual_value = actual
            .get(&row.row_id)
            .copied()
            .flatten()
            .expect("CASE result should be non-NULL");
        let matching = case_expression
            .branches()
            .iter()
            .filter(|branch| source_domains_match(&row, branch.source_domains()))
            .map(|branch| expression_integer(branch.result()))
            .chain(
                source_domains_match(&row, case_expression.else_source_domains()).then(|| {
                    case_expression
                        .else_result()
                        .map(expression_integer)
                        .expect("fixture has explicit ELSE")
                }),
            )
            .collect::<Vec<_>>();

        assert_eq!(
            matching,
            vec![actual_value],
            "CASE source domains disagreed with DuckDB for source row {row:?}"
        );
    }
}

#[test]
fn local_relation_locations_preserve_or_downgrade_exactness_explicitly() {
    let connection = duckdb_connection();

    for sql in [
        "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows WHERE a > 0) SELECT row_id FROM x WHERE b <= 1",
        "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows WHERE a > 0), y AS (SELECT row_id, a, b, c FROM x WHERE b <= 1) SELECT row_id FROM y WHERE c <> 0",
        "SELECT row_id FROM (SELECT row_id, a, b, c FROM predicate_rows WHERE a > 0) x WHERE b <= 1",
    ] {
        let protocol = analyze_duckdb(sql);
        let query = first_query(&protocol);
        assert!(
            query.condition_exactness().is_exact(),
            "safe local-relation fixture should stay exact: {sql}\n{:?}",
            query.condition_exactness().residual_conditions()
        );
        assert_eq!(
            predicted_row_ids(query.column_domains()),
            row_ids(&connection, sql),
            "local-relation domains disagree with DuckDB: {sql}"
        );
    }

    let set_sql =
        "SELECT row_id FROM predicate_rows WHERE a = 1 UNION ALL SELECT row_id FROM predicate_rows WHERE a = 2";
    let set_protocol = analyze_duckdb(set_sql);
    assert_eq!(
        first_query(&set_protocol).condition_exactness().status(),
        ConditionExactnessStatus::Residual
    );
}

#[test]
fn multi_layer_composed_domains_match_duckdb_for_identity_hops() {
    let connection = duckdb_connection();
    connection
        .execute_batch(
            "CREATE VIEW stage_rows AS
             SELECT row_id, a, b, c FROM predicate_rows WHERE a > 0",
        )
        .expect("create stage view");

    let dialect = dialect_from_name("duckdb").expect("DuckDB dialect");
    let inputs = [
        SqlInput::inline(
            "CREATE VIEW stage_rows AS
             SELECT row_id, a, b, c FROM predicate_rows WHERE a > 0",
        ),
        SqlInput::inline("SELECT row_id FROM stage_rows WHERE b <= 1"),
    ];
    let bundle = analyze_inputs(&inputs, "duckdb", dialect.as_ref()).expect("analyze pipeline");
    let final_layer = bundle.layers().last().expect("final layer");
    let ComposedSemantics::Resolved(semantics) = final_layer.composed_semantics() else {
        panic!("identity pipeline should compose");
    };
    assert!(semantics.condition_exactness().is_exact());

    assert_eq!(
        predicted_row_ids(semantics.column_domains()),
        row_ids(&connection, "SELECT row_id FROM stage_rows WHERE b <= 1"),
    );
}

#[derive(Clone, Copy)]
struct DeterministicRng(u64);

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }

    fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}

fn random_column(rng: &mut DeterministicRng) -> &'static str {
    ["a", "b", "c"][rng.index(3)]
}

fn random_value(rng: &mut DeterministicRng) -> i64 {
    [-2, -1, 0, 1, 2][rng.index(5)]
}

fn random_atom(rng: &mut DeterministicRng) -> String {
    let column = random_column(rng);
    match rng.index(12) {
        0 => format!("{column} = {}", random_value(rng)),
        1 => format!("{column} <> {}", random_value(rng)),
        2 => format!("{column} < {}", random_value(rng)),
        3 => format!("{column} <= {}", random_value(rng)),
        4 => format!("{column} > {}", random_value(rng)),
        5 => format!("{column} >= {}", random_value(rng)),
        6 => {
            let first = random_value(rng);
            let second = random_value(rng);
            let (lower, upper) = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            format!("{column} BETWEEN {lower} AND {upper}")
        }
        7 => {
            let first = random_value(rng);
            let second = random_value(rng);
            let (lower, upper) = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            format!("{column} NOT BETWEEN {lower} AND {upper}")
        }
        8 => format!("{column} IN ({}, {})", random_value(rng), random_value(rng)),
        9 => format!(
            "{column} NOT IN ({}, {})",
            random_value(rng),
            random_value(rng)
        ),
        10 => format!("{column} IS NULL"),
        _ => format!("{column} IS NOT NULL"),
    }
}

fn random_predicate(rng: &mut DeterministicRng, depth: usize) -> String {
    if depth == 0 || rng.index(4) == 0 {
        return random_atom(rng);
    }

    match rng.index(3) {
        0 => format!(
            "({} AND {})",
            random_predicate(rng, depth - 1),
            random_predicate(rng, depth - 1)
        ),
        1 => format!(
            "({} OR {})",
            random_predicate(rng, depth - 1),
            random_predicate(rng, depth - 1)
        ),
        _ => {
            let operand = random_predicate(rng, depth - 1);
            if rng.bool() {
                format!("NOT ({operand})")
            } else {
                format!("NOT NOT ({operand})")
            }
        }
    }
}

#[test]
fn seeded_predicate_trees_never_overclaim_exactness() {
    const CASES: u64 = 3_000;
    let connection = duckdb_connection();

    for seed in 1..=CASES {
        let mut rng = DeterministicRng::new(seed);
        let predicate = random_predicate(&mut rng, 3);
        let sql = format!("SELECT row_id FROM predicate_rows WHERE {predicate} ORDER BY row_id");
        let protocol = analyze_duckdb(&sql);
        let query = first_query(&protocol);

        if query.condition_exactness().is_exact() {
            assert_eq!(
                predicted_row_ids(query.column_domains()),
                row_ids(&connection, &sql),
                "seed={seed}; query={sql}"
            );
        } else {
            assert!(
                !query.condition_exactness().residual_conditions().is_empty(),
                "seed={seed}; residual query has no reason; query={sql}"
            );
        }
    }
}

#[test]
fn known_soundness_reproductions_stay_in_the_conformance_matrix() {
    let connection = duckdb_connection();

    // TASK-36: computed output domains must remain sound.
    let computed = "SELECT a + 1 FROM predicate_rows WHERE a = 1";
    let protocol = analyze_duckdb(computed);
    let domain = first_query(&protocol).output().columns()[0].domain();
    for value in query_optional_i64(&connection, computed) {
        assert!(output_domain_admits_integer(domain, value));
    }

    // TASK-38 / TASK-43: subquery membership is explicit residual semantics.
    let exists =
        "SELECT row_id FROM predicate_rows p WHERE EXISTS (SELECT 1 FROM predicate_rows q WHERE q.a = p.a)";
    let protocol = analyze_duckdb(exists);
    assert!(!first_query(&protocol).condition_exactness().is_exact());

    // TASK-45: implicit cross-relation equality is a canonical join correlation, not a
    // scalar domain, so the row-condition contract remains exact.
    let implicit =
        "SELECT left_rows.row_id FROM left_rows, right_rows WHERE left_rows.x = right_rows.y";
    let protocol = analyze_duckdb(implicit);
    let query = first_query(&protocol);
    assert!(query.condition_exactness().is_exact());
    assert!(query.column_domains().is_empty());

    // TASK-43: cross-column correlation stays residual.
    let correlated =
        "SELECT row_id FROM predicate_rows WHERE (a = 1 AND b = 2) OR (a = 2 AND b = 1)";
    let protocol = analyze_duckdb(correlated);
    assert!(!first_query(&protocol).condition_exactness().is_exact());
}

fn resolved_query(sql: &str) -> ResolvedComposedSemantics {
    let dialect = dialect_from_name("duckdb").expect("DuckDB dialect");
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref())
        .unwrap_or_else(|error| panic!("conformance fixture failed: {sql}\n{error}"));
    match bundle
        .layers()
        .last()
        .expect("conformance query layer")
        .composed_semantics()
    {
        ComposedSemantics::Resolved(semantics) => semantics.clone(),
        other => panic!("conformance fixture could not compose {sql}: {other:?}"),
    }
}

fn residual_reasons(semantics: &ResolvedComposedSemantics) -> BTreeSet<String> {
    semantics
        .condition_exactness()
        .residual_conditions()
        .iter()
        // A local-only diagnostic supplements the original predicate residual;
        // it must not count as a new semantic reason in plain-copy comparisons.
        .filter(|residual| residual.identity() != "diagnostic:unresolved_local_predicate")
        .map(|residual| format!("{:?}", residual.reason()))
        .collect()
}

fn assert_complete_plain_copy(
    connection: &Connection,
    inlined: &str,
    wrapped: &str,
    location: &str,
) {
    let expected = resolved_query(inlined);
    let actual = resolved_query(wrapped);
    assert!(
        expected.condition_exactness().is_exact(),
        "baseline must be allow-listed: {inlined}; residuals={:?}",
        expected.condition_exactness().residual_conditions()
    );
    assert_eq!(
        actual.condition_exactness().status(),
        expected.condition_exactness().status(),
        "completeness failure: location={location}; query={wrapped}; expected=exact; actual={:?}",
        actual.condition_exactness().residual_conditions()
    );
    assert_eq!(
        actual.column_domains(),
        expected.column_domains(),
        "completeness failure: location={location}; query={wrapped}; expected domains={:?}; actual domains={:?}",
        expected.column_domains(),
        actual.column_domains()
    );
    assert_eq!(
        actual.join_equalities().len(),
        expected.join_equalities().len(),
        "completeness failure: location={location}; query={wrapped}; expected join equalities={:?}; actual join equalities={:?}",
        expected.join_equalities(),
        actual.join_equalities()
    );
    assert_eq!(
        predicted_row_ids(actual.column_domains()),
        row_ids(connection, wrapped),
        "completeness oracle mismatch: location={location}; query={wrapped}"
    );
}

#[test]
fn every_scalar_allow_list_shape_is_complete_at_all_plain_copy_locations() {
    let connection = duckdb_connection();
    for predicate in [
        "a = 1",
        "a <> 1",
        "a < 1",
        "a <= 1",
        "a > 0",
        "a >= 0",
        "a BETWEEN 0 AND 1",
        "a NOT BETWEEN 0 AND 1",
        "a IN (0, 1, 2)",
        "a NOT IN (0, 1, 2)",
        "a IS NULL",
        "a IS NOT NULL",
        "a IS DISTINCT FROM 1",
        "a IS NOT DISTINCT FROM 1",
        "a >= 0 AND a <= 1",
        "a = 0 OR a = 2",
    ] {
        let inlined = format!("SELECT row_id FROM predicate_rows WHERE {predicate}");
        let wrapped = [
            (
                "cte",
                format!(
                    "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}) SELECT row_id FROM x"
                ),
            ),
            (
                "chained_cte",
                format!(
                    "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}), y AS (SELECT row_id, a, b, c FROM x) SELECT row_id FROM y"
                ),
            ),
            (
                "derived",
                format!(
                    "SELECT row_id FROM (SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}) d"
                ),
            ),
            (
                "outer_filter",
                format!(
                    "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows) SELECT row_id FROM x WHERE {predicate}"
                ),
            ),
        ];
        assert_exact_matches_oracle(&connection, predicate);
        for (location, sql) in wrapped {
            assert_complete_plain_copy(&connection, &inlined, &sql, location);
        }

        let dialect = dialect_from_name("duckdb").expect("DuckDB dialect");
        let inputs = [
            SqlInput::inline(format!(
                "CREATE VIEW stage_rows AS SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}"
            )),
            SqlInput::inline("SELECT row_id FROM stage_rows"),
        ];
        let bundle = analyze_inputs(&inputs, "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("multi-layer fixture {predicate}: {error}"));
        let ComposedSemantics::Resolved(actual) = bundle
            .layers()
            .last()
            .expect("final composed layer")
            .composed_semantics()
        else {
            panic!("multi-layer identity path should compose: {predicate}");
        };
        let expected = resolved_query(&inlined);
        assert!(
            actual.condition_exactness().is_exact(),
            "multi-layer completeness: predicate={predicate}; residuals={:?}",
            actual.condition_exactness().residual_conditions()
        );
        assert_eq!(actual.column_domains(), expected.column_domains());
        assert!(actual.join_equalities().is_empty());
        assert_eq!(
            predicted_row_ids(actual.column_domains()),
            row_ids(&connection, &inlined),
            "multi-layer domain oracle: predicate={predicate}"
        );
    }
}

#[test]
fn seeded_plain_copy_equivalence_checks_exact_and_residual_predicates() {
    // Separate seeds from the soundness suite: this checks completeness and
    // representation stability, not merely whether asserted domains are sound.
    for seed in 1..=1_000 {
        let mut rng = DeterministicRng::new(seed);
        let predicate = random_predicate(&mut rng, 3);
        let inlined = format!("SELECT row_id FROM predicate_rows WHERE {predicate}");
        let baseline = resolved_query(&inlined);
        let variants = [
            (
                "cte",
                format!(
                    "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}) SELECT row_id FROM x"
                ),
            ),
            (
                "chained_cte",
                format!(
                    "WITH x AS (SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}), y AS (SELECT row_id, a, b, c FROM x) SELECT row_id FROM y"
                ),
            ),
            (
                "derived",
                format!(
                    "SELECT row_id FROM (SELECT row_id, a, b, c FROM predicate_rows WHERE {predicate}) d"
                ),
            ),
        ];
        for (location, sql) in variants {
            let actual = resolved_query(&sql);
            assert_eq!(
                actual.condition_exactness().status(),
                baseline.condition_exactness().status(),
                "seed={seed}; location={location}; query={sql}; expected={:?}; actual residuals={:?}",
                baseline.condition_exactness().status(),
                actual.condition_exactness().residual_conditions()
            );
            assert_eq!(
                residual_reasons(&actual),
                residual_reasons(&baseline),
                "seed={seed}; location={location}; query={sql}; expected residual reasons={:?}; actual={:?}",
                residual_reasons(&baseline),
                actual.condition_exactness().residual_conditions()
            );
            if baseline.condition_exactness().is_exact() {
                assert_eq!(
                    actual.column_domains(),
                    baseline.column_domains(),
                    "seed={seed}; location={location}; query={sql}; expected domains={:?}; actual={:?}",
                    baseline.column_domains(),
                    actual.column_domains()
                );
            }
        }
    }
}

fn canonical_composed_equalities(
    semantics: &ResolvedComposedSemantics,
) -> BTreeSet<(String, String, String, String)> {
    semantics
        .join_equalities()
        .iter()
        .map(|join| {
            let left = (
                join.left().relation().to_string(),
                join.left().column().to_string(),
            );
            let right = (
                join.right().relation().to_string(),
                join.right().column().to_string(),
            );
            let (left, right) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            (left.0, left.1, right.0, right.1)
        })
        .collect()
}

fn row_triples(connection: &Connection, sql: &str) -> BTreeSet<(i64, i64, i64)> {
    let mut statement = connection
        .prepare(sql)
        .expect("prepare three-source oracle");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap_or_else(|error| panic!("execute three-source oracle {sql}: {error}"))
        .map(|row| row.expect("read oracle triple"))
        .collect()
}

#[test]
fn explicit_and_implicit_two_source_join_claims_are_complete() {
    let connection = duckdb_connection();
    let explicit = "SELECT l.row_id, r.row_id FROM left_rows l JOIN right_rows r ON l.x = r.y AND l.a > 0 AND r.b <= 2";
    let implicit = "SELECT l.row_id, r.row_id FROM left_rows l, right_rows r WHERE l.x = r.y AND l.a > 0 AND r.b <= 2";
    let expected = resolved_query(explicit);
    assert!(
        expected.condition_exactness().is_exact(),
        "explicit join baseline: {:?}",
        expected.condition_exactness().residual_conditions()
    );
    assert_eq!(expected.join_equalities().len(), 1);
    assert_eq!(
        canonical_composed_equalities(&expected),
        BTreeSet::from([(
            "left_rows".to_string(),
            "x".to_string(),
            "right_rows".to_string(),
            "y".to_string(),
        )])
    );
    let expected_rows = row_pairs(&connection, explicit);
    assert_eq!(expected_rows, row_pairs(&connection, implicit));
    for (location, sql) in [
        ("implicit_where", implicit.to_string()),
        ("cte", "WITH j AS (SELECT l.row_id AS l_id, r.row_id AS r_id FROM left_rows l JOIN right_rows r ON l.x = r.y AND l.a > 0 AND r.b <= 2) SELECT l_id, r_id FROM j".to_string()),
        ("chained_cte", "WITH j AS (SELECT l.row_id AS l_id, r.row_id AS r_id FROM left_rows l JOIN right_rows r ON l.x = r.y AND l.a > 0 AND r.b <= 2), k AS (SELECT l_id, r_id FROM j) SELECT l_id, r_id FROM k".to_string()),
        ("derived", "SELECT l_id, r_id FROM (SELECT l.row_id AS l_id, r.row_id AS r_id FROM left_rows l JOIN right_rows r ON l.x = r.y AND l.a > 0 AND r.b <= 2) j".to_string()),
    ] {
        let actual = resolved_query(&sql);
        assert!(
            actual.condition_exactness().is_exact(),
            "join completeness failure: location={location}; query={sql}; residuals={:?}",
            actual.condition_exactness().residual_conditions()
        );
        assert_eq!(
            canonical_composed_equalities(&actual),
            canonical_composed_equalities(&expected),
            "join equality completeness: location={location}; query={sql}"
        );
        assert_eq!(
            actual.column_domains(),
            expected.column_domains(),
            "join domain completeness: location={location}; query={sql}"
        );
        assert_eq!(
            expected_rows,
            row_pairs(&connection, &sql),
            "join oracle: location={location}; query={sql}"
        );
    }
}

#[test]
fn three_source_join_shapes_are_complete_against_duckdb() {
    let connection = duckdb_connection();
    connection
        .execute_batch(
            "CREATE TABLE third_rows (row_id BIGINT NOT NULL, c BIGINT, z BIGINT);
         INSERT INTO third_rows VALUES (21, 1, 1), (22, 2, 2), (23, 3, 3), (24, NULL, 2);",
        )
        .expect("populate third source");

    let explicit = "SELECT l.row_id, r.row_id, t.row_id FROM left_rows l JOIN right_rows r ON l.x = r.y JOIN third_rows t ON r.b = t.c WHERE l.a > 0";
    let implicit = "SELECT l.row_id, r.row_id, t.row_id FROM left_rows l, right_rows r, third_rows t WHERE l.x = r.y AND r.b = t.c AND l.a > 0";
    let expected = resolved_query(explicit);
    assert!(
        expected.condition_exactness().is_exact(),
        "three-source baseline residuals={:?}",
        expected.condition_exactness().residual_conditions()
    );
    assert_eq!(
        expected.join_equalities().len(),
        2,
        "both physical equalities must be emitted"
    );
    let expected_rows = row_triples(&connection, explicit);
    assert_eq!(expected_rows, row_triples(&connection, implicit));

    for (location, sql) in [
        ("implicit_where", implicit.to_string()),
        ("cte", format!("WITH j AS ({explicit}) SELECT * FROM j")),
        (
            "chained_cte",
            format!("WITH j AS ({explicit}), k AS (SELECT * FROM j) SELECT * FROM k"),
        ),
        ("derived", format!("SELECT * FROM ({explicit}) j")),
    ] {
        let actual = resolved_query(&sql);
        assert!(
            actual.condition_exactness().is_exact(),
            "three-way join completeness: location={location}; query={sql}; residuals={:?}",
            actual.condition_exactness().residual_conditions()
        );
        assert_eq!(
            canonical_composed_equalities(&actual),
            canonical_composed_equalities(&expected),
            "three-way equality completeness: location={location}; query={sql}"
        );
        assert_eq!(
            actual.column_domains(),
            expected.column_domains(),
            "three-way domain completeness: location={location}; query={sql}"
        );
        assert_eq!(
            expected_rows,
            row_triples(&connection, &sql),
            "three-way oracle: location={location}; query={sql}"
        );
    }
}

fn typed_conformance(sql: &str, sql_type: &str) -> ResolvedComposedSemantics {
    let schema = RelationSchema::new(
        "typed_rows",
        vec![
            SchemaColumn::from_sql_type("row_id", "BIGINT", "duckdb").expect("row ID type"),
            SchemaColumn::from_sql_type("value", sql_type, "duckdb")
                .expect("typed conformance column"),
        ],
    )
    .expect("typed conformance relation");
    let catalog = RelationCatalog::from_schemas(&[schema]).expect("typed catalog");
    let dialect = dialect_from_name("duckdb").expect("DuckDB dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "typed-conformance",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("typed conformance query should analyze");
    match bundle.layers()[0].composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics.clone(),
        other => panic!("typed conformance could not compose: {other:?}"),
    }
}

#[test]
fn typed_and_untyped_scalar_exactness_agree_when_literal_semantics_are_portable() {
    for (data_type, predicate) in [
        ("INTEGER", "value >= 1"),
        ("BIGINT", "value BETWEEN 0 AND 2"),
        ("DECIMAL(10,2)", "value > 1.5"),
        ("DATE", "value >= DATE '2024-01-01'"),
        ("TIME", "value < TIME '12:00:00'"),
        ("BOOLEAN", "value = TRUE"),
    ] {
        let sql = format!("SELECT row_id FROM typed_rows WHERE {predicate}");
        let typed = typed_conformance(&sql, data_type);
        let untyped = resolved_query(&sql);
        assert!(
            typed.condition_exactness().is_exact(),
            "typed completeness: type={data_type}; query={sql}; residuals={:?}",
            typed.condition_exactness().residual_conditions()
        );
        assert_eq!(
            typed.condition_exactness().status(),
            untyped.condition_exactness().status(),
            "typed/untyped completeness: type={data_type}; query={sql}"
        );
        assert!(
            typed
                .column_domains()
                .iter()
                .all(|domain| !matches!(domain.domain(), ValueDomain::Unknown(_))),
            "portable typed predicates must not silently lose domains: {sql}"
        );
    }
}

#[test]
fn typed_comparison_exceptions_remain_explicit_until_assumptions_are_modeled() {
    // TASK-53 will add conditional comparison semantics for strings, floats, and timestamps.
    // INTERVAL literals remain a distinct parser-normalization boundary.
    // Until then, an unconditional exactness claim would be unsound.
    for (data_type, predicate) in [
        ("VARCHAR", "value = 'keep'"),
        ("DOUBLE", "value > 1.5"),
        ("TIMESTAMP", "value >= TIMESTAMP '2024-01-01 00:00:00'"),
        ("INTERVAL", "value >= INTERVAL '1 day'"),
    ] {
        let sql = format!("SELECT row_id FROM typed_rows WHERE {predicate}");
        let typed = typed_conformance(&sql, data_type);
        assert!(
            !typed.condition_exactness().is_exact(),
            "typed comparison must remain conditional: type={data_type}; query={sql}"
        );
        assert!(
            !typed.condition_exactness().residual_conditions().is_empty(),
            "typed comparison residual must explain unsupported semantics: {sql}"
        );
    }
}

#[test]
fn seeded_two_and_three_source_joins_preserve_exact_equalities() {
    const CASES: u64 = 120;
    let connection = duckdb_connection();
    connection
        .execute_batch(
            "CREATE TABLE third_rows (row_id BIGINT NOT NULL, c BIGINT, z BIGINT);
             INSERT INTO third_rows VALUES (21, 1, 1), (22, 2, 2), (23, 3, 3), (24, NULL, 2);",
        )
        .expect("populate three-way oracle");

    for seed in 1..=CASES {
        let mut rng = DeterministicRng::new(seed);
        let two_relation_equality = if rng.bool() { "l.x = r.y" } else { "l.a = r.b" };
        let third_relation_equality = if rng.bool() { "r.b = t.c" } else { "l.x = t.z" };
        let lower = rng.index(3);
        let upper = rng.index(3) + 1;
        let triple = rng.bool();
        let (explicit, implicit, expected_count) = if triple {
            (
                format!("SELECT l.row_id, r.row_id, t.row_id FROM left_rows l JOIN right_rows r ON {two_relation_equality} JOIN third_rows t ON {third_relation_equality} WHERE l.a >= {lower} AND r.b <= {upper}"),
                format!("SELECT l.row_id, r.row_id, t.row_id FROM left_rows l, right_rows r, third_rows t WHERE {two_relation_equality} AND {third_relation_equality} AND l.a >= {lower} AND r.b <= {upper}"),
                2,
            )
        } else {
            (
                format!("SELECT l.row_id, r.row_id FROM left_rows l JOIN right_rows r ON {two_relation_equality} WHERE l.a >= {lower} AND r.b <= {upper}"),
                format!("SELECT l.row_id, r.row_id FROM left_rows l, right_rows r WHERE {two_relation_equality} AND l.a >= {lower} AND r.b <= {upper}"),
                1,
            )
        };
        let baseline = resolved_query(&explicit);
        assert!(
            baseline.condition_exactness().is_exact(),
            "seed={seed}; explicit query={explicit}; residuals={:?}",
            baseline.condition_exactness().residual_conditions()
        );
        assert_eq!(
            canonical_composed_equalities(&baseline).len(),
            expected_count,
            "seed={seed}; explicit query={explicit}"
        );
        let wrapped = [
            ("implicit", implicit),
            ("cte", format!("WITH j AS ({explicit}) SELECT * FROM j")),
            (
                "chained_cte",
                format!("WITH j AS ({explicit}), k AS (SELECT * FROM j) SELECT * FROM k"),
            ),
            ("derived", format!("SELECT * FROM ({explicit}) j")),
        ];
        for (location, sql) in wrapped {
            let actual = resolved_query(&sql);
            assert!(
                actual.condition_exactness().is_exact(),
                "seed={seed}; location={location}; query={sql}; residuals={:?}",
                actual.condition_exactness().residual_conditions()
            );
            assert_eq!(
                canonical_composed_equalities(&actual),
                canonical_composed_equalities(&baseline),
                "seed={seed}; location={location}; query={sql}; equality mismatch"
            );
            assert_eq!(
                actual.column_domains(),
                baseline.column_domains(),
                "seed={seed}; location={location}; query={sql}; domain mismatch"
            );
            if triple {
                assert_eq!(
                    row_triples(&connection, &sql),
                    row_triples(&connection, &explicit),
                    "seed={seed}; location={location}; three-source oracle mismatch"
                );
            } else {
                assert_eq!(
                    row_pairs(&connection, &sql),
                    row_pairs(&connection, &explicit),
                    "seed={seed}; location={location}; two-source oracle mismatch"
                );
            }
        }
    }
}

#[test]
fn daily_revenue_cte_chain_keeps_join_and_grouping_conditions_exact() {
    let connection = duckdb_connection();
    connection
        .execute_batch(
            "CREATE TABLE third_rows (row_id BIGINT NOT NULL, c BIGINT, z BIGINT);
             INSERT INTO third_rows VALUES (21, 1, 1), (22, 2, 2), (23, 3, 3), (24, NULL, 2);",
        )
        .expect("populate revenue source");
    let inlined = "
        SELECT l.row_id AS order_id,
               CASE WHEN SUM(r.b * t.c) > 0 THEN CAST(SUM(r.b * t.c) AS BIGINT) ELSE 0 END AS revenue
        FROM left_rows l
        JOIN right_rows r ON l.x = r.y
        JOIN third_rows t ON r.y = t.z
        WHERE l.a > 0 AND r.b <= 2 AND t.c > 0
        GROUP BY l.row_id
    ";
    let with_ctes = "
        WITH orders AS (
            SELECT row_id AS order_id, x AS product_id, a FROM left_rows WHERE a > 0
        ),
        items AS (
            SELECT row_id AS item_id, y AS product_id, b FROM right_rows WHERE b <= 2
        ),
        products AS (
            SELECT row_id AS product_row_id, z AS product_id, c FROM third_rows WHERE c > 0
        ),
        line_items AS (
            SELECT o.order_id, i.b, p.c
            FROM orders o
            JOIN items i ON o.product_id = i.product_id
            JOIN products p ON i.product_id = p.product_id
        )
        SELECT order_id,
               CASE WHEN SUM(b * c) > 0 THEN CAST(SUM(b * c) AS BIGINT) ELSE 0 END AS revenue
        FROM line_items
        GROUP BY order_id
    ";
    let expected = resolved_query(inlined);
    let actual = resolved_query(with_ctes);
    assert!(
        expected.condition_exactness().is_exact(),
        "inlined revenue query must be exact: {:?}",
        expected.condition_exactness().residual_conditions()
    );
    assert!(
        actual.condition_exactness().is_exact(),
        "daily-revenue CTE chain must be exact; query={with_ctes}; residuals={:?}",
        actual.condition_exactness().residual_conditions()
    );
    assert_eq!(
        canonical_composed_equalities(&actual),
        canonical_composed_equalities(&expected),
        "CTE revenue chain must preserve both physical equalities"
    );
    assert_eq!(
        actual.column_domains(),
        expected.column_domains(),
        "CTE revenue chain must preserve source filter domains"
    );
    assert_eq!(
        query_optional_i64(&connection, &format!("SELECT revenue FROM ({inlined}) q ORDER BY order_id")),
        query_optional_i64(&connection, &format!("SELECT revenue FROM ({with_ctes}) q ORDER BY order_id")),
        "CTE revenue chain must compute the same aggregates as DuckDB"
    );
}
