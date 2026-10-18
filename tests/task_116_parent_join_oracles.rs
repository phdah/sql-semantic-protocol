//! TASK-116 independent parent oracle for all fifteen frozen TASK-71 mandatory cells.
//! Compare literal source/terminal bags and row identities against DuckDB, not
//! the result of the protocol's own analysis or its child fixtures.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::{json, Value};

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, ComparisonOperator,
    ConfiguredSqlInput, JoinExecution, JoinKind, Predicate, ProtocolStatement, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-116-acceptance-freeze.json");

fn sorted(mut rows: Vec<Value>) -> Vec<Value> {
    rows.sort_by_key(Value::to_string);
    rows
}

fn numeric_rows(db: &Connection, sql: &str, width: usize) -> Vec<Value> {
    let mut stmt = db.prepare(sql).expect("independent terminal SQL");
    let rows = stmt
        .query_map([], |row| {
            let columns = (0..width)
                .map(|index| {
                    row.get::<usize, Option<i64>>(index).map_or_else(
                        |err| panic!("typed INTEGER column {index}: {err}"),
                        |v| v.map_or(Value::Null, |v| json!(v)),
                    )
                })
                .collect::<Vec<_>>();
            Ok(Value::Array(columns))
        })
        .expect("DuckDB output rows")
        .map(|row| row.expect("numeric row"))
        .collect::<Vec<_>>();
    sorted(rows)
}

fn identity_rows(db: &Connection, sql: &str, width: usize) -> Vec<Value> {
    let mut stmt = db.prepare(sql).expect("source-identity SQL");
    let rows = stmt
        .query_map([], |row| {
            let ids = (0..width)
                .map(|index| {
                    row.get::<usize, Option<String>>(index).map_or_else(
                        |err| panic!("source identity {index}: {err}"),
                        |v| v.map_or(Value::Null, |v| json!(v)),
                    )
                })
                .collect::<Vec<_>>();
            Ok(Value::Array(ids))
        })
        .expect("DuckDB identified rows")
        .map(|row| row.expect("source identity row"))
        .collect::<Vec<_>>();
    sorted(rows)
}

fn source_rows(db: &Connection, relation: &str) -> Vec<Value> {
    let mut stmt = db
        .prepare(&format!("SELECT id, k, a FROM {relation}"))
        .expect("complete source bag query");
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "k": row.get::<_, Option<i64>>(1)?,
                "a": row.get::<_, Option<i64>>(2)?,
            }))
        })
        .expect("physical source rows")
        .map(|row| row.expect("source row"))
        .collect::<Vec<_>>();
    sorted(rows)
}

fn seed(db: &Connection, case: &Value) {
    let is_fk = case["id"] == "J71-FK-KEY";
    // A referenced primary key must exist before the referencing table.
    for name in ["r", "l", "t"] {
        if !case["physical_sources"][name].is_array() {
            continue;
        }
        let ddl = if is_fk && name == "r" {
            "CREATE TABLE r (id VARCHAR NOT NULL, k INTEGER PRIMARY KEY, a INTEGER)".to_string()
        } else if is_fk && name == "l" {
            "CREATE TABLE l (id VARCHAR NOT NULL, k INTEGER REFERENCES r(k), a INTEGER)".to_string()
        } else {
            format!("CREATE TABLE {name} (id VARCHAR NOT NULL, k INTEGER, a INTEGER)")
        };
        db.execute_batch(&ddl).expect("typed physical source");
    }
    for name in ["r", "l", "t"] {
        let Some(rows) = case["physical_sources"][name].as_array() else {
            continue;
        };
        let mut source_ids = BTreeSet::new();
        for row in rows {
            let id = row["id"].as_str().expect("source identity");
            assert!(source_ids.insert(id), "{}: duplicate source ID", case["id"]);
            let k = row["k"].as_i64();
            let a = row["a"].as_i64();
            db.execute(
                &format!("INSERT INTO {name}(id,k,a) VALUES (?1,?2,?3)"),
                duckdb::params![id, k, a],
            )
            .expect("complete physical row seeded");
        }
        assert_eq!(
            source_rows(db, name),
            sorted(rows.clone()),
            "{}: immutable whole {name} bag",
            case["id"]
        );
    }
}

