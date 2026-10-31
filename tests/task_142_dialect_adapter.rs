//! TASK-142 dialect identity, source-independent dbt parity and fail-closed boundaries.
//! All project dialects are tested through the public analyzer. Native engine
//! oracles are separately executed in task_142_recursive_cte.rs.

mod common;

use common::DIALECTS;
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_finite_cte_bag_plan,
    AnalysisBundle, ConfiguredSqlInput, CteRecursionSetMode, Error, PhysicalCteProof,
    PhysicalCteRef, ProtocolStatement, QueryStatement, RelationCatalog, SqlInput,
};

fn sql_bundle(sql: &str, dialect_name: &str) -> AnalysisBundle {
    let dialect = dialect_from_name(dialect_name).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task142",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(
        &configured,
        &RelationCatalog::from_schemas(&[]).unwrap(),
    )
    .unwrap()
}

fn compiled_bundle(sql: &str) -> AnalysisBundle {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.demo.task142": {
                "unique_id": "model.demo.task142",
                "resource_type": "model",
                "relation_name": "main.task142",
                "name": "task142",
                "original_file_path": "models/task142.sql",
                "language": "sql",
                "raw_code": "SELECT 0 WHERE FALSE",
                "compiled_code": sql,
                "depends_on": {"nodes": []}
            }
        },
        "sources": {}
    });
    let catalog = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": {},
        "sources": {},
        "errors": null
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let parsed_catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_artifacts(&parsed, &parsed_catalog, "duckdb", dialect.as_ref()).unwrap()
}

fn query(bundle: &AnalysisBundle) -> &QueryStatement {
    bundle.inputs().iter().flat_map(|input| input.statements()).find_map(
        |statement| match statement {
            ProtocolStatement::Query(query) => Some(query),
            _ => None,
        }
    ).expect("analyzed task-142 query")
}

fn exact(bundle: &AnalysisBundle) -> sql_semantic_protocol::PhysicalCteExact {
    let layer = bundle.layers().last().unwrap();
    let result = physical_finite_cte_bag_plan(bundle, layer.id(), &[], None);
    let PhysicalCteProof::Exact(exact) = result else {
        panic!("canonical recursive physical proof missing: {result:?}");
    };
    *exact
}

#[test]
fn all_exposed_dialects_are_classified_at_the_parser_boundary_or_prove_same_bag() {
    let freeze: Value = serde_json::from_str(
        include_str!("../docs/task-142-acceptance-freeze.json")
    ).unwrap();
    let frozen_names = freeze["dialects"].as_array().unwrap().iter().map(
        |dialect| dialect["name"].as_str().unwrap()
    ).collect::<Vec<_>>();
    assert_eq!(frozen_names, DIALECTS, "freeze must include all exposed dialects");

    let mut classified = 0;
    let mut constructed = 0;
    let mut parser_boundaries = 0;
    for entry in freeze["cases"].as_array().unwrap() {
        if entry["classification"] != "mandatory_exact" {
            continue;
        }
        let sql = entry["sql"].as_str().unwrap();
        for name in DIALECTS {
            let dialect = dialect_from_name(name).expect("exposed dialect");
            match analyze_sql(sql, name, dialect.as_ref()) {
                Err(Error::Parse(_)) => {
                    parser_boundaries += 1;
                }
                Err(other) => panic!("{name}: non-parser failure: {other}"),
                Ok(_) => {
                    let bundle = sql_bundle(sql, name);
                    let program = query(&bundle).cte_program()
                        .unwrap_or_else(|| panic!("{name}: parsed recursion lost canonical proof"));
                    assert_eq!(program.stages().len(), 2);
                    let recursion = program.stages()[0].recursion().unwrap();
                    assert_eq!(recursion.column(), "x");
                    let expected_mode = if entry["id"] == "78-C142-P-ALL" {
                        CteRecursionSetMode::All
                    } else {
                        CteRecursionSetMode::Distinct
                    };
                    assert_eq!(recursion.set_mode(), expected_mode, "{name}");
                    let proof = exact(&bundle);
                    assert!(proof.sources.is_empty());
                    assert_eq!(proof.terminal.len(), 3, "{name}");
                    assert_eq!(proof.terminal_domains["x"].integer_bounds, Some((1, 3)), "{name}");
                    constructed += 1;
                }
            }
            classified += 1;
        }
    }
    assert_eq!(classified, 2 * DIALECTS.len());
    assert_eq!(constructed + parser_boundaries, classified);
    assert!(constructed > 0, "at least the DuckDB parser must accept the core");
}

