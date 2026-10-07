# SQL Semantic Protocol

SQL Semantic Protocol is a dialect-independent semantic representation of SQL queries.

Its purpose is to translate SQL syntax into a stable, deterministic, machine-readable description of what a query means, rather than how the query was written.

The project includes a SQL parser and semantic analyzer that accepts SQL from supported dialects, analyzes the parsed query, and emits the SQL Semantic Protocol. The parser is a producer of the protocol. Consumers should depend on the protocol rather than on the parser's AST or the syntax of the original SQL.

The protocol describes semantics such as:

- the relations and columns a query depends on
- the columns produced by the query and their lineage
- the constraints placed on values by predicates
- declared primary, unique, and foreign keys with provenance and enforcement evidence
- the allowed value domains of columns, including bounded, unbounded, excluded, or disjoint ranges
- relationships between columns and relations introduced by joins and predicates
- other query semantics required to understand the resulting rows and output schema
- semantics that could not be resolved, represented explicitly as unknown or unsupported

The protocol deliberately separates SQL parsing from applications that need to reason about SQL. A consumer should not need to understand a Snowflake `WHERE` clause, a PostgreSQL AST, or a particular SQL parser. It should instead operate on the normalized semantic representation.

The first consumer is `sql-tdg`, the SQL Test Data Generator. It will use the protocol's value-domain and constraint information to determine which input values can satisfy a query and generate appropriate test data. This replaces the SQL parsing and interval derivation currently implemented inside `sql-tdg`.

The protocol is intentionally broader than test-data generation. Future consumers can use the same semantic representation for query analysis, lineage, validation, rewriting, and compilation. One planned use case is a SQL compiler that consumes the protocol and produces equivalent SQL according to a target dialect, formatting rules, or other output restrictions.

The protocol is therefore the contract between SQL and applications that need to reason about SQL semantics:

`SQL -> parser/analyzer -> SQL Semantic Protocol -> consumers`

## Protocol contract

The Cargo package version is the single active protocol version emitted by the library and CLI. It represents one or many SQL inputs with the same root document shape: `inputs`, `layers`, and `graph`. A single SQL string is therefore represented as one element in `inputs`, not by switching to a different protocol version.

The active contract is defined by [`schema/protocol.schema.json`](schema/protocol.schema.json), documented in [`docs/protocol.md`](docs/protocol.md), and demonstrated by [`examples/protocol.json`](examples/protocol.json) and [`examples/protocol-simple.json`](examples/protocol-simple.json).

Versioned protocol artifacts such as `protocol-v0.1*` and `protocol-v0.2*` remain in the repository as immutable historical references. Current runtime code and tests use the unversioned active contract paths above.

TASK-13 resolves transformation layers into a deterministic relation dependency graph. TASK-14 composes semantics through that graph: final outputs expose transitive physical lineage, value domains propagate through safe direct projections and renames, and ambiguous, cyclic, or non-invertible paths remain explicit instead of being guessed. Disconnected pipelines compose independently.

### Set operations

UNION, UNION ALL, INTERSECT, and EXCEPT are analyzed as parser-independent set-operation semantics. Non-standard SQL MINUS syntax is normalized to EXCEPT when the selected sqlparser dialect accepts it.

Set outputs align columns positionally. Output names come from the left branch, while field lineage includes the corresponding columns from every contributing branch. Nested and chained operations retain their recursive operator tree in the optional `set_operation` field. Omitted set quantifiers normalize to DISTINCT semantics.

If branches expose incompatible arity, output semantics remain unresolved and an explicit diagnostic is emitted. Branch-local value constraints are retained when compatible; conflicting constraints on the same source column degrade to an explicit unknown domain rather than being guessed. BY NAME alignment is represented in the operation tree but output composition remains explicitly unsupported.

### Aggregation and grouping

Grouped aggregates are represented separately from ordinary scalar functions. Aggregate expressions preserve the function name, argument forms including `COUNT(*)`, `DISTINCT` arguments, and a normalized `FILTER (WHERE ...)` predicate when present.

SELECT-level duplicate elimination and grouping are emitted under `aggregation` when they affect the query. The object records ordinary `DISTINCT`, PostgreSQL-style `DISTINCT ON`, `GROUP BY ALL`, ordinary grouping expressions, and parser-supported `GROUPING SETS`, `ROLLUP`, and `CUBE` forms. Dialect-specific GROUP BY modifiers that are not modeled safely remain explicit diagnostics.

HAVING is analyzed with projected aliases available in grouped scope. Aggregate comparisons remain aggregate-result semantics and do not get rewritten into scalar constraints on their source columns.

### Window functions

Window calls are emitted as `window_function` expressions, distinct from ordinary function calls. The function arguments remain normalized expressions, while the resolved window specification records an optional local window name, `PARTITION BY` expressions, ordered `ORDER BY` expressions, and an explicit `ROWS`, `RANGE`, or `GROUPS` frame when present.

Function arguments, partition keys, ordering expressions, and frame-bound expressions all contribute to dependency and output-lineage analysis. Named windows are resolved within the local SELECT scope, including safe inheritance from another named window. Conflicting overrides, missing or cyclic names, and window options the protocol does not model remain explicit diagnostics.

