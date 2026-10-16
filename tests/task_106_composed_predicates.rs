//! TASK-106: compose typed expression obligations through relational graphs.
//!
//! Covers the missing integration between the TASK-101..105 canonical
//! expression proofs and the TASK-68 physical-source DAG engine: typed
//! filters conjoined across transparent layers, filtered joins, grouped
//! aggregation with HAVING, computed-alias boundaries, and joint
//! positive/NOT TRUE physical witnesses with DuckDB oracles.
mod common;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name,
    physical_joint_row_count_plan, physical_row_count_plan, physical_source_plan, AnalysisBundle,
    BooleanTruthCase, BooleanWitnessDirection, ComposedSemantics, ConfiguredInputAnalysisError,
    ConfiguredSqlInput, Error, GroupBoundaryKind, JoinKind, LiteralValue, ProtocolStatement,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain, WitnessDirection,
};

fn catalog() -> RelationCatalog {
    let schemas = ["t", "l", "r", "stage", "mart"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["a", "b", "k"]
                    .into_iter()
                    .map(|name| {
                        SchemaColumn::from_sql_type(name, "INTEGER", "postgresql")
                            .expect("integer column")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    RelationCatalog::from_schemas(&schemas).expect("catalog")
}

fn analyze_chain(queries: &[&str], dialect: &str) -> AnalysisBundle {
    let dialect_impl = dialect_from_name(dialect).expect("recognized dialect");
    let sources = queries
        .iter()
        .map(|query| SqlInput::inline(*query))
        .collect::<Vec<_>>();
    let ids = (0..queries.len())
        .map(|index| format!("q-{index}"))
        .collect::<Vec<_>>();
    let inputs = sources
        .iter()
        .zip(&ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, dialect, dialect_impl.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&inputs, &catalog()).expect("analysis")
}

fn mart_layer(bundle: &AnalysisBundle) -> usize {
    bundle.layers().len() - 1
}

fn assert_exact_both(witness: &sql_semantic_protocol::BooleanWitness, sql: &str) {
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "{sql}: qualifying must be exact TRUE, got {:?}",
        witness.qualifying()
    );
    assert!(
        matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{sql}: rejected must be exact NOT TRUE, got {:?}",
        witness.rejected()
    );
}

fn assert_range_bounds(domain: &ValueDomain, sql: &str, lower: &str, upper: &str) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("{sql}: expected ranges domain, got {domain:?}");
    };
    assert_eq!(ranges.ranges().len(), 1, "{sql}: one range, got {domain:?}");
    let range = &ranges.ranges()[0];
    assert_eq!(
        range.lower().expect("lower").value().value(),
        &LiteralValue::Number(lower.to_string()),
        "{sql}: lower bound"
    );
    assert!(
        range.lower().expect("lower").inclusive(),
        "{sql}: closed lower"
    );
    assert_eq!(
        range.upper().expect("upper").value().value(),
        &LiteralValue::Number(upper.to_string()),
        "{sql}: upper bound"
    );
    assert!(
        range.upper().expect("upper").inclusive(),
        "{sql}: closed upper"
    );
}

#[test]
fn filtered_chain_conjoins_typed_range_filters() {
    // Both layers use witness-candidate range predicates: a lone
    // column-to-literal comparison already has a scalar domain and emits no
    // separate boolean witness by design.
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a BETWEEN 1 AND 10",
            "CREATE TABLE mart AS SELECT a, b FROM stage WHERE b BETWEEN 6 AND 100",
        ],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) =
        bundle.layers()[mart_layer(&bundle)].composed_semantics()
    else {
        panic!("mart layer must resolve");
    };
    assert_eq!(
        semantics.boolean_witnesses().len(),
        2,
        "both layer filters must compose"
    );
    for composed in semantics.boolean_witnesses() {
        assert_exact_both(composed.witness(), "chained range filters");
    }
    // Provenance stays honest: the stage filter is already physical, while
    // the downstream filter through the filtered stage producer remains
    // intermediate. The joint terminal proof conjoins both against one
    // shared physical row instead of relabeling either witness.
    let mart_id = bundle.layers()[mart_layer(&bundle)].id().to_string();
    for composed in semantics.boolean_witnesses() {
        if composed.origin_layer_id() == mart_id.as_str() {
            assert_eq!(composed.boundary_kind(), GroupBoundaryKind::Intermediate);
            assert_eq!(composed.witness().source_relation(), "stage");
        } else {
            assert_eq!(composed.boundary_kind(), GroupBoundaryKind::Physical);
            assert_eq!(composed.witness().source_relation(), "t");
        }
    }
    let plan = physical_source_plan(&bundle, bundle.layers()[mart_layer(&bundle)].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Feasible(_)),
        "joint qualifying must be feasible: {:?}",
        plan.qualifying()
    );
    assert!(
        matches!(plan.rejected(), WitnessDirection::Feasible(_)),
        "joint rejected must be feasible: {:?}",
        plan.rejected()
    );
    // Strongest justified domains survive composition: the mart output for
    // `a` keeps the upstream [1, 10] bound.
    let column = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "a")
        .expect("projected a");
    assert_range_bounds(column.domain(), "mart.a", "1", "10");
}

