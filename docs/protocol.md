# SQL Semantic Protocol contract

This document describes the single active SQL Semantic Protocol contract. Its version is always the Cargo package version emitted through `PROTOCOL_VERSION`, and `schema/protocol.schema.json` carries the same version as its `protocol_version` constant.

Versioned contract files are immutable historical snapshots. Runtime code, release automation, and active-contract tests use the unversioned paths so a version bump updates content rather than requiring path renames.

## Inputs and deterministic identity

`inputs` contains every analyzed input unit. An input has a unique `id`, source metadata, dialect, and its statements in source order.

Implementations should preserve caller order and generate deterministic IDs when the caller does not provide one. The canonical generated form is `input-0001`, `input-0002`, and so on, with width expanding rather than imposing an input-count limit. IDs are opaque references to consumers; only uniqueness and determinism within equivalent analysis matter.

Inline source labels are optional. File sources retain their path. Raw SQL text is intentionally not part of the semantic protocol.

## Source schemas
For dbt inputs, `catalog.json` is the authoritative source of warehouse-introspected columns and
types when a relation is present there. When a physical dependency is absent from the catalog,
`manifest.json` column `data_type` declarations are accepted as lower-authority schema evidence
when the declared schema is complete. The adapter never lets a manifest declaration override
catalog evidence. Missing declared datatypes fail explicitly with the affected relation and column
names rather than being guessed. `analyze_dbt_manifest_with_schemas` supports the same
typed schema validation and `dbt_manifest` provenance without a catalog; it is equivalent
to `analyze_dbt_artifacts` with an empty catalog. With no catalog, every physical
dependency must have declared columns and types; absent declarations fail with the relation
name. The CLI uses this path only when the default adjacent `catalog.json` does not exist.
An explicitly provided catalog path is mandatory even if it is missing.

Optional `source_schemas` metadata carries typed schema evidence for physical source relations.
Each entry has a canonical relation identity, its columns, and may include `source_kind`.
`dbt_catalog` means warehouse-introspected evidence; `dbt_manifest` means declared fallback
evidence. Caller-supplied generic schemas omit `source_kind` unless an adapter attached provenance.

Datatypes use one parser-independent recursive model rather than dialect-specific names. The model
covers booleans; signed and unsigned integer widths; exact decimals; floating point; character,
binary, and bit strings; dates, times, timestamps, and intervals; UUID; JSON/semi-structured
documents; arrays; maps; structs/tuples/nested records; unions; enums; sets; nullable wrappers;
table-valued types; geometry/geography; PostgreSQL regclass and text-search values; and explicit
custom, any, unspecified, and trigger types. Nested fields carry their own canonical datatype.

Dialect syntax is normalized at the protocol boundary. Equivalent storage aliases intentionally
collapse to one logical type. Timestamp variants retain a common `timestamp` datatype for 1.x
compatibility, with optional `timestamp_zone` on each schema column (`with_time_zone` or
`without_time_zone`). When SQL metadata does not explicitly prove a timezone, the field is
omitted. Catalog, manifest, ODCS physical-type, and caller-supplied schema evidence use this
same representation. JSON, JSONB, Snowflake VARIANT/OBJECT, and Redshift SUPER normalize
to `json`; representation-only wrappers such as ClickHouse LowCardinality normalize to the
underlying logical type. Semantically relevant nullability remains explicit.
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


### ODCS v3.2 schema evidence

ODCS v3.2 YAML is an external metadata evidence source for the same protocol-owned schema and
constraint types. Adapter-created source schemas use `source_kind: "external_metadata"`. Schema
objects resolve `physicalName` when present and otherwise `name` through the same canonical
relation resolver used by SQL and dbt.

For datatypes, ODCS `physicalType` is preferred and normalized through the selected SQL dialect.
`logicalType` is lower-authority fallback evidence. Existing dbt catalog and dbt manifest schema
evidence retains higher authority during bundle enrichment, but contradictory ODCS evidence is
surfaced explicitly instead of being hidden by precedence.

ODCS `required`, primary-key positions, property uniqueness, relationships, and enums map into
the canonical `relation_constraints` representation. ODCS declarations use
`external_metadata` constraint provenance and `unknown` enforcement. External contract
references are resolved only from contracts explicitly supplied to the adapter; the adapter does
not perform network or filesystem fetching.