fn assert_domain(rows: &[Value], case: &Value) {
    let domains = case["output_domains"].as_array().expect("typed domains");
    assert_eq!(
        rows.first()
            .map_or(domains.len(), |r| r.as_array().unwrap().len()),
        domains.len()
    );
    for (index, domain) in domains.iter().enumerate() {
        let observed = rows.iter().map(|row| &row[index]).collect::<Vec<_>>();
        let integers = observed
            .iter()
            .filter_map(|value| value.as_i64())
            .collect::<Vec<_>>();
        let lower = integers.iter().copied().min();
        let upper = integers.iter().copied().max();
        assert_eq!(lower, domain["lower"].as_i64(), "{}: lower", case["id"]);
        assert_eq!(upper, domain["upper"].as_i64(), "{}: upper", case["id"]);
        assert_eq!(
            observed.iter().any(|value| value.is_null()),
            domain["contains_null"].as_bool().unwrap(),
            "{}: NULL",
            case["id"]
        );
        assert_eq!(
            rows.is_empty(),
            domain["empty"].as_bool().unwrap(),
            "{}: empty",
            case["id"]
        );
        assert_eq!(domain["type"], "signed_integer");
        assert_eq!(domain["lower_inclusive"], true);
        assert_eq!(domain["upper_inclusive"], true);
    }
}

#[test]
fn every_mandatory_parent_cell_has_an_independent_closed_world_duckdb_oracle() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("versioned TASK-116 freeze");
    let cases = freeze["mandatory"]
        .as_array()
        .expect("fifteen required cases");
    assert_eq!(cases.len(), 15);
    assert_eq!(freeze["fixture_ids"], json!(["71-C116-P", "71-C116-N"]));
    let mut seen = BTreeSet::new();
    let mut positive = 0;
    let mut negative = 0;
    for case in cases {
        let id = case["id"].as_str().expect("canonical parent ID");
        assert!(seen.insert(id), "duplicate parent cell {id}");
        let fixture = case["fixture"].as_str().expect("frozen P/N fixture");
        match fixture {
            "71-C116-P" => positive += 1,
            "71-C116-N" => negative += 1,
            _ => panic!("{id}: unrecognized frozen fixture"),
        }
        assert_eq!(case["classification"], "exact_required");
        assert_eq!(case["dialects"].as_array().unwrap().len(), 13);
        assert_eq!(case["adapter_obligations"].as_array().unwrap().len(), 3);
        assert!(!case["source_instance_ids"].as_object().unwrap().is_empty());
        let db = Connection::open_in_memory().expect("DuckDB oracle");
        seed(&db, case);

        // Physical SQL, rather than the analyzed protocol, determines terminal truth.
        let actual = numeric_rows(
            &db,
            case["duckdb_sql"].as_str().unwrap(),
            case["output_domains"].as_array().unwrap().len(),
        );
        let expected = sorted(case["expected_output_rows"].as_array().unwrap().clone());
        assert_eq!(
            actual, expected,
            "{id}: complete terminal output with multiplicity"
        );
        assert_eq!(
            actual.len(),
            case["expected_terminal_count"].as_u64().unwrap() as usize
        );
        assert_domain(&actual, case);

        // Source-instance identities distinguish duplicates and physical vs synthetic NULL.
        assert_eq!(
            identity_rows(
                &db,
                case["identity_sql"].as_str().unwrap(),
                case["identity_column_count"].as_u64().unwrap() as usize,
            ),
            sorted(
                case["expected_source_identities"]
                    .as_array()
                    .unwrap()
                    .clone()
            ),
            "{id}: exact source-origin multiplicities"
        );
        if let Some(negative_sql) = case["not_true_sql"].as_str() {
            let rejected = identity_rows(
                &db,
                negative_sql,
                case["not_true_column_count"].as_u64().unwrap() as usize,
            );
            assert_eq!(
                rejected,
                sorted(case["expected_not_true_pairs"].as_array().unwrap().clone()),
                "{id}: SQL FALSE or UNKNOWN, not TRUE"
            );
            let sources = &case["physical_sources"];
            let pairs =
                sources["l"].as_array().unwrap().len() * sources["r"].as_array().unwrap().len();
            assert_eq!(
                case["expected_source_identities"].as_array().unwrap().len() + rejected.len(),
                pairs,
                "{id}: complete positive/NOT TRUE Cartesian pair partition"
            );
        }
        if id == "J71-FK-KEY" {
            assert!(
                db.execute("INSERT INTO l(id,k,a) VALUES ('bad',99,NULL)", [])
                    .is_err(),
                "enforced physical foreign key must reject missing referenced source"
            );
        }
    }
    assert!(positive > 0 && negative > 0);
    assert_eq!(seen.len(), 15);
}

