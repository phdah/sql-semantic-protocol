use sql_semantic_protocol::{
    analyze_inputs, ComposedSemantics, CompositionFailureReason, MergeAction, MergeMatchKind,
    LiteralValue, ProtocolStatement, RelationResolution, SqlInput, ValueDomain, WriteKind,
};
use sqlparser::dialect::{GenericDialect, SnowflakeDialect};

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
    assert_eq!(lower.value().value(), &LiteralValue::Number("10".to_string()));
    assert!(lower.inclusive());
    assert_eq!(upper.value().value(), &LiteralValue::Number("20".to_string()));
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
