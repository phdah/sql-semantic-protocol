# Semantic model and behavior

Understand how SQL and metadata become normalized, deterministic protocol outcomes. For the normative JSON shape, see [the protocol contract](protocol.md). For runnable commands, see [CLI usage](cli.md).

## Protocol contract

The Cargo package version is the single active protocol version emitted by the library and CLI. It represents one or many SQL inputs with the same root document shape: `inputs`, `layers`, and `graph`. A single SQL string is therefore represented as one element in `inputs`, not by switching to a different protocol version.

The active contract is defined by [`schema/protocol.schema.json`](../schema/protocol.schema.json), documented in [`docs/protocol.md`](../docs/protocol.md), and demonstrated by [`examples/protocol.json`](../examples/protocol.json) and [`examples/protocol-simple.json`](../examples/protocol-simple.json).

Versioned protocol artifacts such as `protocol-v0.1*` and `protocol-v0.2*` remain in the repository as immutable historical references. Current runtime code and tests use the unversioned active contract paths above.

The analyzer resolves transformation layers into a deterministic relation dependency graph. It composes semantics through that graph: final outputs expose transitive physical lineage, value domains propagate through safe direct projections and renames, and ambiguous, cyclic, or non-invertible paths remain explicit instead of being guessed. Disconnected pipelines compose independently.

### Set operations

The branch-aware contract preserves each SELECT operand's row predicates, source domains and positional output evidence independently. Duplicate counts follow one typed operator rule per node, with NULL-safe equality for entire aligned tuples. A separate `membership` proof emits qualifying and non-qualifying witness directions, each exact only when the full branch-count combination is proven. The direct supported class is a row-preserving, one-relation SELECT of plain input columns without correlated or unknown filters, joins, aggregation, DISTINCT or row limits, with disjoint physical source dependencies across branches. The proof enumerates small source tuple counts (0, 1, and 2) to cover duplicate semantics and zero-result cancellation. More complex branches, shared physical inputs, unresolved positional alignment and unsupported tree shapes remain residual with explicit reasons.

