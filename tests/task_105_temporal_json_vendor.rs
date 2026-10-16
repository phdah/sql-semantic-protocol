//! TASK-105: certified temporal literals and auditable opaque-expression boundaries.
mod common;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name, AnalysisBundle,
    BooleanTruthCase, BooleanWitnessDirection, ComposedSemantics, ConfiguredSqlInput, LiteralType,
    LiteralValue, ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SetMode,
    SqlInput, ValueDomain,
};

fn query(sql: &str) -> sql_semantic_protocol::QueryStatement {
    let dialect = dialect_from_name("postgresql").unwrap();
    let protocol = analyze_sql(sql, "postgresql", dialect.as_ref()).unwrap();
    let ProtocolStatement::Query(query) = &protocol.statements()[0] else {
        panic!("SELECT must produce query semantics");
    };
    query.clone()
}

fn assert_single_value(domain: &ValueDomain, kind: LiteralType, value: &str) {
    let ValueDomain::Set(set) = domain else {
        panic!("expected singleton domain, got {domain:?}");
    };
    assert_eq!(set.mode(), SetMode::Include);
    assert_eq!(set.values().len(), 1);
    assert_eq!(set.values()[0].literal_type(), kind);
    assert_eq!(set.values()[0].value(), &LiteralValue::Text(value.into()));
}

fn typed_bundle(
    sql: &str,
    relation: &str,
    columns: &[(&str, &str)],
    dialect_name: &str,
) -> AnalysisBundle {
    let schema = RelationSchema::new(
        relation,
        columns
            .iter()
            .map(|(name, datatype)| {
                SchemaColumn::from_sql_type(*name, datatype, dialect_name).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name(dialect_name).unwrap();
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "task105",
            &SqlInput::inline(sql),
            dialect_name,
            dialect.as_ref(),
        )],
        &catalog,
    )
    .unwrap()
}

fn composed(bundle: &AnalysisBundle) -> &sql_semantic_protocol::ResolvedComposedSemantics {
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("typed source query must resolve");
    };
    semantics
}

#[test]
fn explicit_date_literal_identity_cast_has_an_exact_calendar_domain() {
    for dialect_name in common::DIALECTS {
        let dialect = dialect_from_name(dialect_name).unwrap();
        for cast in ["CAST", "TRY_CAST", "SAFE_CAST"] {
            let sql = format!("SELECT {cast}(DATE '2024-02-29' AS DATE) AS v");
            let protocol = analyze_sql(&sql, dialect_name, dialect.as_ref()).unwrap();
            let ProtocolStatement::Query(query) = &protocol.statements()[0] else {
                panic!("SELECT query expected");
            };
            assert_single_value(
                query.output().columns()[0].domain(),
                LiteralType::Date,
                "2024-02-29",
            );
        }
    }
}

#[test]
fn invalid_or_session_sensitive_temporal_literals_have_unknown_outcomes() {
    for literal in [
        "DATE '2023-02-29'",
        "DATE '1900-02-29'",
        "DATE '2024-04-31'",
        "DATE '0000-01-01'",
        "TIME '24:00:00'",
        "TIME '12:00:60'",
        "TIME '12:00:00+02:00'",
        "TIMESTAMP '2024-02-30 12:00:00'",
        "TIMESTAMP '2024-03-10 02:30:00 America/New_York'",
    ] {
        let sql = format!("SELECT {literal} AS v");
        let analyzed = query(&sql);
        assert!(
            matches!(
                analyzed.output().columns()[0].domain(),
                ValueDomain::Unknown(_)
            ),
            "{sql}: invalid/calendar/session-dependent literal must not be exact"
        );
        assert!(sql_semantic_protocol::to_json(
            &analyze_sql(
                &sql,
                "postgresql",
                dialect_from_name("postgresql").unwrap().as_ref()
            )
            .unwrap()
        )
        .contains("temporal_literal"));
    }
}

