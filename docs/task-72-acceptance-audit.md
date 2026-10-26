# TASK-72 original-parent acceptance audit

**State:** parent review in progress; original AC #1..#6 remain unchecked until
final-head CI and independent review. TASK-91 is a separate release gate.

## Source of truth

`docs/task-72-acceptance-freeze.json` records the finite parent closure
inventory and references child-owned frozen expectations instead of copying
their source bags or recomputing terminal values. Its executable guard is
`tests/task_72_parent_closure.rs`.

The merged child evidence is:

| Child | Source and terminal proof | Parser and adapter matrix |
| --- | --- | --- |
| TASK-117 | `tests/task_117_set_bags.rs`: six DISTINCT/ALL multiset laws, complete two-column NULL histograms, positive and impossible membership | `tests/task_117_dialect_matrix.rs`, `tests/task_117_set_adapters.rs` |
| TASK-118 | `tests/task_118_nested_set_bags.rs`: nested three-branch bags, shared physical identities, SQL TRUE/NOT TRUE, CTE/model | `tests/task_118_dialect_matrix.rs`, `tests/task_118_set_adapters.rs` |
| TASK-119 | `tests/task_119_ordered_set_bags.rs`: positional aliases, explicit NULL ordering, LIMIT/OFFSET/FETCH, full rejected bag | `tests/task_119_dialect_matrix.rs`, `tests/task_119_set_adapters.rs` |
| TASK-120 | `tests/task_120_parent_set_matrix.rs`: independent complete DuckDB input/output oracles for 21 mandatory cases | `tests/task_120_set_adapters.rs`: 63 mandatory parity cells; 273 parser law cells |

All four child tasks are Done on `main`, and their PRs #127 through #130
are closed. Independent child CI was green at the reviewed implementation heads,
including [TASK-120's nine-job run](https://github.com/phdah/sql-semantic-protocol/actions/runs/39250351880).
The new parent closure commit needs its own CI.

## Original acceptance trace

| Original TASK-72 AC | Mandatory portable evidence | Typed conditional boundary |
| --- | --- | --- |
| #1 Set laws, trees, NULL, multiple columns and types | TASK-117's six two-column positive/negative laws, TASK-118's nested tree, TASK-120 matrix | MINUS parser/vendor laws; coercion beyond attested compatible source types |
| #2 Alignment, aliases, local branches and physical sharing | TASK-118 shared CTE/model and filtered branches; TASK-119 positional aliases; TASK-120 composed fixture | BY NAME/CORRESPONDING, unproved casts and collations, nested branch limits and nontransparent joined/grouped producers |
| #3 Positive/nonmembership and frequencies | All 21 TASK-120 closed-source Exact cases; independent rejected/impossible counters | Unknown producer/type law never yields a fabricated exact or empty bag |
| #4 Branch/set shaping and compatibility | TASK-119's exact sorted LIMIT/OFFSET/FETCH/NULL cases | Underordered ties, nested branch shaping, native dialect ordering/coercion laws |
| #5 Independent execution | TASK-117/118/119/120 complete DuckDB physical and terminal bags, including NULLs, duplicates, shared producers, empty and impossible outcomes | No native non-DuckDB vendor engine execution claim |
| #6 Public contract, tests and adapters | 13 parser identities; 273 mandatory parser cases; 63 SQL/dbt/ODCS exact adapter cells; schema/golden regressions in full Rust CI | Six explicit conditional cases, no silent unsupported variant or loss of domain information |

The expected physical values are not derived from production evaluator results.
The TASK-120 freeze owns 21 mandatory and six conditional cases, including
the positive and negative fixtures for all six set laws. The parent guard
checks every original AC, fixture reference, stable source/terminal expectation,
dialect/adapter breadth and conditional release owner.

## Conditional release dispositions

The six TASK-120 conditional classes are MINUS, BY NAME, CORRESPONDING,
mixed-type coercion/collation, local branch LIMIT and ambiguous ORDER ties.
TASK-120 remains the evidence owner. Each stays Residual/parser-boundary
until the required typed execution or semantic law is attested. In particular,
a parser accepting MINUS does not prove its native engine output; a
non-exact terminal result must not be serialized as the empty set.

Arbitrary joined/grouped branch production is not covered by the portable
TASK-118 exact source-producer boundary. Parent freeze v2 now tracks
`72-PARENT-C-joined-grouped-branch` with explicit unknown terminal domain,
Residual positive/NOT TRUE evidence and null (not fabricated empty) bags.
TASK-85 owns the required whole-source producer composition, TASK-89 owns
integrated execution evidence, and TASK-91 owns any release disposition.
This **conditional coverage boundary** is not a constructive proof or native
vendor certification. Cross-feature release approval is not inferred from
TASK-72 portable signoff.

## Two-axis review

**Standards:** The new guard is a public integration test over frozen
contract data; it does not add AST parsing, consumer-specific semantics,
dependencies, runtime behavior, protocol-schema changes or copied bag facts.
Potential design risk is metadata duplication, mitigated by keeping exactly
one source of numeric results in the child matrices.

**Spec:** AC #1..#6 are individually mapped to executable evidence; all six
portable operators have positive and negative nullable cases, the composed
and ordered cases are separate, and conditional variants remain explicitly
unclaimed. Parent closure must not be inferred from a child green build.

## Merge/signoff gate

Before marking the parent Done, confirm on **this PR's final head**:
`cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`;
`cargo test`; `cargo doc --no-deps`; no-default-features checks;
schema/golden regressions; and `make dbt-e2e`. Review that all 21
mandatory cases, 273 parser cases and 63 adapter cases still execute.
Only then mark TASK-72 AC #1..#6 complete and proceed to downstream
dependency queues. No release approval is implied.
