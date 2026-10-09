use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, to_bundle_json, ComposedSemantics, CompositionFailureReason, LiteralValue,
    MergeAction, MergeMatchKind, ProtocolStatement, RelationResolution, SqlInput, ValueDomain,
    WriteEffectAction, WriteIdempotence, WriteKind, WritePostState, WriteUncertainty,
};
use sqlparser::dialect::{GenericDialect, MySqlDialect, SnowflakeDialect};

#[test]
fn insert_select_records_append_semantics_and_partial_downstream_link() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "INSERT INTO stage.orders (id, amount)
                 SELECT id, amount FROM raw.orders WHERE amount > 10",
            ),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id, amount FROM stage.orders"),
        ],
        "generic",
        &dialect,
    )
    .expect("INSERT-select chain should analyze");

    let insert_layer = &bundle.layers()[0];
    assert_eq!(insert_layer.write_kind(), Some(WriteKind::Append));
    assert_eq!(insert_layer.consumes(), &["raw.orders".to_string()]);
    assert_eq!(
        insert_layer.produces()[0].relation_name(),
        Some("stage.orders")
    );

    let ProtocolStatement::Query(insert) = &bundle.inputs()[0].statements()[0] else {
        panic!("INSERT-select should expose source-query semantics");
    };
    let write = insert.write().expect("INSERT should carry write semantics");
    assert_eq!(write.kind(), WriteKind::Append);
    assert_eq!(write.target(), "stage.orders");
    assert_eq!(
        write.target_columns(),
        &["id".to_string(), "amount".to_string()]
    );
    assert_eq!(insert.dependencies(), &["raw.orders".to_string()]);
    assert_eq!(insert.output().columns().len(), 2);

    let insert_composed = match insert_layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics,
        other => panic!("inserted rows should compose through their source query: {other:?}"),
    };
    assert_eq!(insert_composed.dependencies(), &["raw.orders".to_string()]);
    assert_eq!(
        insert_composed.output().columns()[0].lineage()[0].relation(),
        "raw.orders"
    );

    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.relation() == "stage.orders")
        .expect("downstream reader should link to the INSERT writer");
    assert_eq!(edge.resolution(), RelationResolution::Partial);
    assert_eq!(edge.producer_layer_ids(), &[insert_layer.id().to_string()]);

    match bundle.layers()[1].composed_semantics() {
        ComposedSemantics::Unresolved(semantics) => {
            assert_eq!(
                semantics.reason(),
                CompositionFailureReason::PartialProducer
            );
            assert!(semantics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "partial_relation_producer"));
        }
        other => panic!("downstream full-relation semantics must remain partial: {other:?}"),
    }
}

