//! TASK-192 original S013: prove correlated EXISTS on physical source-row
//! occurrences, including duplicates, NULL and unmatched rows. This is a
//! source-local membership witness, not scalar-domain or vendor signoff.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name,
    physical_finite_exists_bag_plan, ComposedSemantics, ConditionExactnessStatus,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteExistsProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    SubqueryMembershipCase, SubqueryMembershipDirection, SubqueryMembershipKind, ValueDomain,
};

const MATRIX: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn native_ids(db: &Connection, query: &str) -> Vec<i64> {
    let mut statement = db.prepare(query).expect("native original SQL");
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("native complete identity rows");
    let mut ids = rows
        .map(|row| row.expect("native source occurrence"))
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn original_exists() -> (Value, String) {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen original inventory");
    let sql = matrix["cases"]
        .as_array()
        .expect("43 original queries")
        .iter()
        .find(|case| case["id"] == "88-C177-S013")
        .expect("S013")["sql"]
        .as_str()
        .expect("original SQL")
        .to_string();
    (matrix, sql)
}

#[test]
fn original_correlated_exists_has_complete_duplicate_preserving_true_false_bags() {
    let (_, sql) = original_exists();
    let db = Connection::open_in_memory().expect("DuckDB");
    // Two outer occurrences of id=1 and two inner matches of id=1. SQL
    // EXISTS is a SEMIJOIN: inner multiplicity does not multiply outer rows.
    db.execute_batch(
        "CREATE TABLE orders (id INTEGER, amount INTEGER);
         CREATE TABLE customers (id INTEGER, name VARCHAR);
         INSERT INTO orders VALUES
           (-1,10), (0,10), (1,10), (1,NULL), (2,19), (3,20), (NULL,30);
         INSERT INTO customers VALUES
           (1,'one'), (1,'duplicate'), (3,'three'), (NULL,'null');",
    )
    .expect("independently authored complete physical source bags");
    assert_eq!(
        native_ids(&db, "SELECT rowid FROM orders"),
        vec![0, 1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        native_ids(&db, "SELECT rowid FROM customers"),
        vec![0, 1, 2, 3]
    );
    assert_eq!(native_ids(&db, &sql), vec![1, 1, 3]);

    let condition = "EXISTS (SELECT 1 FROM customers c WHERE c.id = o.id)";
    let not_condition = "NOT EXISTS (SELECT 1 FROM customers c WHERE c.id = o.id)";
    for (test, selected) in [
        (format!("({condition}) IS TRUE"), vec![2, 3, 5]),
        (format!("({condition}) IS FALSE"), vec![0, 1, 4, 6]),
        (format!("({condition}) IS UNKNOWN"), vec![]),
        (format!("({condition}) IS NOT TRUE"), vec![0, 1, 4, 6]),
        (not_condition.to_string(), vec![0, 1, 4, 6]),
    ] {
        assert_eq!(
            native_ids(&db, &format!("SELECT o.rowid FROM orders o WHERE {test}")),
            selected,
            "complete source occurrence truth: {test}"
        );
    }
    assert_eq!(
        native_ids(&db, "SELECT o.id FROM orders o WHERE NOT EXISTS (SELECT 1 FROM customers c WHERE c.id = o.id) AND o.id IS NOT NULL"),
        vec![-1, 0, 2],
        "NOT EXISTS preserves outer duplicate and nullable row identities"
    );
}

#[test]
fn s013_has_separate_physical_source_correlations_and_exact_both_direction_witness() {
    let (matrix, sql) = original_exists();
    let ddl = SqlInput::inline(
        "CREATE TABLE orders (id INTEGER, amount INTEGER);
         CREATE TABLE customers (id INTEGER, name VARCHAR)",
    );
    let parser = dialect_from_name("duckdb").expect("DDL parser");
    let source = analyze_inputs(&[ddl], "duckdb", parser.as_ref())
        .expect("authoritative physical source schemas");
    let catalog = RelationCatalog::from_schemas(source.source_schemas()).expect("source catalog");

    for name in matrix["dialects"].as_array().expect("13 parser dialects") {
        let name = name.as_str().expect("dialect");
        let dialect = dialect_from_name(name).expect("public parser");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "task192-original-s013",
            &input,
            name,
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .unwrap_or_else(|error| panic!("S013/{name}: {error}"));
        let layer = bundle.layers().last().expect("original query layer");
        let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
            panic!("S013/{name}: missing composed source");
        };
        assert_eq!(
            composed.condition_exactness().status(),
            ConditionExactnessStatus::Residual,
            "S013/{name}: subquery witness is source-local, not whole-query scalar Exact"
        );
        assert!(
            matches!(
                composed.output().columns()[0].domain(),
                ValueDomain::Unbounded | ValueDomain::Unknown(_)
            ),
            "S013/{name}: correlated EXISTS cannot infer a finite projected id set"
        );

        let [entry] = composed.subquery_witnesses() else {
            panic!("S013/{name}: require one source-local EXISTS witness");
        };
        let witness = entry.witness();
        assert_eq!(witness.kind(), SubqueryMembershipKind::Exists);
        assert_eq!(witness.outer_relation(), "o");
        assert_eq!(witness.outer_source_relation(), Some("orders"));
        assert_eq!(witness.inner_relation(), Some("customers"));
        let [equality] = witness.correlations() else {
            panic!("S013/{name}: require exactly one source correlation");
        };
        assert_eq!(equality.outer().relation(), Some("orders"));
        assert_eq!(equality.outer().name(), "id");
        assert_eq!(equality.inner().relation(), Some("customers"));
        assert_eq!(equality.inner().name(), "id");
        assert_eq!(
            witness.qualifying(),
            &SubqueryMembershipDirection::Exact(vec![SubqueryMembershipCase::MatchingRow]),
            "S013/{name}: complete matching candidate witness"
        );
        assert_eq!(
            witness.rejected(),
            &SubqueryMembershipDirection::Exact(vec![SubqueryMembershipCase::NoCandidates]),
            "S013/{name}: NULL key or missing match is NOT TRUE"
        );
    }
}

fn signed(value: Option<i64>) -> LiteralValue {
    value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()))
}

