---
id: TASK-26
title: Bootstrap Release Please and release 1.0.0
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-27
---

## Description

Complete the Release 1.0.0 milestone by establishing automated releases with Release Please and bootstrapping the first stable SQL Semantic Protocol release.

The application and protocol are one versioned product. Release Please must manage that shared version so the Cargo package version, emitted `protocol_version`, active protocol contract, Git tag, and GitHub release remain aligned. Conventional Commits are the release-classification input.

The bootstrap release is `1.0.0`. After 1.0.0, a breaking protocol change requires a major release. Backward-compatible protocol or application features use a minor release, and compatible fixes or internal application changes use a patch release. Other breaking public API changes follow normal SemVer as well.

## Acceptance Criteria

- [ ] Release Please is configured for the Rust application and creates release PRs from Conventional Commits.
- [ ] The initial Release Please bootstrap targets version `1.0.0`.
- [ ] The Cargo package version and emitted `protocol_version` are guaranteed to be identical.
- [ ] The active protocol schema/documentation version is kept aligned with the application release version.
- [ ] The release workflow creates the matching Git tag and GitHub release.
- [ ] Release classification treats breaking protocol changes as major, backward-compatible features as minor, and compatible fixes/internal changes as patch releases.
- [ ] CI verifies the version invariants needed to prevent application/protocol release drift.
- [ ] The Release 1.0.0 milestone is ready to close only after all preceding milestone tasks are complete and the 1.0.0 release has been produced.
