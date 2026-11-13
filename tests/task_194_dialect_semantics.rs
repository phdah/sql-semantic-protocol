//! TASK-194's original 22 sets variants, including 286 named parser cells.
//! Parser/canonical equality is checked separately from native bag correctness.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, to_json, DiagnosticArea, ProtocolStatement,
};

const FREEZE: &str = include_str!("../docs/task-194-acceptance-freeze.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("committed acceptance evidence")
}

fn without_dialect(mut document: Value) -> Value {
    for input in document["inputs"]
        .as_array_mut()
        .expect("canonical input array")
    {
        input["dialect"] = Value::Null;
    }
    document
}

#[test]
fn freeze_contains_only_the_original_22_set_variants_and_all_286_dialect_cells() {
    let fixture = parse(FREEZE);
    let inventory = parse(ORIGINAL);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["task_id"], "TASK-194");
    assert_eq!(fixture["case_count"], 22);
    assert_eq!(fixture["parser_cells"], 286);
    assert_eq!(fixture["dialects"], inventory["dialects"]);
    assert_eq!(fixture["task_complete"], false);
    assert_eq!(fixture["release_approved"], false);

    let originals = inventory["cases"]
        .as_array()
        .expect("complete original inventory");
    let original_sets = originals
        .iter()
        .filter(|row| {
            row["feature_id"]
                .as_str()
                .is_some_and(|name| name.starts_with("sets."))
        })
        .map(|row| row["id"].as_str().expect("case ID"))
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut cells = BTreeSet::new();
    for row in fixture["cases"].as_array().expect("22 fixed SQL cases") {
        let id = row["id"].as_str().expect("original case ID");
        assert!(ids.insert(id), "{id}: duplicate");
        assert!(original_sets.contains(id), "{id}: unrelated family");
        let prior = originals
            .iter()
            .find(|old| old["id"] == id)
            .expect("fixed source SQL");
        assert_eq!(row["sql"], prior["sql"], "{id}: immutable original SQL");
        assert_eq!(row["variant"], prior["variant"], "{id}: immutable variant");
        assert_eq!(
            row["feature_id"], prior["feature_id"],
            "{id}: owned set family"
        );
        assert_eq!(
            row["original_observed_mask"], prior["full_document_classification"],
            "{id}: preserve historical parser observation, not a signoff"
        );
        let dialects = row["cells"].as_array().expect("all 13 named dialect cells");
        assert_eq!(dialects.len(), 13, "{id}");
        for (i, cell) in dialects.iter().enumerate() {
            let dialect = cell["dialect"].as_str().expect("exposed name");
            let cell_id = cell["id"].as_str().expect("stable cell");
            assert_eq!(cell["dialect"], fixture["dialects"][i]);
            assert_eq!(cell_id, format!("{id}/{dialect}"));
            assert!(cells.insert(cell_id), "{cell_id}: duplicate");
            assert_eq!(
                cell["original_observed_status"],
                prior["dialect_cells"][i]["observed_status"]
            );
            assert_eq!(cell["native_vendor_execution"], "unattested");
        }
    }
    assert_eq!(original_sets, ids);
    assert_eq!(cells.len(), 286);
}