#[test]
fn contradictory_single_layer_filter_is_impossible() {
    // One conjunctive scope already proves the empty TRUE region locally:
    // neither arm alone is contradictory, only their conjunction is.
    let bundle = analyze_chain(
        &["SELECT a FROM t WHERE a IN (1, 3) AND a BETWEEN 101 AND 200"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("layer must resolve");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(
        matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
        "empty TRUE region must be impossible: {:?}",
        witness.qualifying()
    );
    assert!(
        matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "rejection stays feasible: {:?}",
        witness.rejected()
    );
}

#[test]
fn contradictory_chain_filters_prove_impossible_true_and_feasible_rejection() {
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a FROM t WHERE a IN (1, 3)",
            "CREATE TABLE mart AS SELECT a FROM stage WHERE a BETWEEN 101 AND 200",
        ],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) =
        bundle.layers()[mart_layer(&bundle)].composed_semantics()
    else {
        panic!("mart layer must resolve");
    };
    // No feasible row can satisfy both filters, so the composed output for
    // `a` is empty rather than a fabricated plan.
    assert!(
        semantics
            .column_domains()
            .iter()
            .any(|domain| matches!(domain.domain(), ValueDomain::Empty)),
        "contradiction must surface as an empty composed domain"
    );
    // Certified same-row filters contradict each other, so TRUE is
    // impossible and NOT TRUE remains feasible. This is stronger than
    // reporting a conflicting-domains gap without a row-level proof.
    let plan = physical_source_plan(&bundle, bundle.layers()[mart_layer(&bundle)].id());
    assert_eq!(plan.gap(), None, "{plan:?}");
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Impossible),
        "necessary contradiction must prove TRUE impossible: {plan:?}"
    );
    assert!(
        matches!(plan.rejected(), WitnessDirection::Feasible(_)),
        "a row failing the conjunction must remain constructible: {plan:?}"
    );
    let terminal = bundle.layers()[1].id();
    assert!(
        matches!(
            physical_row_count_plan(&bundle, terminal, 2),
            WitnessDirection::Impossible
        ),
        "positive rows cannot appear from a necessarily contradictory filter chain"
    );
    assert!(
        matches!(
            physical_joint_row_count_plan(&bundle, &[(terminal, 1)]),
            WitnessDirection::Impossible
        ),
        "joint count goals must preserve necessary impossibility"
    );
    assert!(
        matches!(
            physical_row_count_plan(&bundle, terminal, 0),
            WitnessDirection::Feasible(_)
        ),
        "empty physical source remains a valid zero-output construction"
    );
}

#[test]
fn filtered_join_preserves_typed_key_equalities() {
    let bundle = analyze_chain(
        &["SELECT l.a, r.b FROM l INNER JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("join layer must resolve");
    };
    // The typed join key composes to a physical equality with stable
    // relation-instance endpoints.
    let equality = semantics
        .join_equalities()
        .iter()
        .find(|equality| equality.join_kind() == JoinKind::Inner)
        .expect("typed inner join key must compose");
    assert_eq!(
        (equality.left().relation(), equality.left().column()),
        ("l", "k")
    );
    assert_eq!(
        (equality.right().relation(), equality.right().column()),
        ("r", "k")
    );
    // Boolean row witnesses are single-source by design: a two-source join
    // scope emits no local witness, so consumers must use the join equality
    // contract instead of a scalar row proof.
    assert!(
        semantics.boolean_witnesses().is_empty(),
        "multi-source scope must not emit a single-source witness"
    );
    // Multi-source join populations live in the TASK-68 join engine, not in
    // the single-source row plan, which stays explicitly residual.
    let plan = physical_source_plan(&bundle, bundle.layers()[0].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Residual { .. }),
        "single-source plan must not fabricate a join proof: {:?}",
        plan.qualifying()
    );
}

