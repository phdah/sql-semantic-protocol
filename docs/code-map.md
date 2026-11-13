TASK-192 bounded computed arithmetic and relational EXISTS:
`src/physical_realization/signed_computed_bags.rs` implements complete
typed 16/32/64-bit overflow-safe one-source addition/ABS TRUE/NOT TRUE/UNKNOWN
bag proof, exercised in `tests/task_192_signed_computed_bags.rs` for
original S027/S032 × 13 parsers. `src/physical_realization/exists_bags.rs`
continues to own original S013 complete correlated EXISTS, including empty
vs missing inner-world evidence in `tests/task_192_correlated_exists.rs`.
The exact bounded/necessary-open-world-residual split is recorded in
`docs/task-192-bounded-semantic-closures.json`. This does not assert
vendor overflow or independent review approval.

TASK-192 complete candidate canonical wire fixture: `docs/task-192-proposed-full-documents.json` owns 43 analyzer-derived, **unreviewed** complete public documents; `tests/task_192_full_document_goldens.rs` tests the entire document over all 557 accepted original parser cells plus both STRUCT parser exceptions. Separate independently frozen original SQL/type/native physical oracles check semantic facts in `tests/task_192_semantic_contract.rs`, `tests/task_192_independent_query_shapes.rs` and `tests/task_192_native_partitions.rs`. The 559-cell `docs/task-192-review-queue.json` requires distinct reviewer approval before TASK-192 Done; do not conflate source-generated regression snapshots with independent semantics.

TASK-192 original S011 typed ordered NOT closure: `src/analysis/exactness.rs::typed_signed_ordered_not` has the single authoritative signed-integer type-law guard; `src/analysis/domains.rs` turns the original NOT comparison into an inclusive complement domain only with that guard. `src/domain.rs` separately inverts only ordinary numeric equality NOT (with SQL NULL still UNKNOWN). `tests/task_192_not_equality.rs` checks S011 signed/untyped/FLOAT, independent full 3VL DuckDB bags, 13 parser dialects and conservative NOT negatives. Original S010 disjunction is whole-query Exact **only** when the single source has authoritative signed-integer types for both compared columns; untyped or FLOAT remains Residual. Neither variant invents a projected `id` bound. Independent reviewer approval still required.

TASK-192 S013 physical occurrence proof: `src/physical_realization/exists_bags.rs` accepts a schema-backed `__physical_row_id` text identity distinct from numeric/null/duplicate SQL `id`; `tests/task_192_correlated_exists.rs` asserts all original S013 13-parser physical EXISTS/NOT TRUE source bags, partner multiplicity, exact terminal `id` bag and failure for duplicate physical identity. `docs/task-192-original-query-shapes.json` / `tests/task_192_independent_query_shapes.rs` additionally pin 43 manually derived original outer-source/output/predicate shapes rather than taking the first analyzer output as the only expected shape. Neither constitutes independent release review.

TASK-192 bounded residual verification: `tests/task_192_nullable_truth.rs` proves original S010/S011 typed source-local truth, with signed-only S011 Exact and S010 still whole-query Residual; `tests/task_192_computed_witness.rs` checks S027-S032 full native partitions and overflow-negative typed witnesses; `tests/task_192_string_witness.rs` checks safe LIKE and trailing-space equality under explicit binary/no-padding attestations and negative vendor-law cases. `docs/task-192-independent-review-handoff.md` is the human review route, `docs/coverage-manifest.json` owns statuses; external native vendor signoff is not inferred.

TASK-192 nullable 3VL closure: `src/analysis/predicates.rs` retains explicit NULL guards for numeric `IS TRUE`/`IS FALSE`; `src/analysis/exactness.rs` distinguishes exact same-column numeric `OR IS NULL` and guarded NOT Boolean witnesses from unrepresentable scalar column domains. `tests/task_192_nullable_truth.rs` uses all 13 public parsers, typed source witnesses and independent complete DuckDB original TRUE/FALSE/UNKNOWN/NOT TRUE bags; `docs/task-192-semantic-cell-ledger.json` owns expected case-level classification without native-vendor proof.

TASK-192 second pass (2026-11-11): `docs/task-192-semantic-cell-ledger.json` freezes all 559 named exact/conditional/residual public cells; `tests/task_192_semantic_contract.rs` verifies each expected domain kind, parser boundary, original evidence pointer and full-document normalization. `src/analysis/predicates.rs` implements the safe numeric comparison IS UNKNOWN -> IS NULL equivalence; `tests/task_192_projection_counterexamples.rs` checks original SQL for negative/NULL/duplicate ID counterexamples. `tests/task_192_adapter_parity.rs` now analyzes each original SQL as compiled dbt, not a substitute model. S012 and S017 remain unapproved mandatory residuals; independent reviewers and external vendor owners retain signoff.

TASK-192 emitted-domain soundness: `tests/task_192_typed_soundness.rs` checks all 41 original scalar DuckDB terminal bags against 13 emitted public domains and prevents 31 unrelated source predicates from inventing finite `id` bounds. The task-owned acceptance freeze independently categorizes 10 precise signed-domain cases, 31 unconstrained nullable projections and two exceptional runtime/STRUCT boundaries. Consumer compatibility uses temporary `phdah/sql-tdg` PR #49; no release signoff is implied.

TASK-192 expanded native/adapter proofs: `docs/task-192-native-partitions.json` pins eleven additional native 3VL source-rowid partitions and three non-predicate exact error/order/STRUCT outcomes. `tests/task_192_native_partitions.rs` verifies original SQL and whole DuckDB source/terminal bags; `tests/task_192_adapter_parity.rs` tests direct SQL, dbt and ODCS typed evidence across 41 original physical-source queries and all 13 public parsers. `docs/task-192-acceptance-audit.md` owns remaining global typed-domain, vendor and reviewer signoff gates; TASK-192 is still In Progress.

TASK-192 predicate/structural-type matrix (2026-11-10): `docs/task-192-acceptance-freeze.json` preserves 43 original SQL variants and 559 observed 13-dialect parser cells without claiming semantic signoff. `tests/task_192_predicate_matrix.rs` checks these against the original inventory and coverage gate, guards public canonical documents and independently verifies four new nullable/duplicate DuckDB LIKE/ILIKE/ESCAPE/prefix TRUE/FALSE/UNKNOWN/NOT TRUE source and terminal bags. Existing 25 TASK-177 independent native cases remain referenced without duplicated goldens. This is bounded in-progress evidence only; the remaining native, strongest typed, vendor/session, adapter and independent review gates remain open.

TASK-178 additional candidate outcome checks (2026-11-10): `tests/task_178_adapter_family_parity.rs` owns cross-adapter typed-domain parity for native predicate, join, set, window and pagination variants and validates original DuckDB terminal literals against emitted 13-dialect typed membership claims. `tests/task_178_composed_native_oracles.rs` owns JOIN+UNION, derived window filter and joined INSERT source/terminal/negative state goldens. These tests exercise public adapter and analyzer seams and do not certify unsupported vendor runtime or independent whole-matrix acceptance.

TASK-178 canonical dialect variants (2026-11-10): `src/analysis/aggregation.rs` normalizes function-form ROLLUP/CUBE into typed grouping operations, with `src/analysis/queries.rs` guarding regular-group cardinality from synthetic grand-total rows. `src/analysis/expressions.rs` and `functions.rs` retain a stable explicitly unsupported STRUCT CAST across sqlparser custom/dedicated type ASTs. `src/analysis/exactness.rs` owns the canonical direct certified TOP/FETCH/LIMIT residual, preserving a distinct typed WITH TIES flag and unsupported vendor boundaries. Tests: `tests/task_178_grouping_ast_equivalence.rs`, `task_178_remaining_drift.rs`, `task_178_pagination_canonical.rs`, `task_178_native_boundary_adjudications.rs`. First-party 163-variant/13-dialect observed matrix and 14 native exceptions are mapped from `docs/task-178-acceptance-audit.md`; neither claim all-domain or external vendor signoff.

TASK-177 bounded scope completion (2026-11-09): `docs/task-177-scope-split.md` maps original broad #1..#5 acceptance to TASK-178/179/new TASK-191/180, without weakening TASK-88. `tests/task_177_scope_handoff.rs` verifies the frozen observed counts, separate scoped Done vs historical full-matrix unapproved flag, and parent child ordering. The protocol emitted schema/semantic implementation is unchanged.

TASK-177 native candidate proof: `tests/task_177_native_candidate_oracles.rs` validates exact WKT and GEOMETRY type for original S181, explicitly rejects original S176 BINARY(2) length parameters while keeping BLOB control distinct, and verifies that original S242 EXPLAIN references its named source without certifying optimizer-dependent plan exactness. The original native oracle ledger is 228/251 with 23 conditional or missing exact terminals.

TASK-177 additional typed predicate-domain evidence: `docs/task-177-predicate-oracles.json` pins four new exact scalar outcomes (NOT EQUAL NULL-exclusion, disjoint NOT BETWEEN intervals, two AND-constrained ID domains); `tests/task_177_predicate_oracles.rs` checks exact typed bounds, inclusivity and 13 parser dialects; `tests/task_177_predicate_adapter_parity.rs` checks source parity across direct DDL, compiled dbt and ODCS for all ten typed cases. This does not sign off the other mandatory cells or native vendors.

TASK-177 final deterministic native proof: `tests/task_177_final_native_deterministic.rs` and `docs/task-177-final-native-deterministic.json` own 13 additional distinct original SQL source/terminal or catalog/session-state oracles. `tests/task_177_native_boundary_probes.rs` runs original negative and nondeterministic controls without promoting them to positive terminal proofs. The 251-case `tests/task_177_physical_proof_ledger.rs` and `docs/task-177-remaining-original-cases.json` now guard 228 native-backed / 23 conditional or unresolved cases, with independent typed semantics and review still blocked.

TASK-177 expanded native exact SQL proof owners: `tests/task_177_native_expansion.rs` with `docs/task-177-native-expansion.json` owns additional query/ordering/PIVOT/multi-terminal whole-bag oracles; `tests/task_177_write_oracles.rs` and `docs/task-177-write-oracles.json` own duplicate/NULL complete DML before/after-state and RETURNING bags; `tests/task_177_predicate_environment.rs` owns fixed string, temporal UTC session and signed-zero evidence via `docs/task-177-predicate-environment-oracles.json`; `tests/task_177_schema_lifecycle_oracles.rs` owns constraints, DDL and transaction/rollback original SQL source/poststate oracles via `docs/task-177-schema-lifecycle-oracles.json`; `tests/task_177_window_oracles.rs` owns exact ranking/frame native rows via `docs/task-177-window-oracles.json`. These add 89 unique original SQL samples without claiming all typed canonical domains, adapter parity or other vendor laws. `tests/task_177_physical_proof_ledger.rs` guards the complete 228/23 count and rejects duplicate provenance.

