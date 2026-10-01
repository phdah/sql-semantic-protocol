---
id: TASK-10
title: Define multi-query composition contract
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-9
---

## Description

Extend the public protocol contract from one analyzed SQL input to one protocol document that can represent an arbitrary number of SQL inputs, including both related transformation chains and completely independent queries.

The contract must distinguish individual input units, produced datasets, consumed relations, dependency edges, transformation layers, and final outcomes without exposing sqlparser AST types. The design must not impose a fixed query-count limit. Practical limits may come from available memory or runtime, but not from the public API or protocol model.

Because this changes the public protocol shape, follow the project's pre-1.0 semantic-versioning rule and introduce the appropriate next minor protocol version.

## Acceptance Criteria

- [x] A versioned protocol contract can represent multiple SQL inputs in one document.
- [x] Every input unit has a deterministic identity suitable for diagnostics and graph edges.
- [x] The contract can represent named produced datasets and anonymous query results.
- [x] Related inputs can be represented as a dependency graph while unrelated inputs remain separate graph components in the same document.
- [x] The contract defines terminal/final outcomes independently for every graph component.
- [x] The contract distinguishes local layer semantics from composed/transitive semantics where both are needed.
- [x] Missing producers, ambiguous producers, cycles, and unsupported composition remain explicit rather than guessed.
- [x] Ordering rules guarantee byte-deterministic JSON for equivalent inputs and configuration.
- [x] JSON Schema, semantic documentation, and representative examples are updated for the new protocol version.
