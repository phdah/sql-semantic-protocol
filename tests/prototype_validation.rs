use serde_json::json;
use sql_semantic_protocol::{analyze_sql, to_json, Error};
use sqlparser::dialect::{GenericDialect, SnowflakeDialect};

#[test]
fn representative_generic_query_matches_complete_protocol_document() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.id AS order_id, c.name AS customer_name
         FROM sales.orders AS o
         JOIN crm.customers AS c ON o.customer_id = c.id
         WHERE o.total >= 10 AND o.total < 20",
        "generic",
        &dialect,
    )
    .expect("representative query should analyze");

    let actual: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol should serialize as JSON");

    let expected = json!({
        "protocol_version": "0.2.0",
        "inputs": [
            {
                "id": "input-0001",
                "source": {
                    "kind": "inline",
                    "label": null
                },
                "dialect": "generic",
                "statements": [
            {
                "kind": "query",
                "sources": [
                    {
                        "kind": "relation",
                        "name": "sales.orders",
                        "alias": "o"
                    },
                    {
                        "kind": "relation",
                        "name": "crm.customers",
                        "alias": "c"
                    }
                ],
                "dependencies": [
                    "crm.customers",
                    "sales.orders"
                ],
                "joins": [
                    {
                        "kind": "inner",
                        "left": {
                            "relation": "sales.orders",
                            "alias": "o"
                        },
                        "right": {
                            "relation": "crm.customers",
                            "alias": "c"
                        },
                        "condition": {
                            "kind": "comparison",
                            "left": {
                                "kind": "column",
                                "relation": "o",
                                "name": "customer_id"
                            },
                            "operator": "eq",
                            "right": {
                                "kind": "column",
                                "relation": "c",
                                "name": "id"
                            }
                        }
                    }
                ],
                "predicates": {
                    "where": {
                        "kind": "and",
                        "operands": [
                            {
                                "kind": "comparison",
                                "left": {
                                    "kind": "column",
                                    "relation": "o",
                                    "name": "total"
                                },
                                "operator": "gte",
                                "right": {
                                    "kind": "literal",
                                    "type": "integer",
                                    "value": 10
                                }
                            },
                            {
                                "kind": "comparison",
                                "left": {
                                    "kind": "column",
                                    "relation": "o",
                                    "name": "total"
                                },
                                "operator": "lt",
                                "right": {
                                    "kind": "literal",
                                    "type": "integer",
                                    "value": 20
                                }
                            }
                        ]
                    },
                    "having": null,
                    "qualify": null
                },
                "column_domains": [
                    {
                        "column": {
                            "relation": "sales.orders",
                            "name": "total"
                        },
                        "domain": {
                            "kind": "ranges",
                            "ranges": [
                                {
                                    "lower": {
                                        "value": {
                                            "kind": "literal",
                                            "type": "integer",
                                            "value": 10
                                        },
                                        "inclusive": true
                                    },
                                    "upper": {
                                        "value": {
                                            "kind": "literal",
                                            "type": "integer",
                                            "value": 20
                                        },
                                        "inclusive": false
                                    }
                                }
                            ]
                        }
                    }
                ],
                "output": {
                    "columns": [
                        {
                            "name": "order_id",
                            "expression": {
                                "kind": "column",
                                "relation": "o",
                                "name": "id"
                            },
                            "domain": {
                                "kind": "unbounded"
                            },
                            "lineage": [
                                {
                                    "relation": "sales.orders",
                                    "column": "id"
                                }
                            ]
                        },
                        {
                            "name": "customer_name",
                            "expression": {
                                "kind": "column",
                                "relation": "c",
                                "name": "name"
                            },
                            "domain": {
                                "kind": "unbounded"
                            },
                            "lineage": [
                                {
                                    "relation": "crm.customers",
                                    "column": "name"
                                }
                            ]
                        }
                    ]
                },
                "diagnostics": []
            }
                ]
            }
        ],
        "layers": [
            {
                "id": "layer-0001",
                "statement": {
                    "input_id": "input-0001",
                    "statement_index": 0
                },
                "produces": [
                    {
                        "kind": "anonymous",
                        "layer_id": "layer-0001"
                    }
                ],
                "consumes": [
                    "crm.customers",
                    "sales.orders"
                ],
                "composed_semantics": {
                    "status": "resolved",
                    "dependencies": [
                        "crm.customers",
                        "sales.orders"
                    ],
                    "column_domains": [
                        {
                            "column": {
                                "relation": "sales.orders",
                                "name": "total"
                            },
                            "domain": {
                                "kind": "ranges",
                                "ranges": [
                                    {
                                        "lower": {
                                            "value": {
                                                "kind": "literal",
                                                "type": "integer",
                                                "value": 10
                                            },
                                            "inclusive": true
                                        },
                                        "upper": {
                                            "value": {
                                                "kind": "literal",
                                                "type": "integer",
                                                "value": 20
                                            },
                                            "inclusive": false
                                        }
                                    }
                                ]
                            }
                        }
                    ],
                    "output": {
                        "columns": [
                            {
                                "name": "order_id",
                                "expression": {
                                    "kind": "column",
                                    "relation": "o",
                                    "name": "id"
                                },
                                "domain": {
                                    "kind": "unbounded"
                                },
                                "lineage": [
                                    {
                                        "relation": "sales.orders",
                                        "column": "id"
                                    }
                                ]
                            },
                            {
                                "name": "customer_name",
                                "expression": {
                                    "kind": "column",
                                    "relation": "c",
                                    "name": "name"
                                },
                                "domain": {
                                    "kind": "unbounded"
                                },
                                "lineage": [
                                    {
                                        "relation": "crm.customers",
                                        "column": "name"
                                    }
                                ]
                            }
                        ]
                    },
                    "diagnostics": []
                }
            }
        ],
        "graph": {
            "edges": [
                {
                    "consumer_layer_id": "layer-0001",
                    "relation": "crm.customers",
                    "resolution": "external",
                    "producer_layer_ids": []
                },
                {
                    "consumer_layer_id": "layer-0001",
                    "relation": "sales.orders",
                    "resolution": "external",
                    "producer_layer_ids": []
                }
            ],
            "components": [
                {
                    "id": "component-0001",
                    "layer_ids": [
                        "layer-0001"
                    ],
                    "final_outcomes": [
                        {
                            "kind": "anonymous",
                            "layer_id": "layer-0001"
                        }
                    ],
                    "diagnostics": []
                }
            ],
            "diagnostics": []
        }
    });

    assert_eq!(actual, expected);
}