TASK-177 original SQL source-complete relational evidence: `tests/task_177_relational_bag_oracles.rs` and `docs/task-177-relational-bag-oracles.json` own 98 native DuckDB complete input/terminal bag cases across nine relation/aggregation/window/subquery families, as well as explicit conditional DuckDB syntax rejection checks. `tests/task_177_physical_proof_ledger.rs` and `docs/task-177-physical-proof-ledger.json` reconcile those cases with 25 predicate oracles and five drift oracles (228 unique original sample IDs, 23 missing after five new oracle suites), plus ten typed source-adapter parity cases. These are not 13-vendor, full-domain or independently approved release proofs.

TASK-177 exhaustive public canonical dialect comparison: `tests/task_177_canonical_equivalence.rs` compares the entire emitted public protocol for all 251 first-party SQL samples and 13 dialect parsers, ignoring only the explicit input dialect label. `docs/task-177-equivalence-snapshot.json` pins 3,069 identical documents, 23 different documents, 116 explicitly Unsupported and 55 parser-rejected cells. `docs/task-177-canonical-drift-boundaries.json` pins the five affected SQL shapes, their exact first differing JSON paths, semantic owners and unapproved dispositions. `tests/task_177_cross_dialect_native_oracles.rs` supplies first-party DuckDB NULL/duplicate/group-subtotal/LATERAL/STRUCT physical output oracles for these five originals; native vendor laws and candidate sql-tdg certification remain separate gates.

TASK-177 dialect parser inventory: `docs/task-177-acceptance-freeze.json` owns the conservative TASK-66 251-variant/13-dialect evidence baseline; `docs/task-177-parser-samples.json` fixes one explicit SQL shape per named variant, and `docs/task-177-parser-snapshot.json` freezes 3,208 parsed and 55 rejected SQL-sample/dialect masks captured in CI, while `docs/task-177-parser-boundaries.json` names 12 rejected sample families and their unapproved release dispositions.
`docs/task-177-analysis-boundaries.json` enumerates all 116 public-analyzer
Unsupported cells across 10 case families with original semantic owner and a
blocking release disposition. Neither classification certifies exact outcomes. `tests/task_177_dialect_inventory.rs` owns two complete DuckDB TRUE/NOT TRUE/UNKNOWN fixture oracles with typed interval assertions, while `tests/task_177_parser_matrix.rs` owns exhaustive parser-boundary regression. These are parser and two local-oracle proofs, not a v3-wide semantic, adapter, or vendor-execution signoff. TASK-88/TASK-91 remain separate gates.

TASK-176 paired adapter parity: `tests/task_176_adapter_write_parity.rs` owns direct SQL, dbt manifest/catalog and ODCS equivalence for table, view, ephemeral and incremental declaration versus physical source/read and explicit MERGE/DELETE effects. It verifies 65 task-owned parser-dialect cells, full native DuckDB TRUE/FALSE/UNKNOWN and duplicate bags, canonical typed domains and Exact proof strength from one complete physical catalog. `docs/task-176-acceptance-freeze.json` and `docs/coverage-manifest.json` record mandatory, conditional and outstanding signoff gates. DBT materialization remains unattested without independent engine evidence; TASK-87 parent and TASK-91 release gate remain open.

TASK-175 dbt relation and foreign-key evidence: `src/relation.rs::RelationCatalog::resolve` owns dialect-aware quoted and schema-qualified relation identity, while `src/dbt.rs::parse_manifest_relation_constraints` and `parse_dbt_constraint_array` require independently resolvable FK and relationships `to` targets before admitting canonical constraints. Missing or ambiguous targets emit scoped diagnostics instead of fabricating keys. `src/dbt.rs::required_physical_constraint_columns` and `relation_schemas_from_artifacts` retain typed FK-only sources and catalog authority. `tests/task_175_adapter_boundaries.rs` checks quoted references, missing metadata, thirteen portable parser dialects, and direct/dbt/ODCS plus native full-bag oracles. `docs/task-175-acceptance-freeze.json` and `docs/coverage-manifest.json` track the bounded acceptance; TASK-87 parent remains open.

TASK-174 dbt materialization evidence: `src/dbt.rs::parse_dbt_manifest` retains manifest `config.materialized` and unique input identity; `src/materialization.rs::ProducerEvidence` is the canonical immutable declared-storage and explicitly unattested execution contract. `src/dbt.rs::analyze_dbt_with_catalog` analyzes named table models as CTAS, views as logical definitions, ephemeral models as anonymous SELECT producers, and incremental models as query-only surrogates without an executed MERGE/INSERT target claim. The optional root `producer_evidence` sidecar is serialized by `src/emission/document.rs` with `schema/protocol.schema.json`. `tests/task_174_materializations.rs` verifies the finite freeze, NULL/duplicate and NOT TRUE full DuckDB bags, direct SQL/dbt/ODCS domain parity, and 13 portable parser dialect paths. `docs/task-174-acceptance-freeze.json` and `docs/coverage-manifest.json` own unverified CI and review gates; TASK-87 parent stays open.

TASK-173 authoritative metadata fact ownership: `src/dbt.rs::analyze_dbt_with_schema_evidence` and `relation_schemas_from_artifacts` reconcile dbt manifest compiled SQL, dependency and declared constraints with warehouse-introspected `catalog.json` datatypes, retaining declared-fallback `SchemaSourceKind` when catalog evidence is absent; `src/odcs.rs::property_data_type` retains valid authoritative physicalType even when lower-authority logicalType is unsupported, with explicit non-fatal diagnostics. `tests/task_173_metadata_authority.rs` covers provenance, absent type evidence, direct DDL and 13 parser dialects plus a native complete DuckDB TRUE/FALSE/UNKNOWN fixture. `docs/task-173-acceptance-freeze.json` and `docs/coverage-manifest.json` own the finite freeze and outstanding CI/signoff. TASK-87 parent remains open.

TASK-164 certified mixed-script DuckDB state and ALTER default provenance: `src/physical_realization/transactions.rs::physical_duckdb_transaction_plan` carries only exact, in-script CREATE-attested scalar column default transitions from `AnalysisBundle::advance_physical_alter_defaults` across independently parsed SQL units, preserving typed catalog and transaction rollback. `tests/task_164_mixed_scripts.rs` covers full native DDL/INSERT/MERGE/DELETE/SELECT, rollback, duplicates, NULL, idempotence, invalid lifecycle, and ALTER default oracles; `tests/task_164_dialect_matrix.rs` classifies all public parser dialects without promoting parser acceptance to vendor execution, while `tests/task_164_adapter_parity.rs` exercises representable SQL/dbt/ODCS canonical metadata. Freeze and signoff remain in `docs/task-164-acceptance-freeze.json` and `docs/coverage-manifest.json`; TASK-84 parent remains open.

TASK-163 native DDL rollback and conditional engine/session laws: `src/physical_realization/transactions.rs::physical_duckdb_transaction_plan_with_laws` and `src/analysis/transactions.rs::physical_duckdb_sql_transaction_plan_with_laws` require the same complete canonical DuckDB source catalog before promoting a rolled-back CREATE/DROP schema mutation to Exact. They retain earlier Impossible/Residual and fail closed for temp relation lifetime, deferred FK checks, concurrent isolation, implicit vendor DDL commit and other vendor transaction options, without inferring engine behavior from sqlparser. `tests/task_163_session_laws.rs` owns native full-bag rollback, NULL/duplicate/rejected-row and unproved-law boundaries; `docs/task-163-acceptance-freeze.json` and `docs/coverage-manifest.json` track unverified mandatory/conditional cells. TASK-84 parent stays open.

TASK-162 single-session DuckDB transactions: `src/analysis/transactions.rs::physical_duckdb_sql_transaction_plan` lowers each real DuckDB SQL control through sqlparser without leaking AST types; `src/physical_realization/transactions.rs::physical_duckdb_transaction_plan` replays committed or rolled-back complete typed catalogs over TASK-161 canonical state graph, including independently attested PRIMARY KEY/constraint enforcement via `SqlWithEvidence`, otherwise explicit Residual for unknown enforcement, and negative proofs. TASK-164 now carries proven, in-script CREATE-attested ALTER scalar defaults across independently analyzed SQL units; unproved casts and vendor DDL rules remain Residual. Invalid/nested controls are Impossible, unsupported vendor modifiers or SAVEPOINT/ROLLBACK TO/RELEASE remain Residual (pinned DuckDB engine oracle). `tests/task_162_transactions.rs`, `tests/task_162_sql_frontend.rs`, `tests/task_162_adapter_parity.rs`, `tests/task_162_dialect_matrix.rs`, and `tests/task_162_acceptance_inventory.rs` own native full-bag, SQL frontend, SQL/dbt/ODCS and thirteen-dialect oracles. The versioned `docs/task-162-acceptance-freeze.json` and `docs/coverage-manifest.json` own scope and signoff gates; TASK-84 parent stays open.

TASK-161 sequential typed DML/DDL state graph: `src/physical_realization/sequential_graph.rs::physical_sequential_state_graph_plan` wraps the canonical TASK-160 lifecycle interpreter, derives complete typed before/after live catalogs and versioned physical source/target dependencies with stable statement identities. It reuses `PhysicalWriteTransition` mutation counts and source-occurrence ordinals and `PhysicalLifecycleRead` output/rejection proofs; missing dependencies, unattested sources and transaction/session laws remain Residual, while proved contradictions are Impossible. `tests/task_161_sequential_graph.rs` owns public state versioning and negative paths, `tests/task_161_initial_provenance.rs` owns initial-source edge identity, `tests/task_161_full_oracles.rs` checks independent full DuckDB source/intermediate/terminal catalogs and typed outcome bounds, `tests/task_161_dialect_matrix.rs` checks all 13 parser paths against the frozen matrix, and `tests/task_161_adapter_parity.rs` verifies source-independent SQL/dbt/ODCS outcomes while preserving adapter evidence provenance. `docs/task-161-acceptance-freeze.json` and `docs/coverage-manifest.json` own task signoff; TASK-84 parent stays open until TASK-161..164 are independently closed.