A witness case is a conjunction of exact per-branch counts for a candidate tuple at the named physical or intermediate relation boundary. The generator must enforce these counts using NULL-safe tuple matching, honor branch conditions and domain constraints, and verify zero-count obligations against existing as well as inserted rows. Intermediate/CTE boundaries cannot be treated as physical source insertion targets without an independently proven producer plan. Resolved composition retains originating operation evidence across SQL layer and dbt compiled-query producer graphs. For full details and consumer versioning, see [the set operation protocol contract](protocol.md#set-operations).

UNION, UNION ALL, INTERSECT, and EXCEPT are analyzed as parser-independent set-operation semantics. Non-standard SQL MINUS syntax is normalized to EXCEPT when the selected sqlparser dialect accepts it.

Set outputs align columns positionally. Output names come from the left branch, while field lineage includes the corresponding columns from every contributing branch. Nested and chained operations retain their recursive operator tree in the optional `set_operation` field. Omitted set quantifiers normalize to DISTINCT semantics.

If branches expose incompatible arity, output semantics remain unresolved and an explicit diagnostic is emitted. For proven positional domains, UNION preserves the union of possible output values, INTERSECT narrows to their intersection, and EXCEPT retains the left operand's domain. These operators do not flatten branch-specific physical source predicates into one conjunction: conflicting constraints on the same source column degrade to an explicit unknown source domain instead of being guessed. BY NAME alignment is represented in the operation tree but output composition remains explicitly unsupported.

### Aggregation and grouping

Grouped aggregates are represented separately from ordinary scalar functions. Aggregate expressions preserve the function name, argument forms including `COUNT(*)`, `DISTINCT` arguments, and a normalized `FILTER (WHERE ...)` predicate when present.

SELECT-level duplicate elimination and grouping are emitted under `aggregation` when they affect the query. The object records ordinary `DISTINCT`, PostgreSQL-style `DISTINCT ON`, `GROUP BY ALL`, ordinary grouping expressions, and parser-supported `GROUPING SETS`, `ROLLUP`, and `CUBE` forms. Dialect-specific GROUP BY modifiers that are not modeled safely remain explicit diagnostics.

HAVING is analyzed with projected aliases available in grouped scope. Aggregate comparisons remain aggregate-result semantics and do not get rewritten into scalar constraints on their source columns.

### Window functions

Window calls are emitted as `window_function` expressions, distinct from ordinary function calls. The function arguments remain normalized expressions, while the resolved window specification records an optional local window name, `PARTITION BY` expressions, ordered `ORDER BY` expressions, and an explicit `ROWS`, `RANGE`, or `GROUPS` frame when present.

Function arguments, partition keys, ordering expressions, and frame-bound expressions all contribute to dependency and output-lineage analysis. Named windows are resolved within the local SELECT scope, including safe inheritance from another named window. Conflicting overrides, missing or cyclic names, and window options the protocol does not model remain explicit diagnostics.

Snowflake-style `QUALIFY` can reference a projected window alias. That alias is resolved back to the window expression for predicate semantics, while scalar value-domain derivation deliberately does not infer source-column constraints from a window result.

For a direct supported `QUALIFY ROW_NUMBER() = 1` or `QUALIFY ROW_NUMBER() <= N` (including an alias), the optional `window_witness` has a single `boundary`, physical or intermediate `partition_by` column identities, and ordered `order_by` keys. Each order key records explicit `nulls_first` and direction and requires `strict_unique: true`: generator inputs must give candidate and predecessor rows distinct ordering tuples. No implicit NULL ordering or tie breaking is assumed. `predicate` records `eq` or `lte` plus the nonnegative integer threshold. `qualifying` and `rejected` independently describe an exact inclusive range of rows preceding a candidate in its partition (`min_preceding`, optional `max_preceding`), `impossible`, or `residual` with a reason. A candidate with `p` predecessors has `ROW_NUMBER = p + 1`; `ROW_NUMBER <= 0` is impossible. Exact bounds also refine a projected matching ROW_NUMBER output domain; they do **not** create source-column scalar domains.

Exact local rank witnesses currently require one direct input, plain partition/order columns, an explicit NULLS FIRST/LAST, no explicit frame, no other row filters, joins or unsupported diagnostics. Consumers must satisfy the emitted strict ordering, partition identity and any upstream conditions. Arbitrary ties, implicit NULL placement, computed order keys, `RANK` and `DENSE_RANK` remain residual. When every ordering key is fixed by the partition keys, a rejected-row witness requiring predecessors remains residual because strictly distinct ordering tuples cannot be constructed. Projected-filter witnesses also reject outer grouping, TOP, and other row-shaping operations. This is a constructive proof for controlled witness rows, not a claim that existing source data has unique order keys. `layers[].composed_semantics.window_witnesses` retains local and upstream evidence with origin layer and input boundary classification. Such evidence does not automatically prove downstream row membership.

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


### Typed predicate-domain literals

Boolean filters on plain boolean columns normalize to exact literal domains: `WHERE flag`
and `WHERE flag IS TRUE` require `true`, while `WHERE NOT flag` and
`WHERE flag IS FALSE` require `false`. `IS NOT TRUE` and `IS NOT FALSE`
also allow SQL NULL. Typed-literal errors retain their distinct residual reasons
(`literal_type_mismatch`, `lossy_coercion`, `out_of_range_literal`,
`comparison_semantics`, and `unknown_schema_column`) and are attributed to
their actual condition clause. See [row-condition exactness](../docs/protocol.md#row-condition-exactness)
for the complete residual reason contract.


When typed source-schema evidence is available, predicate-domain literals are checked against the
canonical datatype before a domain can participate in the exact row-condition contract. Exact
boolean, integer, date, time, and interval literals retain their canonical literal type; integer
literals used with decimal columns normalize to decimal literals. Integer bounds are range-checked
against declared signedness and bit width. Lossy numeric coercions are never treated as exact.

Comparison semantics are deliberately conservative where warehouse settings are not represented.
String, floating-point, and timestamp comparisons retain representable domains but are
conditional until the required comparison assumptions are explicitly declared. Unsupported or
lossy conversions remain residual. See [comparison-semantics assumptions](#comparison-semantics-assumptions).
Decimal values preserve exact numeric text; declared precision and scale describe the source type, but the protocol
does not round predicate literals to fit them. Binary, document, collection, geometry, search,
vendor-defined, and otherwise opaque types remain residual unless a future contract defines exact
scalar comparison semantics for them.

A lexical string is never implicitly converted to DATE, TIME, TIMESTAMP, numeric, or another typed
domain. If SQL parsing produced a typed DATE/TIME/INTERVAL literal, that literal may be exact for the
matching canonical family. Without schema evidence, existing lexical-literal domain derivation is
preserved, but it does not claim datatype-aware coercion semantics.

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

### Validating declared schema evidence

Typed relation schemas supplied by a catalog, dbt manifest/catalog, ODCS, or
directly by a caller are also used to validate physical column references.
A column missing from an available schema produces an `unknown_schema_column`
diagnostic and prevents an exact composed row-condition claim. Constraint facts
with undeclared key/foreign-key columns or values incompatible with their
declared datatype are withheld and reported through relation-scoped
`invalid_constraint_column` or `incompatible_accepted_value` diagnostics.
A dbt relationships test with conflicting `to` and `depends_on` targets
produces `inconsistent_relationship_target`. Missing schema evidence does
not itself prove a constraint or column invalid.

## Outcome selection

`analyze_inputs` always analyzes and composes the complete supplied bundle. Each entry in `layers` carries its own composed semantics, while `graph.components[].final_outcomes` identifies the terminal datasets for each independent graph component.

Protocol generation does not have a final-only or all-layer analysis mode. Callers that need specific named outcomes can apply the public `select_targets` projection after analysis, or use repeatable CLI `--target <relation>` options. The projection keeps each selected producer plus every in-bundle ancestor needed to describe it, while unrelated graph components are omitted. The complete analyzed `inputs` remain present as source evidence. With no explicit targets, the complete bundle is emitted unchanged.

Target identifiers use the same exact qualified relation identities as cross-input linking. Unknown targets and relations with multiple in-bundle producers fail explicitly instead of producing an empty or arbitrarily selected result.

## Row-condition exactness

Source-column `column_domains` are useful only when a consumer knows whether they preserve the correlations needed to decide which source-row combinations qualify. Every query scope therefore emits `condition_exactness` with `status: "exact" | "conditional" | "residual"` and a deterministic `residual_conditions` list. A residual identifies a stable `reason`, its clause (`where`, `on`, `having`, `qualify`, `set_operation`, or `row_set_operator`), and a deterministic identity inside the scope.

For one query scope, `exact` means that before row-set shaping, a combination of source rows qualifies exactly when every emitted source-column domain is satisfied and every reported inner-join equality is satisfied. It does not mean that projection, ordering, grouping, or duplicate elimination preserve individual rows. Plain projection, `ORDER BY`, `GROUP BY` without `HAVING`, and ordinary `DISTINCT` therefore do not make the row-condition contract residual.

Exact predicate forms are deliberately allow-listed. Direct column-to-literal comparisons, `IS NULL`/`IS NOT NULL`, literal `IN`/`NOT IN`, literal `BETWEEN`/`NOT BETWEEN`, conjunctions of exact predicates, and disjunctions whose constraints refer to one source column can be exact. Inner joins can additionally use column equality plus safely reducible scalar filters such as `t.x = u.y AND t.a > 5`; the scalar join filter is also emitted in `column_domains`. Cross joins have no row condition and can be exact.

Resolved `composed_semantics.join_equalities` is the canonical consumer-facing representation of those correlations. Each entry names both physical leaf relation/column endpoints, a relation-instance identity, the join kind, and the originating transformation layer. Equalities from referenced CTEs, derived tables, and resolved producer layers compose transitively through plain-copy lineage. A column equality in `WHERE` between distinct relation instances, including comma/CROSS-join syntax, is represented as an implicit inner equality rather than an `unknown` scalar domain. Outer-join equalities are listed with their join kind while the scope remains residual. Repeated/self-joins remain residual when physical instance identity cannot be proven safely.



Typed `composed_semantics.join_witnesses` adds per-join **qualifying** and **rejected**
source-row obligations, retaining `origin_layer_id`, physical `relation`/`column`, and
distinct `relation_instance` identifiers. The canonical `comparison` is one of
`eq`, `neq`, `lt`, `lte`, `gt`, or `gte`; reversing the SQL operands also
reverses the comparison so the endpoints always follow JOIN left/right orientation.
`unknown_comparison_is_match: false` means a SQL `ON` comparison must evaluate
to TRUE, never NULL/UNKNOWN, for rows to match. In particular, an ordinary matched
comparison requires non-NULL keys. With fixed source values, SQL's comparison
settings still apply; this is not proof that a specific pair of values matches.

Directions are `exact` (enumerated `cases`), `impossible`, or `residual`
(with a stable `reason`). A case `matched` requires at least one matching partner
(`min_matches: 1`, `max_matches: null`), allowing duplicates and multi-matches.
`left_unmatched` or `right_unmatched` require **zero** TRUE-matching partners
(`min_matches: 0`, `max_matches: 0`), including when a key is NULL.
A qualifying unmatched row for LEFT, RIGHT or FULL carries
`null_extended_side` (`right` or `left`); those NULLs are output padding and
must not be inserted as a real matching source row. Semi joins output only rows from
their preserved side with a match, anti joins only rows with no match. Rejected
directions describe source rows that do not independently produce an output row;
a FULL join has no such source-row case before downstream filtering.

The analyzer currently proves these witness directions only for one binary JOIN
whose ON/USING condition normalizes to a single physical column-to-column
comparison with unambiguous instance lineage. Self-joins preserve separate alias
identities even when both endpoints name the same physical relation. Join trees,
disjunctions, multiple ON terms, computed operands, null-safe comparisons,
unresolved projection lineage, row-changing upstream producers (including filters,
DISTINCT, aggregation, and LIMIT), and unrecognized join kinds emit **residual**
directions, never a guessed pair of witnesses. Upstream witness evidence is carried
with its originating layer through composition, not silently reinterpreted against
the downstream join. A local witness is resolved against physical input rows only when
any intermediate producer is proven to preserve source row membership, such as a chain
of plain-copy projections without filtering. A filtered stage may remove a matching
partner, so its join emits residual directions rather than an incorrect exact
physical-source non-match. The older whole-scope `condition_exactness` remains residual
for outer/semi/anti joins and repeated relations: proving a local join witness does
not automatically prove exact membership of the entire query. Consumers must inspect
both witness directions, the scope exactness, source datatypes and comparison
assumptions before treating a generated output as fully exact.

Everything else is default-denied unless analysis proves an exact representation. Residual cases include cross-column OR correlations, logical `NOT` outside the directly normalized negated forms above, non-equality column-to-column comparisons outside join equality, computed/function/CAST/pattern predicates, subquery predicates, `HAVING`, `QUALIFY`, outer/semi/anti or otherwise unsupported joins, repeated instances of the same physical relation, `LIMIT`, `OFFSET`, `FETCH`, `TOP`, `DISTINCT ON`, `TABLESAMPLE`, and set operations. UNION is currently residual as well, including branches with different constraints; INTERSECT and EXCEPT are residual.

Nested subqueries carry their own joins, column domains, and exactness contract. A correlated subquery is residual in its nested scope because its membership depends on the outer row. At the bundle layer, resolved `composed_semantics.condition_exactness` is transitive: a layer is exact only when its owning query, every referenced CTE or derived-table scope, and every resolved ancestor layer are exact. Composed residuals include an `origin` with the producing `layer_id` and local `scope` such as `query`, `cte:x`, or `derived:d`. Unreferenced CTEs contribute no residuals or diagnostics. Domain composition is conservative: intersecting an `unknown` domain with any other domain remains `unknown` rather than recovering a narrower known domain.

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

## Comparison-semantics assumptions

Typed string, floating-point, and timestamp filters preserve their representable value domains,
even when warehouse comparison settings are unspecified. `condition_exactness.status` is
`conditional` until the caller declares the assumptions listed in
`condition_exactness.comparison_assumptions`, or `residual` for unsupported semantics.
Use repeatable `--assume binary_collation`, `--assume no_char_padding`,
`--assume no_nan`, `--assume signed_zero_equivalent`, or `--assume session_time_zone`
in direct, dbt, or manifest CLI analysis. Manifest JSON can declare
`"comparison_assumptions": ["binary_collation"]` at the root.
Library callers can use `AnalysisBundle::declare_comparison_assumptions`.
Explicitly timezone-qualified physical schemas preserve `timestamp_zone` on source columns,
without changing the canonical `DataType::Timestamp` public shape.
Typed timestamp bounds use canonical `YYYY-MM-DD HH:MM:SS[.fraction][offset]` values,
with explicit offsets normalized to `+HH:MM` or `-HH:MM`. An offset-bearing literal
against an explicitly timezone-free column yields Unknown and a residual, never an
exact comparison. See the protocol docs for timezone-qualified and unqualified semantics.
No comparison setting is assumed solely from a SQL dialect or metadata source.
See [the protocol contract](../docs/protocol.md#typed-predicate-domain-literals) for guarantees.

## Differential conformance

The standard Rust test suite includes a deterministic differential oracle in
`tests/differential_conformance.rs`. It runs protocol exactness, source-domain, output-domain,
join-equality, and CASE-branch claims against DuckDB. The suite combines a curated matrix with
3,000 seeded AND/OR/NOT predicate trees; failures report the reproducing query and seed.
Completeness checks require allow-listed predicates to remain exact through CTEs, chained CTEs,
derived tables, and multi-layer identity projections. Seeded local-relation equivalence cases
compare exactness and residual reasons with the inlined form. Explicit and implicit two- and
three-source joins verify physical equality identities and domains against DuckDB. Typed-schema
cases assert portable exactness while retaining explicit residuals for comparisons whose
collation, floating-point, or timestamp semantics are not yet represented.

DuckDB is a development-only dependency with default features disabled. The repository does not
enable duckdb-rs's `bundled` feature, so it never compiles DuckDB from source. Cargo config sets
`DUCKDB_DOWNLOAD_LIB=1`, which makes duckdb-rs download and link the matching prebuilt DuckDB
library. Set `DUCKDB_DOWNLOAD_LIB=0` in the environment to use an already installed compatible
system library instead.



## Exact grouped HAVING witnesses

For a supported simple aggregate comparison, `group_witness` separates proof of a qualifying group from proof of a HAVING-rejected group. The local input relation boundary and group keys identify which rows must share group identity; each independent witness case supplies inclusive total-row and non-NULL contribution bounds, plus typed contributor predicates (`every`, `some`, `sum`). Count(*) includes NULL rows, Count(column) excludes NULL contributors. MIN and MAX use universal or existential restrictions over contributors; SUM uses an aggregate sum comparison. The rejected direction includes all-NULL SUM/MIN/MAX groups because HAVING treats NULL comparison results as unknown. COUNT(*) GROUP BY cannot have an existing zero-row group, so `HAVING COUNT(*) < 1` has no qualifying case.

Aggregate predicates alone must not constrain individual source-column domains (for example `SUM(amount) > 10` cannot imply `amount > 10`). When an output column is exactly the aggregate compared by HAVING, its *output* value domain can instead be restricted by the comparison, preserving physical lineage independently. A residual witness status is not evidence of source-group constructibility; it is deliberately safer than guessed inversion. Complex grouping, non-row-preserving producers, disjunction, aggregate FILTER/DISTINCT, and uncertain source identities remain residual. `condition_exactness` continues to reflect the independent source row-domain contract and may stay residual due to HAVING even with a separately exact group witness.

## EXISTS, IN, and NOT IN witness semantics

The analyzer preserves nested-query predicates and emits additional
generator-facing subquery membership evidence when both source sides are
unambiguous. Positive and rejected cases are classified independently,
including the empty-result and three-valued NULL rules:

- `EXISTS` tests whether any correlated candidate survives; `NOT EXISTS`
  reverses the two cases. Duplicate candidates do not change the result.
- `IN` is TRUE on a non-NULL equality match. If the candidate set is empty,
  it is FALSE even for a NULL outer operand. With candidates, unmatched NULL
  values on either side result in UNKNOWN and the outer row is rejected.
- `NOT IN` is TRUE for an empty candidate set, or non-NULL unmatched values
  with no NULL candidate. Either a match or an UNKNOWN result rejects the row.
  The analyzer never models NOT IN as NULL-insensitive anti-join.

Correlations are source equality obligations carried jointly, not
independently sampled value intervals. Inner column domains constrain the
source witnesses but do not imply correlated Cartesian widening. The
whole-query condition exactness remains residual where the canonical
independent domain contract cannot describe membership exactly. Complex
nested row-set shapes, self-joins, nested joins, unknown physical lineage,
or unprovable computed correlations produce named residual directions.
Originating witnesses are retained in multi-layer composition along with
physical/intermediate boundary provenance so consumers can decide which
source is controllable. A dbt/SQL adapter provides the same capability if
it supplies equivalent SQL; metadata-only adapters do not synthesize SQL.

## Correlated single-row boolean filters

A cross-column `OR` is not safely describable as a conjunction of independent
column domains. For a single source such as
`a IS NULL OR b IS NULL`, the analyzer may instead carry a typed
`boolean_witness` whose `any` branches apply to the **same row**.
Its qualifying truth is TRUE, while its rejected truth is FALSE **or UNKNOWN**.
A fully proven conjunctive integer filter can additionally tighten the
independent source and projected output domains. Disjunctions retain their
conservative independent `column_domains`; neither case grants
whole-query row-membership exactness without all other proof obligations.

Signed integer comparisons require authoritative catalog datatypes and
representable signed literals. Overflow-free identity arithmetic (`+a`, `a+0`, `0+a`, `a-0`),
catalog-proven lossless ordinary signed-integer CASTs and constant additions
or subtractions on a widened signed cast are inverted. The arithmetic
result must fit the explicit cast target width for the *entire* source
integer domain; its comparison threshold is translated to the source
column. Uncast nonzero offsets, narrowing and TRY/SAFE_CAST variants,
functions, ambiguous sources and expressions that can overflow remain residual. Ordinary LIKE/NOT LIKE with one trailing wildcard after an
unescaped ASCII alphanumeric prefix is represented as a typed, jointly solved
source-row `string_prefix`; both `binary_collation` and `no_char_padding`
attestations are required before either direction can become exact. SQL NULL
remains UNKNOWN, also under NOT LIKE; embedded wildcards, ILIKE, escaped
patterns and fixed-width or unknown string types default to residual. Repeated columns are evaluated jointly using bounded integer truth
partitions and remain residual if their search space is too large. Pure
conjunctions, including contradictory repeated-column comparisons, use the same
solver. Enforced NOT NULL, primary key and finite accepted-values metadata
can further reject impossible witness directions, including after adapter
enrichment. Unknown enforcement or dependent foreign-key satisfiability stays
residual. Witnesses
retain their originating layer. Only a proven single-relation, identity-only
lineage chain can remap the coupled condition to physical column references;
computed or ambiguous projections, and even identity projections behind
WHERE, DISTINCT or LIMIT, preserve the intermediate boundary. A physical
mapping additionally requires a row-preserving producer chain. Type- and
collation-dependent comparisons remain intermediate across producer layers
unless source-schema parity is independently established.


### DML before/after correctness

An INSERT SELECT, standalone single-target UPDATE or DELETE, or MERGE exposes
a typed mutation effect. It requires caller-supplied **complete** initial target
rows, retains branch selection predicates and necessary domains, and never
invents untouched rows. The complete poststate is an effect applied to that
initial snapshot, except unconditional DELETE which guarantees the target is
empty on successful execution. Row conservation is typed and checkable from
*verified* inserted/updated/deleted target row counts, not estimated from
source cardinality. These count equations say nothing about whether a
mutation will succeed: engine-specific MERGE conflict behavior, composite
keys, NULL semantics, and schema enforcement remain explicit obligations.

The bundle associates every mutation with its resolved source/target relation
identities and whatever canonical target constraints SQL, dbt, or ODCS
evidence provides. Missing constraint evidence remains unknown, rather than
silently being interpreted as no keys. Key uniqueness must be checked across
inserted, updated, and untouched rows of the actual target. Unknown computed
predicates and noninvertible assignments keep conservative domains; their
presence never licenses a generator to claim complete exactness.


## Physical-source row realization (TASK-68, partial)

The Rust library's `physical_source_plan(bundle, target_layer_id)` returns a
deterministic, reference-based physical dependency graph. Each source and
producer is represented once, and producer nodes retain their write kind.
A named derived relation is **not** a directly writable physical source.
Missing, cyclic, ambiguous and partial producers fail closed with typed
`PhysicalProofGap` reasons. Nodes are ordered producer-first.

For now, the library lifts both TRUE and NOT TRUE **single-row** boolean
witnesses to physical sources only when exactly one operator witness is
present and all other producer layers are single-source, one-to-one direct
column projections. The filtering layer must have a direct single table,
no CTE, limit, DISTINCT, join, grouping or other row-shaping, and proven
physical row identity. Projection computed columns cannot be inverted by
this proof. The two directions remain independent and can be residual.

**This is not a whole-DAG constructive solver.** It does not yet prove
joint satisfiability of multiple operator witnesses, multiple physical
sources, full output cardinality, or materialized final states. In these
cases the physical graph remains available and both row classifications
remain explicitly residual. The existing operator-local
`local_constructive_witnesses` API must not be interpreted as complete
physical realization. The protocol JSON contract is unchanged by this
library-only foundational step.