Snowflake-style `QUALIFY` can reference a projected window alias. That alias is resolved back to the window expression for predicate semantics, while scalar value-domain derivation deliberately does not infer source-column constraints from a window result.

### Subqueries and table sources

Scalar subqueries are represented as `scalar_subquery` expressions instead of opaque unsupported expressions. EXISTS and IN/NOT IN subqueries use dedicated predicate kinds, `exists` and `in_subquery`, so membership and negation semantics are not flattened into generic boolean expressions.

Each nested subquery carries a parser-independent semantic summary containing its physical `dependencies`, resolved outer-scope `correlations`, projected `output`, local `predicates`, and nested `diagnostics`. Correlation resolution is conservative: qualified references resolve only when one visible outer source matches, and a local alias shadows an outer alias with the same name.

Derived tables expose only their projected columns to the parent query while preserving physical lineage through those columns. LATERAL derived tables may resolve references to preceding visible sources, so lineage from a lateral projection can flow back to the outer physical relation.

CTEs and derived tables are local semantic scopes rather than physical dependencies. Supported joins and WHERE/HAVING/QUALIFY constraints inside those scopes are carried through to the enclosing query as physical-source joins, column domains, and output lineage. Chained local relations preserve those constraints through direct projections. Local predicates that cannot be represented completely as physical source-column domains, such as EXISTS, IN-subquery, logical OR/NOT, or aggregate/window comparisons, emit `unresolved_local_predicate` rather than disappearing. Outer filters on plain-copy derived-table columns resolve through lineage to the physical source. Carried joins retain their logical relation participants, while equality-column operands resolve to physical source columns whenever plain-copy lineage proves the mapping; unsafe mappings remain explicit diagnostics instead of masquerading as physical columns. Unreferenced CTEs do not contribute relation semantics. When typed catalog metadata is available, wildcard projections over physical relations and CTEs expand to concrete output columns; without schema evidence, wildcard output remains explicitly unresolved.

Table-producing sources whose output schema cannot yet be modeled safely, including unresolved table functions and UNNEST-like factors, remain explicit `unsupported_table_factor` diagnostics rather than being omitted or assigned invented columns.

## Typed source schemas

Consumers that need declared source datatypes can supply typed relation schemas through
`RelationCatalog::from_schemas`. A `SchemaColumn` can be constructed from the canonical
parser-independent `DataType` model or from dialect-specific SQL syntax with
`SchemaColumn::from_sql_type`. The latter delegates parsing to the selected sqlparser dialect and
normalizes the result immediately.

The canonical model covers numeric widths and signedness, decimals, floating point, character and
binary families, dates/times/timestamps, intervals, UUIDs, JSON and semi-structured documents,
bit strings, arrays, maps, structs/tuples/nested records, unions, enums, sets, nullable wrappers,
table-valued types, geometry/geography, PostgreSQL search/regclass types, and vendor/user-defined
custom types. Dialect storage aliases normalize where their logical meaning is the same: for
example timestamp timezone variants become one timestamp type, JSON/JSONB/VARIANT/OBJECT/SUPER
become one document type, and ClickHouse LowCardinality unwraps to its logical value type.

The resulting analysis bundle preserves those schemas under `source_schemas`, alongside analyzed
value domains. Adapter-supplied schemas also carry `source_kind`: `dbt_catalog` for
warehouse-introspected catalog evidence and `dbt_manifest` for manifest-declared fallback
evidence. This allows consumers such as `sql-tdg` to generate unconstrained source columns
without reparsing SQL or maintaining a second datatype contract. Unknown vendor extensions remain
explicit `custom` datatypes instead of being discarded or guessed.

The public `dialect_from_name` and `parse_data_type` helpers delegate dialect handling to
sqlparser while keeping consumers independent from sqlparser AST types.


### ODCS v3.2 metadata

Open Data Contract Standard v3.2 YAML can enrich the same canonical schema and constraint model
used by SQL and dbt. Use `parse_odcs_yaml` for one self-contained contract or
`parse_odcs_documents` with explicitly named `OdcsDocument` values when relationships cross
contract boundaries. The adapter never fetches external contracts.

ODCS schema `physicalName` is used when present, otherwise `name`, and is resolved through the
existing `RelationCatalog` rules. Property `physicalType` is the preferred datatype evidence;
`logicalType` is lower-authority fallback evidence. Bundle enrichment preserves the overall
datatype authority order: dbt catalog, dbt manifest, ODCS physical type, then ODCS logical type.
A contradictory lower-authority ODCS datatype is returned explicitly rather than silently
discarded.

ODCS `required`, `primaryKey`/`primaryKeyPosition`, property `unique`, relationships, and
`enum` declarations normalize into the existing not-null, primary-key, unique-key, foreign-key,
and accepted-values constraints. Evidence uses `external_metadata` provenance with unknown
enforcement. Same-contract and explicitly supplied cross-contract foreign-key references resolve
deterministically; missing, ambiguous, or unsupported references fail rather than being guessed.

## Relation constraint metadata

