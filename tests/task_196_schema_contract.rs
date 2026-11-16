//! TASK-196 original CREATE schema facts across all 13 public dialect parsers.
//!
//! The SQL itself is immutable. Facts are independently specified instead of
//! inferred from the first analyzer output. Parser rejection is not waived.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, CheckCompareOperator, ColumnValueKind, ConstraintValue,
    DataType, GeneratedValueExpression, RelationConstraint, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn original<'a>(freeze: &'a Value, suffix: &str) -> &'a Value {
    let id = format!("88-C177-{suffix}");
    freeze["cases"].as_array().expect("38 original cases").iter()
        .find(|case| case["id"] == id).expect("frozen original CREATE variant")
}

#[test]
fn original_schema_create_variants_preserve_constraints_defaults_generated_and_decimal_types() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("task-own original source");
    let mut verified = 0;
    let mut rejected = 0;
    for (suffix, label) in [
        ("S183", "primary_key"),
        ("S184", "foreign_key"),
        ("S185", "unique"),
        ("S186", "not_null"),
        ("S187", "check"),
        ("S188", "default"),
        ("S189", "generated"),
        ("S190", "decimal_primary_key"),
        ("S192", "odcs_original_sql"),
    ] {
        let case = original(&freeze, suffix);
        let sql = case["sql"].as_str().expect("unchanged SQL");
        for cell in case["cells"].as_array().expect("13 dialect cells") {
            let name = cell["dialect"].as_str().expect("dialect");
            let dialect = dialect_from_name(name).expect("public registry");
            if cell["original_observed_status"] == "R" {
                assert!(
                    analyze_inputs(&[SqlInput::inline(sql)], name, dialect.as_ref()).is_err(),
                    "{suffix}/{name}: parser rejection must stay explicit"
                );
                rejected += 1;
                continue;
            }
            let bundle = analyze_inputs(&[SqlInput::inline(sql)], name, dialect.as_ref())
                .unwrap_or_else(|err| panic!("{suffix}/{name}: {err}"));
            let facts = bundle.relation_constraints().iter()
                .find(|set| set.relation() == "orders");
            let keys = facts.map(|set| set.constraints()).unwrap_or(&[]);
            let generators = facts.map(|set| set.column_value_sources()).unwrap_or(&[]);
            let schema = bundle.source_schemas().iter()
                .find(|schema| schema.relation() == "orders")
                .expect("original CREATE must expose a typed table");
            let matching = match label {
                "primary_key" | "decimal_primary_key" | "odcs_original_sql" =>
                    keys.iter().any(|fact| matches!(fact,
                        RelationConstraint::PrimaryKey(key) if key.columns() == ["id"]
                    )),
                "foreign_key" => keys.iter().any(|fact| matches!(fact,
                    RelationConstraint::ForeignKey(fk)
                    if fk.columns() == ["customer_id"] && fk.referenced_relation() == "customers"
                        && fk.referenced_columns() == ["id"]
                )),
                "unique" => keys.iter().any(|fact| matches!(fact,
                    RelationConstraint::UniqueKey(key) if key.columns() == ["id"]
                )),
                "not_null" => keys.iter().any(|fact| matches!(fact,
                    RelationConstraint::NotNull(rule) if rule.column() == "id"
                )),
                "check" => keys.iter().any(|fact| matches!(fact,
                    RelationConstraint::CheckComparison(rule)
                    if rule.column() == "id" && rule.operator() == CheckCompareOperator::Gt
                        && rule.value() == &ConstraintValue::Integer(0)
                )),
                "default" => generators.iter().any(|rule| {
                    rule.column() == "id" && rule.kind() == ColumnValueKind::Default
                        && rule.value() == Some(&ConstraintValue::Integer(1))
                }),
                "generated" => generators.iter().any(|rule| {
                    rule.column() == "amount" && rule.kind() == ColumnValueKind::Generated
                        && matches!(rule.expression(),
                            Some(GeneratedValueExpression::Add(left, right))
                            if matches!(left.as_ref(), GeneratedValueExpression::Column(col) if col == "id")
                                && matches!(right.as_ref(),
                                    GeneratedValueExpression::Literal(ConstraintValue::Integer(1)))
                        )
                }),
                _ => false,
            };
            assert!(matching, "{suffix}/{name}: missing {label} source semantic fact");
            if matches!(label, "decimal_primary_key" | "odcs_original_sql") {
                let amount = schema.columns().iter()
                    .find(|column| column.name() == "amount")
                    .expect("original DECIMAL column");
                assert_eq!(
                    amount.data_type(),
                    &DataType::Decimal {precision:Some(10),scale:Some(2)},
                    "{suffix}/{name}: decimal storage contract"
                );
            }
            verified += 1;
        }
    }
    assert_eq!(verified + rejected, 9*13);
    assert_eq!(rejected, 1, "original generated column parser rejection");
}