TASK-168 correlated tuple and fanout certification: `src/physical_realization/joint_tuple_goals.rs::physical_joint_tuple_histogram_plan` compares exact typed INTEGER/NULL multi-column tuple frequency requests against the **same** complete two-source JOIN and WINDOW/SET terminal assignment owned by TASK-167; per-column marginal agreement alone cannot prove row correlation. `physical_single_tuple_histogram_plan` reuses canonical finite pagination, navigation/QUALIFY, or ranking interpreters for source-attested page/window bags, preserving complete TRUE/NOT TRUE occurrence identities and typed outcome domains. Invalid histogram shapes fail construction; unknown source/column/comparison semantics remain Residual, while contradictory complete terminal tuple counts are Impossible. `tests/task_167_joint_terminals.rs` owns cross-dialect window/set tuple checks, and `tests/task_168_tuple_fanout.rs` owns single-terminal LIMIT/QUALIFY and DuckDB source/terminal oracle tests. `src/emission/tuple_goals.rs::to_physical_single_tuple_json` reuses canonical source/terminal serializers to emit deterministic Exact/Impossible/Residual tuple evidence, never modifying the root. `tests/task_168_frozen_acceptance.rs` runs all frozen source/terminal DuckDB oracles and 13-dialect classifications; `tests/task_167_adapter_parity.rs` verifies equivalent SQL/dbt/ODCS semantics. `docs/task-168-acceptance-freeze.json` and `docs/coverage-manifest.json` track the versioned scope. **TASK-168 remains To Do pending full mandatory matrix/dialect/adapter/pinned-consumer/independent CI signoff; TASK-85 parent stays open.**

TASK-167 shared grouped and window/set terminal certification: `src/physical_realization/joint_terminals.rs::physical_joint_terminal_bag_plan` certifies two canonical terminal goals against the same closed-world, two-source typed `PhysicalWriteState` assignment. It reuses canonical JOIN/WHERE/GROUP/HAVING, window and set-tree interpreters and compares complete INTEGER/NULL outcome histograms without independently multiplying witnesses. `src/emission/joint_terminals.rs::to_physical_joint_terminal_json` is the opt-in, deterministic typed sidecar with exact physical rows, pair membership, output layer references and complete terminal domains. `tests/task_167_acceptance_inventory.rs` exercises v2's six frozen cases, 78 parser dialect cells, native DuckDB bags, typed domains and protocol sidecar; `tests/task_167_adapter_parity.rs` owns SQL/dbt/ODCS equivalence for window and set terminals. `docs/task-167-acceptance-freeze.json`, its archived immutable v1, `docs/task-167-acceptance-verification.json` and `docs/task-167-acceptance-audit.md` retain per-cell evidence and signoff gates. TASK-85 parent stays open.

TASK-166 shared-source terminal goals: `src/physical_realization/joint_distributions.rs::shared_filtered_histogram_plan` reconciles one complete typed `SourceRows` frequency bag with a direct SQL TRUE/NOT TRUE filter on that same physical column and both terminal output counts, including count and histogram contradictions. `known_histogram_filter_truth` is deliberately restricted to canonical INTEGER/NULL comparisons and NULL tests, leaving all other predicates Residual. `tests/task_166_shared_goals.rs` owns the public API, duplicate/NULL/Impossible and full DuckDB bag oracles; `docs/task-166-acceptance-freeze.json` owns the bounded acceptance matrix. TASK-85 parent remains independently open.

TASK-160 ordered DDL/DML/SELECT lifecycle: `src/physical_realization/lifecycle_scripts.rs::physical_ordered_lifecycle_plan` replays canonical `physical_ordered_write_state_plan` transitions without reparsing SQL, attests each successive live physical catalog and typed schema, and evaluates simple intermediate SELECTs through the existing `src/physical_realization/write_states/projection.rs` canonical interpreter. The complete read bag preserves duplicate/NULL occurrences, selected and NOT TRUE source ordinals and exact typed domains. `tests/task_160_ordered_lifecycle.rs`, `tests/task_160_dialect_matrix.rs` and `tests/task_160_adapter_parity.rs` own the frozen full-bag/native, thirteen-dialect and direct SQL/dbt/ODCS oracles. `docs/task-160-acceptance-freeze.json` and its coverage manifest entry track remaining AC and CI gates. TASK-83 parent remains independently open.

TASK-159 table lifecycle semantics: `src/analysis/ddl_lifecycle.rs` converts single-table DROP/TRUNCATE/RENAME AST into the canonical `src/protocol/writes.rs::WriteLifecycleAction` and `WriteKind::Lifecycle`, rejecting unsupported vendor/multi-target clauses. `src/physical_realization/write_states/lifecycle.rs` owns closed-world complete-bag and relation-existence proofs; ordered execution in `src/physical_realization/write_states.rs` distinguishes empty live tables, dropped identities and renamed physical schemas. `src/emission/statements.rs` and `schema/protocol.schema.json` define the public lifecycle action wire contract. `docs/task-159-acceptance-freeze.json` freezes positive/negative and vendor conditional oracles; `tests/task_159_lifecycle.rs` owns independently asserted native DuckDB and protocol evidence. TASK-83 parent remains open; CI and independent review are required before child signoff.

TASK-158 ALTER column semantics: `src/analysis/alter_columns.rs` normalizes portable SQL ALTER ADD/DROP/RENAME/TYPE AST into `src/protocol/writes.rs::ColumnAlterStep` and `WriteKind::SchemaAlter`, rejecting unsupported options atomically. `src/physical_realization/write_states/alter_columns.rs` proves complete-bag transitions, with `src/physical_realization/write_states.rs` advancing ordered physical schemas, preserving original/updated typed schemas on transitions, and refusing unattested constraints or casts. `src/emission/statements.rs` and `schema/protocol.schema.json` own the wire representation. `tests/task_158_alter_column_snapshot.rs` owns the isolated DuckDB oracle; `tests/task_158_ordered_alter.rs` owns CREATE/INSERT/ALTER replay and negative examples, `tests/task_158_dialect_matrix.rs` the explicit 13-dialect boundary. `docs/task-158-acceptance-freeze.json` is the v4 typed matrix with immutable v1-v3 archived. Frozen DuckDB prestate/target, ordered ALTER default replay, 104 parser cells and 27 direct/dbt/ODCS evidence cells are covered; bounded TASK-158 acceptance has an independent second-pass review and verified implementation-head CI; metadata-head CI and PR merge remain integration gates, and TASK-83 stays open.

TASK-157 CREATE and DDL identity certification: `src/analysis.rs::analyze_statement` owns plain schema-only CREATE and differentiates logical VIEW from stored MATERIALIZED VIEW lifecycle identities in the canonical `src/protocol/writes.rs::WriteDefinitionMode`. `src/physical_realization/write_states.rs` owns ordered state and constraints verification including validation-only future CREATE schema state and canonical source append for view reads; `src/physical_realization/write_states/operators.rs::define_rows` constructs the typed empty CREATE bag, CTAS/REPLACE and frozen MV snapshots, returning original source occurrence IDs to `PhysicalWriteTransition::definition_source_row_ids`; it refuses unattested temporary/external DDL lifecycle laws marked by `WriteOperation::definition_execution_unknown`. `src/physical_realization/view_bags.rs` re-evaluates one live logical view from its complete original or appended source bag without inventing view rows. The active v2 nine-case matrix and preserved v1 are `docs/task-157-acceptance-freeze.json` and `docs/task-157-acceptance-freeze-v1.json`; `docs/task-157-acceptance-audit.md` reconciles original child/parent AC and review/CI gates; `docs/coverage-manifest.json` is the signoff tracker. `tests/task_157_empty_create.rs` owns empty CREATE and duplicate-target proof, `tests/task_157_state_oracles.rs` owns complete DuckDB source/target/domain/TRUE-NOT-TRUE and MV snapshot evidence, `tests/task_157_dialect_matrix.rs` owns 117 frozen parser cells, `tests/task_157_adapter_parity.rs` owns 27 direct/dbt/ODCS evidence classifications, and `tests/task_157_acceptance_inventory.rs` guards typed scope. Active wire contract changes are in `src/emission/statements.rs`, `schema/protocol.schema.json`, and `docs/protocol.md`. TASK-83 parent stays open; native vendor MV refresh/session laws retain explicit conditional evidence boundaries.

TASK-156 computed and sequential MERGE certification: `tests/task_156_sequential_merges.rs` owns complete closed-world two-MERGE DuckDB snapshots, evolving physical source/target occurrence proofs, signed INTEGER/NULL exact domains, enforced PRIMARY KEY impossibility, duplicate-match residual and unsupported transaction laws. `tests/task_156_dialect_matrix.rs` owns 65 explicit parser/canonical dialect cells; `tests/task_156_adapter_parity.rs` owns six case direct SQL/dbt/ODCS equivalent metadata proofs; `tests/task_156_acceptance_inventory.rs` guards the immutable typed contract and TRUE/NOT TRUE partitions. `docs/task-156-acceptance-freeze.json` is active v4 with immutable v1-v4 history; `docs/task-156-acceptance-audit.md` reconciles original child/parent AC and independent review/CI gates. Canonical interpretation remains in `src/physical_realization/write_states.rs` and `src/physical_realization/write_states/operators.rs::merge_rows`. TASK-82 parent stays open.

TASK-155 repeated-match and UPSERT classification: `src/physical_realization/write_states/operators.rs::merge_rows` preserves exact original target/source occurrences and preserves the established `UnprovedOperation` residual for duplicate matching source identities (no invented winner or engine error). `src/analysis/writes.rs::analyze_insert` keeps ON CONFLICT DO UPDATE unsupported pending native equivalence evidence; `src/physical_realization/write_states/constraints.rs` owns enforced PK/UNIQUE and FK checks. `tests/task_155_merge_conflicts.rs` owns complete DuckDB bag classification and ordered ON CONFLICT/MERGE replay, `tests/task_155_fk_unique.rs` owns FK/nullable UNIQUE exact/error boundaries, `tests/task_155_dialect_adapter.rs` owns 52 parser and 39 SQL/dbt/ODCS evidence cells, and `tests/task_155_acceptance_inventory.rs` guards finite typed TRUE/NOT TRUE partition and domain coverage. Versioned v4 acceptance inventory, immutable history, and signoff evidence are in `docs/task-155-acceptance-freeze.json` and `docs/task-155-acceptance-audit.md`. TASK-82 remains independently open.