#[test]
fn date_literal_calendar_fields_are_folded_to_exact_integer_domains() {
    for (field, expected) in [("YEAR", "2000"), ("MONTH", "2"), ("DAY", "29")] {
        for dialect_name in common::DIALECTS {
            let dialect = dialect_from_name(dialect_name).unwrap();
            let sql = format!("SELECT EXTRACT({field} FROM DATE '2000-02-29') AS v");
            let protocol = analyze_sql(&sql, dialect_name, dialect.as_ref()).unwrap();
            let ProtocolStatement::Query(query) = &protocol.statements()[0] else {
                panic!("SELECT query expected");
            };
            let ValueDomain::Set(set) = query.output().columns()[0].domain() else {
                panic!("{sql}/{dialect_name}: expected exact singleton domain");
            };
            assert_eq!(set.mode(), SetMode::Include);
            assert_eq!(set.values().len(), 1);
            assert_eq!(set.values()[0].literal_type(), LiteralType::Integer);
            assert_eq!(
                set.values()[0].value(),
                &LiteralValue::Number(expected.into())
            );
        }
    }
}

#[test]
fn calendar_bounds_compose_and_contradictions_empty_the_output() {
    let bounded = query(
        "SELECT d FROM t WHERE d >= CAST(DATE '2024-02-28' AS DATE) AND d < DATE '2024-03-01'",
    );
    let ValueDomain::Ranges(ranges) = bounded.output().columns()[0].domain() else {
        panic!("calendar bounds must compose into an interval");
    };
    assert_eq!(ranges.ranges().len(), 1);
    let range = &ranges.ranges()[0];
    assert_eq!(
        range.lower().unwrap().value().value(),
        &LiteralValue::Text("2024-02-28".into())
    );
    assert!(range.lower().unwrap().inclusive());
    assert_eq!(
        range.upper().unwrap().value().value(),
        &LiteralValue::Text("2024-03-01".into())
    );
    assert!(!range.upper().unwrap().inclusive());

    let impossible =
        query("SELECT d FROM t WHERE d >= DATE '2024-03-01' AND d < DATE '2024-02-29'");
    assert_eq!(
        impossible.output().columns()[0].domain(),
        &ValueDomain::Empty
    );
}

#[test]
fn opaque_function_domains_and_predicate_residuals_name_the_function() {
    for function in [
        "MY_UDF",
        "RANDOM",
        "CURRENT_TIMESTAMP",
        "JSON_EXTRACT",
        "DATE_TRUNC",
    ] {
        let sql = format!("SELECT {function}(d) AS v FROM t WHERE {function}(d) = 1");
        let analyzed = query(&sql);
        let ValueDomain::Unknown(reason) = analyzed.output().columns()[0].domain() else {
            panic!("opaque function cannot have an exact outcome: {function}");
        };
        assert!(reason.reason().contains(function), "{function}: {reason:?}");
        let witness = analyzed
            .boolean_witness()
            .expect("computed predicate witness");
        let sql_semantic_protocol::BooleanRowConstraint::Residual { reason } = witness.condition()
        else {
            panic!("opaque function predicate must stay residual: {function}");
        };
        assert!(reason.contains(function), "{function}: {reason}");
    }
}

#[test]
fn unsupported_temporal_and_nested_ast_shapes_expose_their_family() {
    for (expression, feature) in [
        ("d AT TIME ZONE 'UTC'", "temporal_timezone"),
        ("INTERVAL '1 month'", "temporal_interval"),
        ("EXTRACT(WEEK FROM d)", "temporal_extract"),
        ("ARRAY[1, 2]", "nested_literal"),
        ("d[1]", "nested_access"),
        ("JSON 'null'", "json_literal"),
    ] {
        let analyzed = query(&format!("SELECT {expression} AS v FROM t"));
        let column = &analyzed.output().columns()[0];
        let sql_semantic_protocol::Expression::Unsupported(semantic) = column.expression() else {
            panic!("{expression}: unsupported expression must be explicit");
        };
        assert_eq!(semantic.feature(), feature, "{expression}");
        let ValueDomain::Unknown(reason) = column.domain() else {
            panic!("{expression}: unproven outcome must be unknown");
        };
        assert!(
            reason.reason().contains(feature),
            "{expression}: {reason:?}"
        );
    }
}

