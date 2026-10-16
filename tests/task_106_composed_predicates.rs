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
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, ConstraintValue, Error, GroupBoundaryKind, JoinKind, LiteralValue,
    PhysicalProofGap, ProtocolStatement, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput, ValueDomain, WitnessDirection,
    WitnessFormula, WitnessObligation,
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
    assert_eq!(
        plan.gap(),
        None,
        "{plan:?}; composed: {:?}; producer: {:?}; filter: {:?}",
        bundle.layers()[1].composed_semantics(),
        bundle.inputs()[0].statements()[0],
        bundle.inputs()[1].statements()[0]
    );
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
    assert_eq!(
        narrowing_plan.gap(),
        Some(PhysicalProofGap::LocalWitnessUnproven)
    );
    assert!(
        !matches!(narrowing_plan.qualifying(), WitnessDirection::Feasible(_)),
        "lossy narrowing must not fabricate a physical qualifying row: {narrowing_plan:?}"
    );
}

#[test]
fn parser_capable_dialects_share_composed_filter_outcomes() {
    let mut expected = None;
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
                let plan = physical_source_plan(&bundle, bundle.layers()[mart_layer(&bundle)].id());
                let actual = (
                    semantics.boolean_witnesses().to_vec(),
                    semantics.output().clone(),
                    semantics.column_domains().to_vec(),
                    plan.sources().to_vec(),
                    plan.qualifying().clone(),
                    plan.rejected().clone(),
                );
                if let Some(expected) = &expected {
                    assert_eq!(&actual, expected, "{name}: canonical outcome drift");
                } else {
                    expected = Some(actual);
                }
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
    let source_layer = |bundle: &AnalysisBundle| {
        bundle
            .layers()
            .iter()
            .position(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|consumed| consumed == "warehouse.raw.orders")
            })
            .expect("source-consuming layer")
    };
    let condition = |bundle: &AnalysisBundle| {
        let layer = &bundle.layers()[source_layer(bundle)];
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
    let dbt_plan = physical_source_plan(&from_dbt, from_dbt.layers()[source_layer(&from_dbt)].id());
    let direct_plan =
        physical_source_plan(&from_sql, from_sql.layers()[source_layer(&from_sql)].id());
    assert_eq!(dbt_plan.sources(), direct_plan.sources());
    assert_eq!(dbt_plan.gap(), direct_plan.gap());
    assert_eq!(dbt_plan.qualifying(), direct_plan.qualifying());
    assert_eq!(dbt_plan.rejected(), direct_plan.rejected());
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

#[test]
fn filtered_join_duckdb_oracle_matches_key_equalities() {
    // A filtered INNER JOIN keeps its typed key equality exact while the
    // multi-source row proof stays explicitly residual: no single-source
    // witness may be fabricated for a two-source scope.
    let bundle = analyze_chain(
        &["SELECT l.a, r.b FROM l INNER JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("join layer must resolve");
    };
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
    assert!(
        semantics.boolean_witnesses().is_empty(),
        "multi-source scope must not emit a single-source witness"
    );
    let plan = physical_source_plan(&bundle, bundle.layers()[0].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Residual { .. }),
        "single-source plan must not fabricate a join proof: {plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(a INTEGER, b INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO l VALUES (2, 0, 1), (99, 0, 2), (NULL, 0, 3);
         INSERT INTO r VALUES (0, 10, 1), (0, 20, 2), (0, 30, 3);
         CREATE TABLE joined AS SELECT l.a, r.b FROM l INNER JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5;",
    )
    .unwrap();
    let matched: i64 = db
        .query_row("SELECT COUNT(*) FROM joined", [], |row| row.get(0))
        .unwrap();
    assert_eq!(matched, 1, "only the in-range join key survives");
    let rejected: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l WHERE (a BETWEEN 1 AND 5) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rejected, 2, "out-of-range and NULL rows are NOT TRUE");
}

#[test]
fn join_on_typed_filter_stays_residual_while_key_stays_exact() {
    // An extra typed predicate inside ON composes with the key equality: the
    // inner-join scope stays exact with the scalar filter carried as a column
    // domain, while the single-source row proof stays residual because the
    // scope has two physical producers.
    let bundle = analyze_chain(
        &["SELECT l.a FROM l INNER JOIN r ON l.k = r.k AND l.a > 0"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("join layer must resolve");
    };
    assert!(
        semantics
            .join_equalities()
            .iter()
            .any(|equality| equality.join_kind() == JoinKind::Inner),
        "key equality must survive an additional ON conjunct"
    );
    assert!(
        semantics.condition_exactness().is_exact(),
        "typed ON scalar filter keeps inner-join scope exact"
    );
    let plan = physical_source_plan(&bundle, bundle.layers()[0].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Residual { .. }),
        "unproved ON row-shaping must not become feasible: {plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         INSERT INTO l VALUES (1, 1), (-2, 2);
         INSERT INTO r VALUES (10, 1), (20, 2);
         SELECT COUNT(*) FROM l INNER JOIN r ON l.k = r.k AND l.a > 0;",
    )
    .unwrap();
    let matched: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l INNER JOIN r ON l.k = r.k AND l.a > 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(matched, 1);
}

#[test]
fn filtered_inner_join_builds_one_complete_typed_population() {
    let bundle = analyze_chain(
        &["SELECT l.a, r.b FROM l INNER JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5"],
        "postgresql",
    );
    let target = bundle.layers()[0].id();
    let proof = physical_row_count_plan(&bundle, target, 2);
    let WitnessDirection::Feasible(cases) = proof else {
        panic!("typed non-key WHERE must compose with the complete join population: {proof:?}");
    };
    assert!(cases.iter().any(|case| {
        case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::Rows {
                    boundary,
                    predicate: WitnessFormula::RowTruth { .. },
                    closed_world: true,
                    ..
                } if boundary.relation() == "l"
            )
        })
    }));

    let on_bundle = analyze_chain(
        &["SELECT l.a, r.b FROM l INNER JOIN r ON l.k = r.k AND l.a > 0"],
        "postgresql",
    );
    assert!(
        matches!(
            physical_row_count_plan(&on_bundle, on_bundle.layers()[0].id(), 2),
            WitnessDirection::Feasible(_)
        ),
        "typed non-key ON conjunct must compose with the key population"
    );

    let key_filter = analyze_chain(
        &["SELECT l.a FROM l INNER JOIN r ON l.k = r.k WHERE l.k > 0"],
        "postgresql",
    );
    assert!(
        matches!(
            physical_row_count_plan(&key_filter, key_filter.layers()[0].id(), 2),
            WitnessDirection::Residual { .. }
        ),
        "join-key restrictions need a key-aware population proof"
    );
}

