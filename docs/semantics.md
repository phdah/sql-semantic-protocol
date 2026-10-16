# Semantic model and behavior

Understand how SQL and metadata become normalized, deterministic protocol outcomes. For the normative JSON shape, see [the protocol contract](protocol.md). For runnable commands, see [CLI usage](cli.md).

## Protocol contract

The Cargo package version is the single active protocol version emitted by the library and CLI. It represents one or many SQL inputs with the same root document shape: `inputs`, `layers`, and `graph`. A single SQL string is therefore represented as one element in `inputs`, not by switching to a different protocol version.

The active contract is defined by [`schema/protocol.schema.json`](../schema/protocol.schema.json), documented in [`docs/protocol.md`](../docs/protocol.md), and demonstrated by [`examples/protocol.json`](../examples/protocol.json) and [`examples/protocol-simple.json`](../examples/protocol-simple.json).

Versioned protocol artifacts such as `protocol-v0.1*` and `protocol-v0.2*` remain in the repository as immutable historical references. Current runtime code and tests use the unversioned active contract paths above.

The analyzer resolves transformation layers into a deterministic relation dependency graph. It composes semantics through that graph: final outputs expose transitive physical lineage, value domains propagate through safe direct projections and renames, and ambiguous, cyclic, or non-invertible paths remain explicit instead of being guessed. Disconnected pipelines compose independently.

### Bounded source-row Boolean witnesses

The canonical Boolean witness can prove SQL TRUE and NOT TRUE independently for
catalog-typed signed integer comparisons, `BETWEEN` / `NOT BETWEEN`, finite
integer `IN` / `NOT IN` lists, nested `NOT` with `AND` / `OR`, and
`IS [NOT] DISTINCT FROM` against an integer or literal NULL. The analyzer
normalizes these into shared integer-comparison and NULL-test obligations,
including De Morgan's laws and explicit UNKNOWN leaves for ordinary comparisons with literal NULL (including NULL-bearing IN lists), instead of introducing a consumer-specific SQL
evaluation language. It preserves SQL UNKNOWN for ordinary comparisons to NULL;
NULL-safe equality is two-valued. Certified total integer CAST/offset NULL tests, COALESCE of signed integer columns and literal NULL, and NULLIF of two signed integer columns preserve precise NULL truth. A non-NULL literal COALESCE fallback additionally proves `IS NULL` impossible and `IS NOT NULL` trivially true. Simple and searched CASE with total signed integer result arms also has exact branch-selecting NULL witnesses, including nullable three-valued guards and implicit NULL ELSE branches: the NOT TRUE region of each guard is derived by folding its UNKNOWN arm back into the accepted region, so fall-through on UNKNOWN is exact rather than residual. Opaque guards, mixed-type or non-literal NULLIF operands, and opaque COALESCE arguments keep explicit residual NULL tests. Exact conjunctive NULL tests and numeric
comparisons refine projected domains when source identity is proven.