#[test]
fn merge_records_condition_actions_and_source_dependencies() {
    let dialect = SnowflakeDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "MERGE INTO dim.customers AS t
                 USING stage.customers AS s
                 ON t.id = s.id
                 WHEN MATCHED AND s.active = FALSE THEN DELETE
                 WHEN MATCHED AND s.id BETWEEN 10 AND 20 THEN UPDATE SET id = t.id
                 WHEN NOT MATCHED THEN INSERT (id, name) VALUES (s.id, s.name)",
            ),
            SqlInput::inline("CREATE TABLE mart.customers AS SELECT id FROM dim.customers"),
        ],
        "snowflake",
        &dialect,
    )
    .expect("MERGE chain should analyze");

    let merge_layer = &bundle.layers()[0];
    assert_eq!(
        merge_layer.write_kind(),
        Some(WriteKind::ConditionalMutation)
    );
    assert_eq!(merge_layer.consumes(), &["stage.customers".to_string()]);
    assert_eq!(
        merge_layer.produces()[0].relation_name(),
        Some("dim.customers")
    );

    let ProtocolStatement::Query(merge) = &bundle.inputs()[0].statements()[0] else {
        panic!("MERGE should expose normalized write semantics");
    };
    let write = merge.write().expect("MERGE should carry write semantics");
    assert_eq!(write.kind(), WriteKind::ConditionalMutation);
    assert_eq!(write.target(), "dim.customers");
    assert!(write.match_condition().is_some());
    assert_eq!(write.merge_clauses().len(), 3);

    assert_eq!(
        write.merge_clauses()[0].match_kind(),
        MergeMatchKind::Matched
    );
    assert!(write.merge_clauses()[0].predicate().is_some());
    assert!(matches!(
        write.merge_clauses()[0].action(),
        MergeAction::Delete
    ));
    assert!(matches!(
        write.merge_clauses()[1].action(),
        MergeAction::Update { assignments } if assignments.len() == 1
    ));
    let MergeAction::Update { assignments } = write.merge_clauses()[1].action() else {
        panic!("second MERGE clause should update");
    };
    let ValueDomain::Ranges(ranges) = assignments[0].value().domain() else {
        panic!("matched write should expose the interval implied by MERGE conditions");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one matched write interval");
    };
    let lower = range.lower().expect("matched interval lower bound");
    let upper = range.upper().expect("matched interval upper bound");
    assert_eq!(
        lower.value().value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(lower.inclusive());
    assert_eq!(
        upper.value().value(),
        &LiteralValue::Number("20".to_string())
    );
    assert!(upper.inclusive());
    assert!(matches!(
        write.merge_clauses()[2].action(),
        MergeAction::Insert { columns, values }
            if columns == &["id".to_string(), "name".to_string()]
                && values.len() == 1
                && values[0].len() == 2
    ));
    assert_eq!(merge.dependencies(), &["stage.customers".to_string()]);

    match merge_layer.composed_semantics() {
        ComposedSemantics::Unresolved(semantics) => {
            assert_eq!(
                semantics.reason(),
                CompositionFailureReason::PartialProducer
            );
        }
        other => panic!("MERGE cannot define complete target semantics: {other:?}"),
    }

    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.relation() == "dim.customers")
        .expect("downstream reader should link to the MERGE writer");
    assert_eq!(edge.resolution(), RelationResolution::Partial);
}

#[test]
fn unsupported_insert_write_forms_remain_explicit() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline("INSERT INTO target VALUES (1)")],
        "generic",
        &dialect,
    )
    .expect("unsupported INSERT form should remain analyzable");

    assert!(bundle.layers().is_empty());
    let ProtocolStatement::Unsupported(statement) = &bundle.inputs()[0].statements()[0] else {
        panic!("INSERT VALUES should not be treated as INSERT-select");
    };
    assert_eq!(statement.category(), "insert");
    assert_eq!(
        statement.diagnostics()[0].code(),
        "unsupported_insert_source"
    );
}