#[test]
fn filtered_join_and_negative_terminals_share_one_complete_assignment() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a, r.b FROM l INNER JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5",
            "SELECT a FROM l WHERE a > 5",
            "SELECT b FROM r WHERE b < 0",
        ],
        "postgresql",
    );
    let goals = [
        (bundle.layers()[0].id(), 2),
        (bundle.layers()[1].id(), 0),
        (bundle.layers()[2].id(), 0),
    ];
    let WitnessDirection::Feasible(cases) = physical_joint_row_count_plan(&bundle, &goals) else {
        panic!("positive filtered join and NOT TRUE terminals need one source assignment");
    };
    assert!(cases.iter().any(|case| {
        case.obligations()
            .iter()
            .filter(|obligation| matches!(obligation, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 2
            && case
                .obligations()
                .iter()
                .filter(|obligation| matches!(obligation, WitnessObligation::OutputRows { .. }))
                .count()
                == 3
    }));
}

#[test]
fn grouped_having_duckdb_oracle_matches_output_domains() {
    // GROUP BY keeps the typed filter bound on the group key while HAVING
    // over COUNT(*) stays an explicit residual: no source-column domain is
    // invented from the aggregate result.
    let bundle = analyze_chain(
        &["SELECT a, COUNT(*) AS n FROM t WHERE a BETWEEN 1 AND 10 GROUP BY a HAVING COUNT(*) > 1"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("grouped layer must resolve");
    };
    let column = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "a")
        .expect("grouped key a");
    assert_range_bounds(column.domain(), "grouped a", "1", "10");
    assert!(
        !semantics.condition_exactness().is_exact(),
        "HAVING must keep an explicit residual"
    );
    let plan = physical_source_plan(&bundle, bundle.layers()[0].id());
    assert!(
        matches!(plan.zero_output(), WitnessDirection::Feasible(_)),
        "an empty physical source cannot produce groups: {plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO t VALUES (3, 0, 1), (3, 0, 2), (7, 0, 3), (99, 0, 4), (NULL, 0, 5);",
    )
    .unwrap();
    let groups: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) AS n FROM t WHERE a BETWEEN 1 AND 10 GROUP BY a HAVING COUNT(*) > 1)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(groups, 1, "only a=3 forms a repeated in-range group");
}

#[test]
fn supported_grouped_having_reaches_complete_physical_rows() {
    let bundle = analyze_chain(
        &["SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2"],
        "postgresql",
    );
    let target = bundle.layers()[0].id();
    let WitnessDirection::Feasible(cases) = physical_row_count_plan(&bundle, target, 2) else {
        panic!("supported COUNT/HAVING must construct complete grouped rows");
    };
    assert!(cases.iter().any(|case| {
        case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::Rows { boundary, closed_world: true, .. }
                    if boundary.relation() == "t"
            )
        }) && case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::OutputRows { layer_id, bounds }
                    if layer_id == target && bounds.minimum() == 2 && bounds.maximum() == Some(2)
            )
        })
    }));
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("grouped layer must resolve");
    };
    let count = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "n")
        .expect("COUNT output");
    let ValueDomain::Ranges(ranges) = count.domain() else {
        panic!(
            "COUNT output must retain a lower bound: {:?}",
            count.domain()
        );
    };
    let range = &ranges.ranges()[0];
    assert_eq!(
        range.lower().expect("lower").value().value(),
        &LiteralValue::Number("2".to_string())
    );
    assert!(range.lower().expect("lower").inclusive());

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO t VALUES (0,10,0),(0,11,1),(1,20,2),(1,21,3);",
    )
    .unwrap();
    let rows: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rows, 2);
}

#[test]
fn qualify_residual_contrasts_duckdb_execution() {
    // QUALIFY is outside the composed filter contract: the scope stays
    // residual even though DuckDB executes the windowed filter.
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
    let plan = physical_source_plan(&bundle, bundle.layers()[0].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Residual { .. }),
        "windowed QUALIFY must not become a single-source filter proof: {plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO t VALUES (2, 0, 1), (1, 0, 1), (5, 0, 2);",
    )
    .unwrap();
    let rows: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a FROM t QUALIFY ROW_NUMBER() OVER (PARTITION BY k ORDER BY a) = 1)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        rows, 2,
        "DuckDB executes QUALIFY while the protocol stays residual"
    );
}

#[test]
fn supported_qualify_reaches_complete_ranked_rows() {
    let bundle = analyze_chain(
        &["SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn <= 2"],
        "duckdb",
    );
    let target = bundle.layers()[0].id();
    let WitnessDirection::Feasible(cases) = physical_row_count_plan(&bundle, target, 4) else {
        panic!("supported ROW_NUMBER/QUALIFY must construct complete partitions");
    };
    assert!(cases.iter().any(|case| {
        case.obligations().iter().any(|obligation| {
            matches!(
                obligation,
                WitnessObligation::Rows { boundary, closed_world: true, .. }
                    if boundary.relation() == "t"
            )
        })
    }));
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("ranked layer must resolve");
    };
    let rank = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "rn")
        .expect("ROW_NUMBER output");
    assert_range_bounds(rank.domain(), "rn", "1", "2");

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO t VALUES (0,10,0),(0,11,1),(0,12,2),(1,20,3),(1,21,4),(1,22,5);",
    )
    .unwrap();
    let rows: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn <= 2)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rows, 4);
}

#[test]
fn computed_alias_output_domain_stays_unknown_with_lineage() {
    // A computed projection across a materialization boundary must not invent
    // source bounds, but the downstream WHERE filter still narrows the
    // composed output: v keeps (5, +inf) while the source inversion stays
    // residual.
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
    let column = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "v")
        .expect("projected v");
    let ValueDomain::Ranges(ranges) = column.domain() else {
        panic!(
            "downstream filter must narrow the composed output domain: {:?}",
            column.domain()
        );
    };
    assert_eq!(ranges.ranges().len(), 1);
    let range = &ranges.ranges()[0];
    assert_eq!(
        range.lower().expect("lower").value().value(),
        &LiteralValue::Number("5".to_string())
    );
    assert!(
        !range.lower().expect("lower").inclusive(),
        "v > 5 stays exclusive"
    );
    assert!(range.upper().is_none(), "no invented upper bound");
    assert_eq!(
        semantics.dependencies(),
        &["t".to_string()],
        "physical lineage must survive the residual"
    );
    let plan = physical_source_plan(&bundle, bundle.layers()[mart_layer(&bundle)].id());
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Residual { .. }),
        "computed inversion must stay residual: {plan:?}"
    );
}

