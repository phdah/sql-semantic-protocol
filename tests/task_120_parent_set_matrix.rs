//! TASK-120: independent parent set-operation signoff at the public SQL/physical seam.
//! The expected counts and source identities are frozen separately from the evaluator.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_sql, dialect_from_name,
    physical_finite_set_bag_plan, physical_finite_set_tree_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFiniteSetProof, PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn inventory() -> Value {
    serde_json::from_str(include_str!("../docs/task-120-acceptance-freeze.json"))
        .expect("TASK-120 immutable independent acceptance table")
}

fn schema(relation: &str) -> RelationSchema {
    RelationSchema::new(
        relation,
        ["id", "k", "v"]
            .into_iter()
            .map(|column| {
                SchemaColumn::from_sql_type(
                    column,
                    if column == "id" { "VARCHAR" } else { "INTEGER" },
                    "postgresql",
                )
                .expect("typed source")
            })
            .collect(),
    )
    .expect("relation schema")
}

fn states(case: &Value) -> Vec<PhysicalWriteState> {
    case["source_bags"]
        .as_object()
        .expect("closed world source bags")
        .iter()
        .map(|(name, rows)| {
            let rows = rows
                .as_array()
                .expect("physical source")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("source fields")
                        .iter()
                        .map(|(column, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(number) = value.as_i64() {
                                LiteralValue::Number(number.to_string())
                            } else {
                                LiteralValue::Text(value.as_str().expect("row id").to_string())
                            };
                            (column.clone(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("valid physical row")
                })
                .collect();
            PhysicalWriteState::new(name, rows).expect("named physical source")
        })
        .collect()
}

type Histogram = BTreeMap<Vec<Option<i128>>, u64>;

fn expected_histogram(case: &Value) -> Histogram {
    case["expected_terminal_tuple_histogram"]
        .as_array()
        .expect("independently frozen terminal tuples")
        .iter()
        .map(|entry| {
            let tuple = entry["tuple"]
                .as_array()
                .expect("tuple")
                .iter()
                .map(|value| value.as_i64().map(i128::from))
                .collect::<Vec<_>>();
            (tuple, entry["count"].as_u64().expect("count"))
        })
        .collect()
}

fn duckdb_histogram(case: &Value) -> Histogram {
    let db = Connection::open_in_memory().expect("DuckDB oracle");
    for (source, rows) in case["source_bags"].as_object().expect("source bags") {
        db.execute_batch(&format!(
            "CREATE TABLE {source}(id VARCHAR, k INTEGER, v INTEGER)"
        ))
        .expect("independent physical schema");
        let mut expected_rows = Vec::new();
        for row in rows.as_array().expect("rows") {
            let literal = |name: &str| {
                row[name]
                    .as_i64()
                    .map_or_else(|| "NULL".to_string(), |n| n.to_string())
            };
            let id = row["id"].as_str().expect("stable physical row id");
            db.execute_batch(&format!(
                "INSERT INTO {source} VALUES ('{id}',{}, {})",
                literal("k"),
                literal("v")
            ))
            .expect("independent seed");
            expected_rows.push((id.to_string(), row["k"].as_i64(), row["v"].as_i64()));
        }
        expected_rows.sort();
        let mut stmt = db
            .prepare(&format!("SELECT id,k,v FROM {source} ORDER BY id"))
            .expect("source snapshot");
        let actual_rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .expect("source query")
            .collect::<Result<Vec<_>, _>>()
            .expect("complete source bag");
        assert_eq!(actual_rows, expected_rows, "{source}: source identity/bag");
    }
    let cols = case["output_columns"]
        .as_array()
        .expect("two output aliases");
    let key = cols[0].as_str().expect("key alias");
    let value = cols[1].as_str().expect("value alias");
    let sql = case["sql"].as_str().expect("frozen SQL");
    let mut stmt = db
        .prepare(&format!(
            "SELECT {key}, {value}, COUNT(*) FROM ({sql}) terminal GROUP BY {key}, {value}"
        ))
        .expect("DuckDB terminal SQL");
    stmt.query_map([], |row| {
        let key = row.get::<_, Option<i64>>(0)?;
        let value = row.get::<_, Option<i64>>(1)?;
        let count = row.get::<_, u64>(2)?;
        Ok((vec![key.map(i128::from), value.map(i128::from)], count))
    })
    .expect("DuckDB execution")
    .collect::<Result<Histogram, _>>()
    .expect("full terminal tuple frequencies")
}

fn check_domains(
    id: &str,
    actual: &[sql_semantic_protocol::PhysicalSetColumnDomain],
    case: &Value,
) {
    assert_eq!(actual.len(), 2, "{id}: two typed outputs");
    for (index, domain) in actual.iter().enumerate() {
        let frozen = &case["typed_terminal_domains"][index];
        let expected_bounds = match (frozen["lower"].as_i64(), frozen["upper"].as_i64()) {
            (Some(lower), Some(upper)) => Some((i128::from(lower), i128::from(upper))),
            _ => None,
        };
        assert_eq!(
            domain.bounds(),
            expected_bounds,
            "{id}: typed bounds {index}"
        );
        assert_eq!(
            domain.contains_null,
            frozen["contains_null"].as_bool().expect("frozen NULL"),
            "{id}: typed NULL {index}"
        );
        let allowed = frozen["allowed"]
            .as_array()
            .expect("frozen finite domain")
            .iter()
            .map(|number| i128::from(number.as_i64().expect("signed integer")))
            .collect::<Vec<_>>();
        assert_eq!(domain.values, allowed, "{id}: exact values {index}");
    }
}

#[test]
fn frozen_matrix_is_finite_and_covers_every_original_parent_criterion() {
    let freeze = inventory();
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["72-C120-P", "72-C120-N"])
    );
    assert_eq!(
        freeze["dialect_identities"]
            .as_array()
            .expect("dialects")
            .len(),
        13
    );
    let mandatory = freeze["mandatory_cases"]
        .as_array()
        .expect("mandatory cells");
    let conditional = freeze["conditional_cases"]
        .as_array()
        .expect("conditional cells");
    assert_eq!(mandatory.len(), 21);
    assert_eq!(conditional.len(), 6);
    let mut seen = BTreeSet::new();
    let mut mapped = BTreeSet::new();
    for case in mandatory.iter().chain(conditional) {
        let id = case["id"].as_str().expect("stable fixture variant");
        assert!(seen.insert(id), "{id}: duplicate matrix cell");
        assert_eq!(
            case["dialect_cells"]
                .as_array()
                .expect("13 dialect classifications")
                .len(),
            13,
            "{id}: every supported dialect needs explicit classification"
        );
        assert_eq!(
            case["typed_terminal_domains"]
                .as_array()
                .expect("typed outputs")
                .len(),
            2
        );
        assert_eq!(
            case["output_columns"]
                .as_array()
                .expect("output names")
                .len(),
            2
        );
        for ac in case["mapped_original_ac"]
            .as_array()
            .expect("parent criteria")
        {
            mapped.insert(ac.as_u64().expect("criterion number"));
        }
        if case["classification"] == "conditional" {
            assert_eq!(case["owner"], "TASK-120", "{id}: residual must have owner");
            assert!(case["release_disposition"].as_str().is_some(), "{id}");
        }
    }
    assert_eq!(mapped, BTreeSet::from([1, 2, 3, 4, 5, 6]));
}