fn source_row(physical_id: &str, columns: &[(&str, LiteralValue)]) -> PhysicalWriteRow {
    let mut values = BTreeMap::from([(
        "__physical_row_id".to_string(),
        LiteralValue::Text(physical_id.to_string()),
    )]);
    for (name, value) in columns {
        values.insert((*name).to_string(), value.clone());
    }
    PhysicalWriteRow::new(values).expect("typed complete physical source occurrence")
}

fn physical_sources() -> (RelationCatalog, Vec<PhysicalWriteState>) {
    let column = |name, data_type| {
        SchemaColumn::from_sql_type(name, data_type, "duckdb").expect("physical source data type")
    };
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "orders",
            vec![
                column("id", "INTEGER"),
                column("amount", "INTEGER"),
                column("__physical_row_id", "VARCHAR"),
            ],
        )
        .expect("outer source schema"),
        RelationSchema::new(
            "customers",
            vec![
                column("id", "INTEGER"),
                column("name", "VARCHAR"),
                column("__physical_row_id", "VARCHAR"),
            ],
        )
        .expect("inner source schema"),
    ])
    .expect("physical schema catalog");

    let outer = [
        (Some(-1), Some(10)),
        (Some(0), Some(10)),
        (Some(1), Some(10)),
        (Some(1), None),
        (Some(2), Some(19)),
        (Some(3), Some(20)),
        (None, Some(30)),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (id, amount))| {
        source_row(
            &format!("o{index}"),
            &[("id", signed(id)), ("amount", signed(amount))],
        )
    })
    .collect();
    let inner = [
        (Some(1), "one"),
        (Some(1), "duplicate"),
        (Some(3), "three"),
        (None, "null"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (id, name))| {
        source_row(
            &format!("c{index}"),
            &[
                ("id", signed(id)),
                ("name", LiteralValue::Text(name.to_string())),
            ],
        )
    })
    .collect();
    (
        catalog,
        vec![
            PhysicalWriteState::new("orders", outer).expect("outer bag"),
            PhysicalWriteState::new("customers", inner).expect("inner bag"),
        ],
    )
}