Widening signed integer CAST, TRY_CAST and SAFE_CAST share one exact source identity only when all catalog-admissible values fit the target, so no failure-to-NULL or overflow is possible. For finite, catalog-typed inputs, narrowing TRY_CAST and SAFE_CAST produce exact SQL NULL for out-of-range members; a potentially throwing ordinary CAST remains unknown. Narrowing TRY_CAST and SAFE_CAST of a direct signed source also have exact IS NULL and IS NOT NULL source-row witnesses, covering source NULL and both signed overflow boundaries. Exactness still requires authoritative source datatype evidence and a complete,
consistent assignment of all coupled source columns. Arithmetic inversion also admits a bounded affine subset: signed unary minus,
constant addition/subtraction and constant multiplication (including zero), modulo-one identities, including
literal-on-left operations and nesting. Each operator is checked against the
entire catalog-admissible signed range and the attested result width before
rewriting an integer comparison into one scalar source constraint. Catalog-attested integer division may propagate a finite exact integral quotient when both operands and the result are binary64-exact, and division by signed one is invertible on a wholly exact source interval. Fractional division and zero divisors remain residual. Positive-divisor remainder with nonnegative integer input propagates finite outputs or bounded ranges; general periodic remainder inversion stays residual, but unattainable comparisons (for example, an integer remainder modulo 3 equaling 4) have proven TRUE/NOT TRUE classifications using universal absolute remainder bounds. Signed overflow and implicit-coercion uncertainty remain residual.
Nondivisible equality uses an exact SQL UNKNOWN-preserving contradiction for
ordinary comparison and a two-valued contradiction for NULL-safe equality.
The same width proof certifies NULL tests of total arithmetic expressions.
Catalog-typed integer projections also propagate exact finite sets and safely
bounded intervals through total arithmetic, including exhaustively enumerable unions of bounded ranges and correlated addition, cancellation, squares, unit division, and nonzero self-remainder. Syntactically identical operands are evaluated once per admitted source value rather than as an independent Cartesian product. Independent finite operands use exhaustive cross-products, never samples used to assert impossibility. Excluded NULLs remain excluded,
while nullable finite sets explicitly retain NULL. Bare integer literal arithmetic is exact only while both operands and the result fit the deterministic signed 32-bit portable subset; an explicit signed cast can establish a wider result width. Arithmetic results that could overflow, require implicit coercion, or have
unattested dialect rounding preserve an unknown output domain.
A catalog-certified globally total and injective signed affine projection may
also propagate finite excluded source values through its exact inverse. Zero
multipliers, overflow-prone non-widened expressions and non-injective
operations cannot certify an excluded output. Constant decimal
expressions and catalog-typed finite DECIMAL source domains have a checked fixed-point subset: operand and output values
must be exactly representable in binary64, every intermediate calculation
must fit bounded decimal precision, and catalog decimals must respect their attested precision and scale. Nonterminating divisions, zero divisors,
nonportable decimal fractions and negative-dividend remainder semantics are
unknown rather than rounded or approximated.
Explicit signed integer CASTs of finite, exactly integral numeric literals or
canonical ASCII integer text have exact value domains when all values fit.
Catalog-typed finite DECIMAL inputs also admit exact integral conversion; finite string-source conversions require separately attested enforced binary comparison provenance and canonical ASCII text values. For out-of-range integral inputs, ordinary CAST emits an unknown domain with runtime-dependent failure behavior, while TRY_CAST and SAFE_CAST yield SQL NULL.
Unproven text formatting, implicit/session-dependent coercion, and fractional conversion rules remain residual.
This is a local witness, not a proof of complete output-value distribution,
general decimal arithmetic or implicit coercion. NULL-containing finite integer IN/NOT IN lists retain
SQL UNKNOWN, including impossible qualifying branches. Correlated signed
integer cross-column conjunctions additionally prove joint impossibility
from total-order invariants: strict order cycles, equality with
disequality, conflicting NULL with non-NULL requirements, and literal-bound
cycles detected through integer difference constraints with exhaustive type
bounds and enforced accepted values. Complementary null-safe disjunctions
prove tautologies through the same joint check on their NOT TRUE region.
Mixed AND/OR constraints, including small disjunctive integer IN lists,
can be split into at most 64 conjunctive cases (128 atoms per case);
each case must be proved contradictory. Larger formulas, unproved branches,
and non-integer predicates remain residual unless an independent conjunct
already establishes impossibility. Pure NULL tests need no integer type
evidence. Unparenthesized `IS [NOT] DISTINCT FROM ... AND/OR ...` remains a
parser-boundary residual because sqlparser binds the right operand
loosely. MySQL `<=>` normalizes to `IS NOT DISTINCT FROM` and inherits its
typed laws exactly: catalog-typed integer operands are exact with no
attestation, while text operands need the binary/no-padding attestations like
every other text equality. Attested equality-family comparisons between two
catalog-proven variable-width text columns are exact source-row obligations
in both directions, with shared byte candidates across the two sides so mixed
conjunctions with LIKE prefixes stay jointly satisfiable; text ordering has
no proven collation order. Reflexive text disequality (`a <> a`,
`a IS DISTINCT FROM a`) and an ordinary text equality coupled with a NULL
requirement are symbolically impossible once attested, and an
attestation-time impossible direction empties projected output domains. ILIKE,
SIMILAR TO, RLIKE/REGEXP, PostgreSQL regex and `~~` spellings, `LIKE ANY`,
COLLATE expressions, fixed-width CHAR operands, Unicode/non-ASCII patterns,
T-SQL character classes, backslash patterns, interior/leading wildcards and
unsafe ESCAPE modes remain explicit residuals in every parser-capable
dialect; a successful parse never implies engine or collation parity.
A wildcard-free LIKE/NOT LIKE with a portable literal pattern normalizes to
an attested string equality (`eq`/`neq`), preserving SQL UNKNOWN for NULL.
Unsafe casts, non-order-based or unbounded cross-column relations,
computed BETWEEN bounds and
dialect-sensitive regex, collations, coercions or opaque functions
remain explicit residuals. Prefix `LIKE` requires separate binary-collation
and no-padding attestations. An explicit single-character `ESCAPE` is supported only when it never appears in the safe prefix and is not the trailing `%` wildcard. Operator-local witnesses do **not** establish a
complete physical-source realization across arbitrary producer DAGs. TASK-70
and the protocol 3.0.0 release gate remain open for unproved expression families.