#[test]
fn conditional_post_outer_filter_keeps_synthetic_null_rows_without_creating_sources() {
    // The original J71-OUTER-WHERE form is not covered by the TASK-111
    // reversed-ON case: WHERE is evaluated AFTER right null-extension.
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR NOT NULL,k INTEGER,a INTEGER);
         CREATE TABLE r(id VARCHAR NOT NULL,k INTEGER,a INTEGER);
         INSERT INTO l VALUES ('l1',1,NULL),('l2',2,NULL),('l3',NULL,NULL);
         INSERT INTO r VALUES ('r1',1,NULL);",
    )
    .expect("complete source bags");
    assert_eq!(
        identity_rows(
            &db,
            "SELECT l.id FROM l LEFT JOIN r ON l.k=r.k WHERE r.k IS NULL",
            1,
        ),
        sorted(vec![json!(["l2"]), json!(["l3"])]),
        "two preserved outer rows, no invented physical right NULL"
    );
    assert_eq!(
        numeric_rows(
            &db,
            "SELECT l.k FROM l LEFT JOIN r ON l.k=r.k WHERE r.k IS NULL",
            1,
        ),
        sorted(vec![json!([2]), json!([null])]),
        "post-null-extension selection must retain NULL-join-key left row"
    );
    assert_eq!(
        identity_rows(&db, "SELECT l.id,r.id FROM l JOIN r ON l.k=r.k", 2),
        vec![json!(["l1", "r1"])],
        "only the left key 1 has a true physical partner"
    );
    assert_eq!(
        source_rows(&db, "r"),
        vec![json!({"id":"r1","k":1,"a":null})]
    );
}