#[test]
fn all_mandatory_set_families_have_complete_exact_source_and_duckdb_bags() {
    let freeze = inventory();
    let catalog = RelationCatalog::from_schemas(&[schema("a"), schema("b"), schema("c")])
        .expect("source catalog");
    for case in freeze["mandatory_cases"]
        .as_array()
        .expect("mandatory matrix")
    {
        let id = case["id"].as_str().expect("case");
        let sql = case["sql"].as_str().expect("SQL");
        let dialect = dialect_from_name("generic").expect("generic dialect");
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            id,
            &input,
            "generic",
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .unwrap_or_else(|error| panic!("{id}: cannot analyze: {error}"));
        let target = bundle.layers().last().expect("terminal layer").id();
        let source_states = states(case);
        let (actual, source_names, domains, terminal_len) =
            if case["variant"] == "six_operator_nullable_bag" {
                let proof = physical_finite_set_bag_plan(&bundle, target, &source_states);
                let PhysicalFiniteSetProof::Exact(exact) = proof else {
                    panic!("{id}: unproved mandatory two-branch bag: {proof:?}");
                };
                for tuple in &exact.tuples {
                    for (source, identities) in
                        [("a", &tuple.left_row_ids), ("b", &tuple.right_row_ids)]
                    {
                        let expected_ids = case["source_bags"][source]
                            .as_array()
                            .expect("entire physical source")
                            .iter()
                            .filter(|row| {
                                [
                                    row["k"].as_i64().map(i128::from),
                                    row["v"].as_i64().map(i128::from),
                                ] == tuple.values
                            })
                            .map(|row| row["id"].as_str().expect("row id").to_string())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            *identities, expected_ids,
                            "{id}/{source}: every source identity"
                        );
                    }
                }
                (
                    exact
                        .tuples
                        .iter()
                        .filter(|t| t.output_count > 0)
                        .map(|t| (t.values.to_vec(), t.output_count))
                        .collect::<Histogram>(),
                    exact.sources.to_vec(),
                    exact.terminal_domains.to_vec(),
                    exact.terminal.len(),
                )
            } else {
                let proof = physical_finite_set_tree_plan(&bundle, target, &source_states);
                let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
                    panic!("{id}: unproved mandatory set tree: {proof:?}");
                };
                for branch in &exact.branches {
                    let physical = source_states
                        .iter()
                        .find(|source| source.relation() == branch.source)
                        .expect("one canonical physical source per branch");
                    let mut actual_ids = branch
                        .selected_row_ids
                        .iter()
                        .chain(&branch.false_row_ids)
                        .chain(&branch.unknown_row_ids)
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut expected_ids = physical
                        .rows()
                        .iter()
                        .map(|row| match row.values().get("id") {
                            Some(LiteralValue::Text(id)) => id.clone(),
                            _ => panic!("{id}: unidentifiable source row"),
                        })
                        .collect::<Vec<_>>();
                    actual_ids.sort();
                    expected_ids.sort();
                    assert_eq!(
                        actual_ids, expected_ids,
                        "{id}: complete TRUE/FALSE/UNKNOWN partition"
                    );
                }
                (
                    exact
                        .tuples
                        .iter()
                        .filter(|t| t.output_count > 0)
                        .map(|t| (t.values.clone(), t.output_count))
                        .collect::<Histogram>(),
                    exact.sources,
                    exact.terminal_domains,
                    exact.terminal.len(),
                )
            };
        let expected = expected_histogram(case);
        assert_eq!(actual, expected, "{id}: complete multiset including NULLs");
        assert_eq!(
            terminal_len,
            case["expected_count"]
                .as_u64()
                .expect("full expected count") as usize,
            "{id}: exact terminal bag"
        );
        let expected_sources = case["source_bags"]
            .as_object()
            .expect("physical inputs")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            source_names, expected_sources,
            "{id}: physical source identities"
        );
        check_domains(id, &domains, case);
        assert_eq!(
            duckdb_histogram(case),
            expected,
            "{id}: independent DuckDB execution"
        );
    }
}