The audited `task_70_predicate_evidence.family_evidence` entries in
`docs/coverage-manifest.json` distinguish locally certified subsets from
release-blocking residual families. These are not cross-dialect physical-source
oracles; the 13-dialect TASK-88/TASK-89 and final TASK-91 gates remain separate.


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
boolean, integer, validated date and time literals retain their canonical literal type; integer
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
domain. If SQL parsing produced a validated typed DATE/TIME literal, that literal may be exact for the
matching canonical family. Interval expressions remain explicitly unsupported. Without schema evidence, existing lexical-literal domain derivation is
preserved, but it does not claim datatype-aware coercion semantics.

### Temporal, JSON and opaque-expression boundary (TASK-105)

The machine-readable inventory is
[`task_105_expression_evidence.variants`](coverage-manifest.json), exercised by
`tests/task_105_temporal_json_vendor.rs` across every exposed parser dialect,
including the `postgres` alias. Each entry identifies its SQL expression, AST
family, certified outcome or missing law, and explicit parser rejections.
Hash-prefixed JSON operators can start comments in some dialects; the fixtures
are parenthesized so that tokenization cannot silently certify a different query.
Parser acceptance is not evidence of execution support in a warehouse.

The certified calendar subset is:

- A DATE literal must be a valid fixed-width Gregorian `YYYY-MM-DD` in years
  0001..9999, including century/leap-day validation. Calendar ordering composes
  DATE thresholds into inclusive/exclusive intervals and proves contradictory
  intervals empty. Scalar `NOT` of a direct DATE threshold reverses its operator
  while preserving SQL UNKNOWN for a NULL source value.
- Formatting-free `CAST`, `TRY_CAST`, and `SAFE_CAST` from an already validated
  DATE literal to DATE are identities. YEAR, MONTH and DAY `EXTRACT` from such
  a literal produce exact integer singleton domains. These are normalized to
  existing canonical literal nodes, rather than interpreted by a consumer.
- Clock literals require valid hours/minutes/seconds and no offset; leap seconds
  and `24:00:00` remain unsupported. Timestamp literals require the existing
  Gregorian/clock/numeric-offset representation. Offset-free timestamp bounds
  on timezone-aware or unqualified timestamp columns remain conditional on
  `session_time_zone`, including timezone-qualified literal spellings. No
  session zone is inferred. Source comparison preconditions still come from
  explicit column zone evidence and declarations, as documented above.
- Plain copies from a single ungrouped local relation preserve its output domains,
  including folded calendar constants. Joins, grouping and ambiguous scopes do
  not inherit a non-NULL constant through this path, because they can synthesize
  NULL or lack proven identity. Existing producer composition carries DATE bounds.

Source-level SQL NULL tests are exact independently of the value's temporal,
JSON, array or struct datatype. Their canonical row constraints preserve TRUE,
NOT TRUE, tautological and impossible directions; an ordinary comparison with
literal SQL NULL stays UNKNOWN. This does **not** evaluate a JSON path or compare
nested values. JSON document `null`, SQL NULL and a missing JSON path are distinct;
`JSON 'null'` never becomes a scalar SQL NULL or an ordinary text literal.

General temporal casts, text parsing/date styles, precision changes, source-valued
EXTRACT, weeks/epochs, month-end arithmetic, intervals, named zones, DST folds/gaps,
JSON paths/operators, array indexing, struct comparisons and vendor functions
remain explicit unsupported expressions or residual predicates. Output domains
retain their family-specific reason. Opaque UDF/function nodes preserve their
names and arguments and have named unknown domains and residual truth obligations;
no return datatype, NULL behavior, determinism, stochastic result, clock value or
session law is invented. The protocol remains extensible through these existing
nodes; externally validated UDF execution evidence is not accepted by this task.