#[test]
fn inventoried_variants_are_certified_or_fail_closed_in_every_exposed_dialect() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let evidence = &manifest["task_105_expression_evidence"];
    assert_eq!(evidence["owner"], "TASK-105");
    assert_eq!(evidence["release_approved"], false);
    let variants = evidence["variants"].as_array().unwrap();
    let mut identities = std::collections::BTreeSet::new();
    let mut boundary_changes = Vec::new();
    for variant in variants {
        let id = variant["id"].as_str().unwrap();
        assert!(identities.insert(id), "duplicate variant {id}");
        assert!(!variant["family"].as_str().unwrap().is_empty());
        assert!(!variant["ast"].as_str().unwrap().is_empty());
        let expression = variant["expression"].as_str().unwrap();
        // Parentheses ensure MySQL's hash-comment tokenization cannot silently
        // turn a JSON #> fixture into the different query SELECT payload.
        let sql = format!("SELECT ({expression}) AS v FROM t");
        for dialect_name in common::DIALECTS {
            let dialect = dialect_from_name(dialect_name).unwrap();
            let analyzed = analyze_sql(&sql, dialect_name, dialect.as_ref());
            let rejected = variant["parser_rejected"]
                .as_array()
                .unwrap()
                .iter()
                .any(|name| name == dialect_name);
            if analyzed.is_err() != rejected {
                boundary_changes.push(format!(
                    "{id}/{dialect_name}: parser_rejected={} ({:?})",
                    analyzed.is_err(),
                    analyzed.as_ref().err()
                ));
                continue;
            }
            let Ok(protocol) = analyzed else { continue };
            let ProtocolStatement::Query(query) = &protocol.statements()[0] else {
                panic!("SELECT expected for {id}");
            };
            let domain = query.output().columns()[0].domain();
            match variant["outcome"].as_str().unwrap() {
                "unknown" => {
                    assert!(!variant["reason"].as_str().unwrap().is_empty());
                    let ValueDomain::Unknown(reason) = domain else {
                        panic!("{id}/{dialect_name}: opaque outcome became precise: {domain:?}");
                    };
                    assert!(!reason.reason().is_empty());
                }
                kind => {
                    let expected_kind = match kind {
                        "date" => LiteralType::Date,
                        "time" => LiteralType::Time,
                        "timestamp" => LiteralType::Timestamp,
                        "integer" => LiteralType::Integer,
                        _ => panic!("unknown inventory outcome {kind}"),
                    };
                    let value = variant["value"].as_str().unwrap();
                    if expected_kind == LiteralType::Integer {
                        let ValueDomain::Set(set) = domain else {
                            panic!("{id}: expected singleton")
                        };
                        assert_eq!(set.values().len(), 1);
                        assert_eq!(set.values()[0].literal_type(), expected_kind);
                        assert_eq!(set.values()[0].value(), &LiteralValue::Number(value.into()));
                    } else {
                        assert_single_value(domain, expected_kind, value);
                    }
                }
            }
        }
    }
    assert!(
        boundary_changes.is_empty(),
        "inventory parser boundaries changed:\n{}",
        boundary_changes.join("\n")
    );
}