Relation constraints are canonical and source-independent: any supported adapter that can prove an equivalent fact must emit the same canonical constraint. Direct SQL DDL, dbt metadata/tests, and future external metadata adapters therefore converge on one representation rather than owning separate semantics.

Optional `relation_constraints` metadata describes canonical relation and column constraints
independently from query-derived value domains. Supported facts are primary keys, unique keys,
foreign keys, non-null columns, and finite accepted-value sets. Unsupported constraint metadata is
never silently discarded: relation-scoped cases remain on the relation entry and diagnostics that
cannot be assigned to a canonical relation are emitted in the optional top-level
`constraint_diagnostics` array. Composite keys preserve declared
column order, and foreign keys preserve both local columns and the referenced relation/columns.

Each constraint carries one or more evidence records with a source kind, stable source identity,
and enforcement state. SQL DDL declarations, dbt constraints, and dbt generic tests normalize into
the same canonical model. Missing enforcement evidence remains `unknown`: a declaration or a dbt
test is never promoted into a proven warehouse-enforced constraint.

Identical facts from multiple sources coalesce their evidence. Different unique keys and foreign
keys coexist, while contradictory primary-key declarations remain visible with an explicit
`conflicting_primary_key` diagnostic. Independent accepted-value declarations for the same column
combine by intersection. An empty intersection is retained as an explicit unsatisfiable constraint
with an `unsatisfiable_accepted_values` diagnostic rather than choosing one source.

NULL semantics are explicit: primary-key and `not_null` constraints reject NULL, while unique-key,
foreign-key, and accepted-values constraints admit NULL unless a separate `not_null` constraint
applies. Library consumers can query this through `RelationConstraint::admits_null`.

Accepted values preserve canonical scalar literal types and the dbt `quote` setting. A dbt
`quote: false` string is treated as SQL syntax, not as a string value. Portable scalar literals
are normalized into `ConstraintValue`; raw expressions that cannot be reduced safely produce
`unsupported_dbt_accepted_value` instead of being exposed as strings that consumers would need
to parse.

The analyzer does not infer primary keys from `unique + not_null`, and it does not invent or
propagate keys through transformations unless a future analyzer can prove that property.

For example, a composite foreign key is emitted as relation metadata rather than flattened into
column flags:

```json
{
  "relation_constraints": [
    {
      "relation": "analytics.order_items",
      "constraints": [
        {
          "kind": "foreign_key",
          "columns": ["tenant_id", "order_id"],
          "referenced_relation": "analytics.orders",
          "referenced_columns": ["tenant_id", "order_id"],
          "evidence": [
            {
              "source_kind": "sql_ddl",
              "source_id": "foreign_key:order_items_order_fk",
              "enforcement": "unknown"
            }
          ]
        }
      ]
    }
  ]
}
```

Direct SQL analysis captures parser-supported column- and table-level `PRIMARY KEY`, `UNIQUE`,
and `FOREIGN KEY` declarations, column-level `NOT NULL`, and finite accepted-value sets from
non-negated `CHECK (column IN (...))` constraints, including queryless `CREATE TABLE`
statements. The dbt adapter reads explicit model/column constraints, including `not_null`, plus
built-in `unique`, `relationships`, `not_null`, and `accepted_values` tests from
`manifest.json`. Unsupported dbt test kinds attached to a known relation are surfaced explicitly
instead of being silently treated as supported. `catalog.json` remains authoritative only for
warehouse-introspected columns and datatypes.

## Outcome selection

`analyze_inputs` always analyzes and composes the complete supplied bundle. Each entry in `layers` carries its own composed semantics, while `graph.components[].final_outcomes` identifies the terminal datasets for each independent graph component.

Protocol generation does not have a final-only or all-layer analysis mode. Callers that need specific named outcomes can apply the public `select_targets` projection after analysis, or use repeatable CLI `--target <relation>` options. The projection keeps each selected producer plus every in-bundle ancestor needed to describe it, while unrelated graph components are omitted. The complete analyzed `inputs` remain present as source evidence. With no explicit targets, the complete bundle is emitted unchanged.

Target identifiers use the same exact qualified relation identities as cross-input linking. Unknown targets and relations with multiple in-bundle producers fail explicitly instead of producing an empty or arbitrarily selected result.

## Row-condition exactness

Source-column `column_domains` are useful only when a consumer knows whether they preserve the correlations needed to decide which source-row combinations qualify. Every query scope therefore emits `condition_exactness` with `status: "exact" | "residual"` and a deterministic `residual_conditions` list. A residual identifies a stable `reason`, its clause (`where`, `on`, `having`, `qualify`, `set_operation`, or `row_set_operator`), and a deterministic identity inside the scope.

For one query scope, `exact` means that before row-set shaping, a combination of source rows qualifies exactly when every emitted source-column domain is satisfied and every reported inner-join equality is satisfied. It does not mean that projection, ordering, grouping, or duplicate elimination preserve individual rows. Plain projection, `ORDER BY`, `GROUP BY` without `HAVING`, and ordinary `DISTINCT` therefore do not make the row-condition contract residual.