Direct SQL, paired dbt artifacts and manifest-declared-schema analysis share these
rules. Catalog datatypes are authoritative when present; declared manifest types
are the schema fallback. The legacy relation-only manifest API does not provide
equivalent typed zone evidence. Existing ODCS evidence enters the same catalog
analysis path. DuckDB oracles cover certified calendar outcomes, NULL/duplicate
filter membership and counterexamples separating JSON null from SQL NULL; no
other vendor engine or whole-graph physical realization is certified here.

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

CASE expressions use expression kind `case` and preserve an optional simple-CASE operand, ordered WHEN/THEN branches, and the optional ELSE expression. Their output domains are the conservative union of branch result domains. Each WHEN branch also emits `source_domains`, and the explicit or implicit ELSE emits `else_source_domains`. Reachable source domains are an OR of alternatives, where each alternative is an AND of physical source-column domains after accounting for earlier branches not matching. Unreachable branches are explicit, and conditions that cannot be reduced safely emit an unknown reason. A CASE without ELSE includes SQL NULL only when the fallthrough remains reachable under the known source restrictions.

A reachable branch result that is a direct column reference is refined by its guard: the guard's true region is intersected with the WHERE-proven source-column domain, and only values inside that region contribute to the branch's domain. When a guard cannot be reduced safely, its branch falls back to the unfiltered arm domain; when a branch's result cannot be resolved back to a physical column, or a union member is unknown, the branch falls back to `unbounded` rather than guessing. An unreachable branch contributes nothing. These refinements are sound only against the same-scope WHERE-proven domains; an alias-shifted guard that cannot be resolved to a physical column falls back conservatively.

`COALESCE` and `NULLIF` remain canonical function expressions (they are not desugared into `case`); their output domains are derived directly. `COALESCE` evaluates argument domains left to right, emits only non-NULL values from each argument, and stops at the first argument proven non-NULL; SQL NULL remains possible only if every argument may be NULL. A never-NULL fallback yields a NULL-free result without admitting an unreachable later fallback. A non-NULL literal fallback additionally decides the NULL-test witness: `IS NULL` can never be TRUE and `IS NOT NULL` holds on every row. `NULLIF(left, literal)` subtracts the literal's singleton domain from the left domain and adds NULL only when the left operand may be NULL or the operands may coincide. Scalar subqueries, unary negation of literals, and opaque functions keep explicit unknown domains.

Simple `CASE` with a column operand and signed integer `WHEN` values synthesizes three-valued equality guards, so a NULL operand falls through exactly like an UNKNOWN searched guard. Both simple and searched CASE select their arm under SQL TRUE guards, and the remaining FALSE-or-UNKNOWN region advances to the next arm. Source-column NULL tests and signed integer CASE/COALESCE/NULLIF comparisons produce exact or impossible source-row witness directions when physical source datatype and reference identity are certified. NULLIF comparison witnesses preserve the UNKNOWN region on equal-value results even through nested NOT/AND/OR. A uniquely resolved CASE alias projected by an unfiltered one-table derived SELECT may reuse that physical row identity for a NULL-test witness; filtered/grouped/joined or ambiguous derived producers remain residual.

Catalog-backed arithmetic evaluates finite CASE arms against their branch-selecting column restrictions and filters out branches contradictory with WHERE. For example, `CASE WHEN a > 1 THEN a + 10 ELSE 0 END` with `a IN (1, 3)` emits `{0, 13}` rather than an unreachable 11. Uncertified guard domains, opaque functions, mixed-type coercions and noninvertible computed predicates retain explicit residuals or unknown outcome domains. NULLIF numeric literal comparisons conservatively include SQL-equivalent integer/decimal values when deriving NULL output possibilities.

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
representable signed literals. Overflow-free affine arithmetic, including unary signs, constant addition, subtraction and multiplication, identity division and modulo-one, is inverted only when every intermediate fits its attested signed width over the *entire* source domain. A narrowing signed CAST is invertible only when all catalog-admissible values fit. Finite TRY_CAST and SAFE_CAST narrowing may instead prove exact successful values plus NULL-on-failure, and their direct-source NULL tests can distinguish source NULL from out-of-range values. Potentially throwing ordinary casts, fractional or zero-divisor operations, noninvertible periodic predicates, functions, ambiguous sources and expressions that can overflow remain residual. Ordinary LIKE/NOT LIKE with one trailing wildcard after an
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