#[test]
fn temporal_and_nested_sql_null_tests_have_exact_or_impossible_truth_directions() {
    for datatype in [
        "DATE",
        "TIME",
        "TIMESTAMP WITHOUT TIME ZONE",
        "JSON",
        "BIGINT[]",
        "STRUCT(a BIGINT)",
    ] {
        for (predicate, qualifying, rejected, empty) in [
            (
                "payload IS DISTINCT FROM NULL",
                BooleanWitnessDirection::Exact(BooleanTruthCase::True),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue),
                false,
            ),
            (
                "payload IS NULL OR payload IS NOT NULL",
                BooleanWitnessDirection::Exact(BooleanTruthCase::True),
                BooleanWitnessDirection::Impossible,
                false,
            ),
            (
                "payload IS NULL AND payload IS NOT NULL",
                BooleanWitnessDirection::Impossible,
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue),
                true,
            ),
            (
                "payload = NULL",
                BooleanWitnessDirection::Impossible,
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue),
                true,
            ),
        ] {
            let bundle = typed_bundle(
                &format!("SELECT payload FROM t WHERE {predicate}"),
                "t",
                &[("payload", datatype)],
                "duckdb",
            );
            let semantics = composed(&bundle);
            let witness = semantics.boolean_witnesses()[0].witness();
            assert_eq!(witness.qualifying(), &qualifying, "{datatype}: {predicate}");
            assert_eq!(witness.rejected(), &rejected, "{datatype}: {predicate}");
            let domain = semantics.output().columns()[0].domain();
            if empty {
                assert_eq!(domain, &ValueDomain::Empty, "{datatype}: {predicate}");
            } else if predicate == "payload IS DISTINCT FROM NULL" {
                let ValueDomain::Set(set) = domain else {
                    panic!("NULL exclusion required")
                };
                assert_eq!(set.mode(), SetMode::Exclude);
                assert_eq!(set.values().len(), 1);
                assert_eq!(set.values()[0].value(), &LiteralValue::Null);
            } else {
                assert_eq!(domain, &ValueDomain::Unbounded);
            }
        }
    }
}

#[test]
fn nested_comparisons_and_path_null_tests_stay_residual_despite_schema_evidence() {
    for (datatype, predicate) in [
        ("JSON", "payload = payload"),
        ("BIGINT[]", "payload = payload"),
        ("STRUCT(a BIGINT)", "payload = payload"),
        ("JSON", "JSON_EXTRACT(payload, '$.missing') IS NULL"),
        ("JSON", "(payload -> 'a') IS NULL"),
        ("JSON", "payload = JSON 'null'"),
    ] {
        let bundle = typed_bundle(
            &format!("SELECT payload FROM t WHERE {predicate}"),
            "t",
            &[("payload", datatype)],
            "duckdb",
        );
        let semantics = composed(&bundle);
        assert!(
            !semantics.condition_exactness().is_exact(),
            "{datatype}: {predicate}"
        );
        assert!(!semantics
            .condition_exactness()
            .residual_conditions()
            .is_empty());
        if let Some(witness) = semantics.boolean_witnesses().first() {
            assert!(matches!(
                witness.witness().qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ));
            assert!(matches!(
                witness.witness().rejected(),
                BooleanWitnessDirection::Residual { .. }
            ));
        }
        assert!(
            matches!(
                semantics.output().columns()[0].domain(),
                ValueDomain::Unbounded | ValueDomain::Unknown(_)
            ),
            "{datatype}: {predicate}"
        );
    }
}