TASK-154 unmatched MERGE branches: the canonical `src/physical_realization/write_states/operators.rs::merge_rows` evaluates target-only BY SOURCE UPDATE/DELETE and source-only BY TARGET INSERT against one complete physical preimage without introducing independent source rows. `tests/task_154_merge_unmatched.rs` owns the 12-case native DuckDB full-bag and dual ON TRUE/NOT TRUE evidence; `tests/task_154_dialect_adapter.rs` owns 65 parser cells and direct/dbt/ODCS parity; `tests/task_154_acceptance_inventory.rs` guards the active v4 matrix and manifest. Historical v1/v2/v3 are retained. Vendor parser limitations and SQL/dbt/ODCS ordered replay parity live in the dialect/adapter test. The ordered three-step replay in the same DuckDB test owns intermediate target state and zero invented source assertions. TASK-82 parent remains independent.

TASK-144 logical view and independently closed CTE acceptance:
`src/analysis.rs` retains all recognized CREATE VIEW producer layers while
`src/protocol/writes.rs` exposes `view_execution_unknown` for unverified
materialization, temporary/session and other DDL clauses. The optional field
is emitted in `src/emission/statements.rs` and constrained in the active JSON
schema. `src/physical_realization/view_bags.rs` re-evaluates only a canonical
one-leaf logical view from the complete original physical bag without
treating the view as a new controllable source. The shared occurrence-aware
projection lives in `src/physical_realization/write_states/projection.rs`.
`tests/task_144_frozen_reads.rs` owns full raw/recursive/bounded typed
truth; `tests/task_144_shadow_oracles.rs` owns two-leaf shadowing;
`tests/task_144_dialects.rs` pins parser identities;
`tests/task_144_adapter_parity.rs` verifies compiled dbt/ODCS semantics;
`tests/task_144_view.rs` checks native view/full-bag truth and residuals.
The v1 original freeze is preserved in `docs/task-144-acceptance-freeze-v1.json`;
the reviewed v2 matrix owns nine explicit cases and 126 dialect cells.

TASK-144 ordered materialized CTE scope proof:
`src/physical_realization/cte_scripts.rs` composes the existing complete
ordered write-state interpreter and bounded CTE bag evaluator without an
independently supplied post-write target. It fails closed for nonterminal
reads and unsupported SQL/DDL. `tests/task_144_ordered_cte.rs` owns the
independent two-step DuckDB source/producer/target/terminal and NULL/duplicate
oracles. `docs/task-144-acceptance-freeze.json` owns the preimplementation
fixture/dialect/adapter matrix, with other parent signoff oracles still pending.

TASK-143 finite filtered/computed CTEs: `src/analysis/cte.rs` owns SQL filter and arithmetic lowering into `src/protocol/cte.rs` typed lexical stages. `src/physical_realization/cte_bags.rs` evaluates one complete physical source bag, applies SQL three-valued filters before checked integer projections, and reuses producer bags for downstream joins. `tests/task_143_frozen_matrix.rs`, `tests/task_143_filtered_producers.rs`, `tests/task_143_count.rs` and `tests/task_143_duckdb_oracles.rs` own positive/negative full-bag oracles; `tests/task_143_write.rs` proves complete ordered INSERT CTE source/target bags and atomic CHECK impossibility through existing `src/analysis/writes.rs` and `src/physical_realization/write_states/operators.rs` canonical write seams; `tests/task_143_dialects.rs`, `tests/task_143_adapters.rs`, and `tests/task_143_residual.rs` own dialect, adapter, and unsafe conditional boundary evidence. `docs/task-143-acceptance-freeze.json` is the active v4 fixture/dialect/adapter inventory (v1, v2 and v3 archived), including a widened-target overflow counterexample in `tests/task_143_write.rs`; `src/physical_realization/write_states/operators.rs` guards computed local CTE values before target coercion. `tests/task_143_acceptance_inventory.rs` enforces complete case/dialect/adapter accounting. Independent review and historical freeze-timing disposition remain outstanding; parent TASK-78 remains open.

TASK-141 lexical and physical CTE source identity:
`src/analysis/cte.rs` lowers a bounded ordinary WITH/JOIN syntax subset into
parser-independent `CteProgram` stages in `src/protocol/cte.rs`.
`src/physical_realization/cte_bags.rs` evaluates one complete attested physical
bag per source and CTE producer, retaining distinct SQL aliases, typed producer
graph identities, FALSE/UNKNOWN rejections and exact terminal domains.
`tests/task_141_physical.rs` checks the canonical full physical bags;
`tests/task_141_frozen_oracles.rs` checks independent DuckDB values and
occurrence IDs; `tests/task_141_adapter_matrix.rs` covers dbt/ODCS parity.
The frozen cases and review gates are in `docs/task-141-proof-matrix.json`
and `docs/task-141-acceptance-audit.md`. TASK-78 remains a separate parent.

TASK-152 ordered INSERT/UPDATE/DELETE state certification:
`tests/task_152_ordered_mutations.rs` proves complete pre/post state bags,
per-statement affected ordinals and counts, typed NULL/frequency bounds,
three-valued TRUE/NOT TRUE partitions, enforced PK/CHECK contradictions,
the FK deferral residual boundary and native DuckDB outcomes.
`tests/task_152_dialect_adapter.rs` owns the 39 canonical parser-dialect
cells and cross-source SQL/dbt/ODCS typed-schema parity (including enforced
FK references to a complete shared physical source). The immutable
`docs/task-152-acceptance-freeze.json` is linked from
`docs/coverage-manifest.json`, with historical v1-v3 retained for audit; the independent
review is in `docs/task-152-acceptance-audit.md`. Existing canonical
`src/physical_realization/write_states.rs` and its `constraints.rs`
submodule own the interpreter; no SQL reparsing or public wire change.
TASK-81 remains a separate parent signoff.

TASK-151 bounded correlated UPDATE/DELETE:
`src/analysis/writes.rs` normalizes one complete independent EXISTS/NOT EXISTS
read source with the active target alias in lexical scope; 
`src/physical_realization/write_states/operators.rs` evaluates the canonical
subquery predicate per target occurrence using the same closed read-only bag.
The existing `WriteOperation::returning_columns` carries target-qualified
column identity. Oracle and frozen per-dialect/evidence-source certification:
`tests/task_151_qualified_returning.rs`,
`tests/task_151_dialect_adapter.rs` and 
`docs/task-151-acceptance-freeze.json`. Vendor ORDER/LIMIT and
partition-qualified mutation semantics are explicit residual boundaries,
not silently evaluated as unqualified writes. TASK-81 remains separate.

TASK-150 bounded joined UPDATE FROM / DELETE USING: `src/analysis/writes.rs`
selects a single named independent source without JOINs or target self-source;
`src/protocol/writes.rs` owns the canonical `joined_source` reference and
conservative write state effect. `src/physical_realization/write_states/operators.rs`
evaluates every matching source against the same target preimage and requires a
single match for UPDATE, but existential matching for DELETE. `src/analysis.rs`
recognizes CTE-wrapped UPDATE/DELETE as DML instead of a read-only query; the
read-only row-local producer in `src/analysis/writes.rs` is canonically
materialized through shared `operators.rs` CTE logic into a complete
`PhysicalWriteTransition::intermediate_states()` bag. Wire shape:
`src/emission/statements.rs`, `schema/protocol.schema.json` and
`docs/protocol.md`. Public full-bag DuckDB oracle:
`tests/task_150_joined_writes.rs`; finite pending acceptance inventory:
`docs/task-150-acceptance-freeze.json`. TASK-81 remains independent.

TASK-149 single-target UPDATE/DELETE proof: `src/analysis/writes.rs` normalizes row-local selectors, direct RETURNING projections and explicit unquoted SET DEFAULT markers; `src/protocol/writes.rs` owns the canonical typed assignment marker. `src/physical_realization/write_states/operators.rs` applies atomically selected updates/deletes against the complete target bag, requiring authoritative scalar defaults and preserving SQL UNKNOWN and duplicate occurrences. `write_states.rs` owns the before/after transition, row counts and projected UPDATE postimages or DELETE preimages; `src/emission/statements.rs` and `schema/protocol.schema.json` own the public wire shape. The versioned freeze is `docs/task-149-acceptance-freeze.json` (historical v1 retained); `tests/task_149_{update_delete,frozen_matrix,dialect_adapter}.rs` own independent native DuckDB, all 13 dialect classifications and direct/dbt/ODCS schema-parity evidence. TASK-81 and TASK-150 remain separate closure work.

TASK-148 ordered INSERT certification: `src/physical_realization/write_states.rs` executes canonical write layers against one evolving complete relation map; `src/physical_realization/write_states/operators.rs` implements VALUES/DEFAULT/SELECT/CTE append and maintains local intermediate source bags; `src/physical_realization/write_states/projection.rs` evaluates row-local source predicates. Independent DuckDB execution and typed full-target/CTE/source truth oracles are in `tests/task_148_ordered_insert.rs` and `tests/task_148_negative_matrix.rs`. All 13 parser dialects plus direct SQL/dbt catalog/ODCS equivalent evidence are checked in `tests/task_148_adapter_dialect.rs`. `docs/task-148-acceptance-freeze.json` is the versioned bounded contract and `docs/task-148-acceptance-audit.md` is the signoff audit. TASK-80 parent acceptance remains separately gated.

TASK-147 targeted conflict/RETURNING: `src/analysis/writes.rs` converts explicit
ON CONFLICT key columns/DO NOTHING and simple RETURNING columns into the
parser-independent `src/protocol/writes.rs` contract. The optional wire
fields are emitted by `src/emission/statements.rs` and validated by
`schema/protocol.schema.json`. `src/physical_realization/write_states/constraints.rs`
requires an enforced matching key and proves tuple collision without
assuming NULL/collation laws; `write_states/operators.rs` suppresses only
proven conflicts; `write_states.rs` projects only inserted rows into
`PhysicalWriteTransition::returning`. Complete target/source/RETURNING DuckDB
oracle: `tests/task_147_insert_conflict.rs`; dialect boundaries:
`tests/task_147_dialect_matrix.rs`. Conditional vendor/overwrite cases are
frozen in `docs/task-147-acceptance-freeze.json` pending native evidence.
TASK-80 parent remains separate.
TASK-146 bounded INSERT SELECT source composition: `src/analysis/writes.rs`
owns parser-side certification of a single nonrecursive row-local CTE,
`src/protocol/writes.rs` owns the canonical normalized producer and consumer,
and `src/physical_realization/write_states/{projection,operators}.rs` owns
the single complete physical input bag and ordered target poststate.
`src/emission/statements.rs` and `schema/protocol.schema.json` describe
the optional CTE stage pair. Independent DuckDB physical oracles live in
`tests/task_146_insert_select.rs` and the independent full-bag oracle is
`tests/task_146_acceptance_matrix.rs`; dialect and adapter parity are
`tests/task_146_dialect_adapter.rs`. The versioned acceptance inventory is
`docs/task-146-acceptance-freeze.json`. TASK-80 parent remains independent.