## Multi-parent and repeated-source join realizability (TASK-94)

The join population composer lifts authoritative local equijoin witnesses
through independent or shared row-preserving materialized parents. Each
key's physical lineage is type-checked across every producer edge. One
closed physical population per distinct leaf is enough even for aliased
self-joins. Synchronized distinct non-NULL keys generate exact one-to-one
matches, while one shared key repeated on both sides generates exact
bag-duplicate matches. An explicitly empty whole-side population certifies
outer-join unmatched rows and NULL extension; semi/anti membership follows
the original local join kind.

For joint terminal requests, only combinations with compatible full source
counts and matching assignments to each reused key are composed. A conflict
between independently *sufficient* join examples remains residual because
other source arrangements could be feasible. Unsupported CROSS/multi-join
local evidence, non-invertible parents and incomplete source schemas remain
residual, never invented physical key-generation instructions.

## Joint physical-source realizability (TASK-93, partial)

`physical_joint_source_plan` merges the dependency paths for multiple
terminal output counts into a canonical producer-first graph, retaining
shared leaves, write kinds, typed operator-local witnesses and unresolved
producer boundaries. The `physical_joint_row_count_plan` result is a
separate complete-source construction, not the union of feasible local
examples. Request permutation cannot change its canonical representation.

For jointly controlled positive filter goals with the same source count,
all rows can satisfy a single coupled typed conjunction. Jointly rejected
zero-output filters require SQL NOT TRUE of each predicate; mixed positive
and negative goals must use one identical physical row assignment with
full closed-world coverage. An impossible case is reported only for a
proven contradiction; individually feasible but jointly unresolved source
populations remain residual. Typed integer/NULL facts are supported;
string collation, extra source cardinalities and complex operator
correlations remain conservative. Typed scalar predicates can cross identity-preserving materialization chains
when each producer reference has matching source and output datatypes and
plain-copy lineage at every edge. Missing schemas,
changed types, opaque producer projections and partial writes remain residual.
Cross-row join-pair, ranking and other correlated local witnesses are retained
on graph nodes, but count proofs remain residual until all their physical
row identities and multiplicities are solved; local examples never suffice.

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

The optional JSON contract and closed-world zero-output direction are documented in [the protocol contract](protocol.md). This is not a whole-DAG constructive solver. It does not yet prove
joint satisfiability of multiple operator witnesses, multiple physical
sources, full output cardinality, or materialized final states. In these
cases the physical graph remains available and both row classifications
remain explicitly residual. The existing operator-local
`local_constructive_witnesses` API must not be interpreted as complete
physical realization. The protocol JSON contract is unchanged by this
library-only foundational step.

The closed-world zero-output direction additionally proves zero groups
for explicitly non-empty ordinary GROUP BY keys, including multi-source
joins followed by HAVING and downstream filters. Empty grouping sets,
ROLLUP/CUBE and global aggregate projections are not certified: global
aggregates can emit one row even for empty physical inputs. Nested
aggregates within arithmetic or functions are also excluded from
non-grouped row-local proofs.

A full before/after state can be realized for unconditional DELETE of a
schema-backed, independently controlled physical target. The Rust API
`physical_unconditional_delete_plan` enforces complete initial row
cardinality, entire-relation coverage, and `StateRows(before=N, after=0)`.
Other mutations and any in-bundle conflicting producer ownership remain
unproved.

A nonempty, fully controlled physical source can deliberately produce
**zero** terminal rows when an exact one-source filter chain proves a SQL
NOT TRUE assignment for every physical row. The
`physical_rejected_row_count_plan` Rust API carries both source count
and complete closed-world negative truth; the joint API can reuse that
assignment alongside a transparent positive terminal of the same
physical source count.

A two-parent materialized equijoin also has a narrow positive-count
construction: each parent must preserve its independent physical
source rows and integer join key exactly, with no earlier filtering
or row shaping. The joined output may then pass through further
single-parent transparent producer layers. Shared physical sources,
mixed operators and incomplete key evidence remain residual.