#[test]
fn duckdb_calendar_oracle_matches_literal_domains_at_century_boundaries() {
    let db = duckdb::Connection::open_in_memory().unwrap();
    for date in ["1900-02-28", "2000-02-29", "2024-02-29", "2100-03-01"] {
        for expression in [
            format!("CAST(DATE '{date}' AS DATE)"),
            format!("TRY_CAST(DATE '{date}' AS DATE)"),
            format!("EXTRACT(YEAR FROM DATE '{date}')"),
            format!("EXTRACT(MONTH FROM DATE '{date}')"),
            format!("EXTRACT(DAY FROM DATE '{date}')"),
        ] {
            let analyzed = query(&format!("SELECT {expression} AS v"));
            let ValueDomain::Set(set) = analyzed.output().columns()[0].domain() else {
                panic!("certified literal expression needs a singleton: {expression}");
            };
            assert_eq!(set.values().len(), 1);
            let expected = match set.values()[0].value() {
                LiteralValue::Text(value) | LiteralValue::Number(value) => value,
                other => panic!("unexpected value: {other:?}"),
            };
            let actual: String = db
                .query_row(
                    &format!("SELECT CAST({expression} AS VARCHAR)"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(&actual, expected, "{expression}");
        }
    }
}

fn date_domain_selects(domain: &ValueDomain, date: Option<&str>) -> bool {
    match domain {
        ValueDomain::Empty => false,
        ValueDomain::Ranges(ranges) => date.is_some_and(|date| {
            ranges.ranges().iter().any(|range| {
                let bound_matches = |bound: Option<&sql_semantic_protocol::Bound>, lower: bool| {
                    bound.is_none_or(|bound| {
                        assert_eq!(bound.value().literal_type(), LiteralType::Date);
                        let LiteralValue::Text(value) = bound.value().value() else {
                            panic!("date bound")
                        };
                        let ordering = date.cmp(value.as_str());
                        (bound.inclusive() && ordering.is_eq())
                            || if lower {
                                ordering.is_gt()
                            } else {
                                ordering.is_lt()
                            }
                    })
                };
                bound_matches(range.lower(), true) && bound_matches(range.upper(), false)
            })
        }),
        other => panic!("oracle fixture expects a date interval, got {other:?}"),
    }
}

#[test]
fn duckdb_three_valued_calendar_filter_oracle_checks_domains_and_impossibility() {
    let db = duckdb::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(d DATE); INSERT INTO t VALUES (NULL), ('2024-02-27'), ('2024-02-28'), ('2024-02-29'), ('2024-02-29'), ('2024-03-01');").unwrap();
    for predicate in [
        "d >= CAST(DATE '2024-02-28' AS DATE) AND d < DATE '2024-03-01'",
        "d >= DATE '2024-03-01' AND d < DATE '2024-02-29'",
        "d >= DATE '2024-02-29' AND d <= DATE '2024-02-29'",
        "NOT (d < DATE '2024-02-29')",
    ] {
        let sql = format!("SELECT d FROM t WHERE {predicate}");
        let bundle = typed_bundle(&sql, "t", &[("d", "DATE")], "duckdb");
        let semantics = composed(&bundle);
        assert!(semantics.condition_exactness().is_exact(), "{predicate}");
        let domain = semantics.output().columns()[0].domain();
        let mut statement = db
            .prepare(&format!("SELECT CAST(d AS VARCHAR), ({predicate}) FROM t"))
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<bool>>(1)?,
                ))
            })
            .unwrap();
        for row in rows {
            let (date, truth) = row.unwrap();
            let qualifying = date_domain_selects(domain, date.as_deref());
            assert_eq!(qualifying, truth == Some(true), "{predicate}: {date:?}");
            assert_eq!(
                !qualifying,
                truth != Some(true),
                "NOT TRUE: {predicate}: {date:?}"
            );
        }
    }
}

