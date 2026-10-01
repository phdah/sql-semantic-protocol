---
id: TASK-24
title: Add catalog-aware relation resolution
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-13
---

## Description

Add optional catalog/schema-aware relation resolution for bundles where textual SQL identifiers are insufficient to decide whether two references identify the same relation.

The core analyzer must remain usable without external metadata. Catalog information is an optional resolution input that can canonicalize identifiers, apply default catalog/schema context, and disambiguate partially qualified names.

Resolution must remain deterministic and conservative. External metadata may resolve identity, but it must never invent query semantics that are not present in SQL or explicitly supplied metadata.

## Acceptance Criteria

- [ ] The public API defines an optional parser-independent relation-resolution/catalog input.
- [ ] Callers can supply default catalog and schema context per input.
- [ ] Fully and partially qualified relation references can resolve to a canonical relation identity when metadata is sufficient.
- [ ] Resolution honors quoted identifier semantics and dialect-specific identifier normalization without unsafe global case folding.
- [ ] The resolver can distinguish same-named relations in different catalogs/schemas.
- [ ] Missing metadata falls back to the existing conservative textual resolution behavior.
- [ ] Conflicting or ambiguous metadata produces explicit diagnostics/errors rather than arbitrary linking.
- [ ] The analysis layer depends on a small resolver contract rather than on a specific database/catalog implementation.
- [ ] Tests cover default schemas, multi-schema ambiguity, quoted names, and cross-input linking.
