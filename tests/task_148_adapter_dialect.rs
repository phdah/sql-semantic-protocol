//! TASK-148: shared parser dialects and provenance-independent ordered INSERT states.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

const DIALECTS: [&str; 13] = [
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];
const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";
const SCRIPT: [&str; 2] = [
    "INSERT INTO warehouse.analytics.stg_orders(id,amount) SELECT id,amount FROM warehouse.raw.orders WHERE id > 1",
    "INSERT INTO warehouse.analytics.stg_orders(id,amount) VALUES (4,40)",
];

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "amount"]
                .into_iter()
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    }))
    .unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let analyzed =
        analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let known = RelationCatalog::new(&[TARGET, SOURCE]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task148parity
name: Equivalent ordered INSERT metadata
version: 1.0.0
status: active
schema:
  - name: stg_orders
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: orders
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    RelationCatalog::from_schemas(contract.source_schemas()).unwrap()
}

fn state(relation: &str, values: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        values
            .iter()
            .map(|(id, amount)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), LiteralValue::Number(id.to_string())),
                    (
                        "amount".to_string(),
                        LiteralValue::Number(amount.to_string()),
                    ),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn execute(catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let sqls = SCRIPT
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let names = ["task148-parity-1", "task148-parity-2"];
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = sqls
        .iter()
        .zip(names)
        .map(|(sql, name)| ConfiguredSqlInput::new(name, sql, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    physical_ordered_write_state_plan(
        &bundle,
        &[state(SOURCE, &[(1, 10), (2, 20)]), state(TARGET, &[(0, 0)])],
    )
}

#[test]
fn ordered_script_is_source_adapter_independent() {
    let direct = execute(&direct_catalog());
    let dbt = execute(&dbt_catalog());
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &direct
    else {
        panic!("expected exact two-append script: {direct:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(
        transitions[0].before().rows(),
        state(TARGET, &[(0, 0)]).rows()
    );
    assert_eq!(
        transitions[0].after().rows(),
        state(TARGET, &[(0, 0), (2, 20)]).rows()
    );
    assert_eq!(transitions[1].before(), transitions[0].after());
    assert_eq!(
        transitions[1].after().rows(),
        state(TARGET, &[(0, 0), (2, 20), (4, 40)]).rows()
    );
    assert_eq!(transitions[0].affected().inserted(), 1);
    assert_eq!(transitions[1].affected().inserted(), 1);
    assert_eq!(
        final_states.iter().find(|s| s.relation() == SOURCE),
        Some(&state(SOURCE, &[(1, 10), (2, 20)]))
    );
    assert_eq!(
        direct, dbt,
        "dbt-introspected types cannot change exact rows"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn ordered_script_is_odcs_equivalent_when_metadata_is_equivalent() {
    assert_eq!(execute(&direct_catalog()), execute(&odcs_catalog()));
}

#[test]
fn all_thirteen_dialects_have_explicit_parser_or_canonical_outcomes() {
    const FAMILIES: [(&str, bool); 4] = [
        ("INSERT INTO target(id,score) VALUES (1,10),(2,20)", false),
        ("INSERT INTO target DEFAULT VALUES", false),
        ("INSERT INTO target(id,score) SELECT id,score FROM source WHERE id > 1", false),
        ("INSERT INTO target(id,score) WITH filtered AS (SELECT id,score FROM source WHERE id > 1) SELECT id,score FROM filtered", true),
    ];
    let mut classifications = 0_usize;
    let mut supported = [0_usize; 4];
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for (index, (sql, cte)) in FAMILIES.iter().enumerate() {
            classifications += 1;
            match analyze_inputs(&[SqlInput::inline(*sql)], dialect, parser.as_ref()) {
                Err(_) => {} // Parser boundary; not a native engine capability claim.
                Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => {} // Explicit fail-closed canonical boundary.
                    ProtocolStatement::Query(query) => {
                        let write = query.write().expect("INSERT cannot silently become SELECT");
                        assert_eq!(write.kind(), WriteKind::Append, "{dialect}: {sql}");
                        if *cte {
                            assert!(write.insert_cte_source().is_some(), "{dialect}: {sql}");
                        }
                        supported[index] += 1;
                    }
                    _ => panic!("{dialect}: INSERT parser returned an unclassified statement"),
                },
            }
        }
    }
    assert_eq!(classifications, 52);
    assert!(
        supported.iter().all(|count| *count > 0),
        "all four families must have at least one certified parser path: {supported:?}"
    );
}


#[test]
fn all_frozen_parser_cells_match_exact_expected_statement_classifications() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-148-acceptance-freeze.json")).unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(),6);
    let mut inspected = 0_usize;
    for case in cases {
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).unwrap();
            let expected = case["parser_cells"][dialect].as_array().unwrap();
            let statements = case["statements"].as_array().unwrap();
            assert_eq!(statements.len(),expected.len());
            for (index, sql) in statements.iter().enumerate() {
                let result = analyze_inputs(
                    &[SqlInput::inline(sql.as_str().unwrap())],
                    dialect,
                    parser.as_ref(),
                );
                let observed = match result {
                    Err(_) => "parser_boundary",
                    Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                        ProtocolStatement::Query(_) => "canonical",
                        ProtocolStatement::Unsupported(_) => "unsupported",
                        _ => "unclassified",
                    },
                };
                let expected_status = expected[index].as_str().unwrap();
                assert!(matches!(expected_status,"parser_boundary"|"canonical"|"unsupported"));
                assert_eq!(observed,expected_status,
                    "{} in {} statement #{}",case["id"],dialect,index);
                inspected += 1;
            }
        }
    }
    assert_eq!(inspected,182);
}