#[test]
fn all_thirteen_portable_dialects_preserve_canonical_set_output_shape() {
    let freeze = inventory();
    for case in freeze["mandatory_cases"]
        .as_array()
        .expect("cases")
        .iter()
        .filter(|case| case["variant"] != "set_level_row_shaping")
    {
        let id = case["id"].as_str().expect("id");
        for name in freeze["dialect_identities"]
            .as_array()
            .expect("13 dialects")
        {
            let name = name.as_str().expect("dialect");
            let dialect = dialect_from_name(name).expect("exposed parser identity");
            let protocol = analyze_sql(case["sql"].as_str().expect("SQL"), name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let query = protocol
                .statements()
                .iter()
                .find_map(|statement| match statement {
                    ProtocolStatement::Query(query) => Some(query),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{id}/{name}: query missing"));
            let set = query
                .set_operation()
                .unwrap_or_else(|| panic!("{id}/{name}: set tree missing"));
            assert_eq!(set.branches()[0].output().columns().len(), 2, "{id}/{name}");
        }
    }
}

#[test]
fn impossible_populations_and_incomplete_source_evidence_have_distinct_proofs() {
    use sql_semantic_protocol::{
        ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
        RelationConstraint, RelationConstraintSet,
    };

    let freeze = inventory();
    let case = freeze["mandatory_cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "72-C120-P-sum")
        .expect("nullable source bag");
    let catalog = RelationCatalog::from_schemas(&[schema("a"), schema("b")])
        .expect("catalog");
    let parser = dialect_from_name("generic").expect("generic");
    let input = SqlInput::inline(case["sql"].as_str().expect("sql"));
    let configured = [ConfiguredSqlInput::new("classification", &input, "generic", parser.as_ref())];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("analysis");
    let target = bundle.layers().last().expect("terminal").id().to_string();
    let rows = states(case);

    let residual = physical_finite_set_bag_plan(&bundle, &target, &rows[..1]);
    assert!(matches!(residual, PhysicalFiniteSetProof::Residual { .. }));

    let provenance = ConstraintProvenance::new(
        ConstraintSourceKind::ExternalMetadata,
        "task-120-enforced-not-null",
    )
    .expect("named evidence");
    let evidence = vec![ConstraintEvidence::new(
        provenance,
        ConstraintEnforcement::Enforced,
    )];
    let not_null = RelationConstraint::not_null("v", evidence).expect("typed not-null");
    let constraints = RelationConstraintSet::new("a", vec![not_null])
        .expect("source constraint");
    bundle.enrich_relation_constraints(&[constraints]);
    let impossible = physical_finite_set_bag_plan(&bundle, &target, &rows);
    assert!(
        matches!(impossible, PhysicalFiniteSetProof::Impossible { .. }),
        "known NULL violates an enforced NOT NULL: {impossible:?}"
    );
}

#[test]
fn uncertified_order_ties_and_casts_remain_typed_residuals() {
    let freeze = inventory();
    let catalog = RelationCatalog::from_schemas(&[schema("a"), schema("b")])
        .expect("source catalog");
    for suffix in ["order-tie", "coercion", "branch-limit"] {
        let case = freeze["conditional_cases"]
            .as_array()
            .expect("conditional cases")
            .iter()
            .find(|case| case["id"].as_str().is_some_and(|id| id.ends_with(suffix)))
            .expect("named conditional law");
        let parser = dialect_from_name("generic").expect("generic");
        let input = SqlInput::inline(case["sql"].as_str().expect("SQL"));
        let configured = [ConfiguredSqlInput::new(suffix, &input, "generic", parser.as_ref())];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .expect("parser accepted conditional law");
        let proof = physical_finite_set_tree_plan(
            &bundle,
            bundle.layers().last().expect("terminal").id(),
            &states(case),
        );
        assert!(
            matches!(proof, PhysicalFiniteSetTreeProof::Residual { .. }),
            "{suffix}: unsupported portable laws must not be Exact: {proof:?}"
        );
    }
}