A certified local join, GROUP BY/HAVING, ranked window, or set count can
be transported through *downstream* single-parent, value- and
row-preserving materializations. This reuses the independently validated
physical construction, not merely its apparent output count. Complete
typed scalar distributions are likewise mapped through validated
physical-column lineage and actual source/producer catalog schemas.
Repeated output aliases share a single physical histogram; contradictory
complete frequencies for direct copies are impossible. At a terminal SELECT
following a fully row-preserving typed producer chain, reversible signed
integer additions, subtractions and negation may also be inverted bucket
by bucket with checked arithmetic at the source integer width, including
successive reversible transformations in materialized producers. A canonical
`physical_joint_distribution_plan` simultaneously realizes compatible
complete source histograms across multiple terminals. Shared physical columns
are assigned only one histogram and conflicting full bags or mandatory row
counts are impossible; disjoint columns can share an independently typed
complete row assignment. A composed source can also carry boolean and
variable-text scalar frequencies when storage length and types are proven. NULL
propagation, overflow, materialization datatype equality and every projected
column are checked. Lossy, ambiguous or unguarded computations and unknown
columns remain residual. No intermediate table is independently writable.

A narrow positive group or rank population may now be constructed even
when its source is a materialized, identity-preserving producer rather than a
direct physical table. The outcome-goal `groups` or `ranked` witness
references only the actual physical relation and schema-certified keys,
including renamed GROUP BY, partition and ordering columns. A group witness
must still have a locally exact COUNT(*) / HAVING law; a rank witness must
have a locally exact ROW_NUMBER / QUALIFY law. Group keys use distinct
physical integer values, and ranked partitions use complete physical rows.
The producer chain cannot filter, compute, duplicate or coerce mapped
columns. Two-branch `UNION`, `UNION ALL`, `INTERSECT`, and `EXCEPT` local
tuple laws can also be transported across independently materialized
row-preserving single-column branches when every branch column maps
through type-certified producer scopes to a **different** physical source.
The resulting `set_tuples` obligations name the physical source columns
and retain the local duplicate and NULL-equality multiplicity law, rather
than claiming that derived relations can be written to directly.
Identical, independently materialized branches can also reuse a single
typed physical tuple column when their entire bag counts agree; arbitrary
shared-source operator interactions remain residual.

The physical-source Rust API proves exact counts through schema-backed
transparent producer chains and identity-preserving WHERE chains with a
jointly proven SQL-TRUE physical-row predicate. Positive filtered cases
close the entire physical source at the requested count and require every
row to qualify. Shared-source count plans retain that predicate. Distinct positive
integer/NULL-sensitive filters on the same physical source can now be
conjoined when exact source schema evidence proves a *single* common row
assignment; the canonical `RowTruth` tree records the combined condition.
Unsatisfiable conjunctions are impossible only when a necessary unfiltered
source count fixes the entire population. Without that count, extra source
rows could independently satisfy each filter, so the result is residual.
Multiple independently rejected zero-output filters can likewise be
jointly certified against a nonempty shared source by requiring each
canonical row predicate to be SQL NOT TRUE on the **same** physical row,
retaining FALSE and UNKNOWN behavior. If no positive source count fixes
the input size, the joint planner can choose a one-row nonempty,
entire-relation-closed assignment for several zero-count terminals.
A contradictory nonempty rejection conjunction does not make a zero
terminal impossible: the existing all-empty physical construction
remains a distinct sufficient alternative. Other unproved combinations, including
detached string-collation predicates, remain residual. The legacy outcome-goal adapter
remains residual for positive filtered counts because its `source_rows`
witness cannot encode required predicate obligations. Already-certified physical join, grouping, QUALIFY and set population
laws can be composed when their complete assignments are compatible.
In particular, two lossless materialized copies of the same physical
bag cancel under EXCEPT/EXCEPT ALL at any certified positive source
count, including NULLs and duplicates; a transparent positive terminal
may reuse that exact count. Exact nonempty transparent source
counts also permit an INNER JOIN to have zero output through a completely
empty physical partner, with one source counted and its other partner
explicitly closed. By contrast, an unfiltered LEFT, RIGHT or FULL JOIN
cannot have zero output when a verified preserved input has nonzero
cardinality, which is reported as impossible rather than a missing
sufficient join candidate. Both laws retain join-kind and materialized
producer provenance. Direct SQL and dbt manifest/catalog evidence must
reach the same physical-row truth and closed-world population.
Arbitrary operator combinations and unproved correlated source
assignments remain typed residuals. General
DML state construction is handled conservatively by the separate ordered
physical-state API described below; unsupported operator populations remain
unverified.