#[test]
fn joint_duplicate_null_typed_assignment_shares_one_source() {
    // One complete source with duplicates and NULLs satisfies a positive
    // range terminal and a joint NOT TRUE terminal on the same rows.
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a BETWEEN 1 AND 10",
            "CREATE TABLE mart AS SELECT a, b FROM stage WHERE b BETWEEN 6 AND 100",
        ],
        "postgresql",
    );
    let mart = bundle.layers()[mart_layer(&bundle)].id();
    let joint = physical_joint_row_count_plan(&bundle, &[(mart, 1)]);
    assert!(
        matches!(joint, WitnessDirection::Feasible(_)),
        "one qualifying row must be jointly constructible: {joint:?}"
    );
    let empty = physical_row_count_plan(&bundle, mart, 0);
    assert!(
        matches!(empty, WitnessDirection::Feasible(_)),
        "an empty physical source remains a valid zero construction: {empty:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (3, 7), (3, 7), (NULL, 7), (3, 1);
         CREATE TABLE stage AS SELECT a, b FROM t WHERE a BETWEEN 1 AND 10;
         CREATE TABLE mart AS SELECT a, b FROM stage WHERE b BETWEEN 6 AND 100;",
    )
    .unwrap();
    let positive: i64 = db
        .query_row("SELECT COUNT(*) FROM mart", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        positive, 2,
        "duplicate qualifying rows survive both filters"
    );
    let rejected: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM t WHERE (a BETWEEN 1 AND 10 AND b BETWEEN 6 AND 100) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rejected, 2, "NULL and out-of-range duplicates are NOT TRUE");
}

#[test]
fn partial_unsupported_lossy_and_missing_evidence_stay_residual() {
    // AC4: only necessary contradictions or complete closed-world absence may
    // prove impossibility. Everything else must fail closed.
    let partial = analyze_chain(
        &[
            "INSERT INTO mart (a, b) SELECT a, b FROM t WHERE a BETWEEN 1 AND 10",
            "CREATE TABLE downstream AS SELECT a, b FROM mart",
        ],
        "postgresql",
    );
    match partial.layers()[1].composed_semantics() {
        ComposedSemantics::Unresolved(unresolved) => assert_eq!(
            unresolved.reason(),
            sql_semantic_protocol::CompositionFailureReason::PartialProducer
        ),
        other => {
            panic!("downstream full read over a partial write must stay unresolved: {other:?}")
        }
    }
    let partial_plan = physical_source_plan(&partial, partial.layers()[1].id());
    assert_eq!(partial_plan.gap(), Some(PhysicalProofGap::PartialProducer));

    let unsupported = analyze_chain(
        &["SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) + 1 > 2"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(unsupported_semantics) =
        unsupported.layers()[0].composed_semantics()
    else {
        panic!("unsupported HAVING layer must resolve with residual");
    };
    assert!(
        !unsupported_semantics.condition_exactness().is_exact(),
        "unsupported aggregate HAVING must stay residual"
    );
    let unsupported_plan = physical_source_plan(&unsupported, unsupported.layers()[0].id());
    assert_eq!(
        unsupported_plan.gap(),
        Some(PhysicalProofGap::UnsupportedOperator)
    );

    // Lossy narrowing TRY_CAST inversion must not fabricate a physical row;
    // this complements the certified widening test above.
    let dialect = dialect_from_name("postgresql").unwrap();
    let narrowing_sources = ["CREATE TABLE stage AS SELECT TRY_CAST(a AS INTEGER) AS v FROM t"]
        .into_iter()
        .map(SqlInput::inline)
        .collect::<Vec<_>>();
    let narrowing_inputs = narrowing_sources
        .iter()
        .zip(["stage"])
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let schema = |relation: &str, column: &str, data_type: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, data_type, "postgresql").unwrap()],
        )
        .unwrap()
    };
    let narrowing_catalog = RelationCatalog::from_schemas(&[
        schema("t", "a", "BIGINT"),
        schema("stage", "v", "INTEGER"),
    ])
    .unwrap();
    let narrowing_bundle =
        analyze_configured_inputs_with_catalog(&narrowing_inputs, &narrowing_catalog).unwrap();
    let narrowing_plan = physical_source_plan(&narrowing_bundle, narrowing_bundle.layers()[0].id());
    assert_eq!(narrowing_plan.gap(), Some(PhysicalProofGap::NoWitness));
    assert!(
        matches!(
            narrowing_plan.qualifying(),
            WitnessDirection::Residual { .. }
        ),
        "lossy narrowing must stay residual: {narrowing_plan:?}"
    );

    // Missing source constraints (no catalog) must stay residual, never exact.
    match analyze_sql(
        "SELECT a FROM t WHERE a BETWEEN 1 AND 10",
        "postgresql",
        dialect.as_ref(),
    ) {
        Ok(protocol) => {
            let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                panic!("expected query");
            };
            let witness = query.boolean_witness().expect("local witness");
            let BooleanWitnessDirection::Residual { reason } = witness.qualifying() else {
                panic!("untyped range must stay residual without catalog evidence");
            };
            assert_eq!(reason, "predicate has no resolved source columns");
        }
        Err(Error::Parse(_)) => {}
        Err(error) => panic!("unexpected analysis error: {error}"),
    }
}

#[test]
fn computed_insert_select_arithmetic_uses_catalog_domains() {
    // INSERT SELECT reuses the same catalog-certified arithmetic prover as
    // SELECT projections (via analyze_query), so a branch-constrained
    // `a BETWEEN 1 AND 5` plus 4 yields the exact finite set {5,..,9}.
    let bundle = analyze_chain(
        &["INSERT INTO mart (a) SELECT a + 4 AS a FROM t WHERE a BETWEEN 1 AND 5"],
        "postgresql",
    );
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("insert layer must resolve");
    };
    let column = semantics
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "a")
        .expect("projected a");
    let ValueDomain::Set(set) = column.domain() else {
        panic!(
            "computed INSERT must carry exact set domain: {:?}",
            column.domain()
        );
    };
    let mut values = set
        .values()
        .iter()
        .map(|literal| literal.value().clone())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    assert_eq!(
        values,
        vec![
            LiteralValue::Number("5".to_string()),
            LiteralValue::Number("6".to_string()),
            LiteralValue::Number("7".to_string()),
            LiteralValue::Number("8".to_string()),
            LiteralValue::Number("9".to_string()),
        ]
    );
}