Exact predicate forms are deliberately allow-listed. Direct column-to-literal comparisons, `IS NULL`/`IS NOT NULL`, literal `IN`/`NOT IN`, literal `BETWEEN`/`NOT BETWEEN`, conjunctions of exact predicates, and disjunctions whose constraints refer to one source column can be exact. Inner joins can additionally use column equality plus safely reducible scalar filters such as `t.x = u.y AND t.a > 5`; the scalar join filter is also emitted in `column_domains`. Cross joins have no row condition and can be exact.

Everything else is default-denied unless analysis proves an exact representation. Residual cases include cross-column OR correlations, logical `NOT` outside the directly normalized negated forms above, non-equality column-to-column comparisons outside join equality, computed/function/CAST/pattern predicates, subquery predicates, `HAVING`, `QUALIFY`, outer/semi/anti or otherwise unsupported joins, repeated instances of the same physical relation, `LIMIT`, `OFFSET`, `FETCH`, `TOP`, `DISTINCT ON`, `TABLESAMPLE`, and set operations. UNION is currently residual as well, including branches with different constraints; INTERSECT and EXCEPT are residual.

Nested subqueries carry their own joins, column domains, and exactness contract. A correlated subquery is residual in its nested scope because its membership depends on the outer row. At the bundle layer, resolved `composed_semantics.condition_exactness` currently carries the owning statement's scope contract; transitive propagation of residuals across local relations and producer layers is a separate composition concern.

Analyzer diagnostics follow the same default-deny rule. Diagnostics that can affect membership become an `analysis_diagnostic` residual unless the AST construct was already classified directly. Diagnostics that are independently non-membership-only include ordering/layout diagnostics, output/lineage/wildcard diagnostics, and expression/function/window diagnostics when they occur outside a row condition; if the same expression occurs in a WHERE/ON predicate, the normalized predicate classifier supplies the residual. `unsupported_top`, `unsupported_prewhere`, `unsupported_connect_by`, `unsupported_limit`, `unsupported_fetch`, and set-operation diagnostics are not duplicated because their AST operators already create explicit residuals.


The diagnostic policy is deterministic. These diagnostics are non-membership metadata or output-shape concerns and therefore do not independently make a scope residual: `unsupported_order_by`, `unsupported_cluster_by`, `unsupported_distribute_by`, `unsupported_sort_by`, `unsupported_exclude`, `unsupported_select_into`, `unsupported_value_table_mode`, `ambiguous_output_lineage`, `unresolved_output_lineage`, `unresolved_wildcard`, `ambiguous_named_window`, `cyclic_named_window`, `unresolved_named_window`, `unsupported_window_order_option`, `unsupported_window_override`, `unsupported_group_by_modifier`, `unsupported_lock`, `unsupported_for_clause`, `unsupported_settings`, `unsupported_format_clause`, `unsupported_merge_output`, `unsupported_merge_insert_row`, and `unsupported_queryless_create_table`.

`unsupported_expression` and `unsupported_function` are also not converted directly from diagnostics because their membership effect is classified from the normalized predicate when they occur in a row condition. Likewise, `unsupported_top`, `unsupported_prewhere`, `unsupported_connect_by`, `unsupported_limit`, `unsupported_fetch`, `set_operation_arity_mismatch`, `unresolved_set_operation_output`, and `unsupported_set_operation_alignment` are not duplicated as `analysis_diagnostic` residuals because their owning AST construct already emits a specific residual condition. Every other statement-level or nested-query diagnostic is default-denied into an `analysis_diagnostic` residual.

NULL membership is part of the value-domain contract and is available through `ValueDomain::admits_null()`: `unbounded` admits NULL; ordered `ranges` do not; an include set admits NULL only when NULL is listed; an exclude set admits NULL unless NULL is listed; `empty` rejects NULL; and `unknown` returns an unknown NULL-membership result. Consequently, `IS NULL`, `IS NOT NULL`, `<>`, `NOT IN`, `IS DISTINCT FROM`, and range predicates preserve their SQL NULL behavior in the emitted domains.

## Output value domains

Every projected output column carries a `domain` independently from the query's source-column `column_domains`. Source-column domains describe values required to satisfy predicates. Output domains describe values the produced expression can return.

The analyzer derives output domains only when SQL semantics make them safe. Literals produce singleton domains, boolean-valued expressions produce `{false, true}`, `COUNT` is bounded below by zero, and `ROW_NUMBER` is bounded below by one. A `QUALIFY` predicate on a projected alias can further refine that derived domain without incorrectly constraining the physical columns used by the expression.

CASE expressions use expression kind `case` and preserve an optional simple-CASE operand, ordered WHEN/THEN branches, and the optional ELSE expression. Their output domains are the conservative union of branch result domains. Each WHEN branch also emits `source_domains`, and the explicit or implicit ELSE emits `else_source_domains`. Reachable source domains are an OR of alternatives, where each alternative is an AND of physical source-column domains after accounting for earlier branches not matching. Unreachable branches are explicit, and conditions that cannot be reduced safely emit an unknown reason. A CASE without ELSE includes SQL NULL as a possible result.

Safe constant integer unary and arithmetic expressions are evaluated with checked arithmetic. Functions, transforms, or bounds that cannot be proven safely remain explicit `unknown` output domains rather than guessed.

