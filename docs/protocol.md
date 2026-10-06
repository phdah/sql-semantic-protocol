# SQL Semantic Protocol contract

This document describes the single active SQL Semantic Protocol contract. Its version is always the Cargo package version emitted through `PROTOCOL_VERSION`, and `schema/protocol.schema.json` carries the same version as its `protocol_version` constant.

Versioned contract files are immutable historical snapshots. Runtime code, release automation, and active-contract tests use the unversioned paths so a version bump updates content rather than requiring path renames.

## Inputs and deterministic identity

`inputs` contains every analyzed input unit. An input has a unique `id`, source metadata, dialect, and its statements in source order.

Implementations should preserve caller order and generate deterministic IDs when the caller does not provide one. The canonical generated form is `input-0001`, `input-0002`, and so on, with width expanding rather than imposing an input-count limit. IDs are opaque references to consumers; only uniqueness and determinism within equivalent analysis matter.

Inline source labels are optional. File sources retain their path. Raw SQL text is intentionally not part of the semantic protocol.

## Source schemas
For dbt inputs, complete source schemas come from the paired `catalog.json` artifact rather than
being inferred from model SQL or declared manifest columns. `manifest.json` supplies resource and
relation identity; `catalog.json` supplies warehouse-introspected columns and type strings. The
adapter joins them by dbt `unique_id`, normalizes each catalog type into the canonical protocol
datatype model, and rejects missing catalog coverage for physical dependencies.


Optional `source_schemas` metadata carries declared datatypes for physical source relations when
the caller supplies catalog schema information. Each entry has a canonical relation identity and
its columns in declared order.

Datatypes use one parser-independent recursive model rather than dialect-specific names. The model
covers booleans; signed and unsigned integer widths; exact decimals; floating point; character,
binary, and bit strings; dates, times, timestamps, and intervals; UUID; JSON/semi-structured
documents; arrays; maps; structs/tuples/nested records; unions; enums; sets; nullable wrappers;
table-valued types; geometry/geography; PostgreSQL regclass and text-search values; and explicit
custom, any, unspecified, and trigger types. Nested fields carry their own canonical datatype.

Dialect syntax is normalized at the protocol boundary. Equivalent storage aliases intentionally
collapse to one logical type. Timestamp timezone variants normalize to `timestamp` because
consumers can use one UTC representation; JSON, JSONB, Snowflake VARIANT/OBJECT, and Redshift
SUPER normalize to `json`; and representation-only wrappers such as ClickHouse LowCardinality
normalize to the underlying logical type. Semantically relevant nullability remains explicit.
Unrecognized vendor or user-defined types are preserved as `custom` with their name and
modifiers.

`SchemaColumn::from_sql_type` accepts dialect-specific datatype syntax and performs this
normalization through sqlparser. Consumers can alternatively construct the canonical `DataType`
directly. Raw sqlparser AST types never appear in the protocol contract.

This metadata exists so consumers such as test-data generators can interpret unbounded or
literal-constrained source columns without maintaining a second SQL parser or private schema
contract. When no typed schema metadata is supplied, `source_schemas` is omitted rather than
inferred.

Source schemas are evidence supplied by the caller; they do not weaken or replace analyzed value
domains. Consumers combine the declared type with `composed_semantics.column_domains` and must
still treat unknown, empty, or unresolved semantics explicitly.

## Relation constraints

Optional `relation_constraints` metadata records parser-independent key declarations for named
relations. It is separate from transformation semantics and from `source_schemas`; a relation can
therefore carry key metadata even when a queryless `CREATE TABLE` produces no transformation
layer.

Each relation entry contains an ordered `constraints` array. Constraint `kind` is one of:

- `primary_key`: ordered `columns` forming the declared primary key.
- `unique_key`: ordered `columns` forming one declared unique key.
- `foreign_key`: ordered local `columns`, `referenced_relation`, and ordered
  `referenced_columns`. Local and referenced arity must match.

Composite keys are not flattened into independent single-column facts. Multiple different unique
keys and foreign keys can coexist. Distinct primary-key definitions for the same relation are not
silently resolved: both facts remain present and the relation emits a
`conflicting_primary_key` diagnostic. Invalid or unresolved foreign-key metadata fails or emits
an explicit diagnostic at the adapter boundary rather than guessing a target.