## Typed predicate-domain literals

When typed source-schema evidence is available, predicate-domain literals are checked against the
canonical datatype before a domain can participate in the exact row-condition contract. Exact
boolean, integer, date, time, and interval literals retain their canonical literal type; integer
literals used with decimal columns normalize to decimal literals. Integer bounds are range-checked
against declared signedness and bit width. Lossy numeric coercions are never treated as exact.

Comparison domains remain available for typed string, floating point and timestamp predicates,
but row-condition exactness is `conditional` while warehouse settings have not been declared.
The `comparison_assumptions` array identifies each assumption and its dependent condition:
`binary_collation` for case-sensitive binary string comparison, `no_char_padding` for CHAR,
`no_nan` and `signed_zero_equivalent` for numeric comparison, and `session_time_zone`
for timestamps whose literal needs session interpretation. A caller may attest settings using
the library `AnalysisBundle::declare_comparison_assumptions` API, repeatable CLI
`--assume <name>`, or `comparison_assumptions` in analysis-manifest JSON. The dbt CLI path
accepts the same CLI option. Each requirement is retained with its condition identity and a
`declared` boolean; declared assumptions are also listed at the root. `exact` means all
requirements were attested, `conditional` means requirements remain open and no residuals
exist, and `residual` means at least one predicate still cannot be represented.

A timestamp without time zone compared to an offset-free typed literal is unconditional exact
when schema evidence explicitly identifies the column as timezone-free. A timezone-aware
timestamp comparison with an explicit literal UTC offset is also unconditional exact.
Unqualified timestamp types remain conditional. Contradictory timezone-free schema evidence
and an offset-bearing literal produce Unknown with a `literal_type_mismatch` residual,
never an invented domain; declaring `session_time_zone` cannot override that conflict.

SQL typed literals recognized by sqlparser include `TIMESTAMP '...'`,
`TIMESTAMP WITH TIME ZONE '...'`, and the `TIMESTAMPTZ '...'` alias.
Other dialect-specific timestamp aliases are treated equivalently when sqlparser
recognizes them. The spelling alone does not grant exactness: an explicit UTC offset
and timezone-aware *column* schema evidence are required for unconditional exactness.
With no literal offset, a timezone-aware column requires `session_time_zone`
regardless of the typed literal spelling. Accepted offset spellings are `Z`/`z`,
`+HH`/`-HH`, `+HHMM`/`-HHMM`, and `+HH:MM`/`-HH:MM`; these all
normalize to signed `HH:MM` form.

Timestamp bounds use a single parser-independent textual value contract:
`YYYY-MM-DD HH:MM:SS[.fraction][offset]`. A literal `T` separator is normalized
to a space; seconds are required; trailing fractional zeros are dropped, including
the decimal point if the fraction becomes empty. The wall-clock portion is not shifted
by the analyzer. An explicit offset is normalized to `+HH:MM` or `-HH:MM`;
`Z` and `z` normalize to `+00:00`, and `+HH` / `+HHMM` normalize to
`+HH:00` / `+HH:MM`. Offsets beyond 14 hours are not accepted.

- `timestamp_zone: without_time_zone` only emits offset-free bounds; an offset-bearing
  literal instead gives Unknown and a residual.
- `timestamp_zone: with_time_zone` emits the normalized offset when explicit. An
  offset-free bound represents local wall time and requires `session_time_zone`
  evidence before the condition can be exact.
- An unqualified timestamp column preserves a normalized literal, but remains
  conditional on `session_time_zone` because its storage timezone semantics are unknown.

Malformed or noncanonicalizable timestamp values produce Unknown with the
`comparison_semantics` residual rather than a misleading exact domain.
String comparisons lacking a typed schema use the same binary-collation requirement as
typed strings, so no schema path is silently declared exact. NULL-only conditions
do not depend on any comparison setting. NaN is excluded by the declared `no_nan`
assumption; signed zeros compare equal under `signed_zero_equivalent`.
Binary, document, collection, geometry, search, vendor-defined, and otherwise opaque
types remain residual until exact scalar comparisons are specified.

