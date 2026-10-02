# Protocol 0.2 multi-query composition contract

`schema/protocol-v0.2.schema.json` defines SQL Semantic Protocol version `0.2.0`. It extends the single-input 0.1.0 semantics with an explicit composition model for an arbitrary number of SQL inputs while keeping sqlparser types outside the public contract.

Version 0.2.0 is the single active runtime contract. A one-input invocation and a many-input invocation use the same root document shape and protocol version. Version 0.1.0 remains only as historical reference material and is not emitted by current runtime code. Version 0.2.0 reuses the earlier statement, column-domain, and output definitions for local analysis and adds input identity, transformation layers, relation-resolution edges, graph components, composed semantics, and final outcomes.

## Inputs and deterministic identity

`inputs` contains every analyzed input unit. An input has a unique `id`, source metadata, dialect, and its statements in source order.

Implementations should preserve caller order and generate deterministic IDs when the caller does not provide one. The canonical generated form is `input-0001`, `input-0002`, and so on, with width expanding rather than imposing an input-count limit. IDs are opaque references to consumers; only uniqueness and determinism within equivalent analysis matter.

Inline source labels are optional. File sources retain their path. Raw SQL text is intentionally not part of the semantic protocol.

## Transformation layers

A `layer` points to exactly one statement through `input_id` plus zero-based `statement_index`. The layer records:

- `produces`: named relations or an anonymous query result
- `consumes`: relation names referenced by the local statement
- `composed_semantics`: the transitive, outcome-focused result after following producers

Named datasets use `{"kind":"relation","name":"..."}`. Anonymous query results use `{"kind":"anonymous","layer_id":"..."}`, which makes them addressable without inventing a physical relation name.

The statement stored under the referenced input is the local semantics for that layer. `composed_semantics` is deliberately separate. A resolved composed result contains physical leaf dependencies, composed column domains, and the final output with transitive lineage. Composition does not rewrite or flatten local joins and predicates into a synthetic SQL statement.

If composition cannot be trusted, it is emitted as `status: "unresolved"` with one of `missing_producer`, `ambiguous_producer`, `cycle`, or `unsupported` plus diagnostics. Producers must not guess through these states.

## Dependency graph

Each consumed relation has a graph edge from its consumer layer. `resolution` is one of:

- `resolved`: exactly one producer layer was selected
- `external`: no producer exists in this bundle and the relation is intentionally treated as an external leaf
- `missing`: a producer is required but unavailable
- `ambiguous`: more than one producer could satisfy the relation
- `cycle`: following the producer participates in a dependency cycle
- `unsupported`: the relation could not be composed safely for another explicit reason

This distinction matters because an external warehouse table is valid input, while a missing intermediate model is an incomplete bundle.

Unrelated SQL inputs remain separate connected components in the same protocol document. Components contain their layer IDs and determine final outcomes independently.

## Final outcomes

`graph.components[].final_outcomes` contains the terminal datasets for that component. A component may have multiple final outcomes. A cyclic or otherwise unresolved component may have no final outcome and must carry a diagnostic explaining why.

The protocol does not define a single global "final query". This allows one invocation to describe multiple independent transformation chains and multiple terminal datasets.

## Output scopes

Output scope is a rendering option, not an analysis option. Producers must build the complete bundle, dependency graph, and composed semantics before applying a scope.

- `all` renders every transformation layer and is the compatibility default.
- `final` renders only layers whose produced dataset appears in a component's `final_outcomes`.

The `inputs` and `graph` sections remain complete under both scopes. A final-scope document can therefore contain graph references to intermediate layer IDs whose full layer result is intentionally omitted from `layers`. Terminal layers keep their already-composed physical dependencies, value domains, and transitive column lineage. Standalone anonymous query results are terminal outcomes and remain visible in final scope.

When a bundle contains one transformation layer, final and all-layer rendering are identical.

## OpenLineage interoperability

The SQL Semantic Protocol is the source of truth for semantic composition. OpenLineage is an export target, not part of the core model.

The library adapter emits resolved named layers as OpenLineage `DatasetEvent` objects using schema `2-0-2` and the `LineageDatasetFacet` schema `1-0-0`. The caller provides the dataset namespace and event timestamp because neither can be inferred reliably from SQL text. Composed physical dependencies become dataset-level lineage inputs, while transitive output-column lineage becomes field-level lineage inputs.

The adapter deliberately omits protocol semantics that OpenLineage does not represent directly, including predicate trees and value domains. Those values remain present in the original protocol document. Anonymous outputs and unresolved layers are not exported as datasets because doing so would require inventing identity or precision.

## Deterministic ordering

For equivalent inputs and configuration, producers must serialize arrays deterministically:

- `inputs`: caller input order; generated IDs follow this order
- each input's `statements`: source statement order
- `layers`: by input order, then statement index
- `produces`: named relations lexicographically, then anonymous results by layer ID
- `consumes`: lexicographic normalized relation name
- resolved `dependencies`: lexicographic normalized physical relation name
- composed `column_domains` and `output`: the 0.1.0 ordering rules
- `graph.edges`: consumer layer order, then relation, then producer layer ID
- `components`: by their earliest layer
- `component.layer_ids`: topological order where possible, using layer ID as the tie-breaker; for cyclic components use layer ID order
- `final_outcomes`: named relations lexicographically, then anonymous results by layer ID
- composition diagnostics: severity, then code, then input ID, layer ID, relation, and message

Object member order is not semantically significant.

## Example

`examples/protocol-v0.2.json` contains three inputs. Two form a chain from `raw.orders` through `stage.orders` to `mart.orders`; the third independently reads `raw.customers` and produces an anonymous result. The example therefore demonstrates both related and unrelated queries, named and anonymous outputs, transitive semantics, graph components, and per-component final outcomes.
