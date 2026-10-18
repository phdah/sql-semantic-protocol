# TASK-115 acceptance audit (PR #124)

**Decision: BLOCKED, not signed off.** The frozen contract is eight cases
(71-C115-P/N across four variants), 13 dialects per case, and all five task
acceptance criteria. Passing CI does not certify absent mandatory proof cells.

| AC | Present evidence | Outstanding mandatory signoff |
| --- | --- | --- |
| 1: versioned matrix | Eight versioned P/N source bags, SQL, source and column-instance identities, typed source metadata, full positive/NOT TRUE partitions, terminal bags and inclusive terminal bounds; three explicitly pending adapter proof cells per case; three owned conditional vendor/session boundaries; coverage manifest entry | Canonical adapter proof statuses remain pending; independently reviewed release dispositions and complete per-dialect constructive verdicts are still required |
| 2: canonical physical semantics | Homogeneous three-way INNER/count plan, typed independent WHERE TRUE restrictions, same physical leaves through a filtered stage, and a single COUNT(*) group | **A finite complete mixed-key bag witness** with physical row identity, rejected/NULL/duplicate rows, two composite ON equality edges, original source constraints, and exact negative/impossible proofs. Current `JoinChainPopulation` encodes one common matched key and only survivor counts; it cannot represent the frozen L[1,1,2]/R[1,1]/T[1] bag or a NULL-key NOT TRUE complement |
| 3: independent DuckDB oracles | All eight complete frozen source/terminal bags executed, each TRUE and NOT TRUE identity triple checked, plus separate PK/FK insert rejection; terminal value domains and inclusivity checked | Bind each oracle's **exact** input/terminal bag to the canonical constructed proof, rather than testing DuckDB independently of the protocol's homogeneous witness |
| 4: dialect/adapters | All eight SQL forms parse and retain expected layers in all 13 project dialects; source-equivalent direct SQL/dbt/ODCS positive filtered-chain parity regression | For each of the 104 cells, compare *canonical constructed finite bags* to typed frozen obligations; extend equivalence to composite, constrained and grouped cases; classify actual parser boundaries and conditional native/session limitations without claiming unexecuted vendor certification |
| 5: final-head signoff | CI includes formatting, Clippy, full/no-default Rust tests, docs and dbt E2E | Re-run final-head checks after **all** mandatory implementations; audit actual API assertions, active protocol/schema snapshots, freeze statuses and parent queue; mark task Done only then |

## 2026-10-18 matrix audit iteration

The frozen matrix is now revision `2026-10-18-v2`. This revision **only adds evidence identity and signoff governance**: all eight SQL examples, exact source values, TRUE/NOT TRUE partitions, terminal bags and domain bounds remain unchanged. New executable tests reject missing column-instance identities, missing adapter proof cells and unowned conditional vendor/session boundaries. The adapter cells intentionally stay `pending_executable_proof`; typed columns by themselves are not a successful adapter proof. No native/vendor runtime certification is claimed. The PR remains blocked on the constructive gaps below.

## Required next code changes

1. Add a canonical, typed *finite complete three-source bag* obligation and
   validator. Its proof must account for source row identities, every nullable
   join-key component, the independent source filter, SQL TRUE and NOT TRUE
   complement, and exact terminal multiplicity. Do not repurpose the current
   all-matched homogeneous obligation to claim heterogeneous inputs.
2. Construct those bags through the existing canonical AST/source evidence,
   honoring attested column types and source PK/FK feasibility. Emit
   Residual/Impossible when an obligation cannot be proven without unknown
   values or forbidden inserts. Reuse source bags across stages and GROUP.
3. Add a public API equality assertion per frozen variant, per evidence adapter,
   and per parser-supported dialect; compare the *same* source identities,
   values, selected/rejected triplets, output domains, and COUNT(*) contribution
   law to the independent DuckDB oracle.
4. Only after those checks, update the 104 dialect verdicts, frozen acceptance
   statuses, task checkboxes and TASK-71 queue, then obtain successful
   final-head CI and review the active JSON/schema compatibility.

This audit intentionally does not approve mandatory residuals, mark TASK-115
Done, or unblock TASK-116. The existing favorable proof is **sufficient for
one homogeneous population**, not **exact for every frozen source bag**.