Output domains are preserved through multi-layer composition. A direct projection or rename of an upstream derived column retains the producer's domain in the composed final outcome.

## Composed multi-input workflow

One invocation can analyze related and unrelated transformations together. Query-backed DDL gives named layers that can be linked across inputs, while bare queries remain anonymous outcomes. The emitted protocol always contains every layer; terminal datasets are identified by `graph.components[].final_outcomes`, so consumers choose which outcomes to use without asking the producer to re-run analysis in a different scope.

Inputs can mix inline SQL and files:

```sh
cargo run -- \
  --dialect generic \
  --file sql/stage_orders.sql \
  --sql "CREATE TABLE core.ranked_orders AS SELECT order_id, customer_id, amount, ROW_NUMBER() OVER (PARTITION BY customer_id ORDER BY created_at) AS rn FROM stage.orders QUALIFY rn <= 10" \
  --file sql/customer_summary.sql \
  --sql "CREATE TABLE mart.active_ids AS SELECT id FROM raw.active_accounts UNION ALL SELECT id FROM raw.legacy_accounts"
```

If `stage_orders.sql` produces `stage.orders`, the next statement consumes that exact relation, and a later `mart.customer_summary` layer composes through it to the physical leaf dependencies. The unrelated `mart.active_ids` transformation remains a separate graph component. Both components and all intermediate layers stay in the same deterministic protocol document.

For a three-layer chain such as `raw.orders -> stage.orders -> core.ranked_orders -> mart.customer_summary`, the final layer retains transitive physical lineage and domains that can be propagated safely. For example, a `ROW_NUMBER()` output constrained by `QUALIFY rn <= 10` keeps the derived output domain `[1, 10]` while source-column constraints remain separate. CASE, grouping, nested subqueries, and set operations are represented in the same document when they occur in the supplied workload.

DML writes participate in the same graph without being mistaken for full relation definitions. `INSERT INTO ... SELECT` is modeled as an append: its source query output domains describe the values appended to the target, while a downstream reader links to that writer through a `partial` edge. `MERGE` records its target, source dependencies, match condition, and supported insert/update/delete clauses as a conditional mutation. Every MERGE value written by an UPDATE or INSERT carries its own conservative outcome domain. MATCHED branch predicates and equality conditions can therefore narrow written values to intervals when that is provable. The complete post-MERGE relation remains partial because untouched pre-existing rows are outside the statement's semantics.

## Catalog-aware relation resolution

Catalog metadata is optional. Without it, multi-input analysis keeps the existing conservative textual relation identities unchanged.

For workloads where partially qualified names are not sufficient, configured inputs can carry a default catalog/schema context and analysis can use a parser-independent relation resolver. `RelationCatalog` is the built-in metadata implementation, while `RelationResolver` is the small integration contract for caller-owned catalog providers.

```rust
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, ConfiguredSqlInput, RelationCatalog,
    RelationContext, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

let dialect = PostgreSqlDialect {};
let context = RelationContext::new(Some("warehouse"), Some("analytics"))?;
let catalog = RelationCatalog::new(&[
    "warehouse.raw.orders",
    "warehouse.analytics.orders",
])?;

let stage = SqlInput::inline(
    "CREATE TABLE orders AS SELECT id FROM raw.orders WHERE id >= 1",
);
let configured = [
    ConfiguredSqlInput::new("orders", &stage, "postgresql", &dialect)
        .with_relation_context(&context),
];

let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)?;
```

Default catalog/schema context fills missing qualification before graph linking. If no context is supplied, a unique catalog suffix match can canonicalize a partially qualified reference. Multiple matches fail explicitly instead of selecting a producer arbitrarily. Canonical relation identities are then used by graph edges, transitive dependencies, physical lineage, and composed output-domain propagation.

Quoted identifiers remain exact. For unquoted identifiers the resolver applies the dialect normalization needed for safe matching where that behavior is well-defined by the supported resolver: PostgreSQL and Redshift fold to lowercase, while ANSI and Snowflake fold to uppercase. Other dialects preserve unquoted spelling rather than applying a global case rule. Caller-supplied resolver implementations can provide different metadata behavior through the same parser-independent `RelationResolver` contract.

The direct CLI exposes the built-in resolver with repeatable `--catalog-relation` plus optional `--default-catalog` and `--default-schema` values:

```sh
cargo run -- \
  --dialect postgresql \
  --default-catalog warehouse \
  --default-schema analytics \
  --catalog-relation warehouse.raw.orders \
  --catalog-relation warehouse.analytics.orders \
  --catalog-relation warehouse.analytics.final_orders \
  --target warehouse.analytics.final_orders \
  --sql "CREATE TABLE orders AS SELECT id FROM raw.orders WHERE id >= 5" \
  --sql "CREATE TABLE final_orders AS SELECT id FROM orders WHERE id <= 10"
```

Catalog metadata is optional. Without these options, direct CLI analysis preserves the same metadata-free textual relation identities as the library API.

## Analysis manifests