#[test]
fn single_layer_contradiction_rules_out_any_positive_count() {
    // A necessary single-layer contradiction must fail closed for every
    // positive cardinality (Residual conflicting_domains here, Impossible in
    // the two-layer chain). Zero stays feasible as empty construction.
    let bundle = analyze_chain(
        &["SELECT a FROM t WHERE a IN (1, 3) AND a BETWEEN 101 AND 200"],
        "postgresql",
    );
    let target = bundle.layers()[0].id();
    for rows in [1, 2, 5] {
        assert!(
            !matches!(
                physical_row_count_plan(&bundle, target, rows),
                WitnessDirection::Feasible(_)
            ),
            "necessary contradiction must never yield feasible rows"
        );
    }
    assert!(
        matches!(
            physical_row_count_plan(&bundle, target, 0),
            WitnessDirection::Feasible(_)
        ),
        "empty source remains feasible"
    );
}

#[test]
fn outer_join_contradiction_stays_residual_not_impossible() {
    // Outer joins preserve unmatched sides, so a contradictory NOT TRUE
    // combination must fail closed as residual, never as proven impossible.
    // Only certified INNER contradictions may become Impossible.
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l LEFT JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5",
            "SELECT a FROM l WHERE a > 100",
        ],
        "postgresql",
    );
    let goals = [(bundle.layers()[0].id(), 1), (bundle.layers()[1].id(), 0)];
    let joint = physical_joint_row_count_plan(&bundle, &goals);
    assert!(
        !matches!(joint, WitnessDirection::Impossible),
        "outer-join contradiction must not be proven impossible: {joint:?}"
    );
}

#[test]
fn filtered_join_count_requires_unlimited_bag_output_shape() {
    for sql in [
        "SELECT l.a FROM l JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5 LIMIT 1",
        "SELECT DISTINCT l.a FROM l JOIN r ON l.k = r.k WHERE l.a = 1",
    ] {
        let bundle = analyze_chain(&[sql], "postgresql");
        let result = physical_row_count_plan(&bundle, bundle.layers()[0].id(), 3);
        assert!(
            !matches!(result, WitnessDirection::Feasible(_)),
            "{sql}: row-shaping must not be ignored: {result:?}"
        );
    }

    let sql = "SELECT l.a FROM l JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5";
    let bundle = analyze_chain(&[sql], "postgresql");
    let result = physical_row_count_plan(&bundle, bundle.layers()[0].id(), 3);
    assert!(
        matches!(result, WitnessDirection::Feasible(_)),
        "plain filtered inner join must remain constructible: {result:?}"
    );
}

#[test]
fn outer_join_where_must_not_use_null_extended_rows_as_survivors() {
    let sql = "SELECT l.a FROM l LEFT JOIN r ON l.k = r.k WHERE r.a = 1";
    let bundle = analyze_chain(&[sql], "postgresql");
    let result = physical_row_count_plan(&bundle, bundle.layers()[0].id(), 2);
    assert!(
        !matches!(result, WitnessDirection::Feasible(_)),
        "source-row restrictions cannot certify post-join WHERE: {result:?}"
    );

    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE l(a INTEGER, k INTEGER); CREATE TABLE r(a INTEGER, k INTEGER); INSERT INTO l VALUES (1,0),(2,1);").expect("test inputs");
    let surviving: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l LEFT JOIN r ON l.k = r.k WHERE r.a = 1",
            [],
            |row| row.get(0),
        )
        .expect("outer join oracle");
    assert_eq!(surviving, 0, "NULL-extended right rows fail the WHERE");
}

#[test]
fn contradictory_independent_join_scalar_filters_never_yield_a_complete_population() {
    let sql = "SELECT l.a FROM l JOIN r ON l.k = r.k AND l.a > 5 AND l.a < 2";
    let bundle = analyze_chain(&[sql], "postgresql");
    let result = physical_row_count_plan(&bundle, bundle.layers()[0].id(), 1);
    assert!(
        !matches!(result, WitnessDirection::Feasible(_)),
        "separately exact ON conjuncts have no jointly satisfying source row: {result:?}"
    );

    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         INSERT INTO l VALUES (1,0),(7,0);
         INSERT INTO r VALUES (1,0);",
    )
    .expect("join contradiction inputs");
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM l JOIN r ON l.k = r.k AND l.a > 5 AND l.a < 2",
            [],
            |row| row.get(0),
        )
        .expect("join contradiction oracle");
    assert_eq!(count, 0);
}

fn constrained_stage_restriction() -> RelationConstraintSet {
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(
            ConstraintSourceKind::ExternalMetadata,
            "stage-warehouse-catalog",
        )
        .expect("evidence provenance"),
        ConstraintEnforcement::Enforced,
    )];
    RelationConstraintSet::new(
        "stage",
        vec![RelationConstraint::accepted_values(
            "a",
            vec![ConstraintValue::Integer(20)],
            false,
            evidence,
        )
        .expect("warehouse restriction")],
    )
    .expect("relation constraints")
}

#[test]
fn independent_materialized_check_laws_cannot_disappear_during_identity_inversion() {
    let mut bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a, b, k FROM t",
            "SELECT a FROM stage WHERE a BETWEEN 1 AND 5",
        ],
        "postgresql",
    );
    let target = bundle.layers()[1].id().to_string();
    assert!(
        matches!(
            physical_source_plan(&bundle, &target).qualifying(),
            WitnessDirection::Feasible(_)
        ),
        "an unconstrained identity copy should support typed physical filters"
    );
    bundle.enrich_relation_constraints(&[constrained_stage_restriction()]);
    let plan = physical_source_plan(&bundle, &target);
    assert!(
        !matches!(plan.qualifying(), WitnessDirection::Feasible(_)),
        "stage-specific enforced values cannot be discarded at the producer boundary: {plan:?}"
    );
    assert!(
        !matches!(
            physical_row_count_plan(&bundle, &target, 2),
            WitnessDirection::Feasible(_)
        ),
        "no positive closed-source proof may ignore the materialized CHECK"
    );
}