Lexical strings are never implicitly converted to typed DATE, TIME, TIMESTAMP, or numeric domains.
Without schema evidence, lexical-literal domain derivation is preserved without claiming
datatype-aware coercion semantics.

### Schema-reference validation

When typed schema evidence exists for a physical relation (caller-supplied, dbt
catalog, manifest-declared, or ODCS), a query reference to a column absent
from that schema produces `unknown_schema_column` and a row-condition
`analysis_diagnostic` residual. Such a query must not be treated as an
exact composed outcome. Without schema evidence, lack of a declaration alone
does not prove the reference invalid.

Constraint enrichment validates local key and column references and referenced
foreign-key columns against every available relation schema. Constraints with
missing columns are not emitted as valid facts; they instead produce
`invalid_constraint_column` on the relation's constraint metadata. Accepted
values incompatible with a declared scalar datatype similarly produce
`incompatible_accepted_value` without emitting that constraint. Opaque
types are not rejected based on unsupported conversion guesses. For dbt
relationships tests, when the resolvable `to` target contradicts the
`depends_on` target, `inconsistent_relationship_target` is emitted and
the foreign key is withheld. Validation applies to canonical constraints
regardless of their original adapter or SQL DDL source.

## Relation constraints

Optional `relation_constraints` metadata records parser-independent relation and column
constraints for named relations. It is separate from transformation semantics and from
`source_schemas`; a relation can therefore carry metadata even when a queryless `CREATE TABLE`
produces no transformation layer.

Each relation entry contains a deterministic `constraints` array. Relation-scoped unsupported
metadata remains in that entry's optional `diagnostics` array. Diagnostics that cannot be
assigned to a canonical relation are emitted once in the optional top-level
`constraint_diagnostics` array.

Constraint `kind` is one of:

- `primary_key`: ordered `columns` forming the declared primary key.
- `unique_key`: ordered `columns` forming one declared unique key.
- `foreign_key`: ordered local `columns`, `referenced_relation`, and ordered
  `referenced_columns`. Local and referenced arity must match.
- `not_null`: one non-null `column`.
- `accepted_values`: one `column`, a finite typed `values` array, and the source `quote`
  setting.

Accepted values are emitted as typed scalar objects so strings, booleans, integers, unsigned
integers, non-integral numbers, and null remain distinguishable. Non-integral numbers retain a
deterministic textual representation instead of being round-tripped through floating point.
Canonical values contain literal semantics, never opaque SQL expressions. For dbt
`accepted_values`, `quote: true` preserves JSON scalar typing and string values as strings.
With `quote: false`, string metadata is SQL syntax and is normalized only when it is a portable
scalar literal: NULL, TRUE/FALSE, signed or unsigned integers, JSON-compatible non-integral
numbers, or standard single-quoted SQL strings with doubled-quote escaping. Any other raw
expression emits `unsupported_dbt_accepted_value` and the adapter does not emit that
accepted-values constraint. Consumers can therefore compare `ConstraintValue` directly with the
canonical column `DataType` without parsing SQL.

NULL admission is part of the canonical constraint semantics. Primary-key and `not_null`
constraints reject NULL. Unique-key, foreign-key, and accepted-values constraints apply to
non-NULL values and admit NULL unless a separate `not_null` constraint applies to the same
column. For accepted values, a `ConstraintValue::Null` entry records literal metadata but is not
required for NULL to be admissible. The public `RelationConstraint::admits_null` method exposes
this rule directly to library consumers.

Composite keys are not flattened into independent single-column facts. Multiple different unique
keys and foreign keys can coexist. Distinct primary-key definitions for the same relation are not
silently resolved: both facts remain present and the relation emits a
`conflicting_primary_key` diagnostic. Invalid or unresolved foreign-key metadata fails or emits
an explicit diagnostic at the adapter boundary rather than guessing a target. The dbt adapter
normalizes foreign-key and `relationships` targets from manifest resource unique IDs, exact
canonical relation names, and dbt `ref(...)` or `source(...)` references. Self-referencing
relationships resolve to the attached canonical relation. A target that cannot be mapped to a
relation present in the manifest is rejected rather than emitted as raw Jinja or unmatched text.