#[test]
fn duckdb_json_oracle_keeps_document_null_and_missing_path_separate_from_sql_null() {
    let db = duckdb::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(payload JSON); INSERT INTO t VALUES (NULL), ('null'), ('{}'), ('{\"a\":null}'), ('{\"a\":1}');").unwrap();
    let actual: (i64, i64, i64) = db.query_row(
        "SELECT COUNT(*) FILTER (WHERE payload IS NULL), COUNT(*) FILTER (WHERE (payload -> 'a') IS NULL), COUNT(*) FILTER (WHERE payload = JSON 'null') FROM t",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(actual, (1, 3, 1));
    let bundle = typed_bundle(
        "SELECT payload FROM t WHERE payload IS DISTINCT FROM NULL",
        "t",
        &[("payload", "JSON")],
        "duckdb",
    );
    let witness = composed(&bundle).boolean_witnesses()[0].witness();
    assert_eq!(
        witness.qualifying(),
        &BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    );
    assert_eq!(
        witness.rejected(),
        &BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    );
    for predicate in ["(payload -> 'a') IS NULL", "payload = JSON 'null'"] {
        let bundle = typed_bundle(
            &format!("SELECT payload FROM t WHERE {predicate}"),
            "t",
            &[("payload", "JSON")],
            "duckdb",
        );
        assert!(
            !composed(&bundle).condition_exactness().is_exact(),
            "{predicate}"
        );
    }
}

#[test]
fn certified_calendar_domains_survive_local_relations_and_producer_composition() {
    for sql in [
        "WITH c AS (SELECT CAST(DATE '2000-02-29' AS DATE) AS v) SELECT v FROM c",
        "SELECT v FROM (SELECT CAST(DATE '2000-02-29' AS DATE) AS v) c",
    ] {
        assert_single_value(
            query(sql).output().columns()[0].domain(),
            LiteralType::Date,
            "2000-02-29",
        );
    }
    let bundle = typed_bundle(
        "CREATE VIEW calendar AS SELECT d FROM t WHERE d >= DATE '2024-02-28'; SELECT d FROM calendar WHERE d < DATE '2024-03-01'",
        "t", &[("d", "DATE")], "duckdb",
    );
    let ComposedSemantics::Resolved(terminal) = bundle.layers()[1].composed_semantics() else {
        panic!("terminal must resolve")
    };
    let ValueDomain::Ranges(ranges) = terminal.output().columns()[0].domain() else {
        panic!("composed interval required")
    };
    assert_eq!(ranges.ranges().len(), 1);
    let range = &ranges.ranges()[0];
    assert_eq!(
        range.lower().unwrap().value().value(),
        &LiteralValue::Text("2024-02-28".into())
    );
    assert!(range.lower().unwrap().inclusive());
    assert_eq!(
        range.upper().unwrap().value().value(),
        &LiteralValue::Text("2024-03-01".into())
    );
    assert!(!range.upper().unwrap().inclusive());
}

#[test]
fn local_calendar_domain_inheritance_does_not_cross_a_null_extending_join() {
    let sql = "SELECT c.v FROM (SELECT 1 AS k) t LEFT JOIN (SELECT CAST(DATE '2000-02-29' AS DATE) AS v) c ON 1 = 0";
    let analyzed = query(sql);
    assert!(matches!(
        analyzed.output().columns()[0].domain(),
        ValueDomain::Unbounded | ValueDomain::Unknown(_)
    ));
    let db = duckdb::Connection::open_in_memory().unwrap();
    let value: Option<String> = db
        .query_row(
            &format!("SELECT CAST(v AS VARCHAR) FROM ({sql}) q"),
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        value, None,
        "an unmatched join introduces SQL NULL, not the local constant"
    );
}

#[test]
fn local_calendar_domain_inheritance_does_not_hide_rollup_nulls() {
    let analyzed = query("SELECT v FROM (SELECT DATE '2000-02-29' AS v) c GROUP BY ROLLUP(v)");
    assert!(matches!(
        analyzed.output().columns()[0].domain(),
        ValueDomain::Unbounded | ValueDomain::Unknown(_)
    ));
}

#[test]
fn dbt_catalog_and_manifest_schema_evidence_match_direct_temporal_and_json_outcomes() {
    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_dbt_artifacts, analyze_dbt_manifest_with_schemas, parse_dbt_catalog,
        parse_dbt_manifest,
    };
    for predicate in [
        "d >= CAST(DATE '2024-02-28' AS DATE) AND d < DATE '2024-03-01'",
        "d >= DATE '2024-03-01' AND d < DATE '2024-02-29'",
        "ts >= TIMESTAMP '2024-02-29 00:00:00'",
        "payload IS DISTINCT FROM NULL",
        "JSON_EXTRACT(payload, '$.missing') IS NULL",
        "MY_UDF(d) = 1",
    ] {
        let sql = format!("SELECT d, ts, payload, EXTRACT(YEAR FROM DATE '2000-02-29') AS y, MY_UDF(payload) AS opaque FROM warehouse.raw.events WHERE {predicate}");
        let columns = [
            ("d", "DATE"),
            ("ts", "TIMESTAMP WITHOUT TIME ZONE"),
            ("payload", "JSON"),
        ];
        let declared: serde_json::Map<_, _> = columns
            .iter()
            .map(|(name, datatype)| ((*name).into(), json!({"name":name,"data_type":datatype})))
            .collect();
        let manifest = json!({
            "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json","adapter_type":"postgres"},
            "nodes":{"model.demo.calendar":{
                "unique_id":"model.demo.calendar", "resource_type":"model", "language":"sql",
                "relation_name":"warehouse.analytics.calendar", "compiled_code":sql,
                "depends_on":{"nodes":["source.demo.events"]}
            }},
            "sources":{"source.demo.events":{
                "unique_id":"source.demo.events", "resource_type":"source",
                "relation_name":"warehouse.raw.events", "columns":declared
            }}
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let catalog_columns: serde_json::Map<_, _> = columns
            .iter()
            .enumerate()
            .map(|(index, (name, datatype))| {
                (
                    (*name).into(),
                    json!({"name":name,"type":datatype,"index":index+1}),
                )
            })
            .collect();
        let catalog = json!({
            "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json"},
            "nodes":{}, "sources":{"source.demo.events":{
                "unique_id":"source.demo.events", "metadata":{"type":"BASE TABLE","database":"warehouse","schema":"raw","name":"events"},
                "columns":catalog_columns
            }}
        });
        let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
        let dialect = dialect_from_name("postgresql").unwrap();
        let direct = typed_bundle(&sql, "warehouse.raw.events", &columns, "postgresql");
        for adapted in [
            analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap(),
            analyze_dbt_manifest_with_schemas(&manifest, "postgresql", dialect.as_ref()).unwrap(),
        ] {
            let expected = composed(&direct);
            let actual = composed(&adapted);
            assert_eq!(
                actual.output(),
                expected.output(),
                "output parity: {predicate}"
            );
            assert_eq!(
                actual.column_domains(),
                expected.column_domains(),
                "domain parity: {predicate}"
            );
            assert_eq!(
                actual.condition_exactness(),
                expected.condition_exactness(),
                "exactness parity: {predicate}"
            );
            let actual_witnesses: Vec<_> = actual
                .boolean_witnesses()
                .iter()
                .map(|item| item.witness())
                .collect();
            let expected_witnesses: Vec<_> = expected
                .boolean_witnesses()
                .iter()
                .map(|item| item.witness())
                .collect();
            assert_eq!(
                actual_witnesses, expected_witnesses,
                "truth-direction parity: {predicate}"
            );
        }
    }
}