#[test]
fn grouped_having_keeps_output_domains_and_explicit_residual() {
    let bundle = analyze_chain(
        &["SELECT a, COUNT(*) AS n FROM t WHERE a BETWEEN 1 AND 10 GROUP BY a HAVING COUNT(*) > 1"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("grouped layer must resolve");
    };
    // HAVING over an aggregate must not invent source-column domains, but the
    // surviving group key keeps its typed filter bound.
    let column = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "a")
        .expect("grouped key a");
    assert_range_bounds(column.domain(), "grouped a", "1", "10");
    assert!(
        !semantics.condition_exactness().is_exact(),
        "HAVING must keep an explicit residual, never false exactness"
    );
}

#[test]
fn computed_alias_filter_across_layers_stays_residual() {
    // A computed projection cannot be inverted across a materialization
    // boundary yet; the filter stays an explicit residual instead of a
    // fabricated physical proof.
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a + 1 AS v FROM t",
            "CREATE TABLE mart AS SELECT v FROM stage WHERE v > 5",
        ],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) =
        bundle.layers()[mart_layer(&bundle)].composed_semantics()
    else {
        panic!("mart layer must resolve");
    };
    assert!(
        !semantics.condition_exactness().is_exact(),
        "computed cross-layer filter must stay residual"
    );
    // The same affine predicate is exact within one layer, proving the
    // residual is a composition boundary rather than a missing local proof.
    let direct = analyze_chain(
        &["SELECT a FROM t WHERE CAST(a AS BIGINT) + 1 > 5"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(direct_semantics) = direct.layers()[0].composed_semantics()
    else {
        panic!("direct layer must resolve");
    };
    assert_exact_both(
        direct_semantics.boolean_witnesses()[0].witness(),
        "local affine predicate",
    );
}

#[test]
fn certified_signed_widening_cast_survives_materialized_filter_inversion() {
    let sql = [
        "CREATE TABLE stage AS SELECT CAST(a AS BIGINT) AS v FROM t",
        "CREATE TABLE mart AS SELECT v FROM stage WHERE v BETWEEN 5 AND 10",
    ];
    let dialect = dialect_from_name("postgresql").unwrap();
    let sources = sql
        .iter()
        .map(|text| SqlInput::inline(*text))
        .collect::<Vec<_>>();
    let ids = ["stage", "mart"];
    let inputs = sources
        .iter()
        .zip(ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let schema = |relation: &str, column: &str, data_type: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, data_type, "postgresql").unwrap()],
        )
        .unwrap()
    };
    let catalog = RelationCatalog::from_schemas(&[
        schema("t", "a", "INTEGER"),
        schema("stage", "v", "BIGINT"),
        schema("mart", "v", "BIGINT"),
    ])
    .unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let target = bundle.layers()[1].id();
    let plan = physical_source_plan(&bundle, target);
    assert_eq!(plan.sources(), &["t".to_string()]);
    assert_eq!(plan.gap(), None, "{plan:?}");
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Feasible(_)),
        "total widening CAST must admit a physical qualifying row: {plan:?}"
    );
    assert!(
        matches!(plan.rejected(), WitnessDirection::Feasible(_)),
        "SQL NOT TRUE must remain constructible: {plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (3),(7),(NULL);
         CREATE TABLE stage AS SELECT CAST(a AS BIGINT) AS v FROM t;
         CREATE TABLE mart AS SELECT v FROM stage WHERE v BETWEEN 5 AND 10;",
    )
    .unwrap();
    let actual: i64 = db
        .query_row("SELECT COUNT(*) FROM mart", [], |row| row.get(0))
        .unwrap();
    let negative: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM stage WHERE (v BETWEEN 5 AND 10) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((actual, negative), (1, 2));

    // Reusing the same inverse for BIGINT -> INTEGER would be unsound:
    // TRY_CAST may produce NULL and an ordinary CAST may fail at runtime.
    let narrowing = [
        "CREATE TABLE stage AS SELECT TRY_CAST(a AS INTEGER) AS v FROM t",
        "CREATE TABLE mart AS SELECT v FROM stage WHERE v BETWEEN 5 AND 10",
    ];
    let narrowing_sources = narrowing
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let narrowing_inputs = narrowing_sources
        .iter()
        .zip(ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let narrowing_catalog = RelationCatalog::from_schemas(&[
        schema("t", "a", "BIGINT"),
        schema("stage", "v", "INTEGER"),
        schema("mart", "v", "INTEGER"),
    ])
    .unwrap();
    let narrowing_bundle =
        analyze_configured_inputs_with_catalog(&narrowing_inputs, &narrowing_catalog).unwrap();
    let narrowing_plan = physical_source_plan(&narrowing_bundle, narrowing_bundle.layers()[1].id());
    assert!(
        !matches!(narrowing_plan.qualifying(), WitnessDirection::Feasible(_)),
        "lossy narrowing must not fabricate a physical qualifying row: {narrowing_plan:?}"
    );
}