Every constraint carries one or more `evidence` items. Evidence separates:

- `source_kind`: `sql_ddl`, `dbt_constraint`, `dbt_test`, or `external_metadata`.
- `source_id`: deterministic identity of the declaration/test that supplied the fact.
- `enforcement`: `enforced`, `not_enforced`, or `unknown`.

Identical semantic constraints from multiple sources coalesce while retaining all distinct
evidence. Enforcement is evidence-specific. SQL only records `enforced` or `not_enforced` when
the parsed DDL explicitly states it; otherwise it is `unknown`. dbt model/column constraints and
generic tests are declarations/assertions, so their enforcement is `unknown` rather than inferred
from an adapter or warehouse.

Direct SQL analysis normalizes parser-supported column- and table-level `PRIMARY KEY`, `UNIQUE`,
and `FOREIGN KEY` clauses. The dbt adapter normalizes explicit key constraints and built-in
`unique` / `relationships` tests from `manifest.json`. The catalog artifact does not contribute
constraint facts.

Key metadata is preserved through target selection but is not propagated through projection,
join, aggregation, set operations, INSERT, or MERGE simply because a source key exists. Consumers
must treat absent derived-key metadata as unknown. The public
`AnalysisBundle::enrich_relation_constraints` plus the canonical constraint types provide the
adapter-neutral enrichment boundary for external metadata producers.

## Transformation layers

A `layer` points to exactly one statement through `input_id` plus zero-based `statement_index`. The layer records:

- `produces`: named relations or an anonymous query result
- `consumes`: relation names referenced by the local statement
- optional `write_kind`: `definition`, `append`, or `conditional_mutation` for named relation writes
- `composed_semantics`: the transitive, outcome-focused result after following producers

Named datasets use `{"kind":"relation","name":"..."}`. Anonymous query results use `{"kind":"anonymous","layer_id":"..."}`, which makes them addressable without inventing a physical relation name.

The statement stored under the referenced input is the local semantics for that layer. `composed_semantics` is deliberately separate. A resolved composed result contains physical leaf dependencies, composed column domains, and the final output with transitive lineage. Composition does not rewrite or flatten local joins and predicates into a synthetic SQL statement.

If composition cannot be trusted, it is emitted as `status: "unresolved"` with one of `missing_producer`, `ambiguous_producer`, `cycle`, `partial_producer`, or `unsupported` plus diagnostics. Producers must not guess through these states.

A query-backed CREATE is a `definition` write and fully defines its named relation. `INSERT INTO ... SELECT` is an `append` write: the source query is analyzed normally, so the inserted rows retain dependencies, output value domains, and lineage, but the write does not describe rows already present in the target. `MERGE` is a `conditional_mutation` write and records the source dependencies, match condition, and normalized update/insert/delete clauses. Every normalized UPDATE assignment and INSERT value stores both its expression and its conservative `domain`. For MATCHED branches, the match condition and additional clause predicate constrain those write domains; safe equality propagation can transfer a known interval across equal columns. Non-matching branches do not incorrectly assume the positive match condition. Unsupported DML forms remain explicit diagnostics rather than being treated as complete transformations.

These per-write domains describe values the MERGE can introduce or assign. They do not claim to describe the complete post-MERGE table because untouched target rows can remain. The relation therefore continues to compose as a partial producer even when individual written values have precise intervals.

## Dependency graph

Each consumed relation has a graph edge from its consumer layer. `resolution` is one of:

- `resolved`: exactly one producer layer was selected
- `external`: no producer exists in this bundle and the relation is intentionally treated as an external leaf
- `missing`: a producer is required but unavailable
- `ambiguous`: more than one producer could satisfy the relation
- `cycle`: following the producer participates in a dependency cycle
- `partial`: an in-bundle append or conditional mutation writes the relation but does not fully define its contents
- `unsupported`: the relation could not be composed safely for another explicit reason

This distinction matters because an external warehouse table is valid input, while a missing intermediate model is an incomplete bundle. A partial edge still links the downstream reader to the DML writer, but composition stops with `partial_producer` instead of inventing complete target lineage.