Large bundles can be declared in a versioned JSON manifest instead of repeating every analysis input and option on the command line. Manifest v1 is defined by [`schema/analysis-manifest-v1.schema.json`](schema/analysis-manifest-v1.schema.json) and documented in [`docs/analysis-manifest-v1.md`](docs/analysis-manifest-v1.md).

Each manifest input has a stable unique `id`, exactly one inline `sql` string or `file` path, and an optional per-input `dialect`. The root `dialect` defaults to `generic` and supplies the default for inputs that omit an override. Dialect names continue to resolve through sqlparser rather than a project-maintained list. Relative file paths resolve relative to the manifest file while the declared path is retained as source identity.

Manifests may also declare `catalog_relations` and a bundle-level `relation_context` with `default_catalog` and/or `default_schema`. An input can replace that context for its own analysis. The CLI maps this metadata to the same `RelationCatalog` and `RelationContext` API used by direct Rust callers, so canonical graph links, lineage, and composed output domains are equivalent.

The default `output_scope` is `all`, which preserves the complete analyzed bundle. `output_scope: "targets"` requires one or more exact relation names and applies target projection only after every input has been analyzed and composed.

```json
{
  "manifest_version": "1",
  "dialect": "generic",
  "catalog_relations": [
    "warehouse.stage.orders",
    "warehouse.mart.orders",
    "warehouse.raw.orders"
  ],
  "relation_context": {
    "default_catalog": "warehouse",
    "default_schema": "stage"
  },
  "output_scope": "targets",
  "targets": ["warehouse.mart.orders"],
  "inputs": [
    {
      "id": "stage-orders",
      "file": "sql/stage_orders.sql",
      "dialect": "snowflake"
    },
    {
      "id": "mart-orders",
      "sql": "CREATE TABLE mart.orders AS SELECT * FROM stage.orders",
      "dialect": "postgresql",
      "relation_context": {
        "default_catalog": "warehouse",
        "default_schema": "mart"
      }
    }
  ]
}
```

Run it with:

```sh
cargo run -- --manifest analysis.json
```

The checked-in `tests/fixtures/extended_bundle/analysis-all.json` fixture demonstrates the complete bundle workflow: multiple independent pipelines, PostgreSQL/Generic/Snowflake/MySQL inputs, CREATE TABLE/VIEW layers, INSERT-select semantics, catalog-aware canonical identities, and per-input relation context. Its companion `analysis-target.json` applies target projection only after the whole workload has been analyzed and composed.

```sh
cargo run -- --manifest tests/fixtures/extended_bundle/analysis-all.json
cargo run -- --manifest tests/fixtures/extended_bundle/analysis-target.json
```

The full fixture preserves every layer and identifies terminal outcomes in `graph.components[].final_outcomes`. The targeted fixture keeps only `warehouse.analytics.final_orders` and its required in-bundle ancestors while retaining the complete analyzed input evidence.

`--manifest` is mutually exclusive with `--dbt-manifest` and direct analysis options such as `--dialect`, `--catalog-relation`, `--default-catalog`, `--default-schema`, `--target`, `--sql`, `--file`, `--dir`, and positional SQL. `--dbt-manifest` is mutually exclusive with direct SQL/catalog options but may be combined with `--target`. Output-format options such as OpenLineage export remain CLI-level options and can be combined with either manifest form.

## dbt artifact adapter

dbt is a first-class protocol input. The complete adapter consumes both `manifest.json` and
`catalog.json`, using each artifact only for the evidence it authoritatively owns:

- `manifest.json`: model unique IDs, canonical relation identities, compiled SQL, relation context,
  declared dependency metadata, explicit model/column key constraints, and built-in `unique` /
  `relationships` generic-test declarations.
- `catalog.json`: warehouse-introspected physical columns and database datatypes for models,
  seeds, snapshots, and sources.

The adapter joins the artifacts by dbt resource `unique_id`, normalizes catalog datatypes into the
same parser-independent `DataType` model used by direct callers, and runs compiled model SQL
through the ordinary analyzer, graph builder, composition, and outcome-domain pipeline. dbt-specific
artifact types never appear in the emitted protocol.

The CLI expects `catalog.json` next to `manifest.json` by default:

```sh
cargo run -- --dbt-manifest target/manifest.json
```

Use `--dbt-catalog` when the catalog artifact is stored elsewhere:

```sh
cargo run -- \
  --dbt-manifest artifacts/manifest.json \
  --dbt-catalog warehouse/catalog.json
```

Target projection remains a consumer-side operation and can be applied after complete dbt analysis:

```sh
cargo run -- \
  --dbt-manifest target/manifest.json \
  --target warehouse.analytics.customer_summary
```

The library API keeps artifact loading and dialect selection explicit:

```rust
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, to_bundle_json,
};
use sqlparser::dialect::dialect_from_str;

let manifest_json = std::fs::read_to_string("target/manifest.json")?;
let catalog_json = std::fs::read_to_string("target/catalog.json")?;
let manifest = parse_dbt_manifest(&manifest_json)?;
let catalog = parse_dbt_catalog(&catalog_json)?;
let dialect = dialect_from_str(manifest.adapter_type())
    .ok_or("dbt adapter type is not a sqlparser dialect")?;
let bundle = analyze_dbt_artifacts(
    &manifest,
    &catalog,
    manifest.adapter_type(),
    dialect.as_ref(),
)?;

println!("{}", to_bundle_json(&bundle));
```