#[test]
fn standalone_updates_preserve_before_after_obligations_and_written_domains() {
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "UPDATE accounts SET balance = 9 WHERE id = 1",
        )],
        "generic",
        &GenericDialect {},
    )
    .expect("UPDATE should analyze");
    assert_eq!(bundle.layers()[0].write_kind(), Some(WriteKind::Update));
    let ProtocolStatement::Query(statement) = &bundle.inputs()[0].statements()[0] else {
        panic!("expected normalized UPDATE");
    };
    let write = statement.write().expect("write");
    assert_eq!(write.target(), "accounts");
    assert!(write.selection().is_some());
    assert_eq!(write.assignments().len(), 1);
    let effect = write.state_effect().expect("partial state obligation");
    assert_eq!(effect.post_state(), WritePostState::ApplyToInitial);
    assert_eq!(effect.idempotence(), WriteIdempotence::Unproven);
    assert_eq!(effect.affected_rows().minimum(), 0);
    assert_eq!(effect.affected_rows().maximum(), None);
    assert!(effect
        .reasons()
        .contains(&WriteUncertainty::PredicateExactnessUnverified));
    assert!(matches!(
        effect.branches()[0].action(),
        WriteEffectAction::Mutation(MergeAction::Update { .. })
    ));
    let emitted: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    assert_eq!(
        emitted["inputs"][0]["statements"][0]["write"]["state_effect"]["post_state"],
        "apply_to_initial"
    );
    assert_eq!(
        emitted["inputs"][0]["statements"][0]["write"]["state_effect"]["affected_rows"]["minimum"],
        0
    );
    assert_eq!(
        emitted["inputs"][0]["statements"][0]["write"]["state_effect"]["affected_rows"]["maximum"],
        serde_json::Value::Null
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE accounts(id BIGINT PRIMARY KEY, balance BIGINT);
         INSERT INTO accounts VALUES (1, 3), (2, NULL);
         UPDATE accounts SET balance = 9 WHERE id = 1;",
    )
    .unwrap();
    let first: i64 = db
        .query_row("SELECT balance FROM accounts WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let untouched: Option<i64> = db
        .query_row("SELECT balance FROM accounts WHERE id = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(first, 9);
    assert_eq!(untouched, None);
}

#[test]
fn conditional_delete_preserves_unmatched_rows_and_unconditional_delete_is_idempotent() {
    let selected = analyze_inputs(
        &[SqlInput::inline(
            "DELETE FROM accounts WHERE balance IS NULL",
        )],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let ProtocolStatement::Query(selected_query) = &selected.inputs()[0].statements()[0] else {
        panic!("expected DELETE");
    };
    let write = selected_query.write().unwrap();
    assert_eq!(write.kind(), WriteKind::Delete);
    assert_eq!(
        write.state_effect().unwrap().post_state(),
        WritePostState::ApplyToInitial
    );

    let all = analyze_inputs(
        &[SqlInput::inline("DELETE FROM accounts")],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let ProtocolStatement::Query(all_query) = &all.inputs()[0].statements()[0] else {
        panic!("expected DELETE");
    };
    let effect = all_query.write().unwrap().state_effect().unwrap();
    assert_eq!(effect.post_state(), WritePostState::Empty);
    assert_eq!(effect.idempotence(), WriteIdempotence::Proven);
    assert_eq!(all.layers()[0].write_kind(), Some(WriteKind::Delete));
    assert!(matches!(
        all.layers()[0].composed_semantics(),
        ComposedSemantics::Unresolved(_)
    ));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE accounts(id BIGINT, balance BIGINT);
        INSERT INTO accounts VALUES (1, 1), (2, NULL);
        DELETE FROM accounts WHERE balance IS NULL;",
    )
    .unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
    db.execute_batch("DELETE FROM accounts; DELETE FROM accounts;")
        .unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn insert_select_contract_does_not_invent_initial_rows_or_key_compatibility() {
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "INSERT INTO target (id, score) SELECT id, score FROM upstream",
        )],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
        panic!("expected INSERT SELECT");
    };
    let effect = query.write().unwrap().state_effect().unwrap();
    assert_eq!(effect.post_state(), WritePostState::ApplyToInitial);
    assert_eq!(effect.idempotence(), WriteIdempotence::Unproven);
    assert!(effect
        .reasons()
        .contains(&WriteUncertainty::ConstraintConflictsUnverified));
    assert!(matches!(
        effect.branches()[0].action(),
        WriteEffectAction::InsertQuery
    ));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id BIGINT PRIMARY KEY, score BIGINT);
        CREATE TABLE upstream(id BIGINT, score BIGINT);
        INSERT INTO target VALUES (1, 4);
        INSERT INTO upstream VALUES (1, 10);",
    )
    .unwrap();
    assert!(db
        .execute_batch("INSERT INTO target SELECT id, score FROM upstream")
        .is_err());
    let score: i64 = db
        .query_row("SELECT score FROM target WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(score, 4);
}

