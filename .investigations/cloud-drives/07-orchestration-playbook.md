# Cloud Drives MVP — Orchestration Playbook

This document captures how the assistant dispatches sub-agents, supervises their
work, and commits their output for the cloud drives MVP. It exists because
the informal approach for Sets 1 through 3 produced a concurrency incident
on 2026-04-18: two agents mutated the same files in parallel, one agent was
mistakenly presumed dead and its WIP got committed, and `.tasks/` files were
touched in violation of project conventions.

The rules below apply to every remaining set of the MVP (Set 4 through Set 8
plus any later phases). They override any ad-hoc behavior the assistant might
otherwise default to.

## Operational invariants

### Single-agent concurrency

Only one implementation agent may mutate the tree at a time. The assistant
must never dispatch a new agent on code the current agent is still editing.

If an agent's `<task_result>` is empty or the session appears idle, the
assistant must:

1. Check `mcp_Pty_list` for any PTY sessions still owned by that agent.
2. Check `git status` for ongoing edits (untracked files, modified files).
3. If either is active, the agent is not dead. Use `task_id` to resume the
   same session; do not dispatch a fresh one.
4. Only consider the agent stopped when the PTY sessions are exited AND the
   tree matches the last known good state (or a sensible terminal state).

### Never commit an agent's work without its explicit report

The assistant does not commit implementation work unless the agent has
returned a structured report (see template below). An empty `<task_result>`
is never a green light to commit; it is a signal to investigate.

The only exception is a user instruction that explicitly authorizes
committing partial state.

### `.tasks/` tree is upstream-only

Files under `.tasks/` are maintained by the project founder as part of the
upstream tracker. Feature branches must not edit them routinely. Completed
scope is communicated through commit messages and the PR description,
leaving it to the founder to accept task updates.

If a task update is genuinely beneficial and unavoidable, it lands in a
dedicated commit, clearly messaged, that the founder can drop at merge if
desired. It never hides inside a feature commit.

New task proposals authored during the MVP go under
`.investigations/cloud-drives/` as proposals, not into `.tasks/`.

### Windows hygiene

- Never `git add -A` or `git add .`. The repo contains a reserved `nul`
  file that breaks these commands. Always add explicit paths.
- After any `cargo build` or `cargo test`, `git status` may list phantom
  CRLF-only diffs in unrelated `.rs` files. Restore them with
  `git checkout -- <paths>` before staging. Never commit CRLF-only
  artefacts.
- PowerShell has no heredoc support. Multi-line commit messages go through
  a temp file and `git commit -F <file>`.
- The `rtk` git wrapper swallows `stash@{0}` syntax; always quote it.

### Cargo.lock and generated types

- `Cargo.lock` is tracked and must be committed alongside any
  `Cargo.toml` change. The CI uses `--locked`.
- Every Rust type that carries `#[derive(Type)]` and is reachable from a
  registered action or query must be regenerated with
  `cargo run --bin generate_typescript_types`. The resulting
  `packages/ts-client/src/generated/types.ts` is committed with the Rust
  change.

## Agent dispatch template

Every agent prompt for an implementation set follows this structure. The
assistant fills in the set-specific details but never omits a section.

```
## Required reading (in order, before writing any code)

1. .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md — section for this set
2. .investigations/cloud-drives/07-orchestration-playbook.md — this file
3. .investigations/cloud-drives/<relevant research doc>
4. <Rust source files the agent will extend — list the exact paths>

## Scope (must-do, nothing more)

<Concise description of what this set delivers.>

### Files to create

<Explicit list with expected size and responsibility.>

### Files to modify

<Explicit list of the existing files that must be touched.>

### Files NOT to touch

- Any file under .tasks/ (upstream tracker; proposals go in .investigations/).
- Any file outside the set's listed scope unless specifically enabling it.

## Acceptance criteria

<Bullet list of concrete, testable outcomes.>

## Tests to write

<Bullet list of test functions the agent must add.>

## Process and hygiene

- Work on branch feature/cloud-drives-investigation. Do not switch branches.
- After every cargo build, check git status for CRLF-only diffs on
  unrelated .rs files and restore them via
  `git checkout -- apps/ core/ crates/ xtask/` (excluding your own changed
  files).
- Never `git add -A` or `git add .`. Always add explicit paths.
- Do not touch any file under .tasks/.
- Do not commit. The orchestrator commits after reviewing your report.
- Follow AGENTS.md: tabs, `///` docs on every public item, `thiserror` for
  errors, `tracing` macros never `println!`, `tokio` never `std::sync`.