fn execute_two_stage_variant(
    catalog: &RelationCatalog,
    statements: &[&str; 2],
    with_defaults: bool,
) -> PhysicalWriteStatePlan {
    use sql_semantic_protocol::{
        ColumnValueKind, ColumnValueSource, ConstraintEnforcement, ConstraintEvidence,
        ConstraintProvenance, ConstraintSourceKind, ConstraintValue, RelationConstraintSet,
    };
    let sqls = statements
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let names = ["task148-variant-1", "task148-variant-2"];
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = sqls
        .iter()
        .zip(names)
        .map(|(sql, name)| ConfiguredSqlInput::new(name, sql, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    if with_defaults {
        let sources = [("id", 5), ("amount", 7)]
            .into_iter()
            .map(|(column, value)| {
                ColumnValueSource::new(
                    column,
                    ColumnValueKind::Default,
                    Some(ConstraintValue::Integer(value)),
                    vec![ConstraintEvidence::new(
                        ConstraintProvenance::new(
                            ConstraintSourceKind::ExternalMetadata,
                            "task148-parity-defaults",
                        )
                        .unwrap(),
                        ConstraintEnforcement::Enforced,
                    )],
                )
                .unwrap()
            })
            .collect();
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(TARGET, vec![])
            .unwrap()
            .with_column_value_sources(sources)]);
    }
    physical_ordered_write_state_plan(
        &bundle,
        &[state(SOURCE, &[(1, 10), (2, 20)]), state(TARGET, &[])],
    )
}