#[test]
fn merge_retains_ordered_matched_and_unmatched_effects_without_finalstate_proof() {
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "MERGE INTO target AS t USING upstream AS s ON t.id = s.id
             WHEN MATCHED THEN UPDATE SET score = s.score
             WHEN NOT MATCHED THEN INSERT (id, score) VALUES (s.id, s.score)",
        )],
        "snowflake",
        &SnowflakeDialect {},
    )
    .unwrap();
    let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
        panic!("expected MERGE");
    };
    let effect = query.write().unwrap().state_effect().unwrap();
    assert_eq!(effect.branches().len(), 2);
    assert_eq!(
        effect.branches()[0].match_kind(),
        Some(MergeMatchKind::Matched)
    );
    assert_eq!(
        effect.branches()[1].match_kind(),
        Some(MergeMatchKind::NotMatched)
    );
    assert!(effect
        .reasons()
        .contains(&WriteUncertainty::MatchMultiplicityUnknown));
    assert_eq!(effect.post_state(), WritePostState::ApplyToInitial);
    assert!(matches!(
        bundle.layers()[0].composed_semantics(),
        ComposedSemantics::Unresolved(_)
    ));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id BIGINT PRIMARY KEY, score BIGINT);
         CREATE TABLE upstream(id BIGINT, score BIGINT);
         INSERT INTO target VALUES (1, 1), (3, 3);
         INSERT INTO upstream VALUES (1, 9), (2, 2);
         MERGE INTO target AS t USING upstream AS s ON t.id = s.id
         WHEN MATCHED THEN UPDATE SET score = s.score
         WHEN NOT MATCHED THEN INSERT (id, score) VALUES (s.id, s.score);",
    )
    .unwrap();
    let rows: i64 = db
        .query_row("SELECT COUNT(*) FROM target", [], |row| row.get(0))
        .unwrap();
    let updated: i64 = db
        .query_row("SELECT score FROM target WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let untouched: i64 = db
        .query_row("SELECT score FROM target WHERE id = 3", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!((rows, updated, untouched), (3, 9, 3));
}

#[test]
fn unsupported_multi_relation_writes_remain_explicit() {
    let cases = [
        (
            "UPDATE target SET score = src.score FROM src WHERE target.id = src.id",
            "update",
        ),
        (
            "DELETE FROM target USING src WHERE target.id = src.id",
            "delete",
        ),
    ];
    for (sql, kind) in cases {
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], "generic", &GenericDialect {})
            .expect("parser-supported DML");
        assert!(
            bundle.layers().is_empty(),
            "multi-table mutation must not become a known effect: {sql}"
        );
        let ProtocolStatement::Unsupported(statement) = &bundle.inputs()[0].statements()[0] else {
            panic!("unproven write must be unsupported: {sql}");
        };
        assert_eq!(statement.category(), kind);
    }
}

#[test]
fn update_delete_subqueries_do_not_silently_omit_external_dependencies() {
    let cases = [
        (
            "UPDATE target SET score = (SELECT MAX(score) FROM external_data)",
            "update",
            "unsupported_update_subquery",
        ),
        (
            "DELETE FROM target WHERE id IN (SELECT id FROM external_data)",
            "delete",
            "unsupported_delete_subquery",
        ),
    ];
    for (sql, category, code) in cases {
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], "generic", &GenericDialect {})
            .expect("valid SQL syntax");
        assert!(bundle.layers().is_empty());
        let ProtocolStatement::Unsupported(statement) = &bundle.inputs()[0].statements()[0] else {
            panic!("subquery DML must not assert complete target effects: {sql}");
        };
        assert_eq!(statement.category(), category);
        assert_eq!(statement.diagnostics()[0].code(), code);
    }
}

#[test]
fn partition_scoped_update_delete_cannot_claim_full_target_state() {
    let cases = [
        (
            "DELETE FROM orders PARTITION (p0)",
            "delete",
            "unsupported_delete_target",
        ),
        (
            "DELETE FROM orders PARTITION (p0) WHERE id = 1",
            "delete",
            "unsupported_delete_target",
        ),
        (
            "UPDATE orders PARTITION (p0) SET balance = 7",
            "update",
            "unsupported_update_target",
        ),
        (
            "UPDATE orders PARTITION (p0) SET balance = 7 WHERE id = 1",
            "update",
            "unsupported_update_target",
        ),
    ];

    for (dialect_name, dialect) in [
        (
            "mysql",
            &MySqlDialect {} as &dyn sqlparser::dialect::Dialect,
        ),
        (
            "generic",
            &GenericDialect {} as &dyn sqlparser::dialect::Dialect,
        ),
    ] {
        for (sql, kind, diagnostic_code) in cases {
            let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect)
                .expect("partition-scoped DML should parse");
            assert!(bundle.layers().is_empty(), "{dialect_name}: {sql}");
            assert!(
                bundle.write_state_effects().is_empty(),
                "{dialect_name}: {sql}"
            );
            let ProtocolStatement::Unsupported(statement) = &bundle.inputs()[0].statements()[0]
            else {
                panic!("partition selection cannot be ignored: {dialect_name}: {sql}");
            };
            assert_eq!(statement.category(), kind, "{dialect_name}: {sql}");
            assert_eq!(
                statement.diagnostics()[0].code(),
                diagnostic_code,
                "{dialect_name}: {sql}"
            );
            let emitted: serde_json::Value =
                serde_json::from_str(&to_bundle_json(&bundle)).expect("valid JSON");
            assert!(
                emitted.get("write_effects").is_none(),
                "{dialect_name}: {sql}"
            );
        }
    }
}