TASK-145 source-free INSERT constructors: `src/analysis/writes.rs` recognizes
ordinary VALUES and DEFAULT VALUES, reusing the canonical finite literal bags
in `src/analysis/finite_tables.rs`. `src/protocol/writes.rs` owns the typed
source-free default write marker and effect cardinality; `src/physical_realization/write_states/operators.rs`
applies exact literal rows or one authoritative all-default row, preserving
the complete prestate and schema feasibility. `src/emission/statements.rs`
and `schema/protocol.schema.json` own the emitted action and flag. The v2 finite
acceptance inventory is `docs/task-145-acceptance-freeze.json`; the independent
full-bag DuckDB and typed-domain matrix is `tests/task_145_acceptance_matrix.rs`.
`tests/task_145_insert_values.rs` covers generated fields, attested identity
sequences, missing metadata and canonical wire emission;
`tests/task_145_typed_write.rs` covers Arrow-like state feasibility and attested row-local generated scalar fields (but not sequence allocation);
`tests/task_145_precedence.rs` checks ordered writes and transaction residuals;
`tests/task_145_dialect_adapter.rs` checks all 156 parser cells and 36
SQL/dbt-catalog/ODCS-schema equivalence cases. Vendor-only modifiers fail closed
through `tests/task_145_vendor_boundaries.rs`; their portable semantics belong
to TASK-147. Reviewed signoff evidence is `docs/task-145-acceptance-audit.md`.
TASK-80 parent is separately owned by TASK-145..148.

TASK-136 sampling: `src/analysis/sampling.rs` owns sqlparser AST method/quantity/seed normalization, `src/protocol/sampling.rs` holds source-independent typed evidence, and `src/protocol/query.rs` owns sampled source instances. `src/emission/statements.rs` and `schema/protocol.schema.json` own the optional query `sampling` contract. `src/analysis/sets.rs` refuses a sampled source as an independently materializable lossless set branch. `tests/task_136_sampling.rs` verifies typed JSON, schema and sampled DISTINCT safety; `tests/task_136_frozen_matrix.rs` supplies the native DuckDB exact/Impossible complete-bag oracle. `tests/task_136_dialect_matrix.rs` verifies 143 canonical parser/dialect cells; `tests/task_136_adapter_parity.rs` verifies 33 direct SQL/dbt/ODCS cases, and `tests/task_136_dbt_core_e2e.rs` runs a real compiled model. Versioned child signoff is tracked in `docs/task-136-acceptance-freeze.json` and `docs/task-136-acceptance-audit.md`; native vendor sampling laws remain conditional. Parent TASK-76 is not signed off.

TASK-135 composed pagination: `src/analysis/sets.rs` certifies inner/outer CTE pagination and independently ordered set branch leaves; `src/protocol/{query,sets}.rs` preserves both ordered row-shaping stages and each branch's own constraints. `src/physical_realization/page_composition.rs` proves complete original and terminal bags for source-backed CTE pages; `src/physical_realization/set_trees.rs` applies each branch page before set multiplicity, retaining page-rejected identities and typed terminal domains. `src/emission/{statements,aggregation_sets}.rs` and `schema/protocol.schema.json` own the canonical JSON shape. Oracle fixtures live in `tests/task_135_*.rs`, with real dbt Core seed/model under `tests/fixtures/dbt_core_project`; the signed acceptance matrix is `docs/task-135-acceptance-freeze.json`. Implicit NULL order, unproved peers, and unordered producers are residual; TASK-76 parent is separate.

TASK-134 direct page deduplication: `src/protocol/aggregation.rs` owns parser-independent DISTINCT and DISTINCT ON keys; `src/analysis/sets.rs` normalizes bounded direct ORDER/FETCH/TOP with ties; `src/physical_realization/page_bags.rs` proves complete physical DISTINCT bags, exact per-value contributors and full cutoff peer-group membership. DISTINCT ON without a deterministic selected output fails closed unless every candidate projects the same tuple. `src/emission/aggregation_sets.rs`, `schema/protocol.schema.json` and `docs/protocol.md` own the optional wire-level WITH TIES flag. Mandatory, negative, dialect and adapter oracles are in `tests/task_134_{with_ties,frozen_matrix,adapter_parity,dbt_core_e2e}.rs`; `docs/task-134-acceptance-freeze.json` is the owned acceptance inventory. Parent TASK-76 remains separate.

# Code map: find the smallest canonical owner

TASK-140 finite source-free GROUP BY COUNT(*) over literal VALUES/UNNEST/EXPLODE: `src/analysis/finite_tables.rs` normalizes the whole input bag into canonical `FiniteTableKind::Grouped` with contributor indices; `src/physical_realization/finite_tables.rs` proves exact grouped terminal values, typed domains and requested-row impossibility without SQL reparsing. `tests/task_140_finite_group_bags.rs` owns the independent DuckDB oracle. `FiniteTableKind::Joined` retains both complete independent literal bags and exhaustive equality match pairs in `src/protocol/finite_table.rs`; `src/physical_realization/finite_tables.rs` certifies matched/NOT TRUE Cartesian partitions and terminal grouped identities. Compiled dbt models live under `tests/fixtures/dbt_core_project/models/task140_*.sql` and their native warehouse oracle is `tests/task_140_dbt_core_e2e.rs`. Complete dialect/ODCS parity and independent acceptance audit are tracked under `docs/task-140-acceptance-freeze.json`; TASK-77 parent is not certified.

TASK-133 deterministic direct page bounds: `src/analysis/sets.rs` owns shared
AST-to-`SetRowShaping` normalization for direct SELECT and ordered sets;
`src/protocol/query.rs` stores parser-independent row shaping;
`src/physical_realization/page_bags.rs` proves exact requested-row membership,
physical impossibility and full source/terminal bags and
fail-closed tied/null-order boundaries. `src/emission/aggregation_sets.rs`
owns the shared JSON shape; `tests/task_133_page_bags.rs` and `tests/task_133_frozen_matrix.rs` hold
independent DuckDB, 169 dialect cell and 39 adapter cell oracles;
`tests/task_133_dbt_core_e2e.rs` executes the compiled dbt Core model. `docs/task-133-acceptance-freeze.json` tracks remaining
TASK-133 and TASK-76 signoff obligations.


TASK-139 source-free table generators: `src/analysis/finite_tables.rs` is the only supported AST-to-canonical boundary for typed literal `EXPLODE` (SELECT and FROM forms), alongside `UNNEST`. `src/protocol/finite_table.rs` owns the exact typed row bag, and `src/physical_realization/finite_tables.rs` proves complete positive and missing-row membership. Unattested JSON_TABLE, Snowflake FLATTEN, opaque table UDFs, dynamic shape and native engine-specific laws remain non-exact. The verified v3 acceptance inventory is `docs/task-139-acceptance-freeze.json`; exact and conditional public tests are in `tests/task_139_generators.rs`, `tests/task_139_frozen_matrix.rs` and `tests/task_139_adapter_matrix.rs`, plus `tests/task_139_dbt_core_e2e.rs`. TASK-139 child acceptance is verified; TASK-77 parent remains open.

TASK-138 source-free finite wildcard projections, fixed-key SUM PIVOT and named-column UNPIVOT: `src/analysis/finite_tables.rs` validates complete literal VALUES inputs and SQL operator ASTs before forming canonical typed `FiniteTable` rows in `src/protocol/finite_table.rs`; `src/physical_realization/finite_tables.rs` proves complete physical inputs, per-row contributor identities, closed terminal bags and impossible membership. Opaque inputs, dynamic pivots and unproved arithmetic remain Residual. Public tests: `tests/task_138_finite_star_bags.rs` and `tests/task_138_pivot_unpivot.rs`.

TASK-137 source-free literal tables: `src/analysis/finite_tables.rs` normalizes validated VALUES and UNNEST/EXPLODE literal forms into `src/protocol/finite_table.rs`; `src/physical_realization/finite_tables.rs` proves exact identity/multiplicity/domain and absent-row membership from those canonical rows, not SQL reparsing. `src/emission/statements.rs` and `schema/protocol.schema.json` retain the wire contract; `tests/task_137_finite_tables.rs` compares complete DuckDB output bags. Source-column arrays, opaque generators, joined fanout, unproved element types, and vendor/session semantics remain residual; `docs/task-137-acceptance-freeze.json` preserves the unfinished dialect, adapter and CI matrix.


TASK-131 scalar / quantified SQL-bound proofs: `src/protocol/predicates.rs` defines the canonical `QuantifiedSubqueryPredicate`; `src/analysis/predicates.rs` parses ANY/SOME/ALL, `src/emission/predicates.rs` serializes the JSON wire shape, and `schema/protocol.schema.json` validates it. `src/physical_realization/scalar_quantified.rs` proves complete uncorrelated singleton scalar and source-free quantified terminal bags from attested finite physical sources. `tests/task_131_scalar_quantified.rs` exercises 12 frozen mandatory cases x 13 dialect paths with native complete-bag DuckDB oracles, including nested/LATERAL conditional boundaries; `tests/task_131_adapter_parity.rs` covers SQL, compiled dbt and ODCS source evidence. Full cross-engine native quantifier laws and correlated LATERAL/APPLY remain typed conditional residuals.


This map is a **navigation index**, not a second semantic specification. For a change, start with the narrowest owner below. Follow the interfaces to adjacent modules only when required, and run the focused test first. The canonical protocol is the Rust model in `src/protocol/`; its serialized contract is `schema/protocol.schema.json`. Do not duplicate a semantic rule in an adapter or emitter.