fn assert_two_stage_variant(sql: &[&str; 2], defaults: bool, expected: &[(i64, i64)]) {
    let direct = execute_two_stage_variant(&direct_catalog(), sql, defaults);
    let dbt = execute_two_stage_variant(&dbt_catalog(), sql, defaults);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &direct
    else {
        panic!("complete ordered variant expected: {direct:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[1].before(), transitions[0].after());
    assert_eq!(
        transitions[1].after().rows(),
        state(TARGET, expected).rows()
    );
    assert_eq!(
        final_states.iter().find(|s| s.relation() == SOURCE),
        Some(&state(SOURCE, &[(1, 10), (2, 20)]))
    );
    assert_eq!(direct, dbt);
    #[cfg(feature = "odcs")]
    assert_eq!(
        direct,
        execute_two_stage_variant(&odcs_catalog(), sql, defaults)
    );
}

#[test]
fn cte_select_and_literal_append_preserve_adapter_parity() {
    assert_two_stage_variant(
        &[
            "INSERT INTO warehouse.analytics.stg_orders(id,amount)
         WITH filtered AS (SELECT id,amount FROM warehouse.raw.orders WHERE id > 1)
         SELECT id,amount FROM filtered",
            "INSERT INTO warehouse.analytics.stg_orders(id,amount) VALUES (3,30)",
        ],
        false,
        &[(2, 20), (3, 30)],
    );
}

#[test]
fn attested_defaults_are_proven_identically_for_all_metadata_adapters() {
    assert_two_stage_variant(
        &[
            "INSERT INTO warehouse.analytics.stg_orders DEFAULT VALUES",
            "INSERT INTO warehouse.analytics.stg_orders(id,amount) VALUES (4,40)",
        ],
        true,
        &[(5, 7), (4, 40)],
    );
}


#[test]
fn vendor_insert_modifiers_never_silently_become_portable_append() {
    let vendor_sql = [
        "INSERT OVERWRITE TABLE target SELECT id,score FROM source",
        "INSERT IGNORE INTO target(id,score) VALUES (1,10)",
        "INSERT OR REPLACE INTO target(id,score) VALUES (1,10)",
    ];
    let mut checked = 0;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for sql in vendor_sql {
            if let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)],dialect,parser.as_ref()) {
                assert!(matches!(bundle.inputs()[0].statements()[0],
                    ProtocolStatement::Unsupported(_)),
                    "{dialect}: vendor modifier cannot inherit unconditional append proof: {sql}");
            }
            checked += 1;
        }
    }
    assert_eq!(checked,39);
}


fn classify_negative_across_catalogs(
    catalog: &RelationCatalog,
    duplicate_key: bool,
) -> PhysicalWriteStatePlan {
    use sql_semantic_protocol::{
        ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
        ConstraintSourceKind, RelationConstraint, RelationConstraintSet,
    };
    let input=SqlInput::inline(SCRIPT[0]);
    let dialect=dialect_from_name("postgresql").unwrap();
    let configured=ConfiguredSqlInput::new("task148-negative-parity",&input,"postgresql",dialect.as_ref());
    let mut bundle=analyze_configured_inputs_with_catalog(&[configured],catalog).unwrap();
    if duplicate_key {
        let evidence=vec![ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata,"task148-negative-parity").unwrap(),
            ConstraintEnforcement::Enforced,
        )];
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(TARGET,vec![
            RelationConstraint::primary_key(vec!["id".into()],evidence).unwrap()
        ]).unwrap()]);
    }
    let mut initial=vec![state(TARGET,&[])];
    if duplicate_key {
        initial.push(state(SOURCE,&[(2,20),(2,30)]));
    }
    physical_ordered_write_state_plan(&bundle,&initial)
}

#[test]
fn missing_source_and_duplicate_key_remain_fail_closed_across_adapters() {
    for duplicate_key in [false,true] {
        let direct=classify_negative_across_catalogs(&direct_catalog(),duplicate_key);
        let dbt=classify_negative_across_catalogs(&dbt_catalog(),duplicate_key);
        if duplicate_key {
            assert!(matches!(direct,PhysicalWriteStatePlan::Impossible{..}),"{direct:?}");
        } else {
            assert!(matches!(direct,PhysicalWriteStatePlan::Residual{..}),"{direct:?}");
        }
        assert_eq!(direct,dbt);
        #[cfg(feature="odcs")]
        assert_eq!(direct,classify_negative_across_catalogs(&odcs_catalog(),duplicate_key));
    }
}