Unrelated SQL inputs remain separate connected components in the same protocol document. Components contain their layer IDs and determine final outcomes independently.

## Final outcomes

`graph.components[].final_outcomes` contains the terminal datasets for that component. A component may have multiple final outcomes. A cyclic or otherwise unresolved component may have no final outcome and must carry a diagnostic explaining why.

The protocol does not define a single global "final query". This allows one invocation to describe multiple independent transformation chains and multiple terminal datasets.

All transformation outcomes are always present in `layers`; terminal outcomes are not emitted as a separate reduced protocol. `final_outcomes` classifies which of those already-present outcomes terminate each component. A consumer can therefore operate on every layer, resolve only the referenced terminal layers, or select an individual outcome without asking the producer to regenerate or filter the protocol.

Each terminal layer uses the same `composed_semantics` representation as any other layer, including transitive physical dependencies, value domains, and output lineage. Outcome selection changes only what a consumer chooses to use, never what the protocol producer analyzes or emits.

## Set operations

A query that contains UNION, INTERSECT, or EXCEPT carries an optional `set_operation` tree alongside the existing query semantics. The tree is parser-independent and records `operator`, normalized `quantifier`, and recursive left/right operands. A leaf operand is `{"kind":"query"}`; nested operations use `{"kind":"set_operation", ...}`.

UNION ALL keeps `all`; an omitted quantifier normalizes to `distinct`. Dialect-specific MINUS syntax normalizes to `except`. BY NAME quantifiers are retained so the parsed meaning is not lost, but output-column composition for name-based alignment remains explicitly unsupported.

Set-operation outputs align positionally. Column names follow the left branch. Lineage combines the corresponding branch columns deterministically. An arity mismatch or an unresolved branch prevents the producer from inventing output columns and is reported with a diagnostic.

Column domains remain source-column constraints. Equal constraints from multiple branches can be retained. Different constraints on the same source column degrade to an explicit unknown domain because flattening branch-local alternatives into one scalar restriction would over-claim.

Set-level ORDER BY and LIMIT remain query-level semantics, so their existing explicit unsupported diagnostics attach to the combined set result rather than to an arbitrary branch.

## Aggregation and grouping

Non-window aggregate calls use expression kind `aggregate_function`, distinct from ordinary scalar `function` and `window_function` expressions. Aggregate arguments preserve scalar expressions, `*`, and qualified wildcards. The `distinct` flag belongs to the aggregate argument list, while `filter` contains a normalized predicate or null.

A SELECT carries an optional `aggregation` object when DISTINCT or GROUP BY changes row semantics. `distinct` records duplicate elimination, `distinct_on` preserves PostgreSQL-style DISTINCT ON expressions, and `group_by` is null, `all`, or an ordered list of grouping elements. Grouping elements distinguish ordinary expressions from GROUPING SETS, ROLLUP, and CUBE without exposing sqlparser AST types.

Grouping expressions and aggregate arguments participate in physical dependencies and output lineage. HAVING is analyzed in the grouped SELECT scope, including projected aliases. Aggregate result predicates do not constrain their physical input columns: for example, `HAVING SUM(amount) > 10` is not emitted as a source-column domain for `amount`.

Parser-supported GROUP BY modifiers that do not have a safe protocol representation remain explicit diagnostics rather than being dropped.

## Window functions

Window calls use the expression kind `window_function`. The value contains the normalized underlying function plus a resolved parser-independent window specification:

- `name`: the local named window referenced by the call, or null for an inline specification
- `partition_by`: normalized expressions in SQL order
- `order_by`: normalized expressions plus explicit ascending/descending and NULL ordering when supplied
- `frame`: null when no frame was written, otherwise one `rows`, `range`, or `groups` frame with normalized start and end bounds

Frame shorthand such as `ROWS 2 PRECEDING` normalizes its semantic end bound to `CURRENT ROW`. Bounded offsets remain ordinary protocol expressions. Named windows are resolved only inside the SELECT that defines them. A named specification may safely extend inherited parts that are absent; attempts to replace an already inherited PARTITION BY, ORDER BY, or frame are left unsupported rather than guessed.