Independent accepted-value constraints for the same column and quoting semantics are conjunctive,
so their canonical value set is the deterministic intersection. An empty intersection remains
present and emits `unsatisfiable_accepted_values`. Conflicting dbt quoting semantics are retained
with `conflicting_accepted_values_quoting` instead of choosing one declaration.

Every constraint carries one or more `evidence` items. Evidence separates:

- `source_kind`: `sql_ddl`, `dbt_constraint`, `dbt_test`, or `external_metadata`.
- `source_id`: deterministic identity of the declaration/test that supplied the fact.
- `enforcement`: `enforced`, `not_enforced`, or `unknown`.

Identical semantic constraints from multiple sources coalesce while retaining all distinct
evidence. Enforcement is evidence-specific. SQL only records `enforced` or `not_enforced` when
the parsed DDL explicitly states it; otherwise it is `unknown`. dbt model/column constraints and
generic tests are declarations/assertions, so their enforcement is `unknown` rather than inferred
from an adapter or warehouse.

Canonical relation constraints are source-independent. Every supported adapter that can provide
equivalent evidence must translate it into these same constraint types rather than defining a
source-specific representation.

Direct SQL analysis normalizes parser-supported column- and table-level `PRIMARY KEY`, `UNIQUE`,
and `FOREIGN KEY` clauses, column-level `NOT NULL`, and finite accepted-value sets expressed as
non-negated `CHECK (column IN (...))` constraints. CHECK expressions that cannot be represented
safely as a finite accepted-value set emit an `unsupported_check_constraint` diagnostic instead
of being guessed or silently dropped. The dbt adapter normalizes explicit `primary_key`, `unique`,
`foreign_key`, and `not_null` declarations plus built-in `unique`, `relationships`,
`not_null`, and `accepted_values` tests from `manifest.json`. Unsupported attached dbt generic test kinds are reported with an `unsupported_dbt_test`
diagnostic rather than silently disappearing. Singular tests, which do not carry
`test_metadata`, are reported as `unsupported_dbt_singular_test`. If a test cannot be scoped
to a canonical relation, its diagnostic is emitted in the bundle-level
`constraint_diagnostics` array instead of being dropped.

Built-in dbt tests are carried only when their execution config preserves the canonical constraint
meaning. The default `severity: error`, `warn_if: "!= 0"`, `error_if: "!= 0"`, and
`fail_calc: "count(*)"` are compatible. A non-empty `where`, non-default severity or threshold,
non-null `limit`, or non-default `fail_calc` produces `unsupported_dbt_test_config` and the
adapter does not emit a stronger unconditional constraint. dbt `check` constraints use the same
`unsupported_check_constraint` diagnostic as unsupported SQL CHECK semantics; other unsupported
dbt constraint types, including `custom`, use `unsupported_dbt_constraint`. The catalog artifact
does not contribute constraint facts.

Constraint metadata is preserved through target selection. Key facts are not propagated through
projection, join, aggregation, set operations, INSERT, or MERGE simply because a source key exists.
Consumers must treat absent derived-key metadata as unknown. The public
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

Joins carried through referenced CTEs and derived tables keep their logical `left` and `right` relation participants, because one local relation can depend on more than one physical relation. Equality-column operands inside the join `condition` are different: when each local projection hop is a plain column copy, those operands are rewritten to the physical source relation and column. Computed, ambiguous, or otherwise unprovable mappings become explicit unknown expressions with an `unresolved_join_column_lineage` diagnostic rather than exposing a local alias as though it were physical. CTEs that are not referenced by the query contribute no dependencies or joins to that query's relation semantics.

Predicates inside a referenced CTE or derived table follow the same no-silent-loss rule. WHERE, HAVING, and QUALIFY predicates that reduce safely to independent source-column domains are carried onto physical columns through plain-copy lineage. An outer predicate on a plain-copy derived-table column is likewise resolved to its physical source before domain derivation. Predicates whose full row semantics cannot be represented by those domains are residual in the local scope's exactness contract. Predicates that cannot be mapped through local lineage, including comparisons over aggregate, window, CASE, or other computed local columns, also emit an `unresolved_local_predicate` diagnostic. The analyzer may still retain conservative domains where possible, but an unknown domain remains unknown through later intersections. Diagnostics from unreferenced CTEs do not leak into the enclosing query.