#[test]
fn filtered_join_population_respects_independently_enforced_stage_laws() {
    let mut bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a, b, k FROM l",
            "SELECT s.a FROM stage s JOIN r ON s.k = r.k WHERE s.a BETWEEN 1 AND 5",
        ],
        "postgresql",
    );
    let target = bundle.layers()[1].id().to_string();
    let before = physical_row_count_plan(&bundle, &target, 2);
    assert!(
        matches!(before, WitnessDirection::Feasible(_)),
        "filtered join should be constructible without independent constraints: {before:?}"
    );
    bundle.enrich_relation_constraints(&[constrained_stage_restriction()]);
    let after = physical_row_count_plan(&bundle, &target, 2);
    assert!(
        !matches!(after, WitnessDirection::Feasible(_)),
        "complete join populations must not silently bypass stage-specific constraints: {after:?}"
    );
}

#[test]
fn multiple_join_source_filters_reconcile_joint_true_and_not_true_terminals() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l INNER JOIN r ON l.k = r.k AND l.a BETWEEN 1 AND 5 AND l.b BETWEEN 2 AND 6",
            "SELECT a FROM l WHERE a > 5",
            "SELECT b FROM r WHERE b < 0",
        ],
        "postgresql",
    );
    let positive = physical_row_count_plan(&bundle, bundle.layers()[0].id(), 2);
    assert!(
        matches!(positive, WitnessDirection::Feasible(_)),
        "first certify the conjunctive filtered join population: {positive:?}"
    );
    let goals = [
        (bundle.layers()[0].id(), 2),
        (bundle.layers()[1].id(), 0),
        (bundle.layers()[2].id(), 0),
    ];
    let plan = physical_joint_row_count_plan(&bundle, &goals);
    let WitnessDirection::Feasible(cases) = plan else {
        panic!("two source predicates must remain jointly constructible with negative terminals: {plan:?}");
    };
    assert!(
        cases.iter().any(|case| {
            case.obligations().iter().any(|obligation| {
                matches!(
                    obligation,
                    WitnessObligation::Rows {
                        boundary,
                        predicate: WitnessFormula::All(items),
                        closed_world: true,
                        ..
                    } if boundary.relation() == "l" && items.len() == 3
                )
            })
        }),
        "l requires two ON conjuncts and one NOT TRUE terminal"
    );
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(a INTEGER,b INTEGER,k INTEGER);
         CREATE TABLE r(a INTEGER,b INTEGER,k INTEGER);
         INSERT INTO l VALUES (3,4,0),(4,5,1);
         INSERT INTO r VALUES (1,1,0),(2,2,1);",
    )
    .expect("physical rows");
    let counts = [
        "SELECT COUNT(*) FROM l JOIN r ON l.k=r.k AND l.a BETWEEN 1 AND 5 AND l.b BETWEEN 2 AND 6",
        "SELECT COUNT(*) FROM l WHERE a > 5",
        "SELECT COUNT(*) FROM r WHERE b < 0",
    ]
    .map(|sql| {
        db.query_row(sql, [], |row| row.get::<_, i64>(0))
            .expect("oracle")
    });
    assert_eq!(counts, [2, 0, 0]);
}

#[test]
fn multiple_join_source_filters_with_incompatible_negative_terminal_are_impossible() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l INNER JOIN r ON l.k = r.k AND l.a BETWEEN 1 AND 5 AND l.b BETWEEN 2 AND 6",
            "SELECT a FROM l WHERE a <= 5",
        ],
        "postgresql",
    );
    let targets = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 0)];
    let result = physical_joint_row_count_plan(&bundle, &targets);
    assert!(
        matches!(result, WitnessDirection::Impossible),
        "positive inner-join rows must satisfy a BETWEEN 1 AND 5, while zero rows for a <= 5 requires every row to reject it: {result:?}"
    );
}

#[test]
fn independent_filtered_join_dags_share_one_joint_closed_physical_plan() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l INNER JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 5",
            "SELECT stage.a FROM stage INNER JOIN mart ON stage.k = mart.k WHERE stage.a BETWEEN 10 AND 15",
        ],
        "postgresql",
    );
    let goals = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 3)];
    let result = physical_joint_row_count_plan(&bundle, &goals);
    let WitnessDirection::Feasible(cases) = result else {
        panic!("disjoint certified filtered joins must admit one combined physical population: {result:?}");
    };
    assert!(
        cases.iter().any(|case| {
            case.obligations()
                .iter()
                .filter(|item| {
                    matches!(
                        item,
                        WitnessObligation::JoinPopulation {
                            closed_world: true,
                            ..
                        }
                    )
                })
                .count()
                == 2
                && case
                    .obligations()
                    .iter()
                    .filter(|item| matches!(item, WitnessObligation::ClosedWorld { .. }))
                    .count()
                    == 4
                && case
                    .obligations()
                    .iter()
                    .filter(|item| matches!(item, WitnessObligation::OutputRows { .. }))
                    .count()
                    == 2
        }),
        "each physical leaf is closed once, both join populations survive"
    );

    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(a INTEGER,k INTEGER);
         CREATE TABLE r(a INTEGER,k INTEGER);
         CREATE TABLE stage(a INTEGER,k INTEGER);
         CREATE TABLE mart(a INTEGER,k INTEGER);
         INSERT INTO l VALUES (1,0),(2,1);
         INSERT INTO r VALUES (0,0),(0,1);
         INSERT INTO stage VALUES (10,0),(11,1),(12,2);
         INSERT INTO mart VALUES (0,0),(0,1),(0,2);",
    )
    .expect("complete independent source bags");
    let actual = [
        "SELECT COUNT(*) FROM l JOIN r ON l.k=r.k WHERE l.a BETWEEN 1 AND 5",
        "SELECT COUNT(*) FROM stage JOIN mart ON stage.k=mart.k WHERE stage.a BETWEEN 10 AND 15",
    ]
    .map(|sql| {
        db.query_row(sql, [], |row| row.get::<_, i64>(0))
            .expect("join oracle")
    });
    assert_eq!(actual, [2, 3]);
}