#[test]
fn conditional_parent_inventory_has_named_child_or_parent_evidence_without_native_overclaim() {
    let freeze: Value = serde_json::from_str(FREEZE).unwrap();
    let child_110: Value =
        serde_json::from_str(include_str!("../docs/task-110-acceptance-freeze.json")).unwrap();
    let child_113: Value =
        serde_json::from_str(include_str!("../docs/task-113-acceptance-freeze.json")).unwrap();
    let child_114: Value =
        serde_json::from_str(include_str!("../docs/task-114-acceptance-freeze.json")).unwrap();
    let mut ids = BTreeSet::new();
    let conditional = freeze["conditional"].as_array().unwrap();
    assert_eq!(conditional.len(), 12);
    for cell in conditional {
        let id = cell["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate conditional {id}");
        assert_eq!(cell["classification"], "conditional");
        assert_eq!(cell["dialects"].as_array().unwrap().len(), 13);
        assert_eq!(
            cell["native_vendor_execution"],
            "not_certified_by_portable_DuckDB_or_parser"
        );
        assert_eq!(
            cell["release_disposition"],
            "reviewed_portable_only_native_laws_not_certified"
        );
        assert!(cell["residual_boundary"]
            .as_str()
            .is_some_and(|v| !v.is_empty()));
        assert_eq!(cell["owner"], "TASK-116", "{id}: release boundary owner");
        assert!(cell["native_missing_evidence"].as_str().is_some());
        if id == "J71-OUTER-WHERE" {
            assert_eq!(cell["parent_fixture"], "71-C116-N-OUTER-WHERE");
            continue;
        }
        let child_owner = cell["child_owner"].as_str().expect("source child proof");
        let child = match child_owner {
            "TASK-110" => &child_110,
            "TASK-113" => &child_113,
            "TASK-114" => &child_114,
            other => panic!("{id}: no owning child evidence for {other}"),
        };
        let child_id = cell["child_fixture"]
            .as_str()
            .expect("concrete child oracle");
        assert!(
            child["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case["id"] == child_id),
            "{id}: declared child witness {child_id} missing"
        );
    }
    assert_eq!(freeze["postgres_spelling_alias"], "postgres");
    assert_eq!(freeze["dialect_identities"].as_array().unwrap().len(), 13);
    assert_eq!(ids.len(), 12);
}

#[test]
fn frozen_parent_cells_cover_each_original_acceptance_criterion() {
    let freeze: Value = serde_json::from_str(FREEZE).unwrap();
    for ac in 1..=6 {
        let mandatory = freeze["mandatory"].as_array().unwrap().iter().any(|cell| {
            cell["acceptance_criteria"]
                .as_array()
                .unwrap()
                .contains(&json!(ac))
        });
        let conditional = freeze["conditional"]
            .as_array()
            .unwrap()
            .iter()
            .any(|cell| {
                cell["acceptance_criteria"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(ac))
            });
        assert!(
            mandatory || conditional,
            "parent original AC {ac} has no frozen proof cell"
        );
    }
}

#[test]
fn all_parent_mandatory_and_post_outer_shared_sql_forms_parse_across_canonical_dialects() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("parent inventory");
    let schemas = ["l", "r", "t"]
        .into_iter()
        .map(|name| {
            RelationSchema::new(
                name,
                ["id", "k", "a"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            column,
                            if column == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("typed catalog evidence")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("complete source types");
    let conditional_sql = "SELECT l.k FROM l LEFT JOIN r ON l.k=r.k WHERE r.k IS NULL";
    let mut sql_forms = freeze["mandatory"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            (
                case["id"].as_str().unwrap(),
                case["original_sql"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    sql_forms.push(("J71-OUTER-WHERE", conditional_sql));

    for (id, sql) in sql_forms {
        for dialect in freeze["dialect_identities"].as_array().unwrap() {
            let dialect = dialect.as_str().unwrap();
            let entry = if id == "J71-OUTER-WHERE" {
                freeze["conditional"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|case| case["id"] == id)
                    .expect("outer WHERE parent variant")
            } else {
                freeze["mandatory"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|case| case["id"] == id)
                    .expect("mandatory parent variant")
            };
            assert_eq!(
                entry["dialect_classifications"][dialect]["parser"], "expected_parser_equivalent",
                "{id}/{dialect}: frozen parser expectation"
            );
            let dialect_impl = dialect_from_name(dialect).expect("one of 13 parser identities");
            let source = SqlInput::inline(sql);
            let input = ConfiguredSqlInput::new(id, &source, dialect, dialect_impl.as_ref());
            let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap_or_else(
                |error| panic!("{id}/{dialect}: parser/analysis boundary: {error}"),
            );
            assert_eq!(
                bundle.layers().len(),
                1,
                "{id}/{dialect}: analyzed, not silently omitted"
            );
            let query = bundle.inputs()[0]
                .statements()
                .iter()
                .find_map(|statement| match statement {
                    ProtocolStatement::Query(query) => Some(query),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{id}/{dialect}: missing canonical query"));
            let expected_kinds: &[JoinKind] = match id {
                "J71-I3-CHAIN" | "J71-I3-WHERE" => &[JoinKind::Inner, JoinKind::Inner],
                "J71-LEFT-EMPTY" | "J71-OUTER-REVERSED" | "J71-OUTER-WHERE" => &[JoinKind::Left],
                "J71-RIGHT-EMPTY" => &[JoinKind::Right],
                "J71-FULL-UNMATCHED" => &[JoinKind::Full],
                "J71-SEMI-ABSENCE" => &[JoinKind::LeftSemi],
                "J71-ANTI-ABSENCE" => &[JoinKind::LeftAnti],
                _ => &[JoinKind::Inner],
            };
            let actual_kinds = query
                .joins()
                .iter()
                .map(|join| join.kind())
                .collect::<Vec<_>>();
            assert_eq!(
                actual_kinds, expected_kinds,
                "{id}/{dialect}: canonical join kind and topology"
            );
            assert!(
                query.joins().iter().all(|join| join.condition().is_some()),
                "{id}/{dialect}: cannot silently omit the ON predicate"
            );
            assert!(
                query.diagnostics().is_empty(),
                "{id}/{dialect}: unexpected unsupported semantics: {:?}",
                query.diagnostics()
            );
        }
    }
}

#[test]
fn every_conditional_parent_dialect_cell_checks_the_actual_canonical_boundary() {
    // Check the original parent SQL, not only the child fixture ID or a
    // statement count. A dialect parse is not evidence of native execution.
    let freeze: Value = serde_json::from_str(FREEZE).expect("TASK-116 matrix");
    let schemas = ["l", "r", "t"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "k", "a"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            column,
                            if column == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("typed source column")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("complete source catalog");
    let mut mismatches = Vec::new();
    for cell in freeze["conditional"]
        .as_array()
        .expect("12 conditional cells")
    {
        let id = cell["id"].as_str().expect("parent conditional ID");
        let sql = cell["original_sql"].as_str().expect("original parent SQL");
        for dialect in freeze["dialect_identities"]
            .as_array()
            .expect("13 parser paths")
        {
            let dialect = dialect.as_str().expect("dialect name");
            let parser = dialect_from_name(dialect).expect("known dialect parser");
            let source = SqlInput::inline(sql);
            let configured = ConfiguredSqlInput::new(id, &source, dialect, parser.as_ref());
            let actual = match analyze_configured_inputs_with_catalog(&[configured], &catalog) {
                Err(_) => "parser_boundary",
                Ok(bundle) => {
                    let query = bundle.inputs()[0]
                        .statements()
                        .iter()
                        .find_map(|statement| match statement {
                            ProtocolStatement::Query(query) => Some(query),
                            _ => None,
                        });
                    if let Some(query) = query {
                        let joins = query.joins();
                        let first = joins.first();
                        let expected_count = if id == "J71-ALIAS-REUSE" { 2 } else { 1 };
                        let shape_ok = joins.len() == expected_count && match id {
                            "J71-CROSS" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Cross && j.condition().is_none()
                            }),
                            "J71-USING" | "J71-NATURAL" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Inner && j.condition().is_some()
                            }),
                            "J71-OR-ON" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Inner
                                    && matches!(j.condition(), Some(Predicate::Or(_)))
                            }),
                            "J71-NONEQUI" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Inner
                                    && matches!(
                                        j.condition(),
                                        Some(Predicate::Comparison(c))
                                            if c.operator() == ComparisonOperator::Lt
                                    )
                            }),
                            "J71-NULLSAFE" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Inner
                                    && matches!(
                                        j.condition(),
                                        Some(Predicate::Comparison(c))
                                            if c.operator() == ComparisonOperator::IsNotDistinctFrom
                                    )
                            }),
                            "J71-ASOF" => first
                                .is_some_and(|j| j.execution() == Some(JoinExecution::AsOfNearest)),
                            "J71-RANGE-TIME" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Inner && j.condition().is_some()
                            }),
                            "J71-LATERAL" => first.is_some_and(|j| {
                                j.execution() == Some(JoinExecution::LateralInner)
                            }),
                            "J71-APPLY" => first
                                .is_some_and(|j| j.execution() == Some(JoinExecution::CrossApply)),
                            "J71-ALIAS-REUSE" => joins.iter().all(|j| j.kind() == JoinKind::Inner),
                            "J71-OUTER-WHERE" => first.is_some_and(|j| {
                                j.kind() == JoinKind::Left && j.condition().is_some()
                            }),
                            _ => false,
                        };
                        if !shape_ok {
                            "parsed_unclassified"
                        } else {
                            let codes = query
                                .diagnostics()
                                .iter()
                                .map(|diagnostic| diagnostic.code())
                                .collect::<Vec<_>>();
                            match id {
                                "J71-ASOF" if codes.contains(&"unsupported_asof_nearest_prior") => {
                                    "parsed_asof_residual"
                                }
                                "J71-LATERAL"
                                    if codes.contains(&"unsupported_lateral_row_scope") =>
                                {
                                    "parsed_lateral_residual"
                                }
                                "J71-APPLY" if codes.contains(&"unsupported_cross_apply") => {
                                    "parsed_cross_apply_residual"
                                }
                                "J71-ASOF" | "J71-LATERAL" | "J71-APPLY" => {
                                    "parsed_missing_residual"
                                }
                                "J71-OUTER-WHERE" if codes.is_empty() => {
                                    "expected_parser_equivalent"
                                }
                                _ if codes.is_empty() => "parser_equivalent",
                                _ => "parsed_with_residual",
                            }
                        }
                    } else {
                        "parsed_not_query"
                    }
                }
            };
            let frozen = cell["dialect_classifications"][dialect]["parser"]
                .as_str()
                .expect("reviewed frozen parser status");
            if actual != frozen {
                mismatches.push(json!({"id":id,"dialect":dialect,"observed":actual,"frozen":frozen}));
            }
        }
    }
    assert!(mismatches.is_empty(), "TASK116_CONDITIONAL_MISMATCHES={mismatches:?}");
}