Validated Arrow-like witness sidecar serialization: `src/emission/typed_physical.rs` owns deterministic physical value JSON; `schema/typed-physical-population.schema.json` documents its separate, non-protocol wire shape; `tests/physical_typed_serialization.rs` verifies exact bytes, bits, JSON NULL and failing proofs. Generated expression dependency ordering and portable function execution remain in `src/physical_realization/write_states/generated.rs`.

Physical typed target append and ordered nested DML feasibility: `src/physical_realization/write_states/typed_populations.rs` checks before/after closed-world nested snapshots, scalar-filtered INSERTs, attested scalar-default omissions, scalar UPDATE/DELETE and preservation of untouched Arrow values; `src/physical_realization/write_states/evaluator.rs` owns shared three-valued scalar evaluation. Text-key/foreign-key binary comparison evidence: `src/relation.rs` owns source law attestation, `src/physical_realization/write_states/constraints.rs` applies it, and `src/emission/document.rs` emits the provenance.

Portable Boolean CHECK AST and three-valued validator: `src/computed_boolean_checks.rs` owns canonical recursive predicate/evidence, `src/analysis/constraints.rs` normalizes SQL DDL and dbt expressions, `src/physical_realization/write_states/boolean_checks.rs` evaluates exact TRUE/FALSE/UNKNOWN, and `src/emission/constraints.rs` emits the JSON contract.

Physical schema and default semantics: `src/relation.rs` owns typed columns, collation and timezone identity; `src/column_sources.rs` owns defaults and typed row-local generated-expression/opaque identity evidence; `src/computed_keys.rs` and `src/computed_checks.rs` own functional key and arithmetic CHECK facts; `src/constraints.rs` merges and validates constraints; `src/physical_realization/write_states/{constraints,check_comparisons,type_validation,operators,typed_values,typed_populations}.rs` owns closed-world physical feasibility, numeric CHECK validation and nested Arrow-like value proof. Adapter paths should only translate evidence, never implement independent proof laws.

ASOF ordering lineage and unsupported ASOF/APPLY diagnostics: `src/analysis/joins.rs`; correlated `LATERAL` derived-row fail-closed boundary: `src/analysis/relations.rs`; public safety regressions: `tests/task_114_temporal_lateral_boundaries.rs`. The ASOF eligibility predicate is preserved in the canonical join condition without claiming that all eligible pairs are selected. Complete bounded nearest-prior and correlated equality source-bag proofs are certified for the portable frozen forms; unmatched ASOF preservation, tied maximal predecessors and native vendor/session/table-function laws fail closed.

TASK-114 finite complete row bags: `src/protocol/relations.rs` owns typed ASOF and correlated `JoinExecution`; `src/analysis/joins.rs` owns AST-derived correlation and physical source-instance identity, and `src/analysis/queries.rs` the parser-certified special-join outer row shape. `src/constructive/types.rs` owns selected/nonselected complete identity pair laws `FiniteJoinLaw` and `TemporalJoinLaw`, `src/physical_realization/join_finite.rs` owns certified signed-integer nearest/range and LATERAL/APPLY source populations, and `src/physical_realization/join_temporal.rs` owns explicitly zone-free temporal complete bags. `src/constructive/cases.rs` validates source bag invariants. `src/emission/constructive.rs` and `schema/protocol.schema.json` own the wire contract. Independent public oracles: `tests/task_114_finite_populations.rs`, `tests/task_114_canonical_frozen.rs`, `tests/task_114_temporal_population.rs`, `tests/task_114_special_adapter_parity.rs`, `tests/task_114_frozen_execution.rs`, `tests/task_114_dialect_matrix.rs`, `tests/task_114_dialect_constructive.rs`.

TASK-143 finite scalar IN/NOT IN: `src/subquery_witness.rs` owns the
parser-independent scalar member/correlation keys; `src/physical_realization/in_bags.rs`
evaluates complete source-row bags as SQL TRUE/FALSE/UNKNOWN for scalar
and positional integer tuples, retaining candidate identities, selected/rejected
outer identities and finite terminal domains. Unproved nested filters, non-integer
comparison and native engine differences remain explicit Residual.

TASK-129 finite EXISTS/NOT EXISTS: `src/subquery_witness.rs` owns parser-independent operator-local correlations and membership directions, while `src/physical_realization/exists_bags.rs` evaluates entire caller-attested direct source populations and retains both TRUE and NOT TRUE physical outer identities, complete per-row inner matches, terminal bag, and typed output domain. Unsupported shapes fail closed; adapter parity uses canonical facts only. Frozen evidence: `docs/task-129-acceptance-freeze.json`; public DuckDB, dialect, and adapter proofs: `tests/task_129_exists_bags.rs` and `tests/task_129_adapter_parity.rs`.

TASK-132 grouped and joined correlated EXISTS: `src/analysis/subqueries.rs` owns
parser-certified candidate-preserving and grouped/joined existence classifications;
`src/protocol/values.rs` retains these canonical shape facts without conflating
them with IN/scalar candidate multiplicity. `src/physical_realization/exists_bags.rs`
owns exact conjunctive correlation proofs; `exists_joined.rs` owns full
three-source INNER JOIN identity pairs. `exists_producers.rs` evaluates only
closed-world attested physical leaf bags through canonical filter, scalar CASE,
JOIN and correlated EXISTS producer layers and computes complete terminal
integer/text domains and source provenance. `src/bundle/state.rs` attaches
explicit timestamp-zone evidence without guessing adapter defaults. Frozen
matrix: `docs/task-132-acceptance-freeze.json`; source/oracle tests:
`tests/task_132_composite_correlations.rs`,
`tests/task_132_adapter_parity.rs` and
`tests/protocol_conformance_dbt_e2e.rs`. Unsupported SQL forms, ambiguous
aliases, unverified source metadata and non-DuckDB native engine behavior
remain explicitly uncertified rather than extrapolated.

TASK-115 source-attested finite three-way joins: `src/physical_realization/finite_chains.rs` owns closed-world source-identity, normalized SQL TRUE/NOT TRUE, nullable composite equality, and downstream COUNT(*) bag evaluation. `src/physical_realization/write_states.rs` remains the canonical owner of physical datatype and PK/FK feasibility. `src/physical_realization/joins.rs` and `grouping.rs` retain the separate homogeneous sufficient count construction. The exact frozen eight-case SQL/DuckDB comparison is in `tests/task_115_finite_chain_bags.rs` and `tests/task_115_duckdb_matrix.rs`; `docs/task-115-acceptance-freeze.json` owns source/terminal values and pending adapter and session evidence. This evidence-driven evaluator does not imply that the solver automatically synthesizes those exact bags from cardinality goals.

TASK-119 bounded set row-shaping: `src/analysis/sets.rs` normalizes typed
literal LIMIT/OFFSET and FETCH ONLY with named ORDER BY output positions;
`src/protocol/sets.rs` owns canonical sort keys and row shaping without AST;
`src/physical_realization/set_trees.rs` certifies closed-world integer tuples,
explicit NULL order, total tie-safe sorting and final exact output domains.
Unattested coercion, BY NAME/CORRESPONDING and nested branch LIMIT remain
Residual. Oracles: `tests/task_119_ordered_set_bags.rs`,
`docs/task-119-acceptance-freeze.json`.

TASK-118 finite nested source-bag proof: `src/analysis/sets.rs` certifies the strict,
lossless single-source CTE boundary and retains independent branch semantics;
`src/protocol/sets.rs` owns the recursive operation tree and positional count
laws; `src/physical_realization/set_trees.rs` evaluates complete source snapshots
once per physical relation and proves nested exact membership/absence,
SQL TRUE/FALSE/UNKNOWN branch partitions, typed terminal domains and duplicate
frequencies. The task-owned fixture matrix is
`docs/task-118-acceptance-freeze.json`; independent oracles are in
`tests/task_118_nested_set_bags.rs`, `tests/task_118_dialect_matrix.rs`,
and `tests/task_118_set_adapters.rs`. Unsupported CTE producer shapes,
coercions, order/limits, and unattested scalar expressions remain Residual.

TASK-123 joined aggregate physical proof: `src/physical_realization/aggregate_joins.rs` owns complete two-source INNER JOIN/WHERE/GROUP/HAVING bag enumeration, SQL NOT TRUE rejected pairs, exact group contributor identities and terminal scalar domains; `tests/task_123_join_aggregates.rs` owns the independent DuckDB source/terminal oracle and 13-dialect parser classification; `docs/task-123-acceptance-freeze.json` is the task-specific fixed mandatory/conditional matrix. GROUPING SETS, ROLLUP, CUBE and GROUP BY ALL stay typed Residual until their grouping identities and native/session laws are attested.

TASK-122 DISTINCT and FILTER aggregate contributors and full HAVING group truth: `src/physical_realization/aggregate_bags.rs` owns complete typed integer input bags, distinct contributor identity, row-level FILTER three-valued selection and per-group HAVING TRUE/FALSE/UNKNOWN with exact surviving terminal domains. The frozen source/terminal oracle and complete rejected-group witnesses are in `tests/task_122_aggregate_having.rs` and `docs/task-122-acceptance-freeze.json`; unproved scalar, decimal or vendor laws fail closed.

TASK-121 basic grouped/global closed-world aggregate bags: `src/physical_realization/aggregate_bags.rs` certifies complete typed integer source bags through canonical COUNT(*), COUNT(column), SUM, MIN, MAX and exactly integral AVG. It returns exact terminal source identity, multiplicities, NULLs and finite output domains; unproved AVG scale, overflow, HAVING and DISTINCT fail closed. Public DuckDB regression: `tests/task_121_aggregate_bags.rs`; fixture matrix: `docs/task-121-acceptance-freeze.json`.