### Joint physical operator populations (TASK-95)

Complete physical join key populations can also be homogeneous across multiple keys rather than only a bijection or one common key. The typed `grouped_matched` witness records an explicit group count and exact per-key contributor bag multiplicities, and only composes through named materialized grouping when the group key is traced to the same type-certified physical source and local HAVING aggregate tests admit each group. Derived filters over COUNT(*) can combine exact scalar comparisons using AND/OR/NOT, while arbitrary computed expressions remain residual. Correlated EXISTS and non-NULL IN/NOT IN can use a shared complete key population across transparent sources, never independent candidate samples; SQL NULL-sensitive cases not proved remain residual.

The complete `rank_population` proof assigns all rows of every physical partition, including a strictly unique order value for each row. It composes through certified identity producers and, when matching join-key bag assignments are compatible, through JOIN-to-ROW_NUMBER()/QUALIFY. The source and join populations must agree on grouping keys, per-key contributors and total cardinality. Up to four nested set branches can carry exact source-bag obligations through materialized producers. Two complete lossless aliases of the same physical source have identical bags, which proves EXCEPT/EXCEPT ALL cancellation, including NULL-safe tuple comparison; filtered branches are not interchangeable. A supported COUNT(group-key) aggregate is equal to COUNT(*) only where a non-NULL key assignment was constructed. In all other cases, preserve the typed residual and the canonical DAG provenance.

An exact `group`, `ranked`, or `set_tuples` outcome witness can be promoted
to a `operator_population` obligation only after the complete physical
source DAG is certified: every materialization is resolved, each involved
column has compatible type evidence, and every independent leaf relation
is assigned a **complete** source-row count with an entire-relation closed
world. Set branch counts preserve per-tuple ALL/DISTINCT and NULL equality
semantics. A source table is assigned only once.

A multi-terminal `physical_joint_source_plan` may conjoin such operator
populations with independent sources or with matching transparent
source-count goals. Repeated source assignments are accepted only when the
same typed operator population and exact physical count agree. Incompatible
*sufficient* examples remain residual rather than being mislabeled
impossible; they do not rule out other satisfying populations.

Two narrowly certified multi-operator chains also have constructive
physical plans: a distinct matched equijoin followed by GROUP BY/HAVING
with **exactly one matched row per grouping key**, and an ordinary
GROUP BY/HAVING producer followed by ROW_NUMBER/QUALIFY that selects
the only row of each key partition. The latter retains an intermediate
rank obligation linked by `Producer` to the complete physical group
population, without authorizing an independent write to the group result.

Mixed joins with duplicated keys, join-to-HAVING requiring multiple
contributors, aggregate-derived arbitrary filters, grouped sets,
shared-source set operands, and general subquery/branch correlations
are not yet constructively certified. They remain typed residuals,
not false feasible results. The end-to-end TASK-95 acceptance gate is
not satisfied by this subset.

### Extended mixed-operator count and deliberate-rejection proofs

For an identity-only materialized INNER equijoin, the group composer now
supports both distinct matched keys (one contributor per group) and a
complete common-key bag (one group with multiple contributors). HAVING
`COUNT(*)` uses the existing exact group-count interval; it is never
assumed true merely because a group is present. The mixed join/group
construction can share the same exact physical counts with transparent
source-reading terminals, reconciling each source identity once.

A materialized ordinary GROUP BY/HAVING producer can also feed a single
outer WHERE comparison of its copied `COUNT(*)` result. The producer
supplies a complete group population, so the outer predicate is checked
against every exact per-group count. SQL-FALSE outcomes are constructive
nonempty-source rejections: they are not inferred from an open-world
single-row candidate. Ambiguous, non-count, computed and unsupported
predicates fail closed.

For correlated EXISTS/NOT EXISTS or IN/NOT IN, a locally exact
`no_candidates` case can be lifted through complete row-preserving
materializations of independent outer and inner sources when the whole
inner physical relation is proven empty. The outer source is explicitly
nonempty and fully closed, preserving SQL NULL behavior for IN/NOT IN.
The generated `Membership` obligation is source-backed; correlation
and membership keys are vacuous because no inner row can qualify.
A nonempty inner source is also sufficient for **uncorrelated, unfiltered
EXISTS** to select every outer row, or **NOT EXISTS** to reject every outer
row. This witness assigns exactly one complete inner physical row and the
requested (or one, for zero-output rejection) complete outer population.
It requires independent schema-backed physical leaves, transparent producers,
a locally exact `matching_row` case, and no nested WHERE condition. The
joint composer can reconcile the same assignments with transparent source
readers; a different sufficient inner count is residual, not impossible.
Nonempty correlated matches, IN/NOT IN key comparisons, nested filters,
mixed predicates and repeated physical sources are still residual.


