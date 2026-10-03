---
id: TASK-21
title: Select explicit target outcomes
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-15
---

## Description

Allow callers to request one or more specific named output relations instead of always returning every terminal dataset or every layer.

Target selection is a post-analysis presentation/root-selection concern, not a pruning shortcut for analysis. The analyzer still resolves and composes the complete supplied bundle before the requested projection is applied.

This is especially useful when a large SQL bundle contains many independent pipelines but the caller only needs the semantic protocol for a small subset of final relations.

## Acceptance Criteria

- [x] The public API accepts zero or more explicit target relation identifiers.
- [x] The CLI exposes a repeatable documented `--target <relation>` option.
- [x] With no explicit targets, the complete-bundle behavior established by TASK-15 is unchanged.
- [x] Explicit targets are applied only after complete analysis and semantic composition.
- [x] The projected protocol exposes each selected target and every in-bundle ancestor needed to produce it.
- [x] Target resolution uses the same qualified identifier semantics as cross-query relation linking.
- [x] Unknown targets produce explicit input/configuration errors rather than empty output.
- [x] Ambiguous targets produce explicit errors rather than arbitrary selection.
- [x] Multiple unrelated targets can be selected in the same invocation.
- [x] Tests prove that selecting a deep target preserves complete transitive lineage while unrelated graph components are omitted from the exposed result.
