---
id: TASK-1
title: Define protocol v0 JSON contract
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
dependencies: []
---

## Description

Define the first public SQL Semantic Protocol contract as versioned JSON before expanding the analyzer. The contract must describe SQL semantics independently of the sqlparser AST and be stable enough for external consumers to depend on.

The initial contract should cover the semantics required by the current project direction: source relations, physical dependencies, joins and relation relationships, structured predicates, column value domains, final output columns and lineage, and explicit unknown or unsupported semantics.

A representative shape for a query such as `SELECT b FROM t WHERE a > 10` is:

~~~json
{
  "protocol_version": "0.1.0",
  "source": {
    "dialect": "generic"
  },
  "statements": [
    {
      "kind": "query",
      "sources": [
        {
          "kind": "relation",
          "name": "t",
          "alias": null
        }
      ],
      "dependencies": ["t"],
      "joins": [],
      "predicates": {
        "where": {
          "kind": "comparison",
          "left": {
            "kind": "column",
            "relation": "t",
            "name": "a"
          },
          "operator": "gt",
          "right": {
            "kind": "literal",
            "type": "integer",
            "value": 10
          }
        },
        "having": null,
        "qualify": null
      },
      "column_domains": [
        {
          "column": {
            "relation": "t",
            "name": "a"
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
                  "inclusive": false
                },
                "upper": null
              }
            ]
          }
        }
      ],
      "output": {
        "columns": [
          {
            "name": "b",
            "expression": {
              "kind": "column",
              "relation": "t",
              "name": "b"
            },
            "lineage": [
              {
                "relation": "t",
                "column": "b"
              }
            ]
          }
        ]
      },
      "diagnostics": []
    }
  ]
}
~~~

The exact field names may change while this task is implemented, but the semantic responsibilities above are part of the v0 contract.

## Acceptance Criteria

- [x] A versioned JSON Schema for protocol v0 exists in the repository and is treated as the public protocol contract.
- [x] The schema contains no sqlparser AST types or parser-specific representation details.
- [x] The contract models source relations, physical dependencies, joins, structured predicates, column domains, final output columns, lineage, and diagnostics.
- [x] Predicate structure preserves boolean semantics instead of flattening `AND`, `OR`, or `NOT`.
- [x] Value domains can represent unbounded values, open and closed bounds, disjoint ranges or sets, an empty domain, and an unknown domain.
- [x] Unsupported or unresolved semantics are representable explicitly without inventing precise information.
- [x] Ordering rules for serialized collections are defined so equivalent analysis produces deterministic JSON.
- [x] At least one representative protocol document validates against the JSON Schema.
