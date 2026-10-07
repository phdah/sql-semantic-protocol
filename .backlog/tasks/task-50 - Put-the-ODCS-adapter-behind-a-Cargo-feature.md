---
id: TASK-50
title: Put the ODCS adapter behind a Cargo feature
status: To Do
assignee: []
created_date: '2026-10-07 18:11'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-35
priority: low
type: chore
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
The ODCS adapter (TASK-35) added `saphyr` as an unconditional dependency, which brings roughly eight transitive crates into every consumer, including consumers that never read ODCS (sql-tdg). AGENTS.md requires a minimal dependency footprint and enabling only the features actually used.

## Outcome
The ODCS adapter and its YAML dependency sit behind a Cargo feature, so consumers that do not need ODCS can opt out. The default feature set is the maintainer's decision, documented in the README.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The ODCS adapter and saphyr compile only when their feature is enabled
- [ ] #2 Building with the feature disabled succeeds and drops saphyr from the dependency tree
- [ ] #3 CI checks both feature configurations
- [ ] #4 README documents the feature and the default
<!-- AC:END -->
