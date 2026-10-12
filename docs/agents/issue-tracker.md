# Issue tracker: Backlog.md

Issues, specs, and tickets for this repo live as tasks in the local Backlog.md board under
`.backlog/`. The board is plain Markdown files committed to the repo, so there are two equally
valid ways to work with it:

- **With the `backlog_*` MCP tools** (preferred when they are available): use them for every
  operation and do not hand-edit `.backlog/` files. Before creating, executing, or finalizing a
  task, call `backlog_get_backlog_instructions` with the matching `instruction` and follow it.
- **Without the MCP tools** (e.g. ChatGPT's GitHub connector or any agent that only has file
  access): read and write the task files directly, following
  [Direct file access](#direct-file-access). Missing MCP tools are not a blocker; do not wait for
  or request an MCP server.

GitHub Issues on `phdah/sql-semantic-protocol` are not used for planned work.

## Conventions

These name the MCP tools; each has a direct-file equivalent listed under
[Direct file access](#direct-file-access).

- **Create an issue**: `backlog_task_create` with `title`, `description`, `acceptanceCriteria`,
  `labels`, and `type` (`bug`, `feature`, `enhancement`, `task`, `chore`, `docs`, `spike`).
- **Read an issue**: `backlog_task_view` with the task ID (e.g. `task-24`).
- **List issues**: `backlog_task_list`, filtered by `status`, `labels`, `type`, `assignee`, or
  `search`; `backlog_task_search` for fuzzy text search.
- **Comment on an issue**: `backlog_task_edit` with `commentsAppend`.
- **Apply / remove labels**: `backlog_task_edit` with `labels` (replaces the full set, so read the
  current labels first).
- **Close**: set `status: Done` and a `finalSummary` via `backlog_task_edit`. Done tasks stay in
  `.backlog/tasks/`; do not call `backlog_task_complete` (it moves files to `.backlog/completed/`,
  which this board does not use). For won't-do items, apply `wontfix` and `backlog_task_archive`.

Task IDs are the board's own (`task-N`, subtasks `task-N.M`); there is no `#N` numbering.

## Direct file access

When the `backlog_*` tools are unavailable, operate on the files under `.backlog/` directly. Copy
the structure of an existing task file rather than inventing one.

- **Location and naming**: `.backlog/tasks/task-<N> - <Title-With-Dashes>.md`. Subtasks are
  `task-<N>.<M> - ...md`. Pick the next unused number by listing `.backlog/tasks/`,
  `.backlog/archive/tasks/`, and `.backlog/drafts/` and taking the highest existing ID, compared
  numerically (`task-100` is higher than `task-99`).
- **Frontmatter** (YAML): `id` (uppercase, e.g. `TASK-24`), `title`, `status` (`To Do`,
  `In Progress`, `Done`), `assignee` (list), `created_date` (`'yyyy-mm-dd'`), `labels` (list),
  `dependencies` (list of `TASK-N`), and optionally `updated_date`, `priority`
  (`high`/`medium`/`low`), `type`, `milestone`, `references`, and `parent_task_id` (e.g.
  `TASK-21`) for subtasks. Set `updated_date` to today whenever you edit a task.
- **Body sections**: keep the marker comments, since the Backlog.md tooling parses them:
  - `## Description` wrapped in `<!-- SECTION:DESCRIPTION:BEGIN -->` / `<!-- SECTION:DESCRIPTION:END -->`
  - `## Acceptance Criteria` wrapped in `<!-- AC:BEGIN -->` / `<!-- AC:END -->`, items as
    `- [ ] #1 ...`, checked as `- [x] #1 ...`
  - `## Implementation Notes` wrapped in `<!-- SECTION:NOTES:BEGIN -->` / `<!-- SECTION:NOTES:END -->`
  - Plan, comments, and final summary as further `##` sections in the same style as existing
    tasks.
- **Operations**: create = write a new file; comment = append to the notes/comments section; label
  = edit `labels`; claim = set `assignee` and `status: In Progress`; close = set `status: Done`
  and add a final summary, leaving the file in `.backlog/tasks/`; won't-do = add `wontfix` and
  move the file to `.backlog/archive/tasks/`.
- Commit task file changes alongside the work they describe.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## When a skill says "publish to the issue tracker"

Create a Backlog.md task with `backlog_task_create`, or write a new task file (see
[Direct file access](#direct-file-access)).

## When a skill says "fetch the relevant ticket"

Run `backlog_task_view` with the task ID, or read `.backlog/tasks/task-<N> - *.md`.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a parent task with **subtasks** as tickets.

- **Map**: a task labelled `wayfinder:map`, holding the Notes / Decisions-so-far / Fog sections in
  its description.
- **Child ticket**: a subtask of the map (`parentTaskId` via MCP, `parent_task_id` in the file).
  Labels: `wayfinder:<type>` (`research`/`prototype`/`grilling`/`task`).
- **Blocking**: the task's `dependencies` list of blocker task IDs. A ticket is unblocked when
  every dependency is Done.
- **Frontier query**: `backlog_task_list` with `ready: true` and `unassigned: true`, keeping only
  the map's subtasks that are not Done; the lowest ordinal / ID wins.
- **Claim**: `backlog_task_edit` setting `assignee` and `status: In Progress`, the session's first
  write.
- **Resolve**: `commentsAppend` the answer, close as above, then append a context pointer (gist and
  task ID) to the map's Decisions-so-far.