#[test]
fn total_affine_projection_transports_true_and_not_true_through_materialization() {
    // The explicit BIGINT cast makes +1 total across every INTEGER source
    // value. The materialized predicate v BETWEEN 5 AND 10 therefore has
    // the exact source-row inverse a BETWEEN 4 AND 9, including SQL NULL.
    let sql = [
        "CREATE TABLE stage AS SELECT CAST(a AS BIGINT) + 1 AS v FROM t",
        "CREATE TABLE mart AS SELECT v FROM stage WHERE v BETWEEN 5 AND 10",
    ];
    let dialect = dialect_from_name("postgresql").expect("postgresql");
    let sources = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let inputs = sources
        .iter()
        .zip(["stage", "mart"])
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let schema = |relation: &str, name: &str, datatype: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(name, datatype, "postgresql").unwrap()],
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
    assert!(
        matches!(plan.qualifying(), WitnessDirection::Feasible(_)),
        "invert the total computed expression through its producer: {plan:?}"
    );
    assert!(
        matches!(plan.rejected(), WitnessDirection::Feasible(_)),
        "NULL and out-of-range rows must remain SQL NOT TRUE: {plan:?}"
    );

    for producer in [
        "CREATE TABLE stage AS SELECT CAST(a AS BIGINT) + 1 AS v FROM t LIMIT 1",
        "CREATE TABLE stage AS SELECT DISTINCT CAST(a AS BIGINT) + 1 AS v FROM t",
    ] {
        let shaped_sources = [SqlInput::inline(producer), SqlInput::inline(sql[1])];
        let shaped_inputs = shaped_sources
            .iter()
            .zip(["stage", "mart"])
            .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
            .collect::<Vec<_>>();
        let shaped = analyze_configured_inputs_with_catalog(&shaped_inputs, &catalog).unwrap();
        let result = physical_source_plan(&shaped, shaped.layers()[1].id());
        assert!(
            !matches!(result.qualifying(), WitnessDirection::Feasible(_)),
            "LIMIT or DISTINCT may not be inverted as a complete producer: {result:?}"
        );
    }

    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (3),(4),(7),(9),(10),(NULL);
         CREATE TABLE stage AS SELECT CAST(a AS BIGINT) + 1 AS v FROM t;
         CREATE TABLE mart AS SELECT v FROM stage WHERE v BETWEEN 5 AND 10;",
    )
    .expect("affine oracle inputs");
    let positive: i64 = db
        .query_row("SELECT COUNT(*) FROM mart", [], |row| row.get(0))
        .unwrap();
    let negative: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM stage WHERE (v BETWEEN 5 AND 10) IS NOT TRUE",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((positive, negative), (3, 3));
}

#[test]
fn overflowing_affine_projections_remain_residual() {
    // No source restriction makes INTEGER + 1 total on its full signed
    // range; unlike BIGINT widening it would overflow for the max value.
    let sql = [
        "CREATE TABLE stage AS SELECT CAST(a AS INTEGER) + 1 AS v FROM t",
        "CREATE TABLE mart AS SELECT v FROM stage WHERE v BETWEEN 5 AND 10",
    ];
    let dialect = dialect_from_name("postgresql").unwrap();
    let sources = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let inputs = sources
        .iter()
        .zip(["stage", "mart"])
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let schema = |relation: &str, name: &str, datatype: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(name, datatype, "postgresql").unwrap()],
        )
        .unwrap()
    };
    let catalog = RelationCatalog::from_schemas(&[
        schema("t", "a", "INTEGER"),
        schema("stage", "v", "INTEGER"),
        schema("mart", "v", "INTEGER"),
    ])
    .unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let plan = physical_source_plan(&bundle, bundle.layers()[1].id());
    assert!(
        !matches!(plan.qualifying(), WitnessDirection::Feasible(_)),
        "potential overflow may not be represented as an exact physical row witness: {plan:?}"
    );
}

#[test]
fn overlapping_filtered_joins_reconcile_one_shared_physical_source() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l JOIN r ON l.k = r.k WHERE l.a BETWEEN 1 AND 9",
            "SELECT l.b FROM l JOIN stage ON l.k = stage.k WHERE l.b BETWEEN 2 AND 8",
        ],
        "postgresql",
    );
    let goals = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)];
    let plan = physical_joint_row_count_plan(&bundle, &goals);
    let WitnessDirection::Feasible(cases) = plan else {
        panic!("overlapping joins must jointly reconcile source-row truths: {plan:?}");
    };
    assert!(
        cases.iter().any(|case| {
            let closures = case
                .obligations()
                .iter()
                .filter(|item| matches!(item, WitnessObligation::ClosedWorld { .. }))
                .count();
            let joins = case
                .obligations()
                .iter()
                .filter(|item| {
                    matches!(
                        item,
                        WitnessObligation::JoinPopulation {
                            closed_world: true,
                            ..
                        }
                    )
                })
                .count();
            let shared = case
                .obligations()
                .iter()
                .filter(|item| {
                    matches!(
                        item,
                        WitnessObligation::Rows {
                            boundary,
                            predicate: WitnessFormula::All(parts),
                            closed_world: true,
                            ..
                        } if boundary.relation() == "l" && parts.len() == 2
                    )
                })
                .count();
            closures == 3 && joins == 2 && shared == 1
        }),
        "each shared physical row must satisfy both typed filters"
    );

    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l (a INTEGER,b INTEGER,k INTEGER);
         CREATE TABLE r (a INTEGER,b INTEGER,k INTEGER);
         CREATE TABLE stage (a INTEGER,b INTEGER,k INTEGER);
         INSERT INTO l VALUES (3,4,0),(5,6,1);
         INSERT INTO r VALUES (0,0,0),(0,0,1);
         INSERT INTO stage VALUES (0,0,0),(0,0,1);",
    )
    .expect("shared source example");
    let counts = [
        "SELECT COUNT(*) FROM l JOIN r ON l.k=r.k WHERE l.a BETWEEN 1 AND 9",
        "SELECT COUNT(*) FROM l JOIN stage ON l.k=stage.k WHERE l.b BETWEEN 2 AND 8",
    ]
    .map(|sql| {
        db.query_row(sql, [], |row| row.get::<_, i64>(0))
            .expect("DuckDB count")
    });
    assert_eq!(counts, [2, 2]);
}

#[test]
fn conflicting_overlapping_filtered_join_populations_are_not_fabricated() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l JOIN r ON l.k=r.k WHERE l.a BETWEEN 1 AND 4",
            "SELECT l.a FROM l JOIN stage ON l.k=stage.k WHERE l.a BETWEEN 7 AND 9",
        ],
        "postgresql",
    );
    let plan = physical_joint_row_count_plan(
        &bundle,
        &[(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)],
    );
    assert!(
        !matches!(plan, WitnessDirection::Feasible(_)),
        "one all-TRUE l source cannot simultaneously meet disjoint intervals: {plan:?}"
    );
}

