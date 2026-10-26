# TASK-136 independent acceptance audit

## Contract

TASK-136 is the fourth bounded child of TASK-76 and does not sign off the parent.
The versioned v3 freeze at `docs/task-136-acceptance-freeze.json` owns
stable `76-C136-P` and `76-C136-N` fixture families:
**5 mandatory deterministic cases, 6 conditional laws, 143 dialect obligations,
33 direct SQL / compiled dbt / ODCS adapter obligations.**

The original ten parser spellings were exercised across all 13 exposed sqlparser
dialects in CI [40358327924](https://github.com/phdah/sql-semantic-protocol/actions/runs/40358327924).
The seeded `SAMPLE ... SEED` variant and all 143 canonical proof cells
are guarded by `tests/task_136_dialect_matrix.rs`; their final execution
remains a merge prerequisite.

## Independent output and physical source evidence

| Frozen family | Full physical source | Expected terminal | Membership |
| --- | --- | --- | --- |
| LIMIT | s=[1,2,3], stable s#0..s#2 | [1,2] | two Exact selected, s#2 rejected |
| OFFSET beyond bag | s=[1,2,3] | [] | empty Exact terminal, any requested tuple Impossible |
| FETCH FIRST | s=[1,2,3] | [1,2] | two selected, one rejected |
| T-SQL TOP | s=[1,2,3] | [1,2] | canonical Exact, native DuckDB SQL uses equivalent LIMIT, not a false SQL Server execution claim |
| NULLS LAST | s=[NULL,1,2] | [1,2] | s#0 rejected, nullable domain narrowed to [1,2] inclusive |
| Peer ties | s=[1,1,2] | not uniquely determined per identity | Residual |
| Implicit NULL placement | s=[NULL,1,2] | session-dependent | Residual |
| SYSTEM sampling | s=[1,2,3] | stochastic | Residual |
| BERNOULLI REPEATABLE(7) | s=[1,2,3] | seed algorithm unattested | Residual |
| SAMPLE without seed | s=[1,2,3] | native algorithm unattested | Residual |
| SAMPLE SEED(7) | s=[1,2,3] | native seed algorithm unattested | Residual |

`tests/task_136_frozen_matrix.rs` reconstructs the entire source and
terminal bag with independent DuckDB SQL, then checks the public physical
proof's row identities, exact finite domain, inclusive bounds, NULL
membership and TRUE/NOT TRUE partitions. For every mandatory case,
requesting tuple `k=999` must be `Impossible`; exact positive membership
must be backed by the full physical bag.

`tests/task_136_sampling.rs` additionally checks seeded method/quantity
syntax, the exact emitted sampling JSON/schema contract and the
sampled-`DISTINCT` no-overclaim regression.

## Dialect, adapters, and native execution

`tests/task_136_dialect_matrix.rs` requires all 143 task-owned dialect
parser observations to agree with canonical Exact or Residual membership.
Parser acceptance is **not** native engine certification.

`tests/task_136_adapter_parity.rs` supplies authoritative equivalent
source types through direct SQL, compiled dbt manifests, and ODCS contracts.
It checks all 33 finite and conditional adapter cells against the same
physical source population and query-level sampling facts.

`tests/task_136_dbt_core_e2e.rs` is invoked by `make dbt-e2e` after
real dbt seed/run/docs, asserting DuckDB's complete source and compiled
model output, source-row identities and exact typed domains.
No native non-DuckDB vendor sampling algorithm, session NULL defaults or
underordered row identity is certified. These conditional cases are
reviewed as **Residual** with named owner TASK-136 and pending
native-vendor release disposition, not promoted to Exact.

## Two-axis implementation review

**Repository standards.** SQL AST inspection is confined to
`src/analysis/sampling.rs`; typed enums/structs live in
`src/protocol/sampling.rs`. Only `src/emission/statements.rs` serializes
this fact. No parser dependency leaks into the canonical model or physical
proof. Sampling on a DISTINCT page cannot reuse the complete unsampled
source population. Focused source modules, stable order, no added
dependencies, no unsafe code and schema/public docs are retained.
Fowler code-smell check: a single small sampled-source fact model is reused
by analysis/emission/tests instead of parallel adapter-specific rules.

**Acceptance specification.** Mandatory deterministic TOP/FETCH, NULL,
full source/rejection and impossible output evidence is verified
independently of parsing. Conditional seeded/unseeded sampling and
NULL/tie defaults never claim exact row identity. SQL, dbt and ODCS reuse
the same canonical proof. Original TASK-76 criteria #1..#6 remain
a separate parent audit, not automatically signed by this child.

## Final-head verification and independent signoff

- Frozen mandatory, conditional, dialect, adapter, typed-domain, rejected/Impossible
  and complete native DuckDB cases were reviewed through their public tests.
- [GitHub CI 40361928399](https://github.com/phdah/sql-semantic-protocol/actions/runs/40361928399)
  on implementation head `82d9842d8d77be813ca7f44e680d07a05dac0702`: **9 of 9 checks passed**.
- Rust formatting, strict default/no-default Clippy, full/no-default Rust test
  suites, documentation, no-default dependency guard and real dbt Core
  seed/run/docs E2E are all green.
- **TASK-136 AC #1..#5: independently signed off.** This is the bounded child
  acceptance, not native-vendor stochastic certification. Six conditional
  laws stay typed Residual with owner TASK-136 and pending vendor disposition.
- **TASK-76 original AC #1..#6: open.** The parent remains pending a separate
  review after this PR is merged. Release TASK-91 is separately held.
- Final metadata-only signoff commit CI is a merge gate. No automatic release
  or parent signoff is implied by marking TASK-136 Done.