Two separately materialized, lossless set operands may also resolve to the
**same** typed physical source and column. This is constructive only when the
existing operator-local positive tuple case requires identical multiplicities
on both operands (for example, `UNION DISTINCT` and `INTERSECT` with
identical source projections). The joint population binds the repeated
operand obligations to **one** physical relation, with one exact source-row
count and entire-relation closure. For `UNION ALL`, each input tuple is emitted
twice, so complete positive even-sized output counts are constructible from
half as many distinct physical tuples. An odd row target, a mismatched mapped
column, an unproved shared bag law, or a frequency distribution requiring
per-value inversion remains residual. `EXCEPT` positive shared-source
populations are not yet separately classified as impossible. No independent
rows may be fabricated for copied branches.

### Independent mixed physical operator DAGs

Complete mixed join/group, group/rank, grouped COUNT filter and certified empty/nonempty-inner membership populations can be composed across separate terminal targets when their sets of physical source relations are disjoint. Each local case must prove an exact entire-relation closed source count; the joint case carries both operator laws and any transparent source terminal counts. A repeated physical source across different mixed cases still requires a correlated proof and remains residual, even if the sampled row counts happen to agree.


### Ordered physical write-state construction (TASK-98)

`physical_ordered_write_state_plan(&bundle, &complete_initial_states)` realizes each
canonical SQL write in statement order against one shared physical state per
relation. The caller provides whole, closed-world typed populations, never
sampled rows or independent assignments for intermediate producers. Each
`PhysicalWriteTransition` carries its exact target `before` and `after` rows
and counts of inserted, updated and deleted rows. `final_states` is a
complete relation-to-bag assignment. `PhysicalWriteState::exact_domains(&schema)` derives a closed-world
per-column `PhysicalWriteColumnDomain` with exact NULL and duplicate
frequencies; even an empty table yields a certified empty domain for
each declared column. The rows and domains refer to the same bag.

Supported proof paths are single-source row-preserving or filter-only
INSERT SELECT and CTAS/CREATE OR REPLACE TABLE, target-local UPDATE and
DELETE, and first-applicable MERGE matched, not-matched and
not-matched-by-source branches where each target sees at most one source
match. SQL NULL is evaluated with three-valued Boolean predicates, SET
expressions read the original row, and later statements read the entire
already-written target. The existing canonical `WriteStateEffect` checks
row-affect cardinality for DML; lifecycle kind explicitly distinguishes
CREATE TABLE from view definitions. Normalized target and source aliases
share one physical population.

Before certifying feasibility, the plan verifies complete typed schemas
and attested enforced NOT NULL, key, accepted-values and FK constraints,
including cross-relation FK lookup. Known key/null contradictions are
`Impossible`. Missing complete snapshots, ambiguous aliases or MERGE
multiplicity, unknown expression and collation semantics, unverified
constraint enforcement, implicit column defaults, logical views,
unsupported ALTER or session/transaction laws are explicit
`Residual { gap, reason }` outcomes, **never** fabricated feasible
states. Foreign-key violations remain residual when deferred enforcement
timing is not proved.

This opt-in row construction is independent of the existing abstract
`physical_joint_source_plan` outcome goals: arbitrary JOIN, aggregation,
QUALIFY, nested set or multi-operator witness populations are not
reinterpreted as scalar row tuples without an exact operator-specific
evaluator. There is no implicit guarantee that an arbitrary positive or
negative terminal goal can be realized. The consumer may use the exact
snapshots to run the SQL program in DuckDB and verify its outcomes.

TASK-101 provides executable DuckDB negative examples for ordinary integer
cast overflow, TRY_CAST NULL-on-failure, exact signed boundaries, and safe
decimal and integer arithmetic. These are local scoped proofs. Nonportable
implicit coercion and full cross-dialect execution equivalence remain separate
release gates and are not inferred from DuckDB observations alone.