#[test]
fn certified_calendar_expressions_emit_the_full_existing_literal_contract() {
    for dialect_name in common::DIALECTS {
        let dialect = dialect_from_name(dialect_name).unwrap();
        for (computed, canonical) in [
            (
                "SELECT CAST(DATE '2000-02-29' AS DATE) AS v",
                "SELECT DATE '2000-02-29' AS v",
            ),
            (
                "SELECT EXTRACT(YEAR FROM DATE '2000-02-29') AS v",
                "SELECT 2000 AS v",
            ),
        ] {
            let computed = analyze_sql(computed, dialect_name, dialect.as_ref()).unwrap();
            let canonical = analyze_sql(canonical, dialect_name, dialect.as_ref()).unwrap();
            assert_eq!(
                sql_semantic_protocol::to_json(&computed),
                sql_semantic_protocol::to_json(&canonical),
                "full canonical document differs for {dialect_name}"
            );
        }
        for datatype in ["DATE", "TIME", "JSON"] {
            let bundle = typed_bundle(
                "SELECT payload FROM t WHERE payload IS DISTINCT FROM NULL",
                "t",
                &[("payload", datatype)],
                dialect_name,
            );
            let semantics = composed(&bundle);
            assert_eq!(
                semantics.boolean_witnesses()[0].witness().qualifying(),
                &BooleanWitnessDirection::Exact(BooleanTruthCase::True)
            );
            assert_eq!(
                semantics.boolean_witnesses()[0].witness().rejected(),
                &BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            );
            assert!(semantics.condition_exactness().is_exact());
        }
    }
}