Table-producing factors that do not yet have a trustworthy output-schema representation, including unresolved table functions and UNNEST-like sources, emit explicit `unsupported_table_factor` diagnostics. The analyzer does not invent columns or silently drop those factors.

## Row-condition exactness

Source-column `column_domains` are useful only when a consumer knows whether they preserve the correlations needed to decide which source-row combinations qualify. Every query scope therefore emits `condition_exactness` with `status: "exact" | "conditional" | "residual"` and a deterministic `residual_conditions` list. A residual identifies a stable `reason`, its clause (`select`, `where`, `on`, `having`, `qualify`, `set_operation`, or `row_set_operator`), and a deterministic identity inside the scope.

Residual reason codes are stable consumer-facing identifiers. An individual residual is
emitted once for each distinct predicate-tree location, so two unsupported conditions
with the same code are not collapsed. The `identity` denotes the condition's path
within its owning SQL clause, while `origin` identifies the local scope or layer.

| Reason | Cause |
| --- | --- |
| `cross_column_disjunction` | OR spans multiple source columns and therefore carries correlation not representable by independent domains. |
| `logical_not` | Logical NOT is not on the exactness allow-list. |
| `column_comparison` | A comparison relates source columns outside an equi-join. |
| `computed_expression` | A condition depends on a computed expression rather than a plain source column. |
| `comparison_semantics` | Comparison semantics are not established for the source datatype. |
| `literal_type_mismatch` | A scalar literal has an incompatible canonical type. |
| `lossy_coercion` | Numeric coercion would lose precision. |
| `out_of_range_literal` | A literal exceeds the source datatype's allowed range or precision. |
| `unknown_schema_column` | A referenced physical column is absent from available schema evidence. |
| `subquery_predicate` | A condition uses a subquery predicate. |
| `unsupported_predicate` | A normalized predicate is unknown or unsupported. |
| `constant_false_or_null` | A constant FALSE or NULL condition cannot be represented as independent column domains. |
| `having` | HAVING drops groups based on aggregate or grouped results. |
| `qualify` | QUALIFY drops rows based on window results. |
| `outer_join` | An outer-join condition cannot constrain both inputs as an inner-row equality contract. |
| `unsupported_join_kind` | A non-inner join kind decides membership through semantics not represented by domains/equalities. |
| `repeated_source_instance` | The same physical relation is read through multiple instances whose identities collapse in column domains. |
| `limit` | LIMIT affects which otherwise qualifying rows survive. |
| `offset` | OFFSET affects which otherwise qualifying rows survive. |
| `fetch` | FETCH affects which otherwise qualifying rows survive. |
| `distinct_on` | DISTINCT ON selects rows based on ordering within duplicate groups. |
| `table_sample` | TABLESAMPLE drops otherwise qualifying source rows. |
| `set_operation` | Set-operation row membership is not represented exactly. |
| `top` | TOP limits the qualifying row set. |
| `prewhere` | PREWHERE is parsed but not represented as an exact source predicate. |
| `connect_by` | CONNECT BY changes row membership through recursive traversal. |
| `analysis_diagnostic` | A condition-affecting analysis diagnostic prevents an exact guarantee. |
| `correlated_subquery` | A correlated subquery depends on an outer row outside the local domain contract. |

Typed string, floating-point, and timestamp domains normally retain their values and
use `comparison_assumptions` instead of residuals when comparison settings are
unknown. `comparison_semantics` applies only when no representable conditional
comparison semantics exists. Typed literal failures use `literal_type_mismatch`,
`lossy_coercion`, or `out_of_range_literal` as appropriate. An undeclared physical
column is classified as `unknown_schema_column` in the clause containing the
invalid reference, including `select` for projection-only references. Even
a projection-only invalid column prevents claiming exact row semantics,
rather than hiding an invalid schema reference as an exact result.