#[test]
fn parser_capable_dialects_share_composed_filter_outcomes() {
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).expect("registered dialect");
        let queries = [
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a BETWEEN 1 AND 10",
            "CREATE TABLE mart AS SELECT a, b FROM stage WHERE b BETWEEN 6 AND 100",
        ];
        let sources = queries
            .iter()
            .map(|query| SqlInput::inline(*query))
            .collect::<Vec<_>>();
        let ids = (0..queries.len())
            .map(|index| format!("q-{index}"))
            .collect::<Vec<_>>();
        let inputs = sources
            .iter()
            .zip(&ids)
            .map(|(source, id)| ConfiguredSqlInput::new(id, source, name, dialect.as_ref()))
            .collect::<Vec<_>>();
        match analyze_configured_inputs_with_catalog(&inputs, &catalog()) {
            Ok(bundle) => {
                let ComposedSemantics::Resolved(semantics) =
                    bundle.layers()[mart_layer(&bundle)].composed_semantics()
                else {
                    panic!("{name}: mart must resolve");
                };
                assert_eq!(
                    semantics.boolean_witnesses().len(),
                    2,
                    "{name}: both filters must compose"
                );
            }
            Err(ConfiguredInputAnalysisError::Input(inner))
                if matches!(inner.error(), Error::Parse(_)) =>
            {
                // Parser rejection is a documented boundary, not a proof.
            }
            Err(error) => panic!("{name}: composed filters must analyze: {error}"),
        }
        // Untyped analysis without a catalog must stay residual: no
        // integer type evidence backs the range proof.
        match analyze_sql(
            "SELECT a FROM t WHERE a BETWEEN 1 AND 10",
            name,
            dialect.as_ref(),
        ) {
            Ok(protocol) => {
                let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                    panic!("{name}: expected query");
                };
                let witness = query.boolean_witness().expect("local witness");
                assert!(
                    matches!(
                        witness.qualifying(),
                        BooleanWitnessDirection::Residual { .. }
                    ),
                    "{name}: untyped range must stay residual"
                );
            }
            Err(Error::Parse(_)) => {}
            Err(error) => panic!("{name}: unexpected analysis error: {error}"),
        }
    }
}

#[test]
fn qualify_keeps_an_explicit_residual() {
    // Windowed QUALIFY is outside the composed filter contract: the scope
    // must stay residual rather than claim an exact source-row proof.
    let bundle = analyze_chain(
        &["SELECT a FROM t QUALIFY ROW_NUMBER() OVER (PARTITION BY k ORDER BY a) = 1"],
        "duckdb",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("qualify layer must resolve");
    };
    assert!(
        !semantics.condition_exactness().is_exact(),
        "QUALIFY must keep an explicit residual"
    );
}