#[test]
fn timestamp_comparison_requires_zone_evidence_and_keeps_typed_bounds() {
    use sql_semantic_protocol::{ComparisonAssumption, ConditionExactnessStatus};
    for dialect_name in common::DIALECTS {
        for (datatype, literal, expected) in [
            (
                "TIMESTAMP WITHOUT TIME ZONE",
                "2024-02-29 00:00:00",
                ConditionExactnessStatus::Exact,
            ),
            (
                "TIMESTAMP WITH TIME ZONE",
                "2024-02-29 00:00:00+00:00",
                ConditionExactnessStatus::Exact,
            ),
            (
                "TIMESTAMP WITH TIME ZONE",
                "2024-02-29 00:00:00",
                ConditionExactnessStatus::Conditional,
            ),
            (
                "TIMESTAMP",
                "2024-02-29 00:00:00",
                ConditionExactnessStatus::Conditional,
            ),
        ] {
            let mut bundle = typed_bundle(
                &format!("SELECT ts FROM t WHERE ts >= TIMESTAMP '{literal}'"),
                "t",
                &[("ts", datatype)],
                dialect_name,
            );
            let semantics = composed(&bundle);
            assert_eq!(
                semantics.condition_exactness().status(),
                expected,
                "{datatype}/{dialect_name}"
            );
            let ValueDomain::Ranges(ranges) = semantics.output().columns()[0].domain() else {
                panic!("typed timestamp bound required")
            };
            assert_eq!(ranges.ranges().len(), 1);
            assert!(ranges.ranges()[0].lower().unwrap().inclusive());
            assert_eq!(
                ranges.ranges()[0].lower().unwrap().value().literal_type(),
                LiteralType::Timestamp
            );
            assert_eq!(
                ranges.ranges()[0].lower().unwrap().value().value(),
                &LiteralValue::Text(literal.into())
            );
            if expected == ConditionExactnessStatus::Conditional {
                assert_eq!(
                    semantics.condition_exactness().required_assumptions()[0].assumption(),
                    ComparisonAssumption::SessionTimeZone
                );
                bundle.declare_comparison_assumptions(&[ComparisonAssumption::SessionTimeZone]);
                assert!(composed(&bundle).condition_exactness().is_exact());
            }
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_physical_type_evidence_uses_the_same_calendar_zone_and_json_boundaries() {
    let relation_catalog = RelationCatalog::new(&["t"]).unwrap();
    let metadata = sql_semantic_protocol::parse_odcs_yaml(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task105\nversion: 1.0.0\nschema:\n  - name: t\n    physicalName: t\n    properties:\n      - name: d\n        physicalType: DATE\n      - name: ts\n        physicalType: TIMESTAMP WITHOUT TIME ZONE\n      - name: payload\n        physicalType: JSON\n",
        "postgresql", &relation_catalog,
    ).unwrap();
    let catalog = RelationCatalog::from_schemas(metadata.source_schemas()).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    for predicate in [
        "d >= CAST(DATE '2024-02-29' AS DATE)",
        "ts >= TIMESTAMP '2024-02-29 00:00:00'",
        "payload IS DISTINCT FROM NULL",
        "JSON_EXTRACT(payload, '$.a') IS NULL",
    ] {
        let sql = format!("SELECT d, ts, payload FROM t WHERE {predicate}");
        let from_odcs = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task105",
                &SqlInput::inline(&sql),
                "postgresql",
                dialect.as_ref(),
            )],
            &catalog,
        )
        .unwrap();
        let direct = typed_bundle(
            &sql,
            "t",
            &[
                ("d", "DATE"),
                ("ts", "TIMESTAMP WITHOUT TIME ZONE"),
                ("payload", "JSON"),
            ],
            "postgresql",
        );
        assert_eq!(composed(&from_odcs).output(), composed(&direct).output());
        assert_eq!(
            composed(&from_odcs).condition_exactness(),
            composed(&direct).condition_exactness()
        );
        assert_eq!(
            composed(&from_odcs).column_domains(),
            composed(&direct).column_domains()
        );
    }
}
