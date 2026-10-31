# TASK-142 bounded recursive CTE acceptance audit

Scope: TASK-142 (TASK-78 child 2/4). The frozen task matrix is
[`v2`](task-142-acceptance-freeze.json), with the immutable
[`preimplementation v1`](task-142-acceptance-freeze-v1.json) retained.
No parent or release gate is closed by this child.

## Exact and impossible obligations

The two mandatory Exact fixtures `78-C142-P-ALL` and
`78-C142-P-DISTINCT` use source-free integer seed 1, positive step
`x + 1` under `x < 3`, and produce the independently fixed complete
bag `[(1), (2), (3)]` with stable producer occurrence identities.
The terminal bounds are inclusive `[1,3]`, with NULL excluded.
The `78-C142-N-ABSENCE` obligation checks complete positive
source/producer/terminal bags and Impossible membership for 5 and NULL,
with native DuckDB `NOT EXISTS` and `x = NULL` oracles. The FALSE
termination witness is attached to the final delta source row; unsupported
or cycle/overflow cases return typed Residual rather than a fabricated
finite proof. The canonical recursion node explicitly points to itself,
not an independent physical insertion.

The v2 strengthening restricts Exact claims to a portable signed
32-bit integer sequence. The maximum constructive bag is 10,000 rows;
unproven larger programs are Residual, not truncated Exact. For a strictly
increasing progression, `UNION DISTINCT` and `UNION ALL` have identical
physical occurrences, but retain distinct canonical mode labels.
Zero-step cyclic UNION ALL remains a reviewed conditional Residual,
owned by TASK-142. Native vendor/session recursion laws beyond the
DuckDB executable oracle are **not certified**.

## Parser and adapter parity

`tests/task_142_dialect_adapter.rs` validates the two frozen SQL forms
against all **14** names in the project's authoritative `DIALECTS`
constant, including Databricks. Each dialect's frozen parser-equivalent
claim must match actual parsing or fail CI; parsing success must also
construct the same source-free canonical program, recursive self-edge,
exact terminal bag and integer bounds. This is parser/canonical evidence,
**not** native execution certification for each vendor.

The same SQL is analyzed through direct catalog input and compiled dbt
manifest v12/catalog v1. The test compares canonical query and complete
physical source, stage, rejection, terminal and typed-domain evidence.
ODCS source parity is **not applicable** to these source-free literal
recursive seeds: no external relation or declared source-column fact
exists for an ODCS contract to supply. No equivalent metadata evidence is
silently omitted.

## Review against standards (author pass)

- `AGENTS.md`: no new dependencies, unsafe, lint allowances, or
  parser dialect whitelist. AST normalization stays in `src/analysis/cte.rs`;
  Rust-domain recursion in `src/protocol/cte.rs`; closed-world physical
  execution in `src/physical_realization/cte_bags.rs`.
- The source-free recursion node represents its self-reference as a
  canonical typed producer edge. Unsupported SQL does not borrow the
  nonrecursive CTE evaluator's proof.
- The signed width and bounded step rules protect against host arithmetic
  overclaiming SQL engine outcomes. Output domains are derived from
  the complete final physical bag, not from independently invented rows.
- `src/lib.rs`, `docs/code-map.md`, and `docs/semantics.md` describe
  the new Rust API and proof boundary. There is no public JSON wire-shape
  change, so no protocol-schema migration is required.

## Review against frozen specification (author pass)

The source-free seed, recursive positive step, UNION vs UNION ALL,
finite termination, cyclic Residual, exact terminal outcomes, TRUE/NOT
TRUE membership and conditional uncertainty are tested through public
APIs and DuckDB. The finite source/protocol/terminal matrix is covered,
and parser-vs-engine evidence is kept distinct. No support is claimed
for arbitrary recursive joins, vendor-specific CYCLE clauses, or
unproved negative/overflow sequences.

**Signoff gate:** A separate independent standards/spec review is still
required by TASK-142 AC #5 and Matt's code-review workflow. This author
review does not impersonate an independent reviewer. Final-head CI and
independent review must both pass before TASK-142 can be marked Done.

## CI evidence and unresolved independent review gate

At semantic-code head [`b7981fc827a7`](https://github.com/phdah/sql-semantic-protocol/actions/runs/41191083396), all nine required CI jobs
passed, including full tests, strict Clippy, rustfmt, documentation, both
no-default-features checks, dependency checks and dbt Core E2E. The
all-dialect canonical proof and compiled-dbt output interval assertions were
executed in those tests. Exact SQL, dbt and physical computed output domains
use inclusive integer bounds `[1,3]` for the mandatory cases, and the
negative case has independent Impossible/NOT TRUE evidence.

**Author review:** requirements #1 through #4 have complete executable evidence.
**Independent review:** pending. AC #5 is deliberately not checked;
this file must not be treated as an independent approval. TASK-142 remains
In Progress and TASK-143 must not begin until TASK-142 is Done and merged.