#[test]
fn every_accepted_set_form_has_one_canonical_public_document_and_explicit_fail_closed_arity() {
    let freeze = parse(FREEZE);
    let mut checked = 0;
    for case in freeze["cases"].as_array().expect("22 source SQL cases") {
        let id = case["id"].as_str().expect("original ID");
        let sql = case["sql"].as_str().expect("original SQL text");
        let mut baseline: Option<Value> = None;
        for cell in case["cells"].as_array().expect("13 parser cells") {
            let name = cell["dialect"].as_str().expect("dialect name");
            let cell_id = cell["id"].as_str().expect("case/dialect identity");
            let dialect = dialect_from_name(name).expect("public sqlparser dialect");
            let analysis = analyze_sql(sql, name, dialect.as_ref());
            if id == "88-C177-S084" {
                assert!(
                    analysis.is_err(),
                    "{cell_id}: CORRESPONDING is a parser boundary, not certified SQL"
                );
                assert_eq!(cell["original_observed_status"], "R");
                assert_eq!(cell["boundary_owner"], "TASK-191");
                checked += 1;
                continue;
            }

            let protocol =
                analysis.unwrap_or_else(|error| panic!("{cell_id}: original SQL: {error}"));
            let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                panic!("{cell_id}: set operator must not be silently dropped");
            };
            let width = case["expected_output_columns"]
                .as_u64()
                .expect("native arity") as usize;
            assert_eq!(
                query.output().columns().len(),
                width,
                "{cell_id}: output arity"
            );
            if id == "88-C177-S086" {
                assert_eq!(width, 0);
                assert!(
                    query.diagnostics().iter().any(|diagnostic| {
                        diagnostic.area() == DiagnosticArea::Output
                            && diagnostic.code() == "set_operation_arity_mismatch"
                    }),
                    "{cell_id}: incompatible branches must not emit a plausible output"
                );
            } else {
                assert!(
                    query
                        .diagnostics()
                        .iter()
                        .all(|diagnostic| { diagnostic.code() != "set_operation_arity_mismatch" }),
                    "{cell_id}: valid arity cannot become unsupported"
                );
                let expected_names = case["expected_output_column_names"]
                    .as_array()
                    .expect("positional output names");
                for (column, expected) in query.output().columns().iter().zip(expected_names) {
                    assert_eq!(column.name(), expected.as_str().expect("leftmost name"));
                    if id == "88-C177-S083" {
                        let names = column
                            .lineage()
                            .iter()
                            .map(|origin| origin.column())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            names,
                            vec![column.name(), column.name()],
                            "{cell_id}: BY NAME output must keep matching source-column lineage"
                        );
                    }
                }
                let deps = query
                    .dependencies()
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                let expected_deps = case["expected_source_dependencies"]
                    .as_array()
                    .expect("complete table identities")
                    .iter()
                    .map(|item| item.as_str().expect("source"))
                    .collect::<BTreeSet<_>>();
                assert_eq!(deps, expected_deps, "{cell_id}: physical producer identity");
            }

            let doc = without_dialect(parse(&to_json(&protocol)));
            let columns = doc["inputs"][0]["statements"][0]["output"]["columns"]
                .as_array()
                .expect("canonical output array");
            assert_eq!(columns.len(), width);
            assert!(
                columns.iter().all(|col| col["domain"].is_object()),
                "{cell_id}: output value domains must be explicit"
            );
            if let Some(previous) = &baseline {
                assert_eq!(
                    &doc, previous,
                    "{cell_id}: same SQL must emit same semantics"
                );
            } else {
                baseline = Some(doc);
            }
            assert_eq!(cell["original_observed_status"], "C");
            checked += 1;
        }
    }
    assert_eq!(checked, 286);
}

#[test]
fn by_name_rejects_unmatched_and_duplicated_column_names_instead_of_guessing() {
    let dialect = dialect_from_name("generic").expect("generic parser");
    for sql in [
        "SELECT id, amount FROM orders UNION BY NAME SELECT id FROM customers",
        "SELECT id FROM orders UNION BY NAME SELECT id, amount FROM customers",
        "SELECT id AS key, amount AS key FROM orders UNION BY NAME SELECT id AS key, amount AS other FROM customers",
    ] {
        let protocol = analyze_sql(sql, "generic", dialect.as_ref())
            .expect("BY NAME syntax must remain parsed for diagnostic");
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("BY NAME query lost: {sql}");
        };
        assert!(
            query.output().columns().is_empty(),
            "{sql}: no safe output bijection was proved"
        );
        assert!(
            query
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "unsupported_set_operation_alignment"),
            "{sql}: unresolved semantics require an explicit fail-closed diagnostic"
        );
    }
}


#[test]
fn minus_spelling_keeps_the_original_except_semantics_in_all_13_parsers() {
    let dialects = parse(FREEZE)["dialects"]
        .as_array()
        .expect("frozen dialect list")
        .clone();
    let exception: Value = parse(include_str!("../docs/task-178-syntax-dialect-boundaries.json"));
    let spelling = exception["cases"]
        .as_array()
        .expect("separately frozen syntax variants")
        .iter()
        .find(|case| case["id"] == "88-C178-MINUS")
        .expect("named MINUS portability exception");
    assert_eq!(spelling["expected_mask"], "CCCCCCCCCCCCC");
    let sql = spelling["sql"].as_str().expect("original MINUS syntax");
    let reference = spelling["reference"].as_str().expect("canonical EXCEPT spelling");
    for dialect in &dialects {
        let name = dialect.as_str().expect("parser dialect");
        let parser = dialect_from_name(name).expect("exposed dialect");
        let left = analyze_sql(sql, name, parser.as_ref())
            .unwrap_or_else(|e| panic!("{name}: MINUS parser exception: {e}"));
        let right = analyze_sql(reference, name, parser.as_ref())
            .unwrap_or_else(|e| panic!("{name}: EXCEPT parser exception: {e}"));
        assert_eq!(
            without_dialect(parse(&to_json(&left))),
            without_dialect(parse(&to_json(&right))),
            "{name}: accepted spelling must retain complete canonical output semantics"
        );
    }
    assert_eq!(spelling["release_disposition"], "unreviewed_no_native_vendor_certification");
}
