---
id: TASK-27
title: Add dbt manifest adapter
status: To Do
assignee: []
created_date: '2026-10-02'
labels: []
milestone: m-0
dependencies:
  - TASK-25
---

## Description

Add a dbt-specific adapter that consumes dbt artifacts, primarily the dbt manifest, and maps a dbt project into the SQL Semantic Protocol without making the core protocol or analyzer dbt-specific.

The adapter should use authoritative metadata from dbt artifacts for model identity, dependencies, relation identity, and available SQL instead of inferring model identity from SQL filenames. The adapter is an integration boundary: dbt concepts are translated into the protocol's existing input, dataset, graph, and semantic-analysis concepts before emission.

This is not a goal to become fully dbt-compatible or to reproduce dbt behavior. The goal is to make a dbt project a first-class source for producing the same protocol that direct SQL, file, and manifest inputs produce.

This task is intentionally scheduled after the complete generic bundle workflow is validated and immediately before the 1.0.0 release task.

## Acceptance Criteria

- [ ] A documented adapter accepts a dbt manifest artifact and produces one deterministic SQL Semantic Protocol document.
- [ ] dbt model identity and relation identity come from dbt artifact metadata when available, not from filename inference.
- [ ] dbt dependency metadata is translated into the protocol's existing dependency/layer graph without introducing dbt-specific graph semantics into the core protocol.
- [ ] SQL available through dbt artifacts is analyzed through the same core analyzer as non-dbt SQL inputs rather than through a separate semantic implementation.
- [ ] The adapter preserves explicit unknown/unsupported semantics when required dbt metadata or analyzable SQL is unavailable.
- [ ] dbt-specific types and schema details remain contained at the adapter boundary and do not leak into public core protocol domain types.
- [ ] Supported dbt artifact/schema versions and compatibility expectations are documented explicitly.
- [ ] Tests cover multiple dbt models, model dependencies, configured relation names, at least one source/external dependency, and deterministic repeated emission.
- [ ] An equivalent workload supplied through the dbt adapter and through the generic analysis inputs produces equivalent protocol semantics where the available information is equivalent.
- [ ] README documentation shows how to generate protocol output from a dbt project artifact and clearly describes the adapter's scope and non-goals.
