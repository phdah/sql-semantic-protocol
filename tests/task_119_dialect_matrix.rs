//! TASK-119: exhaust the frozen SQL-by-dialect parser/canonical matrix.
//! A parser boundary is never evidence for a physical or vendor runtime proof.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement, SetOperator};

#[test]
fn every_frozen_set_variant_is_classified_in_every_exposed_dialect() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-119-acceptance-freeze.json"))
            .expect("TASK-119 frozen acceptance cells");
    let cases = freeze["cases"].as_array().expect("cases");
    let dialects = freeze["dialect_identities"].as_array().expect("dialects");
    assert_eq!(cases.len(), freeze["case_count"].as_u64().unwrap() as usize);
    assert_eq!(dialects.len(), 13);

    for case in cases {
        let id = case["id"].as_str().expect("frozen case identity");
        let sql = case["sql"].as_str().expect("frozen SQL");
        let cells = case["dialect_cells"].as_array().expect("dialect cells");
        assert_eq!(cells.len(), dialects.len(), "{id}: complete matrix");
        for (index, dialect) in dialects.iter().enumerate() {
            let name = dialect.as_str().expect("dialect identity");
            assert_eq!(cells[index]["name"], name, "{id}: stable cell order");
            let parser = dialect_from_name(name).expect("exposed dialect");
            let result = analyze_sql(sql, name, parser.as_ref());
            let mandatory = case["classification"] == "mandatory_exact";
            let protocol = match result {
                Ok(protocol) => protocol,
                Err(error) if !mandatory => {
                    assert!(
                        cells[index]["classification"]
                            .as_str()
                            .expect("conditional boundary")
                            .starts_with("conditional_"),
                        "{id}/{name}: untracked parser boundary: {error}"
                    );
                    continue;
                }
                Err(error) => panic!("{id}/{name}: mandatory shared syntax: {error}"),
            };
            let query = protocol
                .statements()
                .iter()
                .find_map(|statement| match statement {
                    ProtocolStatement::Query(query) => Some(query),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{id}/{name}: missing analyzed query"));
            let operation = query
                .set_operation()
                .unwrap_or_else(|| panic!("{id}/{name}: missing set operator"));

            if mandatory {
                assert_eq!(
                    cells[index]["classification"], "mandatory_parser_and_canonical_equivalence",
                    "{id}/{name}: mandatory cell"
                );
                assert!(query.diagnostics().is_empty(), "{id}/{name}: diagnostics");
                assert_eq!(operation.branches().len(), 2, "{id}/{name}: two leaves");
                assert_eq!(
                    query
                        .output()
                        .columns()
                        .iter()
                        .map(|column| column.name())
                        .collect::<Vec<_>>(),
                    vec!["key_out", "value_out"],
                    "{id}/{name}: first branch owns positional names"
                );
                let shaping = operation
                    .row_shaping()
                    .unwrap_or_else(|| panic!("{id}/{name}: missing typed row-shaping law"));
                let (limit, offset) = match id {
                    "72-C119-N-ordered-offset" => (2, 5),
                    "72-C119-P-null-duplicates" => (3, 2),
                    _ => (2, 0),
                };
                assert_eq!(
                    (shaping.limit, shaping.offset),
                    (limit, offset),
                    "{id}/{name}"
                );
                assert_eq!(shaping.keys.len(), 2, "{id}/{name}: total sort");
                assert_eq!(
                    shaping
                        .keys
                        .iter()
                        .map(|key| (key.index, key.descending, key.nulls_first))
                        .collect::<Vec<_>>(),
                    vec![(0, false, Some(false)), (1, false, Some(false))],
                    "{id}/{name}: exact NULLS LAST order"
                );
            } else if id.ends_with("-by-name") || id.ends_with("-corresponding") {
                assert!(
                    operation.multiplicity_rule().is_none() || !query.diagnostics().is_empty(),
                    "{id}/{name}: unattested name alignment must fail closed"
                );
            } else if id.ends_with("-minus") {
                assert_eq!(operation.operator(), SetOperator::Except, "{id}/{name}");
            } else {
                assert!(
                    cells[index]["classification"]
                        .as_str()
                        .expect("conditional boundary")
                        .starts_with("conditional_"),
                    "{id}/{name}: must declare residual or parser condition"
                );
            }
        }
    }
}