A bare boolean column, `NOT flag`, `IS TRUE`, `IS FALSE`,
`IS NOT TRUE`, and `IS NOT FALSE` are normalized to literal comparisons.
`IS NOT TRUE` and `IS NOT FALSE` use null-safe inequality and therefore
**admit SQL NULL**, unlike `NOT flag` and ordinary `= FALSE`.
When typed schema evidence is present, it must identify the column as boolean;
an incompatible datatype produces an explicit typed-literal residual.

For one query scope, `exact` means that before row-set shaping, a combination of source rows qualifies exactly when every emitted source-column domain is satisfied and every reported inner-join equality is satisfied. It does not mean that projection, ordering, grouping, or duplicate elimination preserve individual rows. Plain projection, `ORDER BY`, `GROUP BY` without `HAVING`, and ordinary `DISTINCT` therefore do not make the row-condition contract residual.

Exact predicate forms are deliberately allow-listed. Direct column-to-literal comparisons, `IS NULL`/`IS NOT NULL`, literal `IN`/`NOT IN`, literal `BETWEEN`/`NOT BETWEEN`, conjunctions of exact predicates, and disjunctions whose constraints refer to one source column can be exact. Inner joins can additionally use column equality plus safely reducible scalar filters such as `t.x = u.y AND t.a > 5`; the scalar join filter is also emitted in `column_domains`. Cross joins have no row condition and can be exact.

The CI differential conformance suite treats this allow-list as a completeness contract, not merely
a soundness bound: every allow-listed scalar shape must claim exact status and retain its expected
physical source-column domains when moved through plain-copy CTEs, derived tables, or producer
layers. Two- and three-relation equality joins must preserve physical equality endpoints across
explicit and implicit join syntax. Seeded predicate and join fixtures compare those claims with
DuckDB, and verify that unsupported shapes continue to report residual conditions rather than
claiming unsafe precision. Typed comparison checks cover portable scalar types, while string,
floating-point, and timestamp comparisons remain conditional until their comparison
assumptions are declared.

Resolved `composed_semantics.join_equalities` is the canonical consumer-facing representation of those correlations. Each entry names both physical leaf relation/column endpoints, a relation-instance identity, the join kind, and the originating transformation layer. Equalities from referenced CTEs, derived tables, and resolved producer layers compose transitively through plain-copy lineage. A column equality in `WHERE` between distinct relation instances, including comma/CROSS-join syntax, is represented as an implicit inner equality rather than an `unknown` scalar domain. Outer-join equalities are listed with their join kind while the scope remains residual. Repeated/self-joins remain residual when physical instance identity cannot be proven safely.

Everything else is default-denied unless analysis proves an exact representation. Residual cases include cross-column OR correlations, logical `NOT` outside the directly normalized negated forms above, non-equality column-to-column comparisons outside join equality, computed/function/CAST/pattern predicates, subquery predicates, `HAVING`, `QUALIFY`, outer/semi/anti or otherwise unsupported joins, repeated instances of the same physical relation, `LIMIT`, `OFFSET`, `FETCH`, `TOP`, `DISTINCT ON`, `TABLESAMPLE`, and set operations. UNION is currently residual as well, including branches with different constraints; INTERSECT and EXCEPT are residual.

Nested subqueries carry their own joins, column domains, and exactness contract. A correlated subquery is residual in its nested scope because its membership depends on the outer row. Resolved bundle composition carries this contract transitively. A composed layer is exact only when its owning query, every referenced CTE or derived-table scope, and every resolved ancestor layer are exact. Each composed residual includes an `origin` object containing the originating `layer_id` and scope identity, using `query` for the layer query and identities such as `cte:x` or `derived:d` for local relations. Residuals from unreferenced CTEs do not participate. Domain intersection is default-deny as well: once any contributing constraint for a column is `unknown`, intersection with known, unbounded, or empty domains remains `unknown` with its reason, so composition cannot accidentally regain false precision.

Analyzer diagnostics follow the same default-deny rule. Diagnostics that can affect membership become an `analysis_diagnostic` residual unless the AST construct was already classified directly. Diagnostics that are independently non-membership-only include ordering/layout diagnostics, output/lineage/wildcard diagnostics, and expression/function/window diagnostics when they occur outside a row condition; if the same expression occurs in a WHERE/ON predicate, the normalized predicate classifier supplies the residual. `unsupported_top`, `unsupported_prewhere`, `unsupported_connect_by`, `unsupported_limit`, `unsupported_fetch`, and set-operation diagnostics are not duplicated because their AST operators already create explicit residuals.