TASK-125 portable ranking on complete physical bags: `src/physical_realization/rank_bags.rs` evaluates ROW_NUMBER, RANK and DENSE_RANK with peer groups, explicit NULL order, whole-bag source identities and typed terminal domains. Tied ROW_NUMBER identity, implicit NULL ordering and unsupported window shapes fail closed; frozen cases are in `docs/task-125-acceptance-freeze.json` with independent DuckDB proofs in `tests/task_125_rank_bags.rs`. This does not grant exact grouped/joined ranking or native vendor/session execution laws.
TASK-126 complete navigation and bounded ROWS bags: `src/physical_realization/window_bags.rs` owns parser-independent LAG, LEAD, FIRST_VALUE, LAST_VALUE and COUNT(*) ROWS membership over one validated physical source. It reuses canonical signed order, NULL order, full source identity and exact domain helpers from `rank_bags.rs`. Ambiguous ORDER BY peers, implicit/default RANGE, GROUPS, unproved arguments and vendor-specific laws fail closed. Frozen source and terminal oracles: `docs/task-126-acceptance-freeze.json`; public DuckDB/dialect oracles: `tests/task_126_window_bags.rs`. Common aggregate OVER functions are implemented in TASK-189; additional precision and composition obligations remain TASK-189 signoff blockers.
TASK-127 conjunctive direct-source QUALIFY: `src/physical_realization/window_bags.rs` evaluates parser-independent window comparisons against one shared complete physical bag, including hidden QUALIFY-only windows, peer-stable RANK/DENSE_RANK (with underordered ROW_NUMBER remaining Residual), SQL NULL NOT TRUE, and stable selected/rejected source IDs. Independent evidence lives in `tests/task_127_qualify_bags.rs`; the frozen joined/grouped requirements remain open in `docs/task-127-acceptance-freeze.json` until physical source composition is certified.
TASK-189 common aggregate windows: `src/physical_realization/window_bags.rs` reuses source-attested physical identity and deterministic ROWS membership for COUNT(*), COUNT(column), SUM, AVG, MIN and MAX. Checked signed-integer and binary-exact, scale-attested finite DECIMAL arithmetic preserve full source/terminal bags; nonintegral AVG(INT), unproved integer SUM widening, nonbinary fractions, numeric overflow and session-dependent promotion remain Residual. Uniquely ordered mixed ROW_NUMBER/RANK/DENSE_RANK and navigation/aggregate projections share exact ROWS membership; peers remain Residual. Projected integer COUNT/SUM/AVG/MIN/MAX QUALIFY comparisons classify nullable UNKNOWN as NOT TRUE and preserve stable source identities. Frozen v5 oracle inventory: `docs/task-189-acceptance-freeze.json`; public source/DuckDB/dialect and adapter assertions: `tests/task_189_window_aggregates.rs`, `tests/task_189_window_adapters.rs`, `tests/task_189_decimal_windows.rs`, `tests/task_189_decimal_adapters.rs`, `tests/task_189_native_duckdb_matrix.rs`, and `tests/native/test_task_189_sqlite_native.py` (executed in CI). The v5 audit `docs/task-189-acceptance-audit.md` records explicitly unexecuted vendor/session Residual boundaries and final review prerequisites.


## Starting points

| Change or question | Read first | Follow only if needed | Focused tests |
|---|---|---|---|
| SQL `JOIN`, ON, equality or join kind | `src/analysis/joins.rs` | `src/analysis/relations.rs`, `src/analysis/queries.rs` (row-shape certification), `src/protocol/query.rs`, `src/join_witness.rs`, `src/physical_realization/joins.rs` | `tests/differential_conformance.rs`, `tests/physical_realization/joins.rs`, `tests/protocol_conformance_physical.rs`, `tests/task_106_composed_predicates.rs`, `tests/task_109_composite_join.rs` |
| TASK-113 portable CROSS, USING, NATURAL, OR-on, non-equi and null-safe joins | `src/analysis/joins.rs` (schema-backed NATURAL column intersection) | `src/physical_realization/join_uniform.rs` (complete uniform Boolean pair populations), `src/physical_realization/joins.rs`, `src/constructive/types.rs`, `src/emission/constructive.rs` | `tests/task_113_portable_joins.rs`, `tests/task_113_parser_matrix.rs`, `tests/task_113_join_adapters.rs` |
| CTE, derived table, source alias, table scope | `src/analysis/relations.rs` | `src/analysis/lineage.rs`, `src/analysis/output.rs` (positional output aliases), `src/composition.rs` | `tests/local_relations.rs`, `tests/task_105_temporal_json_vendor.rs` |
| `WHERE`, `HAVING`, `QUALIFY`, SQL truth | `src/analysis/predicates.rs` | `src/analysis/exactness.rs`, `src/boolean_witness.rs`, `src/domain.rs` | `tests/differential_conformance.rs`, `tests/boolean_witness.rs`, `tests/task_103_cross_column.rs`, `tests/task_104_pattern_collation_nullsafe.rs` |
| Catalog-certified finite and interval arithmetic output/write domains | `src/analysis/arithmetic_domains.rs` | `src/analysis/output.rs`, `src/analysis/queries.rs`, `src/analysis/writes.rs` | `tests/task_101_arithmetic.rs`, `tests/advanced_target_validation.rs` |
| Numeric bounds, scalar domains or comparisons | `src/domain.rs` | `src/analysis/domains.rs`, `src/analysis/typing.rs`, `src/protocol/values.rs` | `tests/typed_domains.rs`, `tests/output_domains.rs` |
| Explicit CAST/TRY_CAST/SAFE_CAST literal outcome domains | `src/analysis/cast_domains.rs` | `src/analysis/output.rs` | `tests/output_domains.rs` |
| Gregorian DATE literal identity casts, literal EXTRACT fields, shared calendar validation and ordering | `src/analysis/temporal_literals.rs`, `src/temporal.rs` | `src/analysis/expressions.rs`, `src/analysis/typing.rs`, `src/domain.rs`, `src/physical_realization/write_states/type_validation.rs` | `tests/task_105_temporal_json_vendor.rs`, `tests/typed_domains.rs` |
| Temporal/JSON/nested unsupported AST boundaries and named opaque-function outcomes | `src/analysis/expressions.rs`, `src/analysis/output.rs` | `src/protocol/values.rs`, `src/boolean_witness.rs`, `docs/coverage-manifest.json` (`task_105_expression_evidence`) | `tests/task_105_temporal_json_vendor.rs` |
| Portable exact-decimal literal output arithmetic | `src/analysis/decimal_literals.rs` | `src/analysis/output.rs` | `tests/output_domains.rs` |
| Width-checked scalar integer arithmetic and reversible predicates | `src/boolean_witness/numeric.rs` | `src/boolean_witness.rs`, `src/protocol/functions.rs` | `tests/task_101_arithmetic.rs` |
| CASE/COALESCE/NULLIF output domains, guard refinement, computed comparisons and derived alias witnesses | `src/analysis/output.rs`, `src/boolean_witness.rs` | `src/analysis/arithmetic_domains.rs`, `src/analysis/predicates.rs`, `src/analysis/queries.rs`, `src/domain.rs`, `src/protocol/functions.rs` | `tests/task_102_computed_expressions.rs`, `tests/task_70_predicates.rs` |
| `GROUP BY`, aggregation or grouped physical rows | `src/analysis/aggregation.rs` | `src/group_witness.rs`, `src/physical_realization/grouping.rs`, `src/protocol/aggregation.rs` | `tests/group_witness.rs`, `tests/physical_realization/group_rank_set.rs`, `tests/protocol_conformance_physical.rs` |
| Window/ranking functions | `src/analysis/functions.rs` | `src/window_witness.rs`, `src/physical_realization/ranking.rs`, `src/physical_realization/grouping.rs` | `tests/physical_realization/mixed.rs`, `tests/physical_realization/group_rank_set.rs` |
| `UNION`, `INTERSECT`, `EXCEPT` | `src/analysis/sets.rs` | `src/protocol/sets.rs`, `src/physical_realization/operators.rs`, `src/physical_realization/sets.rs`, `src/physical_realization/set_bags.rs` (TASK-117 exact two-column source bags) | `tests/set_operations.rs`, `tests/physical_realization/nested_sets.rs`, `tests/task_117_set_bags.rs` |
| `INSERT`, `UPDATE`, `DELETE`, `MERGE` | `src/analysis/writes.rs` | `src/analysis/arithmetic_domains.rs`, `src/protocol/writes.rs`, `src/bundle/layers.rs`, `src/emission/statements.rs` | `tests/dml_transformations.rs`, `tests/advanced_target_validation.rs` |
| Ordered closed-world DML target snapshots | `src/physical_realization/write_states.rs` | `src/physical_realization/write_states/{operators,projection,evaluator,constraints}.rs`, `src/protocol/writes.rs`, `src/physical_realization/counts.rs` | `tests/physical_write_states.rs`, `tests/physical_write_adapter_parity.rs` |
| Correlated or nested subquery membership | `src/analysis/subqueries.rs` | `src/subquery_witness.rs`, `src/physical_realization/membership.rs` | `tests/subquery_witness.rs`, `tests/constructive_membership.rs` |
| Output projection, expression, lineage | `src/analysis/output.rs` | `src/analysis/expressions.rs`, `src/analysis/lineage.rs`, `src/protocol/relations.rs` | `tests/output_domains.rs` |
| Physical projection inversion, complete value histograms | `src/physical_realization/distributions.rs` | `src/physical_realization/joint_distributions.rs`, `src/physical_realization/graph.rs`, `src/outcome_goals.rs`, `src/outcome_proofs.rs` | `tests/physical_realization/distributions.rs` |
| SQL DDL constraints | `src/analysis/constraints.rs` | `src/constraints.rs`, `src/protocol/relations.rs` | `tests/dbt_constraints.rs` |
| Multi-query graph, producer/consumer, cycle | `src/bundle/graph_build.rs` | `src/bundle/graph.rs`, `src/bundle/orchestration.rs`, `src/composition.rs` | `tests/composition.rs`, `tests/physical_realization/graph.rs` |
| Transitive composition and column identities | `src/composition.rs` | `src/bundle/composed.rs`, `src/physical_realization/graph.rs` | `tests/composition.rs`, `tests/composition_exactness.rs` |
| Local constructive obligations and strength | `src/constructive/local.rs` | `src/constructive/types.rs`, `src/constructive/cases.rs`, `src/constructive/direction.rs` | `tests/constructive_witness.rs`, `tests/constructive_adapter_parity.rs` |
| Joint physical source proof | `src/physical_realization/composition.rs` (shared operator histogram reconciliation) | `src/physical_realization/joint_counts.rs` (conjunctive source-row and join/group reconciliation), `src/physical_realization/joins.rs` (filtered join populations, overlapping and independent multi-goal combination, join/group shared-key proofs and materialization laws), `src/physical_realization/graph.rs` (certified widening/total affine inversion, identity mapping and constraint guards), `src/boolean_witness.rs` (canonical affine predicate rewrite and physical truth recheck) | `tests/physical_realization/mixed.rs`, `tests/physical_realization/joint_operators.rs`, `tests/physical_realization/joins.rs`, `tests/task_106_composed_predicates.rs` |
| TASK-108 TASK-70 parent acceptance / independent evidence closure | `docs/task-108-acceptance-freeze.json`, `docs/task-108-acceptance-audit.md` | `docs/task-107-acceptance-freeze.json`, `.backlog/tasks/task-70 - solve-expression-and-predicate-variants.md` | `tests/task_108_parent_signoff.rs`, `tests/task_107_frozen_execution_oracles.rs`, `tests/task_107_dialect_classifications.rs` |
| TASK-106 finite acceptance / proof evidence sign-off | `.backlog/tasks/task-106 - compose-computed-predicates-across-relational-graphs.md` (frozen matrix) | `docs/coverage-manifest.json`, `docs/semantics.md` (certified rules and residual boundaries) | `tests/task_106_signoff.rs`, `tests/task_106_composed_predicates.rs`, `tests/physical_write_states.rs`, `tests/physical_write_adapter_parity.rs` |
| Public Rust protocol model | `src/protocol/` (relevant domain module) | `src/lib.rs`, `schema/protocol.schema.json` | `tests/library_api.rs` |
| Protocol JSON output | `src/emission/` (matching output family) | `src/protocol/`, `schema/protocol.schema.json` | `tests/library_api.rs` |
| dbt inputs, compiled SQL and constraints | `src/dbt.rs` | `src/bundle/orchestration.rs`, `src/constraints.rs` | `tests/dbt_manifest.rs`, `tests/dbt_constraints.rs`, `tests/dbt_core_e2e.rs` |
| Data contracts (ODCS) | `src/odcs.rs` | `src/relation.rs`, `src/constraints.rs` | `tests/odcs.rs` |
| CLI inputs and output options | `src/main.rs` | `src/manifest.rs`, `src/emission/document.rs` | `tests/cli.rs` |