#[test]
fn original_s013_has_closed_world_constructive_exists_proofs_on_all_13_dialects() {
    let (matrix, sql) = original_exists();
    let (catalog, sources) = physical_sources();

    for dialect_name in matrix["dialects"].as_array().expect("13 parsers") {
        let dialect_name = dialect_name.as_str().expect("dialect name");
        let parser = dialect_from_name(dialect_name).expect("public parser");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "task192-s013-complete-physical-bags",
            &input,
            dialect_name,
            parser.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .expect("original source-complete query");
        let layer = bundle.layers().last().expect("original query layer");
        let proof = physical_finite_exists_bag_plan(&bundle, layer.id(), &sources);
        let PhysicalFiniteExistsProof::Exact(exact) = proof else {
            panic!("S013/{dialect_name}: complete physical source proof: {proof:?}");
        };
        assert_eq!(
            exact.sources,
            ["orders".to_string(), "customers".to_string()]
        );
        assert_eq!(exact.kind, SubqueryMembershipKind::Exists);
        assert_eq!(exact.selected, ["o2", "o3", "o5"]);
        assert_eq!(exact.rejected, ["o0", "o1", "o4", "o6"]);
        assert_eq!(
            exact.matching_inner,
            BTreeMap::from([
                ("o0".to_string(), vec![]),
                ("o1".to_string(), vec![]),
                ("o2".to_string(), vec!["c0".to_string(), "c1".to_string()]),
                ("o3".to_string(), vec!["c0".to_string(), "c1".to_string()]),
                ("o4".to_string(), vec![]),
                ("o5".to_string(), vec!["c2".to_string()]),
                ("o6".to_string(), vec![]),
            ]),
            "S013/{dialect_name}: prove every matching and absent inner occurrence"
        );
        assert_eq!(
            exact
                .terminal
                .iter()
                .map(|row| row.get("id"))
                .collect::<Vec<_>>(),
            vec![
                Some(&LiteralValue::Number("1".to_string())),
                Some(&LiteralValue::Number("1".to_string())),
                Some(&LiteralValue::Number("3".to_string())),
            ],
            "S013/{dialect_name}: exact duplicate-preserving terminal"
        );
        let domain = exact
            .terminal_domains
            .get("id")
            .expect("typed final id domain");
        assert_eq!(domain.values(), &[1, 3]);
        assert!(!domain.null_allowed());
        assert_eq!(domain.bounds(), Some((1, 3)));
    }
}

#[test]
fn numeric_sql_id_requires_separate_unique_physical_occurrence_identity() {
    let (_, sql) = original_exists();
    let (catalog, mut sources) = physical_sources();
    let parser = dialect_from_name("duckdb").expect("parser");
    let input = SqlInput::inline(&sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-s013-invalid-physical-ids",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let bundle =
        analyze_configured_inputs_with_catalog(&configured, &catalog).expect("original SQL");

    let mut duplicate = sources[0].rows().to_vec();
    duplicate.push(duplicate[2].clone());
    sources[0] = PhysicalWriteState::new("orders", duplicate).expect("syntactically valid bag");
    let layer = bundle.layers().last().expect("query layer");
    assert!(
        matches!(
            physical_finite_exists_bag_plan(&bundle, layer.id(), &sources),
            PhysicalFiniteExistsProof::Impossible { .. }
        ),
        "duplicating an occurrence identity cannot be accepted as an exact semijoin proof"
    );
}

#[test]
fn s013_requires_the_full_inner_bag_and_handles_proven_empty_inner_relation() {
    let (_, sql) = original_exists();
    let (catalog, mut sources) = physical_sources();
    let parser = dialect_from_name("duckdb").expect("parser");
    let input = SqlInput::inline(&sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-s013-world-closure",
        &input,
        "duckdb",
        parser.as_ref(),
    )];
    let bundle =
        analyze_configured_inputs_with_catalog(&configured, &catalog).expect("original SQL");
    let layer = bundle.layers().last().expect("query layer");

    // An observed outer bag is not evidence that *no unknown inner rows*
    // match. Missing inner source must never be treated as an empty source.
    assert!(
        matches!(
            physical_finite_exists_bag_plan(&bundle, layer.id(), &sources[..1]),
            PhysicalFiniteExistsProof::Residual { .. }
        ),
        "missing inner population cannot prove NOT TRUE or zero terminal rows"
    );

    // An *explicitly complete empty* inner population does prove the
    // negative for every outer occurrence, even when its id is SQL NULL.
    sources[1] = PhysicalWriteState::new("customers", Vec::new())
        .expect("complete explicitly empty inner relation");
    let proof = physical_finite_exists_bag_plan(&bundle, layer.id(), &sources);
    let PhysicalFiniteExistsProof::Exact(exact) = proof else {
        panic!("closed empty inner source has an exact negative semijoin proof: {proof:?}");
    };
    assert!(exact.selected.is_empty());
    assert_eq!(exact.rejected, ["o0", "o1", "o2", "o3", "o4", "o5", "o6"]);
    assert!(exact.terminal.is_empty());
    assert!(exact.matching_inner.values().all(Vec::is_empty));
    let id_domain = exact.terminal_domains.get("id").expect("typed empty terminal");
    assert!(id_domain.values().is_empty());
    assert!(!id_domain.null_allowed());
    assert_eq!(id_domain.bounds(), None);
}