#[test]
fn computed_materialization_inherits_strongest_bounded_producer_domain() {
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a FROM t WHERE a BETWEEN 5 AND 10",
            "CREATE TABLE mart AS SELECT CAST(a AS BIGINT) + 2 AS v FROM stage",
        ],
        "postgresql",
    );
    let ComposedSemantics::Resolved(mart) = bundle.layers()[1].composed_semantics() else {
        panic!("computed materialization must have resolved semantics");
    };
    let value = mart
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == "v")
        .expect("materialized computed output");
    assert_range_bounds(value.domain(), "composed affine output", "7", "12");
    assert_eq!(
        (value.lineage()[0].relation(), value.lineage()[0].column()),
        ("t", "a"),
        "computed output keeps stable physical lineage"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (2),(5),(7),(10),(12),(NULL);
         CREATE TABLE stage AS SELECT a FROM t WHERE a BETWEEN 5 AND 10;
         CREATE TABLE mart AS SELECT CAST(a AS BIGINT) + 2 AS v FROM stage;",
    )
    .expect("materialization");
    let (minimum, maximum): (i64, i64) = db
        .query_row("SELECT MIN(v), MAX(v) FROM mart", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!((minimum, maximum), (7, 12));
}

#[test]
fn affine_composition_does_not_invent_domains_for_unknown_producer_values() {
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a FROM t",
            "CREATE TABLE mart AS SELECT CAST(a AS BIGINT) + 2 AS v FROM stage",
        ],
        "postgresql",
    );
    let ComposedSemantics::Resolved(mart) = bundle.layers()[1].composed_semantics() else {
        panic!("semantic graph resolved");
    };
    let output = mart.output().columns().first().unwrap();
    assert!(
        !matches!(output.domain(), ValueDomain::Ranges(_)),
        "unbounded or unknown producer values cannot certify bounded arithmetic"
    );
}

#[test]
fn identical_group_operator_histograms_are_shared_once_across_terminals() {
    let bundle = analyze_chain(
        &[
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
        ],
        "postgresql",
    );
    let goals = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)];
    let result = physical_joint_row_count_plan(&bundle, &goals);
    let WitnessDirection::Feasible(cases) = result else {
        panic!("identical grouping populations must share one physical input: {result:?}");
    };
    assert!(
        cases.iter().any(|case| {
            case.obligations()
                .iter()
                .filter(|obligation| {
                    matches!(obligation, WitnessObligation::OperatorPopulation { .. })
                })
                .count()
                == 1
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count()
                    == 1
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| matches!(obligation, WitnessObligation::OutputRows { .. }))
                    .count()
                    == 2
        }),
        "one physical grouping histogram supports two output goals"
    );

    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (0),(1);")
        .expect("group inputs");
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) AS n FROM t GROUP BY a)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn distinct_group_keys_share_one_joint_source_assignment() {
    let bundle = analyze_chain(
        &[
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
            "SELECT b, COUNT(*) AS n FROM t GROUP BY b",
        ],
        "postgresql",
    );
    let goal = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)];
    let plan = physical_joint_row_count_plan(&bundle, &goal);
    let WitnessDirection::Feasible(cases) = plan else {
        panic!("two independent distinct group keys can be assigned to one bag: {plan:?}");
    };
    assert!(
        cases.iter().any(|case| {
            case.obligations()
                .iter()
                .filter(|obligation| {
                    matches!(obligation, WitnessObligation::OperatorPopulation { .. })
                })
                .count()
                == 2
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count()
                    == 1
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| matches!(obligation, WitnessObligation::OutputRows { .. }))
                    .count()
                    == 2
        }),
        "both group histograms refer to one physical t relation"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER,b INTEGER);
         INSERT INTO t VALUES (0,1),(1,0);",
    )
    .unwrap();
    let counts = [
        "SELECT COUNT(*) FROM (SELECT a,COUNT(*) AS n FROM t GROUP BY a)",
        "SELECT COUNT(*) FROM (SELECT b,COUNT(*) AS n FROM t GROUP BY b)",
    ]
    .map(|sql| db.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap());
    assert_eq!(counts, [2, 2]);
}

#[test]
fn uncorrelated_nonunit_group_histograms_stay_residual() {
    let bundle = analyze_chain(
        &[
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
            "SELECT b, COUNT(*) AS n FROM t GROUP BY b HAVING COUNT(*) > 1",
        ],
        "postgresql",
    );
    let plan = physical_joint_row_count_plan(
        &bundle,
        &[(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 1)],
    );
    assert!(
        !matches!(plan, WitnessDirection::Feasible(_)),
        "unreconciled independent group multiplicities require correlation proof: {plan:?}"
    );
}

#[test]
fn singleton_group_and_matching_rank_partitions_share_one_source_bag() {
    // QUALIFY is a parser-boundary dialect feature: only dialects whose
    // parser accepts it (here duckdb, mirroring the other physical QUALIFY
    // tests) can reach the shared group/rank proof.
    let bundle = analyze_chain(
        &[
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
            "SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn = 1",
        ],
        "duckdb",
    );
    let goals = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)];
    let plan = physical_joint_row_count_plan(&bundle, &goals);
    let WitnessDirection::Feasible(cases) = &plan else {
        panic!("singleton groups and one-row rank partitions share one bag: {plan:?}");
    };
    assert!(
        cases.iter().any(|case| {
            case.obligations()
                .iter()
                .filter(|obligation| {
                    matches!(obligation, WitnessObligation::OperatorPopulation { .. })
                })
                .count()
                == 2
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count()
                    == 1
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| matches!(obligation, WitnessObligation::OutputRows { .. }))
                    .count()
                    == 2
                && case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::OperatorPopulation {
                            witness: sql_semantic_protocol::OutcomeWitness::Groups {
                                groups: 2,
                                rows_per_group: 1,
                                ..
                            }
                        }
                    )
                })
                && case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::OperatorPopulation {
                            witness: sql_semantic_protocol::OutcomeWitness::Ranked { rows: 2, .. }
                        }
                    )
                })
        }),
        "one physical bag carries both the singleton groups and the rank partitions"
    );
    assert!(
        cases.iter().any(|case| {
            let mut group_key = false;
            let mut rank_partition = false;
            for obligation in case.obligations() {
                match obligation {
                    WitnessObligation::OperatorPopulation {
                        witness: sql_semantic_protocol::OutcomeWitness::Groups { key, .. },
                    } => {
                        group_key = key.relation() == Some("t") && key.name() == "a";
                    }
                    WitnessObligation::OperatorPopulation {
                        witness:
                            sql_semantic_protocol::OutcomeWitness::Ranked {
                                relation,
                                partition_key: Some(partition),
                                order_by,
                                ..
                            },
                    } => {
                        rank_partition = relation == "t"
                            && partition.relation() == Some("t")
                            && partition.name() == "a"
                            && order_by.len() == 1
                            && order_by[0].column().relation() == Some("t")
                            && order_by[0].column().name() == "b";
                    }
                    _ => {}
                }
            }
            group_key && rank_partition
        }),
        "the shared bag numbers the same integer key for grouping and ranking"
    );

    let swapped = physical_joint_row_count_plan(
        &bundle,
        &[(bundle.layers()[1].id(), 2), (bundle.layers()[0].id(), 2)],
    );
    assert_eq!(
        swapped, plan,
        "terminal order must not change the joint proof"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (0,10),(1,20);",
    )
    .unwrap();
    let grouped: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) AS n FROM t GROUP BY a)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let ranked: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn = 1)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((grouped, ranked), (2, 2));
}