#[test]
fn every_original_task_71_inventory_cell_is_preserved_without_scope_waivers() {
    let parent: Value =
        serde_json::from_str(include_str!("../docs/task-71-acceptance-freeze.json"))
            .expect("original canonical TASK-71 freeze");
    let child: Value = serde_json::from_str(FREEZE).expect("TASK-116 independent freeze");
    for classification in ["mandatory", "conditional"] {
        let expected = parent[classification].as_array().unwrap();
        let observed = child[classification].as_array().unwrap();
        assert_eq!(
            expected.len(),
            observed.len(),
            "{classification}: original case count unchanged"
        );
        let mut seen = BTreeSet::new();
        for original in expected {
            let id = original["id"].as_str().unwrap();
            let candidate = observed
                .iter()
                .find(|case| case["id"] == id)
                .unwrap_or_else(|| panic!("{classification}: missing original {id}"));
            assert!(
                seen.insert(id),
                "{classification}: duplicate original ID {id}"
            );
            assert_eq!(
                candidate["original_sql"], original["sql"],
                "{id}: preserve frozen original SQL"
            );
            assert_eq!(
                candidate["acceptance_criteria"], original["acceptance_criteria"],
                "{id}: retain all original AC ownership"
            );
            if classification == "mandatory" {
                assert_eq!(
                    candidate["expected_terminal_count"], original["expected"]["terminal_count"],
                    "{id}: cannot weaken original terminal oracle"
                );
                assert_eq!(candidate["classification"], "exact_required");
            } else {
                assert_eq!(candidate["classification"], "conditional");
                assert_eq!(
                    candidate["owner"], "TASK-116",
                    "{id}: conditional review stays parent-owned"
                );
            }
        }
        assert_eq!(seen.len(), observed.len());
    }
}