#[test]
fn insert_select_carries_typed_filters_with_partial_downstream() {
    // A filtered INSERT-select append composes its source filter and lineage
    // through the write layer; a downstream full-relation read stays partial
    // rather than claiming complete producer semantics.
    let bundle = analyze_chain(
        &[
            "INSERT INTO mart (a, b) SELECT a, b FROM t WHERE a BETWEEN 1 AND 10",
            "CREATE TABLE downstream AS SELECT a, b FROM mart",
        ],
        "postgresql",
    );
    let insert_layer = &bundle.layers()[0];
    assert_eq!(
        insert_layer.write_kind(),
        Some(sql_semantic_protocol::WriteKind::Append)
    );
    let ComposedSemantics::Resolved(inserted) = insert_layer.composed_semantics() else {
        panic!("insert layer must resolve");
    };
    assert_exact_both(inserted.boolean_witnesses()[0].witness(), "insert filter");
    assert_eq!(inserted.dependencies(), &["t".to_string()]);
    match bundle.layers()[1].composed_semantics() {
        ComposedSemantics::Unresolved(unresolved) => {
            assert_eq!(
                unresolved.reason(),
                sql_semantic_protocol::CompositionFailureReason::PartialProducer
            );
        }
        other => panic!("downstream full read must stay partial, got {other:?}"),
    }
}

#[test]
fn direct_sql_and_dbt_agree_on_composed_range_filters() {
    const SQL: &str = "SELECT id, amount FROM warehouse.raw.orders WHERE amount BETWEEN 1 AND 10";
    let mut manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = sql_semantic_protocol::parse_dbt_manifest(&manifest.to_string()).unwrap();
    let dbt_catalog =
        sql_semantic_protocol::parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json"))
            .unwrap();
    let dialect = sql_semantic_protocol::dialect_from_name("postgresql").unwrap();
    let from_dbt = sql_semantic_protocol::analyze_dbt_artifacts(
        &manifest,
        &dbt_catalog,
        "postgresql",
        dialect.as_ref(),
    )
    .unwrap();

    let physical = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[physical]).unwrap();
    let source = SqlInput::inline(SQL);
    let from_sql = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &source,
            "postgresql",
            dialect.as_ref(),
        )],
        &direct_catalog,
    )
    .unwrap();
    let condition = |bundle: &AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|consumed| consumed == "warehouse.raw.orders")
            })
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
            panic!("resolved layer expected");
        };
        semantics.boolean_witnesses()[0].witness().clone()
    };
    let dbt = condition(&from_dbt);
    let direct = condition(&from_sql);
    assert_eq!(dbt.condition(), direct.condition());
    assert_eq!(dbt.qualifying(), direct.qualifying());
    assert_eq!(dbt.rejected(), direct.rejected());
    assert!(matches!(
        dbt.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
}

#[test]
fn duckdb_joint_positive_and_negative_rows_match_composed_filters() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO t VALUES (3, 7, 1), (11, 7, 2), (3, 1, 3), (NULL, 7, 4);
         CREATE TABLE stage AS SELECT a, b FROM t WHERE a BETWEEN 1 AND 10;
         CREATE TABLE mart AS SELECT a, b FROM stage WHERE b BETWEEN 6 AND 100;",
    )
    .unwrap();
    let positive: i64 = db
        .query_row("SELECT COUNT(*) FROM mart", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        positive, 1,
        "exactly one row survives both materializations"
    );
    // IS NOT TRUE counts FALSE plus UNKNOWN rows: the NULL row is NOT
    // TRUE without being FALSE, which a plain NOT would filter away.
    let rejected: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM t WHERE (a BETWEEN 1 AND 10 AND b BETWEEN 6 AND 100) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        rejected, 3,
        "FALSE and UNKNOWN rows are all NOT TRUE, including the NULL row"
    );

    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a BETWEEN 1 AND 10",
            "CREATE TABLE mart AS SELECT a, b FROM stage WHERE b BETWEEN 6 AND 100",
        ],
        "postgresql",
    );
    let plan = physical_source_plan(&bundle, bundle.layers()[mart_layer(&bundle)].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Feasible(_)),
        "DuckDB-positive case must have a feasible qualifying proof"
    );
    assert!(
        matches!(plan.rejected(), WitnessDirection::Feasible(_)),
        "DuckDB-negative case must have a feasible rejected proof"
    );
}