#[test]
fn rank_partition_on_different_key_stays_residual_against_singleton_groups() {
    // Same parser boundary as above: duckdb for QUALIFY.
    let bundle = analyze_chain(
        &[
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
            "SELECT b, ROW_NUMBER() OVER (PARTITION BY b ORDER BY a ASC NULLS LAST) AS rn FROM t QUALIFY rn = 1",
        ],
        "duckdb",
    );
    let plan = physical_joint_row_count_plan(
        &bundle,
        &[(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)],
    );
    assert!(
        !matches!(plan, WitnessDirection::Feasible(_)),
        "different rank partitions need their own correlation proof: {plan:?}"
    );
}

#[test]
fn rank_requiring_predecessor_stays_residual_against_singleton_groups() {
    // QUALIFY rn > 1 needs at least two rows per partition, so no
    // singleton-key bag can satisfy both terminals at once. Same parser
    // boundary as above: duckdb for QUALIFY.
    let bundle = analyze_chain(
        &[
            "SELECT a, COUNT(*) AS n FROM t GROUP BY a",
            "SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn > 1",
        ],
        "duckdb",
    );
    let plan = physical_joint_row_count_plan(
        &bundle,
        &[(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)],
    );
    assert!(
        !matches!(plan, WitnessDirection::Feasible(_)),
        "predecessor-requiring ranks must not share the singleton bag: {plan:?}"
    );
}

#[test]
fn join_population_and_group_of_same_key_share_one_complete_source_bag() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l INNER JOIN r ON l.k = r.k",
            "SELECT k, COUNT(*) AS n FROM l GROUP BY k",
        ],
        "postgresql",
    );
    let goals = [(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)];
    let plan = physical_joint_row_count_plan(&bundle, &goals);
    let WitnessDirection::Feasible(cases) = plan else {
        panic!("distinct matched join keys certify exactly two singleton groups: {plan:?}");
    };
    assert!(
        cases.iter().any(|case| {
            case.obligations()
                .iter()
                .filter(|obligation| matches!(obligation, WitnessObligation::ClosedWorld { .. }))
                .count()
                == 2
                && case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinPopulation {
                            closed_world: true,
                            ..
                        }
                    )
                })
                && case.obligations().iter().any(|obligation| {
                    matches!(obligation, WitnessObligation::OperatorPopulation { .. })
                })
                && case
                    .obligations()
                    .iter()
                    .filter(|obligation| matches!(obligation, WitnessObligation::OutputRows { .. }))
                    .count()
                    == 2
        }),
        "both operators share l's DISTINCT key histogram"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(a INTEGER,b INTEGER,k INTEGER);
         CREATE TABLE r(a INTEGER,b INTEGER,k INTEGER);
         INSERT INTO l VALUES (3,4,0),(5,6,1);
         INSERT INTO r VALUES (0,0,0),(0,0,1);",
    )
    .unwrap();
    let counts = [
        "SELECT COUNT(*) FROM l JOIN r ON l.k=r.k",
        "SELECT COUNT(*) FROM (SELECT k,COUNT(*) AS n FROM l GROUP BY k)",
    ]
    .map(|sql| db.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap());
    assert_eq!(counts, [2, 2]);
}

#[test]
fn grouping_a_non_join_key_does_not_inherit_join_key_uniqueness() {
    let bundle = analyze_chain(
        &[
            "SELECT l.a FROM l INNER JOIN r ON l.k = r.k",
            "SELECT b, COUNT(*) AS n FROM l GROUP BY b",
        ],
        "postgresql",
    );
    let result = physical_joint_row_count_plan(
        &bundle,
        &[(bundle.layers()[0].id(), 2), (bundle.layers()[1].id(), 2)],
    );
    assert!(
        !matches!(result, WitnessDirection::Impossible),
        "distinct join keys do not make an independently grouped column necessarily impossible"
    );
}

#[test]
fn composed_affine_projection_preserves_open_interval_boundaries() {
    let bundle = analyze_chain(
        &[
            "CREATE TABLE stage AS SELECT a FROM t WHERE a > 5 AND a < 10",
            "SELECT CAST(a AS BIGINT) + 2 AS v FROM stage",
        ],
        "postgresql",
    );
    let ComposedSemantics::Resolved(composed) = bundle.layers()[1].composed_semantics() else {
        panic!("composed domain");
    };
    let ValueDomain::Ranges(ranges) = composed.output().columns()[0].domain() else {
        panic!("expected narrowed affine interval");
    };
    let [range] = ranges.ranges() else {
        panic!("one affine range");
    };
    let lower = range.lower().expect("lower");
    let upper = range.upper().expect("upper");
    assert_eq!(lower.value().value(), &LiteralValue::Number("7".into()));
    assert_eq!(upper.value().value(), &LiteralValue::Number("12".into()));
    assert!(!lower.inclusive() && !upper.inclusive());
}

#[test]
fn decimal_producer_does_not_gain_integer_affine_certification_from_bounds_alone() {
    let sql = [
        SqlInput::inline("CREATE TABLE stage AS SELECT a FROM t WHERE a BETWEEN 5 AND 10"),
        SqlInput::inline("SELECT CAST(a AS BIGINT) + 2 AS v FROM stage"),
    ];
    let dialect = dialect_from_name("postgresql").unwrap();
    let inputs = sql
        .iter()
        .zip(["stage", "mart"])
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let schema = |relation: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type("a", "DECIMAL(8,2)", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let catalog = RelationCatalog::from_schemas(&[schema("t"), schema("stage")]).unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    let ComposedSemantics::Resolved(mart) = bundle.layers()[1].composed_semantics() else {
        panic!("typed semantic graph");
    };
    assert!(
        !matches!(mart.output().columns()[0].domain(), ValueDomain::Ranges(_)),
        "decimal or fractional producer values cannot be recast as proven whole-integer bounds"
    );
}
