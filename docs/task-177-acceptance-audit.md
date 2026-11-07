# TASK-177 acceptance-gap audit and scope boundary

**Task:** Freeze TASK-66 feature-by-13-dialect parser coverage, child 1/4 of
TASK-88. **Implementation PR:** [#196](https://github.com/phdah/sql-semantic-protocol/pull/196).
This audit does not approve TASK-177, TASK-88 or the v3 release gate.

## Frozen inventory and independently executable evidence

- TASK-66 inventories 53 feature families and 251 named variants.
  The TASK-177 v1 acceptance freeze fixes 3,263 distinct variant/dialect
  identities with 156 already fixture-tested, 105 representative-only and
  3,002 unverified **baseline** parser-evidence claims.
- `docs/task-177-parser-samples.json` attaches an explicit SQL shape to
  each of 251 original variant labels. `tests/task_177_parser_matrix.rs`
  runs each shape through the actual project `sqlparser` dialect parser,
  comparing all 3,263 accepted/rejected results to a fixed, independently CI-observed
  `docs/task-177-parser-snapshot.json` rather than merely testing that a
  classification was computed.
- The observed sqlparser 0.58 result is **3,208 parsed** and **55 rejected**
  across 12 SQL shapes, with rejected sample/dialect pairs, named limitations,
  owner and still-pending review disposition frozen in
  `docs/task-177-parser-boundaries.json`. A rejected sample is not proof
  that an equivalent alternate dialect syntax is impossible.
- `88-C177-P` and `88-C177-N`, via
  `tests/task_177_dialect_inventory.rs`, execute independent complete
  DuckDB physical source and terminal bags, duplicate rows, SQL NULL,
  TRUE/NOT TRUE and UNKNOWN, positive bounded inclusive/exclusive
  output intervals, and impossible selection. These two oracles are
  **not** a substitute for every unimplemented semantic family.
- Existing `tests/task_176_adapter_write_parity.rs` and
  `tests/task_173_metadata_authority.rs` already exercise representable
  direct SQL, dbt manifest/catalog and ODCS facts against canonical
  typed source identity, NULL/duplicate, and physical oracles. Their
  certified source shapes can be reused, but neither test proves every
  TASK-177 variant or native vendor execution.

## Original child acceptance criteria

| AC | Implemented evidence | Unresolved blocker |
| --- | --- | --- |
| #1 Frozen complete acceptance | All 251 stable variant identities and 3,263 dialect identities; sample SQL per original variant; explicit baseline evidence and owners | Most variants do not have typed result domains, exact physical source/terminal snapshots and independently reviewed P/N/Impossible/Residual oracle classification. A combined label (e.g. EXCEPT/MINUS) also needs its syntactic subvariants before claiming full coverage |
| #2 Strongest sound semantics | Canonical outcome-domain contract preserved; no production semantic overclaims; two positive/impossible portable goldens | Variant-level exact semantics, multiplicities, cross-source correlation and certified negative witness contracts remain owner-dependent and unverified for most of the exhaustive matrix |
| #3 Native physical oracle | Two executed DuckDB source/terminal/full-rejection golden fixtures and existing feature-owner CI suites | Need pinned source/terminal execution and independent expected values for every mandatory exact core, not merely parser acceptance, count estimates or sample goldens |
| #4 Dialect and adapters | Fixed 3,263-cell 13-dialect sample snapshot and 55 rejection boundary owners; two shared canonical/domain goldens; prior adapter tests provide known representable parity | No exhaustive canonical-equivalence oracle across all parse-capable variants; no representable adapter parity matrix for every fact; vendor/session semantics require named independent evidence or reviewed conditional disposition |
| #5 CI, consumer and independent signoff | PR CI covers Rust fmt, strict Clippy, tests/doc/no-default features, dbt E2E, active schema/goldens. Full pass must be linked to final commit | Independent review and candidate-pinned sql-tdg consumer are not attested for the entire mandatory matrix; preserve blocking statuses until verified |

## Review, version and release boundaries

- **Parser result** is only a syntax boundary for one specific SQL example.
  A parser rejection is not proof that the named feature is unsupported by
  every dialect syntax, and a parser acceptance does not imply a canonical
  semantic equivalence or valid native engine execution.
- **Canonical Exact** must have independent typed bounds, NULL/inclusivity,
  whole-bag source/terminal rows and both qualifying and NOT TRUE
  membership proofs. Unproved exact variants are blockers, not silently
  granted Residual approval.
- **Conditional Residual** needs identified engine/session/metadata
  assumptions, owner TASK-177/TASK-88/TASK-91 as appropriate, an independent
  reviewing decision and a release disposition. No hypothetical vendor
  result is marked Exact.
- **Parent/child gates:** TASK-178 remains blocked until TASK-177 is fully
  accepted, marked Done, and the PR is merged. TASK-88 parent and TASK-91
  release remain independently unapproved.

## Two-axis review of implementation scope

**Standards:** Only test and documentation artifacts change; protocol
serialization, parser dependency versions and canonical production logic
are unchanged. Explicit ordered dialect and case identities prevent
nondeterministic enumeration. Existing adapter and physical proof owners
are reused; new proof is not copied into consumer-specific paths.

**Spec:** The parser inventory deficiency is concretely reduced through
251 SQL samples and executable per-dialect classification. However, the
original task is much broader than a parser test, and its mandatory
typed semantic, full physical oracle and parity evidence are **not**
completed by the added registry. The PR must remain draft and the child
must remain In Progress pending those requirements.

**Review independence:** This is an author-side standards/spec audit,
not an independent second reviewer or maintainer release disposition.
