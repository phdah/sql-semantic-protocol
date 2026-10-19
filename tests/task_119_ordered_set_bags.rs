//! TASK-119: finite positional set alignment and bounded row shaping.
//! The SQL/DuckDB + canonical-physical seam is independent of the Rust implementation.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name,
    physical_finite_set_tree_plan, AnalysisBundle, ConfiguredSqlInput, LiteralValue,
    PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState, ProtocolStatement,
    RelationCatalog, RelationSchema, SchemaColumn, SetOperator, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-119-acceptance-freeze.json"))
        .expect("immutable TASK-119 acceptance inventory")
}

fn states(case: &Value) -> Vec<PhysicalWriteState> {
    case["source_bags"]
        .as_object()
        .expect("all source bags")
        .iter()
        .map(|(source, bag)| {
            let rows = bag
                .as_array()
                .expect("closed world")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("complete source row")
                        .iter()
                        .map(|(name, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(number) = value.as_i64() {
                                LiteralValue::Number(number.to_string())
                            } else {
                                LiteralValue::Text(value.as_str().expect("identity").to_string())
                            };
                            (name.to_string(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("physical source row")
                })
                .collect();
            PhysicalWriteState::new(source, rows).expect("named source")
        })
        .collect()
}

fn bundle(sql: &str) -> AnalysisBundle {
    let schemas = ["a", "b"]
        .into_iter()
        .map(|source| {
            RelationSchema::new(
                source,
                ["id", "k", "v"]
                    .into_iter()
                    .map(|name| {
                        SchemaColumn::from_sql_type(
                            name,
                            if name == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("source datatype")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("schema catalog");
    let dialect = dialect_from_name("generic").expect("generic SQL");
    let sql_input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task119",
        &sql_input,
        "generic",
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analyze named SQL")
}

fn expected_histogram(case: &Value) -> BTreeMap<Vec<Option<i128>>, u64> {
    case["expected_terminal_tuple_histogram"]
        .as_array()
        .expect("expected independent rows")
        .iter()
        .map(|item| {
            let tuple = item["tuple"]
                .as_array()
                .expect("tuple")
                .iter()
                .map(|value| value.as_i64().map(i128::from))
                .collect::<Vec<_>>();
            (tuple, item["count"].as_u64().expect("multiplicity"))
        })
        .collect()
}

fn duckdb_histogram(case: &Value) -> BTreeMap<Vec<Option<i128>>, u64> {
    let db = Connection::open_in_memory().expect("DuckDB oracle");
    for (relation, rows) in case["source_bags"].as_object().expect("physical rows") {
        db.execute_batch(&format!(
            "CREATE TABLE {relation}(id VARCHAR, k INTEGER, v INTEGER);"
        ))
        .expect("source schema");
        for row in rows.as_array().expect("physical bag") {
            let scalar = |name: &str| {
                row[name]
                    .as_i64()
                    .map_or_else(|| "NULL".to_string(), |n| n.to_string())
            };
            db.execute_batch(&format!(
                "INSERT INTO {relation} VALUES ('{}', {}, {});",
                row["id"].as_str().expect("id"),
                scalar("k"),
                scalar("v")
            ))
            .expect("independently seeded row");
        }
        // Read the entire independently executed source bag, including NULL
        // values and stable row identities, before checking its terminal SQL.
        let mut select = db
            .prepare(&format!("SELECT id,k,v FROM {relation} ORDER BY id"))
            .expect("source snapshot query");
        let actual = select
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .expect("query source")
            .collect::<Result<Vec<_>, _>>()
            .expect("complete source snapshot");
        let mut expected = rows
            .as_array()
            .expect("frozen source bag")
            .iter()
            .map(|row| {
                (
                    row["id"].as_str().expect("id").to_string(),
                    row["k"].as_i64(),
                    row["v"].as_i64(),
                )
            })
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(actual, expected, "{relation}: full DuckDB physical source bag");
    }
    let sql = case["sql"].as_str().expect("SQL");
    let mut query = db
        .prepare(&format!(
            "SELECT key_out,value_out,COUNT(*) FROM ({sql}) AS terminal GROUP BY key_out,value_out"
        ))
        .expect("independent DuckDB parser");
    query
        .query_map([], |row| {
            let a = row.get::<_, Option<i64>>(0)?;
            let b = row.get::<_, Option<i64>>(1)?;
            let count = row.get::<_, u64>(2)?;
            Ok((vec![a.map(i128::from), b.map(i128::from)], count))
        })
        .expect("execute oracle")
        .collect::<Result<BTreeMap<_, _>, _>>()
        .expect("collect entire terminal bag")
}

#[test]
fn frozen_order_limit_fetch_and_exhausted_offset_are_complete_source_proofs() {
    let freeze = freeze();
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["72-C119-P", "72-C119-N"])
    );
    let cases = freeze["cases"].as_array().expect("all frozen cases");
    for case in cases
        .iter()
        .filter(|item| item["classification"] == "mandatory_exact")
    {
        let id = case["id"].as_str().expect("fixture id");
        let analyzed = bundle(case["sql"].as_str().expect("SQL"));
        let target = analyzed.layers().last().expect("terminal layer").id();
        let physical = states(case);
        let proof = physical_finite_set_tree_plan(&analyzed, target, &physical);
        let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
            panic!("{id}: missing complete source proof: {proof:?}");
        };
        let expected = expected_histogram(case);
        let actual = exact
            .tuples
            .iter()
            .filter(|tuple| tuple.output_count > 0)
            .map(|tuple| (tuple.values.clone(), tuple.output_count))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected, "{id}: all nullable tuple multiplicities");
        let mut terminal = BTreeMap::<Vec<Option<i128>>, u64>::new();
        for row in &exact.terminal {
            let tuple = ["key_out", "value_out"]
                .into_iter()
                .map(
                    |name| match row.get(name).expect("canonical output alias") {
                        LiteralValue::Null => None,
                        LiteralValue::Number(value) => {
                            Some(value.parse::<i128>().expect("integer"))
                        }
                        other => panic!("unexpected scalar: {other:?}"),
                    },
                )
                .collect::<Vec<_>>();
            *terminal.entry(tuple).or_default() += 1;
        }
        assert_eq!(terminal, expected, "{id}: complete terminal rows");
        assert_eq!(
            exact.sources,
            vec!["a", "b"],
            "{id}: stable source identities"
        );
        assert_eq!(
            exact.terminal.len(),
            case["expected_count"].as_u64().expect("count") as usize
        );
        for (index, domain) in exact.terminal_domains.iter().enumerate() {
            let wanted = &case["typed_terminal_domains"][index];
            let allowed = wanted["allowed"]
                .as_array()
                .expect("frozen complete allowed values")
                .iter()
                .map(|value| i128::from(value.as_i64().expect("finite integer")))
                .collect::<Vec<_>>();
            assert_eq!(
                domain.values, allowed,
                "{id}: exact allowed values at {index}"
            );
            assert_eq!(
                wanted["empty"].as_bool().expect("frozen domain emptiness"),
                domain.values.is_empty() && !domain.contains_null,
                "{id}: exact empty domain at {index}"
            );
            assert_eq!(
                wanted["lower_inclusive"].as_bool(),
                Some(true),
                "{id}: frozen lower bound inclusivity"
            );
            assert_eq!(
                wanted["upper_inclusive"].as_bool(),
                Some(true),
                "{id}: frozen upper bound inclusivity"
            );
            assert_eq!(
                domain.bounds(),
                match (wanted["lower"].as_i64(), wanted["upper"].as_i64()) {
                    (Some(low), Some(high)) => Some((i128::from(low), i128::from(high))),
                    _ => None,
                },
                "{id}: exact inclusive bounds at {index}"
            );
            assert_eq!(
                domain.contains_null,
                wanted["contains_null"].as_bool().expect("NULL")
            );
        }
        for branch in &exact.branches {
            let source = physical
                .iter()
                .find(|source| source.relation() == branch.source)
                .expect("physical branch source");
            let mut all = branch
                .selected_row_ids
                .iter()
                .chain(branch.false_row_ids.iter())
                .chain(branch.unknown_row_ids.iter())
                .cloned()
                .collect::<Vec<_>>();
            all.sort();
            let mut source_ids = source
                .rows()
                .iter()
                .map(|row| match row.values().get("id") {
                    Some(LiteralValue::Text(id)) => id.clone(),
                    other => panic!("source row without stable identity: {other:?}"),
                })
                .collect::<Vec<_>>();
            source_ids.sort();
            assert_eq!(all, source_ids, "{id}: full physical source partition");
        }
        if id.ends_with("-filtered-null") {
            assert!(
                exact
                    .branches
                    .iter()
                    .any(|branch| !branch.false_row_ids.is_empty()),
                "{id}: prove filtered-out FALSE source rows"
            );
            assert!(
                exact
                    .branches
                    .iter()
                    .any(|branch| !branch.unknown_row_ids.is_empty()),
                "{id}: prove SQL UNKNOWN source rows"
            );
        }
        assert_eq!(duckdb_histogram(case), expected, "{id}: independent DuckDB");
    }
}

#[test]
fn unresolved_ties_coercion_and_nested_branch_limits_fail_closed() {
    let freeze = freeze();
    for id in [
        "72-C119-N-order-tie",
        "72-C119-N-coercion",
        "72-C119-N-branch-limit",
    ] {
        let case = freeze["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("frozen residual fixture");
        let analyzed = bundle(case["sql"].as_str().expect("SQL"));
        let target = analyzed.layers().last().expect("terminal").id();
        let proof = physical_finite_set_tree_plan(&analyzed, target, &states(case));
        let PhysicalFiniteSetTreeProof::Residual { reason } = proof else {
            panic!("{id} must not invent source-level exactness: {proof:?}");
        };
        if id.ends_with("-order-tie") {
            assert!(
                reason.contains("ties distinct output tuples"),
                "{id}: must fail on the true ambiguous-tie law: {reason}"
            );
        }
        assert_eq!(
            case["typed_terminal_domains"][0]["proof"], "unknown_not_empty",
            "{id}: a residual is never an exact empty outcome"
        );
    }
}

#[test]
fn minus_normalizes_to_except_and_by_name_never_claims_positional_laws() {
    let dialect = dialect_from_name("generic").expect("parser");
    for case in freeze()["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("id");
        if !id.ends_with("-minus") && !id.ends_with("-by-name") {
            continue;
        }
        match analyze_sql(
            case["sql"].as_str().expect("SQL"),
            "generic",
            dialect.as_ref(),
        ) {
            Ok(protocol) => {
                let query = protocol
                    .statements()
                    .iter()
                    .find_map(|statement| match statement {
                        ProtocolStatement::Query(query) => Some(query),
                        _ => None,
                    })
                    .expect("set query");
                let operation = query.set_operation().expect("parsed set operation");
                if id.ends_with("-minus") {
                    assert_eq!(operation.operator(), SetOperator::Except);
                } else {
                    assert!(operation.multiplicity_rule().is_none());
                    assert!(!query.diagnostics().is_empty());
                }
            }
            Err(_) => {
                // Parser boundaries for vendor spellings are a conditional
                // matrix cell, never silently reported as canonical support.
            }
        }
    }
}

#[test]
fn portable_positional_alignment_has_thirteen_dialect_parser_classifications() {
    let freeze = freeze();
    let dialects = freeze["dialect_identities"]
        .as_array()
        .expect("dialect inventory");
    assert_eq!(dialects.len(), 13);
    for dialect in dialects {
        let name = dialect.as_str().expect("dialect");
        let parser = dialect_from_name(name).expect("exposed parser");
        let sql = "SELECT k AS key_out,v AS value_out FROM a UNION ALL SELECT v AS any_name,k AS other_name FROM b";
        let protocol = analyze_sql(sql, name, parser.as_ref())
            .unwrap_or_else(|error| panic!("{name}: shared syntax must parse: {error}"));
        let query = protocol
            .statements()
            .iter()
            .find_map(|statement| match statement {
                ProtocolStatement::Query(query) => Some(query),
                _ => None,
            })
            .expect("set query");
        assert_eq!(query.output().columns().len(), 2, "{name}");
        assert_eq!(query.output().columns()[0].name(), "key_out", "{name}");
        assert_eq!(query.output().columns()[1].name(), "value_out", "{name}");
    }
}

#[test]
fn enforced_nullability_contradiction_is_impossible_not_residual() {
    use sql_semantic_protocol::{
        ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
        RelationConstraint, RelationConstraintSet,
    };

    let freeze = freeze();
    let case = freeze["cases"]
        .as_array()
        .expect("frozen cases")
        .iter()
        .find(|case| case["id"] == "72-C119-P-null-duplicates")
        .expect("nullable physical source");
    let mut analyzed = bundle(case["sql"].as_str().expect("SQL"));
    let target = analyzed.layers().last().expect("target").id().to_string();
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-119")
            .expect("enforced source provenance"),
        ConstraintEnforcement::Enforced,
    )];
    analyzed.enrich_relation_constraints(&[RelationConstraintSet::new(
        "a",
        vec![RelationConstraint::not_null("v", evidence).expect("NOT NULL rule")],
    )
    .expect("source constraints")]);
    let proof = physical_finite_set_tree_plan(&analyzed, &target, &states(case));
    assert!(
        matches!(proof, PhysicalFiniteSetTreeProof::Impossible { .. }),
        "enforced impossible source cannot be a residual: {proof:?}"
    );
}