#[test]
fn canonical_source_free_recursive_stages_match_compiled_dbt_without_invented_sources() {
    let freeze: Value = serde_json::from_str(
        include_str!("../docs/task-142-acceptance-freeze.json")
    ).unwrap();
    let mut cases = 0;
    for entry in freeze["cases"].as_array().unwrap() {
        if entry["classification"] != "mandatory_exact" {
            continue;
        }
        let sql = entry["sql"].as_str().unwrap();
        let direct = sql_bundle(sql, "duckdb");
        let dbt = compiled_bundle(sql);
        let direct_query = query(&direct);
        let dbt_query = query(&dbt);
        assert!(direct_query.dependencies().is_empty(), "recursive name is local");
        assert_eq!(dbt_query.dependencies(), direct_query.dependencies());
        assert_eq!(dbt_query.sources(), direct_query.sources());
        assert_eq!(dbt_query.cte_program(), direct_query.cte_program());
        let direct_proof = exact(&direct);
        let dbt_proof = exact(&dbt);
        assert_eq!(direct_proof.source_bags, dbt_proof.source_bags);
        assert_eq!(direct_proof.stage_bags, dbt_proof.stage_bags);
        assert_eq!(direct_proof.terminal, dbt_proof.terminal);
        assert_eq!(direct_proof.terminal_domains, dbt_proof.terminal_domains);
        assert_eq!(direct_proof.false_pairs, dbt_proof.false_pairs);
        assert_eq!(direct_proof.unknown_pairs, dbt_proof.unknown_pairs);

        // One producer consumes its own previous iteration; the terminal
        // consumes that producer. Recursion is not an independent physical leaf.
        let producer = &direct_proof.nodes[0];
        assert_eq!(producer.id, PhysicalCteRef::Producer("terminal/cte:0:n".into()));
        assert_eq!(producer.inputs.len(), 1);
        assert_eq!(producer.inputs[0].reference, producer.id);
        assert_eq!(producer.inputs[0].alias, "n");
        let terminal = &direct_proof.nodes[1];
        assert_eq!(terminal.inputs[0].reference, producer.id);
        cases += 1;
    }
    assert_eq!(cases, 2);
}

#[test]
fn unsupported_recursive_forms_never_borrow_exactness_from_a_matching_cte_name() {
    for sql in [
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x FROM n WHERE x < 3) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 3 OR NULL) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 2147483647) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT 2147483647 UNION ALL SELECT x + 1 FROM n WHERE x <= 2147483647) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT NULL UNION ALL SELECT x + 1 FROM n WHERE x < 3) SELECT x FROM n",
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 3) SELECT x FROM n LIMIT 1",
    ] {
        let bundle = sql_bundle(sql, "duckdb");
        let layer = bundle.layers().last().unwrap();
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle, layer.id(), &[], None),
            PhysicalCteProof::Residual { .. }
        ), "unsupported recursive input must fail closed: {sql}");
    }
}

#[test]
fn zero_iterations_and_inclusive_guard_have_exact_typed_terminal_domains() {
    for (sql, expected) in [
        ("WITH RECURSIVE n(x) AS (SELECT 4 UNION ALL SELECT x + 1 FROM n WHERE x < 3) SELECT x FROM n", vec![4]),
        ("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x <= 2) SELECT x FROM n", vec![1, 2, 3]),
    ] {
        let bundle = sql_bundle(sql, "duckdb");
        let proof = exact(&bundle);
        let actual = proof.terminal.iter().map(|row|
            row.values["x"].clone()
        ).collect::<Vec<_>>();
        assert_eq!(actual, expected.iter().map(|x|
            sql_semantic_protocol::LiteralValue::Number(x.to_string())
        ).collect::<Vec<_>>());
        assert_eq!(
            proof.terminal_domains["x"].integer_bounds,
            Some((*expected.iter().min().unwrap() as i128, *expected.iter().max().unwrap() as i128))
        );
    }
}