Window function arguments, PARTITION BY expressions, ORDER BY expressions, and frame offsets participate in dependency and output-lineage traversal. Ranking functions therefore derive lineage from their partitioning and ordering inputs even when they have no ordinary function arguments.

A QUALIFY predicate may resolve a direct projected alias back to its window expression. Window results do not create scalar source-column domains: comparisons such as `QUALIFY row_number_alias = 1` constrain the computed window result, not the underlying partition or ordering columns.

Unsupported function modifiers and window-ordering options remain explicit diagnostics instead of being silently dropped.

## Subqueries and table sources

Expression and predicate subqueries retain nested semantics rather than becoming parser-shaped or disappearing behind a generic unsupported value.

A scalar subquery is an expression with `kind: "scalar_subquery"`. EXISTS and IN/NOT IN use predicate kinds `exists` and `in_subquery`. All three contain a `subquery` object with:

- `dependencies`: deterministic physical relations read by the nested query
- `correlations`: physical outer-scope columns resolved from qualified correlated references
- `output`: the nested projected output and physical lineage
- `predicates`: nested WHERE, HAVING, and QUALIFY semantics
- `diagnostics`: unsupported or unresolved semantics scoped to the nested query

Correlation resolution is lexical and conservative. A relation alias declared inside the nested SELECT shadows an outer alias with the same name. Ambiguous or unqualified references are not promoted to correlations without enough information to prove the binding.

A derived table is a local relation whose visible columns are exactly its projected output. Parent references cannot reach hidden columns from the derived query. For LATERAL derived tables, preceding visible FROM/JOIN sources are available while computing the derived output lineage, allowing qualified outer references to resolve without treating them as independent physical inputs.

Table-producing factors that do not yet have a trustworthy output-schema representation, including unresolved table functions and UNNEST-like sources, emit explicit `unsupported_table_factor` diagnostics. The analyzer does not invent columns or silently drop those factors.

## Output value domains

Every projected output column carries a `domain` independently from the query's source-column `column_domains`. Source-column domains describe values required to satisfy predicates. Output domains describe values the produced expression can return.

The analyzer derives output domains only when SQL semantics make them safe. Literals produce singleton domains, boolean-valued expressions produce `{false, true}`, `COUNT` is bounded below by zero, and `ROW_NUMBER` is bounded below by one. A `QUALIFY` predicate on a projected alias can further refine that derived domain without incorrectly constraining the physical columns used by the expression.

CASE expressions use expression kind `case` and preserve an optional simple-CASE operand, ordered WHEN/THEN branches, and the optional ELSE expression. Their output domains are the conservative union of branch result domains. Each WHEN branch also emits `source_domains`, and the explicit or implicit ELSE emits `else_source_domains`. A reachable selection is represented as an OR of alternatives; the column domains inside one alternative are an AND. Selection domains account for earlier branches evaluating to anything other than TRUE, including SQL NULL behavior, and local CTE or derived-table columns are remapped to physical source lineage. Unreachable branches are explicit, while conditions that cannot be reduced safely carry an unknown reason. A CASE without ELSE includes SQL NULL as a possible result.

Safe constant integer unary and arithmetic expressions are evaluated with checked arithmetic. Functions, transforms, or bounds that cannot be proven safely remain explicit `unknown` output domains rather than guessed.

Output domains are preserved through multi-layer composition. A direct projection or rename of an upstream derived column retains the producer's domain in the composed final outcome.

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
- composed `column_domains` and `output`: the local statement ordering rules
- `graph.edges`: consumer layer order, then relation, then producer layer ID
- `components`: by their earliest layer
- `component.layer_ids`: topological order where possible, using layer ID as the tie-breaker; for cyclic components use layer ID order
- `final_outcomes`: named relations lexicographically, then anonymous results by layer ID
- composition diagnostics: severity, then code, then input ID, layer ID, relation, and message

Object member order is not semantically significant.

## Example

`examples/protocol.json` contains three inputs. Two form a chain from `raw.orders` through `stage.orders` to `mart.orders`; the third independently reads `raw.customers` and produces an anonymous result. The example therefore demonstrates both related and unrelated queries, named and anonymous outputs, transitive semantics, graph components, and per-component final outcomes.
