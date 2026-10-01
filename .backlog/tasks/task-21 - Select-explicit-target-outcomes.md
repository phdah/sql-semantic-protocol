---
id: TASK-21
title: Select explicit target outcomes
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-15
---

## Description

Allow callers to request one or more specific named output relations instead of always returning every terminal dataset or every layer.

Target selection is a presentation/root-selection concern, not a pruning shortcut for analysis. The analyzer must still resolve and compose every supplied upstream transformation required to describe the requested targets correctly.

This is especially useful when a large SQL bundle contains many independent pipelines but the caller only needs the semantic protocol for a small subset of final relations.

## Acceptance Criteria

- [ ] The public API accepts zero or more explicit target relation identifiers.
- [ ] The CLI exposes a repeatable documented target option such as `--target <relation>`.
- [ ] With no explicit targets, existing final/all-layer behavior is unchanged.
- [ ] In final mode, explicit targets replace automatic terminal-dataset selection.
- [ ] In all-layer mode, the protocol exposes the selected targets and every in-bundle ancestor needed to produce them.
- [ ] Target resolution uses the same qualified identifier semantics as cross-query relation linking.
- [ ] Unknown targets produce explicit input/configuration errors rather than empty output.
- [ ] Ambiguous targets produce explicit ambiguity diagnostics or errors rather than arbitrary selection.
- [ ] Multiple unrelated targets can be selected in the same invocation.
- [ ] Tests prove that selecting a deep target preserves complete transitive lineage while unrelated graph components are omitted from the exposed result.