#[test]
fn corpus_covers_ctes_nested_subqueries_and_functions() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH recent AS (
             SELECT o.id AS order_id, o.customer_id
             FROM raw.orders AS o
             WHERE o.created_at >= '2026-01-01'
         )
         SELECT r.order_id, COALESCE(c.name, 'unknown') AS customer_name
         FROM recent AS r
         JOIN (
             SELECT customer_id, name
             FROM raw.customers
         ) AS c ON r.customer_id = c.customer_id",
        "generic",
        &dialect,
    )
    .expect("CTE and nested subquery should analyze");

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol should serialize as JSON");
    let statement = &json["inputs"][0]["statements"][0];

    assert_eq!(
        statement["dependencies"],
        json!(["raw.customers", "raw.orders"])
    );
    assert_eq!(statement["output"]["columns"][0]["name"], "order_id");
    assert_eq!(
        statement["output"]["columns"][0]["lineage"],
        json!([{"relation": "raw.orders", "column": "id"}])
    );
    assert_eq!(
        statement["output"]["columns"][1]["expression"]["kind"],
        "function"
    );
    assert_eq!(
        statement["output"]["columns"][1]["expression"]["name"],
        "COALESCE"
    );
    assert_eq!(
        statement["output"]["columns"][1]["lineage"],
        json!([{"relation": "raw.customers", "column": "name"}])
    );
}

#[test]
fn snowflake_group_having_and_qualify_keep_known_grouping_semantics() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql(
        "SELECT a
         FROM t
         WHERE a > 0
         GROUP BY a
         HAVING a < 10
         QUALIFY a IS NOT NULL",
        "snowflake",
        &dialect,
    )
    .expect("Snowflake query should parse and analyze");

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol should serialize as JSON");
    let statement = &json["inputs"][0]["statements"][0];

    assert_eq!(json["inputs"][0]["dialect"], "snowflake");
    assert_eq!(statement["predicates"]["where"]["kind"], "comparison");
    assert_eq!(statement["predicates"]["having"]["kind"], "comparison");
    assert_eq!(statement["predicates"]["qualify"]["kind"], "is_null");
    assert_eq!(
        statement["output"]["columns"][0]["lineage"],
        json!([{"relation": "t", "column": "a"}])
    );
    assert_eq!(statement["aggregation"]["distinct"], false);
    assert_eq!(statement["aggregation"]["group_by"]["kind"], "expressions");
    assert_eq!(
        statement["aggregation"]["group_by"]["expressions"][0]["expression"]["name"],
        "a"
    );
    assert!(!statement["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "unsupported_group_by"));
}

#[test]
fn set_operation_preserves_dependencies_output_and_typed_semantics() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT id FROM source_a UNION SELECT id FROM source_b",
        "generic",
        &dialect,
    )
    .expect("set operation should parse");

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol should serialize as JSON");
    let statement = &json["inputs"][0]["statements"][0];

    assert_eq!(statement["dependencies"], json!(["source_a", "source_b"]));
    assert_eq!(statement["set_operation"]["operator"], "union");
    assert_eq!(statement["set_operation"]["quantifier"], "distinct");
    assert_eq!(statement["output"]["columns"][0]["name"], "id");
    assert_eq!(
        statement["output"]["columns"][0]["lineage"],
        json!([
            {"relation": "source_a", "column": "id"},
            {"relation": "source_b", "column": "id"}
        ])
    );
    assert!(!statement["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "unsupported_query_body"));
}

#[test]
fn parse_failure_is_separate_from_unsupported_semantic_analysis() {
    let dialect = GenericDialect {};
    let error = analyze_sql("SELECT (", "generic", &dialect)
        .expect_err("malformed SQL should fail before semantic analysis");

    assert!(matches!(error, Error::Parse(_)));
}

#[test]
fn representative_analysis_is_byte_deterministic() {
    let dialect = GenericDialect {};
    let sql = "SELECT o.id, c.name
               FROM sales.orders AS o
               JOIN crm.customers AS c ON o.customer_id = c.id
               WHERE o.total >= 10 AND o.total < 20";

    let first = analyze_sql(sql, "generic", &dialect).expect("first analysis should succeed");
    let second = analyze_sql(sql, "generic", &dialect).expect("second analysis should succeed");

    assert_eq!(to_json(&first), to_json(&second));
}