The manifest adapter accepts schema versions v10, v11, and v12. The catalog adapter accepts v0 and
v1. Catalog columns are ordered by their warehouse ordinal and their dialect-specific type strings
are normalized through the selected dbt adapter dialect. Catalog schema evidence takes precedence
when present. If a physical dependency is absent from `catalog.json`, the adapter falls back to
column `data_type` declarations in `manifest.json` when the declared schema is complete. Missing
declared datatypes are reported with the relation and affected columns. Catalog-reported metadata
query errors, catalog resources absent from the paired manifest, missing relation identities,
invalid datatypes, or dependencies with neither usable catalog nor manifest schema evidence fail
explicitly rather than producing an apparently complete protocol.

`analyze_dbt_manifest` remains available as a compatibility API for manifest-only semantic
analysis, but it cannot emit complete typed relation schemas. New consumers that need the full
protocol contract should use `analyze_dbt_artifacts`.

Model identity comes from dbt `unique_id`, and produced dataset identity comes from
`relation_name`; filenames are retained only as source metadata. `compiled_code` is preferred.
Plain `raw_code` is accepted only when it contains no Jinja delimiters. Python models,
relation-less models such as ephemerals, missing dependency relations, dependency cycles, and
uncompiled Jinja fail explicitly rather than being guessed or silently omitted.

dbt `depends_on.nodes` provides deterministic model ordering and is mapped to canonical relation
identities from the manifest. The adapter verifies that every declared dependency is also present
in the dependency graph derived from analyzed SQL. Source schemas use catalog metadata when
available and manifest-declared column types only as fallback evidence for missing physical
relations. Both use the same canonical relation identities, so relation resolution, lineage, value
domains, and typed source schemas describe one consistent graph.

CI exercises a complete dbt Core project rather than relying only on hand-authored fixtures.
`make dbt-e2e` runs seed and model execution, generates `catalog.json` from the real warehouse,
then analyzes the generated manifest/catalog pair through both the library and CLI. The fixture
covers predicate and output domains, CASE and arithmetic expressions, joins, window functions,
QUALIFY, named windows, UNION/INTERSECT/EXCEPT, aggregation, HAVING, DISTINCT, ROLLUP, subqueries,
derived/lateral tables, transitive composition, disconnected components, incremental model
configuration, terminal outcome snapshots, and warehouse source-schema datatype emission.

## Differential conformance

The standard Rust test suite includes a deterministic differential oracle in
`tests/differential_conformance.rs`. It runs protocol exactness, source-domain, output-domain,
join-equality, and CASE-branch claims against DuckDB. The suite combines a curated matrix with
3,000 seeded AND/OR/NOT predicate trees; failures report the reproducing query and seed.

DuckDB is a development-only dependency with default features disabled. The repository does not
enable duckdb-rs's `bundled` feature, so it never compiles DuckDB from source. Cargo config sets
`DUCKDB_DOWNLOAD_LIB=1`, which makes duckdb-rs download and link the matching prebuilt DuckDB
library. Set `DUCKDB_DOWNLOAD_LIB=0` in the environment to use an already installed compatible
system library instead.

## OpenLineage export

The SQL Semantic Protocol remains the authoritative semantic representation. The library function `to_openlineage_json` maps resolved named layers to OpenLineage 2.0.2 DatasetEvents using the current Lineage Dataset Facet for dataset-level and field-level lineage. OpenLineage types do not appear in the core protocol model.

The caller supplies one OpenLineage namespace and event timestamp:

```rust
let json = sql_semantic_protocol::to_openlineage_json(
    &bundle,
    "postgresql://warehouse",
    "2026-10-02T07:00:00Z",
)?;
```

Only semantics OpenLineage can represent are exported. Predicate trees, value domains, and other richer outcome semantics remain in the SQL Semantic Protocol. Anonymous outputs and unresolved layers are omitted rather than assigned invented dataset identity or lineage.

The CLI can select the same adapter with `--format openlineage`. A namespace is required because SQL alone cannot determine an OpenLineage dataset namespace. The event time may be supplied explicitly for reproducible output; otherwise the CLI uses the current UTC time:

```sh
cargo run -- \
  --dialect postgresql \
  --format openlineage \
  --namespace postgresql://warehouse \
  --file sql/stage.sql \
  --file sql/core.sql \
  --file sql/mart.sql
```

For byte-reproducible output, add `--event-time 2026-10-02T07:00:00Z`.

## Versioning

SQL Semantic Protocol uses one version for the application and the protocol. The Cargo package version, emitted `protocol_version`, active protocol contract, Git tag, and GitHub release are the same release identity.

SemVer compatibility is defined primarily by the public protocol contract. A breaking protocol change requires a major version bump. Backward-compatible protocol or application features use a minor bump, while compatible fixes and internal application changes use a patch bump. Non-protocol implementation changes therefore do not require a breaking release, but every release still advances the shared application/protocol version.