## Report back (final message)

When done, return a concise report with:

1. Files created and files modified (exact paths, line counts).
2. `git diff --stat` against HEAD (do not commit).
3. Test count before → after.
4. Build, clippy, test, fmt, task-validator outcomes.
5. Any scope deviation encountered, with root-cause explanation.
6. Any new TODOs added (file:line + text).
7. Any blocker or open question for the next set.

Do not write a plan document or meta-commentary; execute and report.
```

## Orchestrator workflow per set

1. **Pre-dispatch**
   - `git status --short` is clean (only `nul` untracked is permitted).
   - No PTY sessions running. `mcp_Pty_list` is empty.
   - The last commit is the agreed tip.
2. **Dispatch**
   - Exactly one `mcp_Task` call for the current set.
   - Prompt follows the template above, with set-specific details.
3. **Supervise (passive)**
   - Do not poll the agent's PTYs or files unless the user asks for status
     or a notification arrives.
   - Do not dispatch any other implementation agent until this one returns.
4. **Receive report**
   - The agent's final message must contain the structured report sections.
   - If the report is empty or missing sections, resume the same session
     with `task_id` and ask for the missing parts.
5. **Verify**
   - `git diff --stat HEAD` matches the file list in the report.
   - No file under `.tasks/` is touched.
   - `git status` shows only the expected paths (plus `nul`).
   - Run `cargo build -p sd-core --lib`, `cargo test -p sd-core --lib`,
     `cargo clippy -p sd-core --lib --no-deps`, in that order, and check
     each exits 0.
   - Regenerate TS types if the set added or modified any `#[derive(Type)]`
     reachable from an action or query. Stage the resulting diff.
   - `cargo run -p task-validator -- validate` exits 0.
6. **Checkpoint with the user**
   - Post a concise summary: commit message proposal, files list, test
     delta, any deviations.
   - If the diff is large (>1000 LOC) or touches unexpected files, ask
     the user to confirm before committing.
7. **Commit**
   - One commit per set. Explicit path `git add`. Message via temp file.
   - Subject line: `cloud: <short scope> (Set N)`.
   - Body: files created, files modified, test delta, deviations, TODOs.
8. **Post-commit**
   - `git show --stat <hash>` and eyeball the changes.
   - If CRLF artefacts slipped in, amend-after-restore is permitted only
     for artefacts (per AGENTS.md amend rules).
   - Clean up temp files (`.tmp-commit-msg.txt`).

## Decision points that require user input

The assistant does not decide alone on these; it asks the user:

- Any scope deviation that changes the deliverable or moves a TODO out of
  scope.
- Large diffs (>1000 LOC) even when the scope is clean.
- Any disagreement between the plan and the actual code (e.g. the
  `#[serde(flatten)]` mismatch that only surfaced at integration time).
- Architecture choices not explicitly resolved in the plan (schema shape,
  naming, API boundaries).

## Incident log

For reference when reviewing the MVP later.

### 2026-04-18 — Set 3 / Set 4 concurrency incident

- Set 3 sub-agent returned an empty `<task_result>` while still working.
- Assistant interpreted the empty result as a crash, verified the tree
  state looked plausible, and committed the WIP as `391e963e3`.
- Assistant then dispatched a Set 4 agent.
- The Set 3 agent was still alive and continued editing files, including
  `core/src/ops/cloud/oauth/flow.rs` to remove an incorrect
  `#[serde(flatten)]` attribute.
- Set 4 agent created `core/src/ops/cloud/oauth/providers/onedrive.rs`.
- Set 3 agent reverted some of Set 4's edits, believing them foreign.
- User killed both sessions to stop the chaos.

Direct fallout:

- `391e963e3` shipped with the incorrect `#[serde(flatten)]` attribute.
  Fixed by `67c3e179a`.
- `providers/onedrive.rs` and its `providers/mod.rs` were left on disk,
  the latter missing, making the tree not build. Both were deleted or
  never staged.
- `.tasks/` files were touched across Sets 1 through 3. Cleaned up by
  `2e8f048dc`.

Lessons codified in the invariants above.
