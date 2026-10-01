# Protocol v0 contract

`schema/protocol-v0.schema.json` is the public contract for SQL Semantic Protocol version `0.1.0`. The schema models query semantics, not parser syntax, and protocol producers must not expose sqlparser AST types through it.

The contract covers source relations, physical dependencies, joins, structured predicates, column value domains, output columns, lineage, and diagnostics. Unknown and unsupported semantics are explicit values rather than omitted information. Fatal SQL parse failures are outside the protocol document and are reported by the producer as errors.

## Predicate structure

Boolean structure is preserved. `and`, `or`, and `not` contain nested predicates, so a producer must not flatten predicates in a way that changes boolean semantics. Unsupported or unresolved predicate fragments use the explicit `unsupported` or `unknown` forms.

## Value domains

A column domain is one of:

- `unbounded`: no known restriction
- `ranges`: one or more ranges, allowing open or closed bounds and disjoint intervals
- `set`: included values or excluded values, selected by `mode`
- `empty`: no value can satisfy the known constraints
- `unknown`: the analyzer cannot determine a safe domain

A null lower or upper range bound means that side is unbounded. Domain values use the same typed literal representation as expressions.

## Deterministic ordering

Protocol producers must use these array ordering rules so equivalent semantic analysis can be serialized deterministically:

- `statements`: original SQL statement order
- `sources`: first semantic appearance in the query, with duplicate relation/alias pairs removed
- `dependencies`: lexicographic order by normalized physical relation name
- `joins`: SQL join order
- boolean predicate `operands`: SQL evaluation-tree order; do not reorder `and` or `or` children
- `column_domains`: lexicographic order by resolved relation name, then column name; unresolved relations sort before resolved relations
- `set.values`: literal type, then canonical JSON value
- `output.columns`: SELECT-list order
- `lineage`: lexicographic order by relation, then column, with duplicates removed
- `diagnostics`: `area`, then `code`, then `message`

Object member order is not semantically significant. A concrete emitter may additionally define a canonical object-member order and whitespace policy when byte-identical JSON is required.

## Example

`examples/protocol-v0.json` is a representative protocol document for:

```sql
SELECT b FROM t WHERE a > 10;
```

It validates against the v0 JSON Schema.