The diagnostic policy is deterministic. These diagnostics are non-membership metadata or output-shape concerns and therefore do not independently make a scope residual: `unsupported_order_by`, `unsupported_cluster_by`, `unsupported_distribute_by`, `unsupported_sort_by`, `unsupported_exclude`, `unsupported_select_into`, `unsupported_value_table_mode`, `ambiguous_output_lineage`, `unresolved_output_lineage`, `unresolved_wildcard`, `ambiguous_named_window`, `cyclic_named_window`, `unresolved_named_window`, `unsupported_window_order_option`, `unsupported_window_override`, `unsupported_group_by_modifier`, `unsupported_lock`, `unsupported_for_clause`, `unsupported_settings`, `unsupported_format_clause`, `unsupported_merge_output`, `unsupported_merge_insert_row`, and `unsupported_queryless_create_table`.

`unsupported_expression` and `unsupported_function` are also not converted directly from diagnostics because their membership effect is classified from the normalized predicate when they occur in a row condition. Likewise, `unsupported_top`, `unsupported_prewhere`, `unsupported_connect_by`, `unsupported_limit`, `unsupported_fetch`, `set_operation_arity_mismatch`, `unresolved_set_operation_output`, and `unsupported_set_operation_alignment` are not duplicated as `analysis_diagnostic` residuals because their owning AST construct already emits a specific residual condition. Every other statement-level or nested-query diagnostic is default-denied into an `analysis_diagnostic` residual.

NULL membership is part of the value-domain contract and is available through `ValueDomain::admits_null()`: `unbounded` admits NULL; ordered `ranges` do not; an include set admits NULL only when NULL is listed; an exclude set admits NULL unless NULL is listed; `empty` rejects NULL; and `unknown` returns an unknown NULL-membership result. Consequently, `IS NULL`, `IS NOT NULL`, `<>`, `NOT IN`, `IS DISTINCT FROM`, and range predicates preserve their SQL NULL behavior in the emitted domains.

## Output value domains

Every projected output column carries a `domain` independently from the query's source-column `column_domains`. Source-column domains describe values required to satisfy predicates. Output domains describe values the produced expression can return.

The analyzer derives output domains only when SQL semantics make them safe. Literals produce singleton domains, boolean-valued expressions produce `{false, true}`, `COUNT` is bounded below by zero, and `ROW_NUMBER` is bounded below by one. A `QUALIFY` predicate on a projected alias can further refine that derived domain without incorrectly constraining the physical columns used by the expression.

CASE expressions use expression kind `case` and preserve an optional simple-CASE operand, ordered WHEN/THEN branches, and the optional ELSE expression. Their output domains are the conservative union of branch result domains. Each WHEN branch also emits `source_domains`, and the explicit or implicit ELSE emits `else_source_domains`. A reachable selection is represented as an OR of alternatives; the column domains inside one alternative are an AND. Selection domains account for earlier branches evaluating to anything other than TRUE, including SQL NULL behavior. Direct copies through CTEs and derived tables preserve the upstream CASE expression and its branch domains. During multi-layer composition, CASE branch domains are remapped through every proven plain-copy identity hop until they name physical source columns; if any hop is computed, ambiguous, missing, or otherwise non-invertible, that branch selection becomes explicit `unknown` instead of retaining an intermediate relation name as if it were physical. Unreachable branches are explicit, while conditions that cannot be reduced safely carry an unknown reason. CASE reachability is local to CASE control flow and does not intersect query-level `WHERE`, `HAVING`, or `QUALIFY` constraints; those remain separately available in `column_domains`. A CASE without ELSE includes SQL NULL as a possible result.

Safe constant integer unary and arithmetic expressions are evaluated with checked arithmetic. Functions, transforms, or bounds that cannot be proven safely remain explicit `unknown` output domains rather than guessed.

Output domains are preserved through multi-layer composition. A direct projection or rename of an upstream derived column retains the producer's domain in the composed final outcome. When that upstream expression is a CASE, the composed output also retains the CASE expression and its safely remapped branch source domains.

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