Release Please manages the shared application/protocol version from Conventional Commits. The first stable bootstrap release is `1.0.0`; after that, breaking changes use major releases, backward-compatible features use minor releases, and compatible fixes or internal changes use patch releases.

The release workflow opens or updates a release PR from `main`. The generated release branch is checked with `cargo publish --dry-run --locked`. Merging the release PR creates the matching `vX.Y.Z` tag and GitHub release, then publishes the same package version to crates.io. Publishing uses the repository secret `CARGO_REGISTRY_TOKEN`. The one-time `release-as: 1.0.0` bootstrap override is removed after `v1.0.0` has been produced.

After publication, the binary can be installed with:

```sh
cargo install sql-semantic-protocol
```

The library can be consumed from crates.io with:

```toml
[dependencies]
sql-semantic-protocol = "1"
```

## CLI

The CLI analyzes SQL and writes the SQL Semantic Protocol JSON document to standard output.

```text
sql-semantic-protocol [--dbt-manifest <path> [--dbt-catalog <path>] | --manifest <path> | [--dialect <name>] [--catalog-relation <relation>]... [--default-catalog <identifier>] [--default-schema <identifier>] [--target <relation>]... [--sql <SQL>]... [--file <path>]... [--dir <path>]... [SQL ...]] [--target <relation>]...
```

The dialect defaults to `generic`. The CLI delegates dialect selection to `sqlparser::dialect::dialect_from_str`, so it accepts any built-in dialect recognized by the pinned `sqlparser` version rather than maintaining a separate dialect list.

With `sqlparser` 0.58, the following built-in dialects are available:

| Dialect | CLI name |
| --- | --- |
| ANSI | `ansi` |
| BigQuery | `bigquery` |
| ClickHouse | `clickhouse` |
| Databricks | `databricks` |
| DuckDB | `duckdb` |
| Generic | `generic` |
| Hive | `hive` |
| Microsoft SQL Server | `mssql` |
| MySQL | `mysql` |
| PostgreSQL | `postgresql`, `postgres` |
| Redshift | `redshift` |
| Snowflake | `snowflake` |
| SQLite | `sqlite` |

Legacy single-input SQL can still be supplied positionally:

```sh
cargo run -- --dialect postgresql "SELECT a FROM t WHERE a > 10"
```

A single file remains unchanged:

```sh
cargo run -- --dialect snowflake --file query.sql
```

For multiple inputs, repeat `--sql`, `--file`, and `--dir` in any mixture. `--dir` recursively discovers regular files whose extension is `.sql` case-insensitively and ignores all other files. Repeat `--catalog-relation` to supply canonical catalog metadata and use `--default-catalog`/`--default-schema` when direct inputs need qualification before graph linking.

Repeat `--target` to project the completed analysis onto one or more named output relations:

```sh
cargo run -- \
  --sql "SELECT id FROM raw.orders" \
  --file sql/enrich_orders.sql \
  --dir sql/reporting \
  --sql "SELECT customer_id FROM raw.customers"
```

For example, to expose only `mart.customer_summary` and its required in-bundle ancestors while still analyzing every supplied input:

```sh
cargo run -- \
  --target mart.customer_summary \
  --file sql/stage_orders.sql \
  --file sql/core_orders.sql \
  --file sql/customer_summary.sql \
  --file sql/unrelated_pipeline.sql
```

Direct CLI inputs are analyzed in command-line occurrence order and receive deterministic IDs `input-0001`, `input-0002`, and so on. Manifest inputs retain their explicit stable IDs. Each `--dir` expands at its command-line position into all recursively discovered SQL files sorted lexicographically by path, so filesystem traversal order cannot affect protocol output. Discovered file paths are retained as source identity. The ID width expands when necessary, so there is no fixed input-count limit. Parse, file, and analysis failures identify the affected input or path.

Positional SQL represents one legacy input and cannot be mixed with `--sql`, `--file`, or `--dir`. If neither explicit input nor positional SQL is supplied, the CLI reads one input from standard input:

```sh
printf '%s\n' 'SELECT a FROM t WHERE a > 10' | cargo run -- --dialect duckdb
```

Every successful invocation emits the current Cargo package version as `protocol_version`. One input produces an `inputs` array with one element; multiple inputs use the same document shape with additional elements.

Query-backed DDL is analyzed through its defining query and records the created relation as the layer output. For example, `CREATE TABLE mart.orders AS SELECT ...` produces `mart.orders`, while a bare `SELECT` produces an anonymous layer result.

### Example output

For:

```sql
SELECT t.b FROM t WHERE t.a > 10
```

the protocol still uses the active version envelope even though there is only one input:

```json
{
  "protocol_version": "<package-version>",
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
      "consumes": ["t"],
      "composed_semantics": {
        "status": "resolved",
        "dependencies": ["t"],
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
              "domain": {
                "kind": "unbounded"
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
    }
  ],
  "graph": {
    "edges": [
      {
        "consumer_layer_id": "layer-0001",
        "relation": "t",
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
}
```

This shape is locked by the active fixture stored in `examples/protocol-simple.json`.

Successful runs emit protocol JSON only. Input errors, SQL parse errors, and analysis failures are written to standard error and use distinct non-zero exit codes.