## Emission module ownership

- `src/emission/document.rs`: top-level protocol shape and bundle fields; **only here** assemble the document
- `src/emission/graph.rs` and `composition.rs`: DAG nodes/edges, layer and composed semantics
- `src/emission/statements.rs`: query, writes and mutation state
- `src/emission/operator_witnesses.rs`, `constructive.rs`: operator and constructive proof serialization
- `src/emission/aggregation_sets.rs`, `predicates.rs`, `expressions.rs`, `scalars.rs`: feature-specific SQL values
- `src/emission/relations_domains.rs`, `constraints.rs`, `outcomes.rs`: value domains and evidence

All emitters read canonical parser-independent values. To change protocol meaning, update its actual canonical owner and schema (when required), rather than silently changing one JSON encoder.

## How to use this map

1. Identify the affected semantic concept and read its **first owner**.
2. Read adjacent interfaces and the narrowly relevant test file; do not ingest whole modules/directories by default.
3. If a feature crosses ownership boundaries, follow explicit API contracts and keep evidence/provenance with the canonical owner.
4. Run a targeted test (`cargo test --test <integration-test-name>`), then the repository's full verification gate before merge.

If a path or responsibility changes, update this map **in the same PR**. Keep this document short and factual; it should not become a parallel product roadmap.

TASK-71 source-side orientation: `src/physical_realization/joins.rs` orients recovered filtered-join equality endpoints by the canonical `Join::left/right` inputs before constructing preserved-side outer/null-extension obligations. Public proof regression: `tests/physical_realization/joins.rs`.

TASK-110 three-instance common-key INNER chain: `src/physical_realization/joins.rs` owns the jointly controlled physical counts and SQL alias mapping; `src/constructive/types.rs` and `cases.rs` own `JoinChainPopulation` and invariants, `src/emission/constructive.rs` owns serialization, `tests/task_110_join_chains.rs` owns public/DuckDB regression.

TASK-115 typed independent-filter and composite-ON homogeneous chain construction: `src/physical_realization/joins.rs` checks every equality component and attaches source-row TRUE constraints on the same physical leaves; `src/physical_realization/grouping.rs` lifts complete positive chains into one COUNT(*) group. `tests/task_115_join_filter_group.rs`, `tests/task_115_adapter_parity.rs`, `tests/task_115_duckdb_matrix.rs` and `docs/task-115-acceptance-audit.md` distinguish these sufficient homogeneous proofs from the still-missing finite heterogeneous/null/constrained complete-bag laws.

TASK-128 bounded advanced window physical proofs: `src/physical_realization/window_bags.rs` implements deterministic source-backed NTILE buckets and complete peer-aware RANGE unbounded/CURRENT and bounded GROUPS frame membership, reusing canonical ORDER BY and explicit NULL ordering from `src/physical_realization/rank_bags.rs`. `docs/task-128-acceptance-freeze.json` owns the typed physical/terminal bag oracles and conditional session boundaries; `tests/task_128_advanced_windows.rs` tests the public entry point against independent DuckDB. Native vendor/session matrix, numeric RANGE offsets, and independent TASK-74 parent signoff remain open.

TASK-73 parent aggregate acceptance: `tests/task_73_parent_closure.rs` pins the original AC and the TASK-121..124 and TASK-115 immutable oracles. `tests/task_73_composed_parent.rs` independently evaluates TASK-115's three-source/two-INNER-join WHERE -> projected CTAS stage -> GROUP/HAVING with exact positive/rejected source triplets and typed terminal domains, checking real DuckDB and all 13 dialects; `tests/task_115_finite_adapter_parity.rs` remains the canonical SQL/dbt/ODCS parity oracle. `docs/task-73-acceptance-freeze.json` and `docs/task-73-acceptance-audit.md` own the bounded parent review. Native vendor runtime laws and TASK-91 are separate gates.

TASK-74 independent parent window acceptance: `docs/task-74-acceptance-freeze.json` pins the original six AC against merged TASK-125/126/189/127/128 immutable complete-bag/typed-domain/parser/adapter matrices. `tests/task_74_parent_closure.rs` independently re-executes joined and grouped upstream QUALIFY SQL against native DuckDB, reads complete physical source and terminal bags and checks typed bounds; it also guards all 34 mandatory and 22 reviewed conditional child case identities plus the parent filtered-input matrix, and exercises a separate parent-frozen nested projected-window rank TRUE/NOT TRUE oracle. `docs/task-74-acceptance-audit.md` owns the independent parent scope and conditional vendor/session dispositions. TASK-91 remains a distinct release gate.

TASK-74 upstream filtered-window physical realization: `src/physical_realization/window_filters.rs` compiles a complete canonical WHERE predicate with attested signed-integer/null laws and evaluates SQL TRUE/FALSE/UNKNOWN for each original source identity. `src/physical_realization/window_bags.rs` constructs partitions only from WHERE-TRUE rows and retains WHERE-not-TRUE identities separately from QUALIFY-rejected identities without changing the public proof struct. `docs/task-74-filtered-qualify-freeze.json` freezes original AC #3 mandatory filtered SQL/oracle variants. `tests/task_74_filtered_qualify.rs` verifies exact complete native DuckDB bags, typed domains, physical identities, 13 dialect paths and Residual failures; `tests/task_74_filtered_adapters.rs` verifies SQL/dbt/ODCS parity.

TASK-77 bounded derived subquery closure: `src/analysis/finite_tables.rs` recognizes exactly one noncorrelated, unfiltered `SELECT *` wrapper around a complete explicitly named and typed source-free UNNEST; `FiniteTableKind::Star` and `with_source` preserve the inner canonical producer bag and row-to-contributor identities, and `src/physical_realization/finite_tables.rs` certifies exact/Impossible outcomes. `tests/task_77_derived_subquery.rs` compares native DuckDB complete NULL/duplicate source and terminal bags and proves unsupported filtered/correlated LATERAL forms cannot become exact. `docs/task-77-acceptance-freeze.json` v2 owns the missing parent-original-AC subquery and LATERAL cells; TASK-91 remains independent.

TASK-142 finite recursive CTE laws: `src/analysis/cte.rs` certifies bounded
source-free integer seeds, guarded positive steps and UNION DISTINCT/ALL
into the parser-independent `CteRecursion` and recursive `CteStage` in
`src/protocol/cte.rs`. `src/physical_realization/cte_bags.rs` executes the
checked finite delta and explicit producer self-reference, preserving
physical row identities, NOT TRUE termination and inclusive typed terminal
bounds; unproven cyclic or overflowing recursion returns `Residual`.
`tests/task_142_recursive_cte.rs` supplies the complete DuckDB and Impossible
oracles, and `tests/task_142_dialect_adapter.rs` owns all 14 exposed dialect
classifications and SQL/dbt compiled-model parity. Versioned acceptance:
`docs/task-142-acceptance-freeze.json`; original v1 remains immutable.

TASK-153 matched MERGE target occurrence identity: `src/physical_realization/write_states/operators.rs` applies the first eligible matched UPDATE/DELETE clause over complete target and source bags and records the exact affected prestate ordinals. `src/physical_realization/write_states.rs` exposes those ordinals through the canonical `PhysicalWriteTransition` (also including BY SOURCE mutations, without a separate copy of selection state). `tests/task_153_merge_matched.rs` owns the frozen full DuckDB pre/post-state and typed outcome-domain comparison, NULL and duplicate-source contrast, plus enforced constraint impossibility. The active v3 acceptance matrix is `docs/task-153-acceptance-freeze.json` (v1/v2 retained), linked from `docs/coverage-manifest.json`. `tests/task_153_dialect_adapter.rs` owns all 65 parser/dialect cells and eight direct/dbt/ODCS proof comparisons; `tests/task_153_acceptance_inventory.rs` owns frozen completeness and parent AC mapping; `tests/task_153_merge_matched.rs` also verifies the independent three-step ordered replay. `docs/task-153-acceptance-audit.md` owns the task signoff crosswalk. TASK-153 was marked Done via explicit project-owner approval after nine-job implementation CI and a recorded code-review follow-up; separate-account independent approval was not obtained. Final metadata-head CI is required before PR #174 squash merge. TASK-82 remains open for independent parent signoff.


TASK-177 physical predicate and adapter seam: `docs/task-177-predicate-oracles.json`
owns 25 original TASK-66 sample identities, a complete integer/NULL source bag,
a duplicate-key customer bag and independent TRUE/FALSE/UNKNOWN/NOT TRUE terminal
expectations. `tests/task_177_predicate_oracles.rs` verifies exact DuckDB physical
bags, ten typed equality/range claims and 13-dialect canonical equality for those
portable samples; `tests/task_177_predicate_adapter_parity.rs` verifies source
facts across direct SQL/dbt/ODCS without attesting warehouse execution.
