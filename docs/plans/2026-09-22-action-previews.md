# Action Validation, Preview, and Folder Merge

> Status: proposal.
> Captured: 2026-09-22
> Owns: validate and preview (preflight) for actions, the shared plan vocabulary
> for filesystem mutations, plan handles and overlay listings, and the
> recursive folder merge operation
> Register: `PROJECT_STATUS.md`
> Companions: `docs/core/product-direction.mdx` (preview, commit, and
> verification as a product promise), `docs/core/design/source-durability.md`
> (the content id ladder), `2026-09-15-source-runtime-reliability.md` (R6 read
> routing that index-based previews depend on)

## Outcome

Every operation that changes the filesystem can answer two questions before it
runs. Validate answers "may this run, and how will it run": permissions,
capacity, safety policy, and execution facts, cheaply enough to ask
continuously. Preview answers "what will exist afterward": a projection of
filesystem state computed from the index, browsable as a virtual tree through
the same listing query the present tree uses. Both take the action's exact
input, both forward to paired devices, and execution re-runs validation on the
server and refuses on errors. The first operation built on both is
folder merge: merge one folder into another, recursing into matching
subfolders, skipping files whose bytes are proven identical, and resolving
name collisions by a policy chosen after seeing the plan.

## Principle: the operation is the unit

The action or query is the unit of Spacedrive behavior. Clients render and
dispatch; they do not compute. The CLI abstracts registered operations and the
two preflight methods, and nothing else: its local conflict detection
(`check_for_simple_conflicts`, `apps/cli/src/domains/file/mod.rs:109-188`) and
its confirmation plumbing are deleted, replaced by rendering validation
findings and plans. The interface follows the same rule:
`FileOperationModal`'s client-side conflict math is replaced by preflight.
This is also the agent contract. A skill or MCP client gets the same three
verbs, validate, preview, execute, which is what makes handing an agent
destructive file operations sane: it validates, shows the plan, then commits
the identical input.

## Where it stands

The preview promise exists in the product documents and half-exists in the
code. `LibraryAction` has a `validate()` hook returning `ValidationResult` and
`ConfirmationRequest` (`core/src/infra/action/mod.rs:21-44`), but it was
never finished:

- `ActionManager::validate_library` (`core/src/infra/action/manager.rs:126`)
  has no caller and no wire method.
- `dispatch_library` (`manager.rs:94-108`) returns an error for any action
  whose validation asks for confirmation, so the mechanism is unreachable from
  every client.
- `FileCopyAction` is the only implementor in the tree.
- Clients grew workarounds: the CLI's local conflict detection and the
  interface's modal both reimplement what the server should answer.

Copy has the merge hazard this plan fixes. Conflict detection runs only at the
top level: for a source folder it checks whether `dest/<name>` exists and
counts the whole tree as one conflict (`copy/action.rs:481-513`). `Skip` and
`AutoModifyName` apply to the entire folder (`copy/job.rs:501-555`). Inside a
directory copy there is no conflict logic at all: the walker calls
`create_dir_all` and then a truncating `File::create` per file
(`copy/strategy.rs:1006`, `strategy.rs:872`). Copying `A` onto an existing
`B/A` therefore already merges at the filesystem level today, silently
overwriting every colliding file regardless of `on_conflict`.

Duplicate knowledge is two-tier and the tiers matter. `ContentId::Candidate`
comes from a sampled hash truncated to 16 hex characters;
`ContentId::Confirmed` comes from a full-byte BLAKE3 integrity hash
(`crates/store/src/content.rs:33-69`). Files at or under 100 KB are always
fully hashed (`core/src/domain/content_identity.rs:109`). There is no two-path
byte-equality API; equality is composed from size, candidate id, and
`generate_integrity_hash` on both sides.

The planning reads exist. `Arena::entries_beneath` and `files_in_scope` list a
subtree from memory, `sd_store::read::children_of` and `contents_beneath`
answer from a store, and R6 routing picks one backend per source. A preview
never needs to walk a disk that the index already covers. Cheap aggregates
exist too: `CopyDatabaseQuery` already answers size and count estimates from
arena rollups without touching the filesystem.

Remote execution exists. `remote_ops::call` forwards one Wire method to a
paired device and executes it through the same registries
(`core/src/service/network/protocol/remote_ops.rs`), so registered validate
and preview methods forward with `--device` like any other.

In the interface, a drop always opens `FileOperationModal` with the operation
hardcoded to copy (`DndProvider.tsx:352`); copy versus move is chosen inside
the modal. Modifier keys during drag are read nowhere, and dnd-kit does not
deliver modifier state at drag end. Folder drop targets exist only in
GridView. The clipboard is a zustand store holding `operation`, `files:
SdPath[]`, and `sourcePath` (`hooks/useClipboard.ts`), and Paste always
targets the current directory rather than a right-clicked folder.

One operation already takes a selection rather than a list. `files.delete`
accepts `DeleteTargets::Comparison`, the same `Comparison` that
`paths.compare` pages, and the job derives the set from the index as it runs
(`core/src/ops/files/delete/compared.rs`): it drains the compare `Matcher` in
key order, checkpoints the cursor after each batch, and for `both` reads
whichever side lacks an integrity hash before removing a file, writing what it
learned back to the stores. Pairs whose bytes differ or whose copy in B is gone
by then are skipped and reported. `sd file delete A --against B --show both`
is the CLI. Compare itself matches on the sampled hash, the rung every store
keys a content by, with integrity hashes deciding where both sides have them,
so a copy verified on one side still matches its unread twin.

V1 has landed: `ValidatedAction` and `PreviewableAction` live in
`core/src/infra/action/preflight.rs` with `Validation`, `Finding`, `Severity`,
`ExecutionFacts` and the read-only `PreviewContext`; `register_validate!` and
`register_preview!` put `validate:<name>` and `preview:<name>` on the wire
through their own inventory registries, the daemon routes them beside
queries, and `handle_library_action` runs a registered validator again over
the payload as sent, refusing on an error finding with the findings as JSON
behind a `refused:` prefix (`Validation::from_refusal`, `RefusedError` in the
TypeScript client). The generator emits `LibraryValidate` and
`LibraryPreview` unions and `WIRE_METHODS.libraryValidates` and
`.libraryPreviews`, with `useLibraryValidate` and `useLibraryPreview` hooks
and `CoreClient::validate` and `preview` in the Rust client. A probe action
in the preflight tests answers both methods over the wire, is refused on an
error finding before it runs, and reads a store without moving its revision.

V2, V3 and V4 have landed as `core/src/ops/files/merge/` and
`core/src/ops/files/plan.rs`. `files.merge` answers both methods: validation
stats the roots, estimates from the arena, checks free space, and counts the
assertions a cross-volume consuming merge would strand; the preview streams
source against destination through the compare engine, directories and
symlinks included, into an `FsPlan` whose basis names the store revisions it
read. The `FolderMergeJob` walks the live tree in one deterministic order,
proves duplicates by reading both sides, applies the policy, leaves conflicts,
prunes consumed sources, checkpoints its cursor, and marks each outcome that
differs from the plan it re-read at start. `sd file merge <sources> --into
<dir>` renders findings, facts and the plan, stops at `--dry-run`, and
dispatches the same input. Remote execution (V5) needs nothing of its own:
preflight and the action forward with `--device` like every other method.
The plan reads the index only; a source outside every tracked source warns
and executes without a plan to diverge from, so `PlanBasis::Filesystem` has
no implementation yet.

V8 has landed. Copy, move, and delete answer both methods
(`core/src/ops/files/copy/preflight.rs`, `core/src/ops/files/delete/preflight.rs`):
copy validates sources, the destination, the cycle, free space, and names a
folder collision; a move says whether it is a rename in place
(`move.atomic`, decided by the filesystem the OS reports) or a copy across
volumes with a count of the assertions that stay behind
(`move.identity_loss`); delete refuses what it cannot reach and warns how
many of the files are the last copy of their content anywhere in the
library (`delete.last_copy`), counted across every store by sampled hash.
Copy's plan reuses the merge planner, so a folder landing beside a folder of
its name shows file by file what it would replace, and a move plans as a
consumed pair. The old hook is gone: `ValidationResult`,
`ConfirmationRequest`, `validate`, `resolve_confirmation`, and
`validate_library` no longer exist; the actions that used `validate` for
structural checks do them in `from_input`. The CLI's `file copy` and `file
delete` render preflight like `file merge`, with `--dry-run`, and its local
conflict detection is deleted. In the interface, `FileOperationModal`
validates and previews the exact input it dispatches, gates confirm on an
error finding, and offers Merge as a third choice when a folder lands on a
same-name folder.

V6 and V7 have landed: `MergeFoldersModal` renders findings, facts, the
plan, the policy picker, and the consume switch, re-validating as they
change; "Merge into '<name>'" appears on a folder when the clipboard holds
only folders, consuming them after a cut; a window-level modifier tracker
(`hooks/useModifierKeys.ts`) lets an Option-drop of folders onto a folder
open the merge dialog directly; and a grid drop target validates the drop
while a drag hovers it and badges an error or warning.

V9 has landed. A preview retains its plan under a handle in memory
(`PlanHandles` on `CoreContext`, ten minutes past last use);
`files.directory_listing` takes `overlay` and answers the directory after
the plan, with ghost rows for what it creates and an `overlay` list of the
change per row, and shows a consumed source's files leaving. The explorer's
preview mode (`hooks/usePlanPreview.ts`, entered from either dialog's
"Browse the result") passes the handle with every listing, marks rows
ghosted, badged, dimmed, or struck, and rebuilds the preview from the same
input when the handle lapses.

## Design

### Two questions, two methods

Validate and preview are different questions and stay separate.

**Validate** is about the operation: addressing correctness, permissions,
capacity, safety policy, and how execution would run. It reads rollups and
registry state, so it stays near constant time. That cheapness is what lets
the interface ask it continuously: enabling the confirm button as options
change in a dialog, or badging a drag target on hover with "read-only
replica" or "not enough space". Validation also has a runtime role: the
dispatcher re-runs it at execute and refuses on errors. Enforcement lives on
the server; the conversation about warnings lives in the client.

**Preview** is about the world: the state of the filesystem after the
operation, projected from the index. Its cost is proportional to the affected
tree, it is always advisory, and its output is a diff that can be browsed as
a virtual filesystem rather than only read as a list.

Both take the action's exact input type. A client builds one input, validates
it, previews it, and dispatches the identical payload. Nothing drifts between
what was shown and what runs.

### Validation

```rust
pub trait ValidatedAction: LibraryAction {
	async fn validate(input: &Self::Input, ctx: &PreviewContext) -> Result<Validation>;
}

pub struct Validation {
	pub findings: Vec<Finding>,
	pub facts: ExecutionFacts,
}

pub struct Finding {
	pub severity: Severity,        // Error | Warning | Info
	pub code: String,              // stable, machine-readable
	pub message: String,
	pub path: Option<SdPath>,
}

pub struct ExecutionFacts {
	pub executes_on: String,       // device slug
	pub strategy: Option<String>,  // reflink, atomic rename, stream, remote
	pub estimated_files: Option<u64>,
	pub estimated_bytes: Option<u64>,
	pub free_space_after: Option<i64>,
}
```

`register_validate!(FileMergeAction, "files.merge")` adds a
`validate:files.merge` wire method through the same inventory pattern as the
other registries. The rules:

- **Validate is a read** with no write handles, like preview.
- **Errors block, warnings inform.** At dispatch, an action that registered a
  validator is validated again on the server; any `Error` finding refuses the
  execution and returns the findings structured. `Warning` and `Info` never
  block. There is no confirmation state machine on the server: the
  "confirmation" is the client showing findings and the user dispatching
  anyway.
- **Errors are scoped.** A finding can block execution while leaving preview
  meaningful. A merge whose source origin is detached validates with an error
  for execution, and its preview still answers from the store. You can plan a
  merge onto a drive that is in a drawer.
- **Findings carry stable codes** so clients and agents can branch on them
  rather than parsing messages.

### Preview

```rust
pub trait PreviewableAction: LibraryAction {
	type Plan: Serialize + Type;

	async fn preview(input: Self::Input, ctx: &PreviewContext) -> Result<Self::Plan>;
}
```

`register_preview!(FileMergeAction, "files.merge")` adds `preview:files.merge`
next to `action:files.merge.input`. The dispatcher gains one arm per method; the
TypeScript client gains `validates` and `previews` method maps with hooks.

Plans are typed per action. Filesystem-mutating actions share `FsPlan`;
actions whose effects are not filesystem state, tag operations for example,
can opt in with their own plan types. The contract:

- **Preview is a read.** `PreviewContext` exposes the index, the stores, and
  the volume registry, and no write handles.
- **Preview is advisory.** The filesystem is live, so a plan computed now can
  be stale at execution. No token or lease pretends otherwise. The job applies
  the same policy per leaf at execution time and reports divergence from the
  plan in its output.
- **Opt-in only.** Both methods are separate traits. Actions without a
  meaningful plan or validation simply have no method registered.

### The plan vocabulary

```rust
pub struct FsPlan {
	pub basis: PlanBasis,
	pub summary: FsPlanSummary,
	pub changes: Vec<PlannedChange>,
	pub truncated: bool,
}

pub enum PlanBasis {
	Index { revisions: Vec<(SourceId, i64)> },
	Filesystem,
}

pub struct PlannedChange {
	pub path: SdPath,
	pub change: ChangeKind,
}

pub enum ChangeKind {
	Create { size: u64 },
	Replace { existing_size: u64, incoming_size: u64 },
	MergeInto,
	Skip { reason: SkipReason },
	Move { from: SdPath },
	Delete { last_copy: bool },
	Conflict { kind: ConflictKind },
}

pub enum SkipReason {
	DuplicateCandidate,
	DuplicateConfirmed,
	Junk,
	Policy,
}
```

`FsPlanSummary` carries complete counts and byte totals per change kind. The
change list is bounded: conflicts are always included up to a cap, other
entries fill the remainder, and `truncated` says whether detail was dropped.
The summary is never truncated.

Two honesty rules keep an index-based preview truthful:

1. **The plan names its basis.** A plan computed from the index carries the
   store revisions it read. A plan that had to walk the filesystem says so.
2. **The plan never overstates a hash.** At preview time the index mostly
   holds sampled hashes, so a duplicate can only be
   `Skip { DuplicateCandidate }`. The job escalates to the integrity hash
   before actually skipping, and the result reports `DuplicateConfirmed`. A
   candidate hash alone never justifies skipping a file, matching the rule in
   `crates/store/src/content.rs`: destructive decisions require the confirmed
   tier.

### Browsing the future: plan handles and overlay listings

A flat change list is a report. Browsing the projected filesystem is the
primitive. The mechanic:

- A preview whose plan is an `FsPlan` is retained by the daemon under a
  **plan handle**, an id with a short TTL refreshed on use, in memory only. A
  lapsed handle is rebuilt by previewing again.
- `files.directory_listing` accepts an optional `overlay: plan_id`. A listing
  served through an overlay is the directory as it would look after the
  operation: planned creates appear, replaced files carry their incoming
  size, skips and deletes are marked per row.
- The explorer gets a preview mode: navigation works exactly as normal, every
  listing passes the overlay, and rows render ghosted, badged, dimmed, or
  struck. You walk the future filesystem with the same query the present one
  uses. The overlay pattern has precedent in the tree: queued tag writes
  already overlay replica listings before the owner acks.
- Execute never takes a handle. The handle is a lens; the action's input is
  the contract.

### Preflight across operations

What validate and preview each mean, operation by operation. Copy, move,
merge, and delete are current or in this plan; batch rename and dedupe are
future operations that adopt preflight as separate work.

**Copy**

- Validate: sources resolve; the destination is a writable directory; a
  folder is never copied into its own descendant; free space on the
  destination volume against bytes needed, answered from arena rollups;
  cross-device requires the devices paired and reachable; a replica
  destination is an error, writes belong to the owner. Facts: the strategy
  the router would pick (reflink, atomic, stream, remote), where it executes,
  estimated files and bytes.
- Preview: per-path `Create` and `Replace` entries; the destination tree as
  it would look; incoming files whose content already exists elsewhere in the
  library flagged through their content ids.

**Move**

- Validate: everything copy checks, plus the fact that decides how the
  operation feels: same volume means an atomic rename with record identity
  preserved; cross volume means copy then delete, with a warning carrying
  real numbers, "47 tagged records lose their identity."
- Preview: both sides of the world after: the source tree gone, the
  destination tree grown.

**Merge**

- Validate: the destination exists and is a directory; no nesting between
  the roots; free space for the non-duplicate delta only; consume mode across
  volumes raises the stranded-assertions warning with counts; a detached
  origin is an execution error while preview stays available from the store.
- Preview: the full plan, and two browsable virtual trees: the destination
  after the merge, and, in consume mode, what the source would still hold,
  exactly the unsettled conflicts. Duplicate skips labeled candidate-tier
  until execution confirms them.

**Delete**

- Validate: trash versus permanent capability for the target filesystem or
  provider; permission; and the aggregate warning only Spacedrive can make:
  "3 of these files are the last copy of their content anywhere in your
  library."
- Preview: the tree without them, freed bytes, and per-row `last_copy` flags
  so the user sees which three before committing. For comparison targets the
  preview also streams the set for its count, bytes, and how many pairs rest
  on a sampled match; that check costs a full stream, so it lives here rather
  than in validate, and the job enforces it per leaf regardless.

**Batch rename** (future, pattern-based)

- Validate: name legality per target filesystem, Windows reserved names,
  path length, illegal characters, and case-fold collisions on
  case-insensitive filesystems, which naive renamers silently eat files with.
- Preview: the listing with new names applied, collisions marked on the
  exact rows.

**Dedupe** (future, over `files.duplicates` groups)

- Validate: every group it would act on has confirmed integrity hashes;
  where only candidates exist, the finding says verification must run first,
  or the action chains a verify job. Keep-policy coherence.
- Preview: per group, which copy stays and which go, reclaimable bytes,
  browsable after-state.

### The merge operation

`files.merge` is a library action dispatching a `FolderMergeJob`, implementing
both methods with an `FsPlan`.

Input:

```rust
pub struct FileMergeInput {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	pub on_conflict: MergeConflictPolicy,
	pub consume_sources: bool,
}

pub enum MergeConflictPolicy {
	Skip,
	Overwrite,
	KeepBoth,
	KeepNewer,
}
```

The destination is unambiguously an existing directory. Merge does not inherit
copy's stat-based guessing about whether a destination is a folder or a new
name; a destination that is missing or is a file is a validation error.

Per relative path, walking source and destination together:

- Present only in the source: copy through `CopyStrategyRouter`, which keeps
  reflink, atomic, streaming, and cross-device selection per file.
- Directory on both sides: recurse. The destination directory is `MergeInto`,
  never a conflict.
- File on both sides, bytes proven identical: skip. Identity means equal size
  and equal integrity hashes, computed at execution. Files at or under 100 KB
  already carry full hashes.
- File on both sides, bytes differ: apply `on_conflict`. `KeepBoth` renames
  the incoming file with the existing `generate_unique_name` convention.
  `KeepNewer` compares modification times and the plan labels it as such,
  since mtime is a claim rather than proof.
- A file on one side and a directory on the other is always a `Conflict` and
  no policy resolves it automatically. It is reported and left alone.
- Symlinks copy as links. A link colliding with a regular file is a conflict.

With `consume_sources` on, each source leaf is removed after its copy lands or
after its bytes are confirmed identical to the destination's, and empty source
directories are pruned from the bottom up. Anything skipped by policy or left
as a conflict stays. The source folder ends holding exactly what the merge did
not settle, which makes the leftover reviewable rather than mysterious.

Job mechanics:

- Dedup key `(source_root, destination_root)`, so an impatient second dispatch
  returns the live job.
- Resume cursor is the last completed relative path in deterministic walk
  order. Copy's resume unit is the top-level source index, which restarts a
  half-copied tree; merge does not repeat that.
- The per-entry plan persists into job state the way `CopyJobMetadata` does,
  so the executed outcome, including plan divergence, is queryable afterward.
- Progress reports phases (verifying, merging, pruning) through
  `GenericProgress` like copy does.
- The job writes no records. The watcher observes the results as it does for
  copy. With `consume_sources` on across volumes, the ledger sees new records
  plus removals, so tags and assertions on the source records do not travel.
  The job counts affected assertions in its output; carrying them forward is a
  registered follow-on, with `sources.assertions.merge` as prior art.

Cross-device: when both roots live on one paired device, the client forwards
the validate, preview, and action methods to it over remote ops and the merge
runs where the bytes are. A merge whose roots span two devices is out of scope
for this plan; `RemoteTransferStrategy` is single-file and a spanning merge
plan would read two indexes.

### Merge in the interface

Three entry points, one dialog.

**The dialog** follows the `FileOperationModal` pattern: a phase machine
created through `dialogManager`. Phase one calls validate and preview and
renders both: findings, counts, bytes, the conflict list, the basis, the
policy picker, and the `consume_sources` switch. Validation re-runs as options
change, cheaply, and gates the confirm button on errors. Confirm dispatches
the same input. The job then lives in the Activity panel like any other.

**Copy, then Merge into.** With folders on the clipboard, right-clicking a
folder shows "Merge into '<name>'". The menu item uses the existing
`condition` hook in `useFileContextMenu`, and its destination is the
right-clicked folder's `sd_path`. This intentionally differs from Paste,
which targets the current directory.

**Drag.** A window-level modifier tracker feeds `handleDragEnd`, since dnd-kit
does not expose modifier state. Dropping a folder onto a folder with the
modifier held opens the merge dialog directly. Without the modifier, the drop
opens `FileOperationModal` as today, and when the drop would collide with an
existing same-name folder the modal offers Merge as a third choice next to
Copy and Move. The feature is discoverable without knowing the key; the key
just preselects it. Hover validation comes with the validate method: a drop target can
badge "read-only replica" or "not enough space" from a validate call while
the drag is still in flight. Folder drop targets currently exist only in
GridView, and widening drop support to the other views stays out of this
plan.

### Replacing the dead validation hook

Preflight replaces validation as it exists today. `ValidationResult`,
`ConfirmationRequest`, `validate`, `resolve_confirmation`, and
`validate_library` are removed. `FileCopyAction::validate`'s metadata moves
into copy's `ValidatedAction` and `PreviewableAction` impls when copy adopts
preflight. The CLI's local conflict detection and confirm plumbing are
deleted in favor of rendering preflight, per the principle above. No
compatibility layer remains.

### Out of scope

- Merges whose roots span two devices.
- Batch rename and dedupe as operations. They are specified here as future
  preflight actions and land as their own work.
- Carrying assertions across a cross-volume consuming merge (registered
  follow-on).
- Drop targets outside GridView.
- Cloud-path merges. `SdPath::Cloud` has no copy strategy today.

## Phases

| Phase | Scope | Exit proof |
|---|---|---|
| V1 | Preflight | `ValidatedAction`, `PreviewableAction`, both registration macros, the dispatcher arms, and the generated `validates` and `previews` maps exist. A test action answers both methods over the wire with the same input its action takes. Dispatching it with a forced `Error` finding refuses server-side and returns the findings structured. Preflight calls perform no writes, proven by a store revision check before and after |
| V2 | `FsPlan` and merge with preflight | `validate:files.merge` returns findings and facts for the good, nested, detached, and full-disk cases. `preview:files.merge` answers from the index for a warm source and names its basis and revisions. On a tree with known duplicates, conflicts, and junk, the summary counts match a hand count. Preview of a detached source's replica works without the drive, and the same input's validation carries the detached execution error |
| V3 | The merge job and CLI | `sd files merge <src> <dst>` renders validation and the plan before dispatch and `--dry-run` stops there; the job merges, skips only integrity-confirmed duplicates, applies the policy, and its persisted per-entry outcome matches the plan or reports divergence. Interrupting mid-merge and restarting the daemon resumes past completed leaves. A second dispatch during the run returns the live job |
| V4 | Consuming merge | With `consume_sources`, the source ends holding exactly the unsettled entries and empty directories are pruned. A conflict is never removed from the source. The output counts assertions left behind |
| V5 | Remote execution | `sd files merge --device titan` validates, previews, and merges two folders on titan without streaming file bytes to the client. The job is visible through remote job activity |
| V6 | Dialog and context menu | Copying a folder and right-clicking another shows "Merge into", the dialog renders real findings and the real plan, validation re-runs as options change and gates confirm on errors, and confirming dispatches the previewed input unchanged. The executed job appears in Activity |
| V7 | Drag | The modifier-drop opens the merge dialog. An unmodified folder drop onto a same-name collision offers Merge in `FileOperationModal`. A drop target with a validation error badges it during hover. Modifier state is read from the tracker, verified on macOS and web |
| V8 | Adoption and teardown | Copy, move, and delete implement both methods, including the cycle check, the identity-loss warning, and the last-copy warning. `FileOperationModal` and the CLI render preflight instead of local conflict math, `check_for_simple_conflicts` is deleted, and the old validation hook is gone from `core/src/infra/action` |
| V9 | Plan handles and overlay listings | A merge preview returns a handle, `files.directory_listing` with `overlay` serves the projected directory, and the explorer's preview mode browses the destination-after and source-after trees with ghosted, badged, dimmed, and struck rows. A lapsed handle rebuilds by re-previewing |

V1 and V2 land together or in sequence; nothing else starts before the preflight
shapes are real. V3 before any UI because the CLI path is the immediate need
and proves the job. V8 after two real actions use preflight so the old
one is removed against working replacements. V9 last: the dialog renders
plans directly, so overlay browsing is an addition rather than a dependency.

## Acceptance

The live case is the Expansion drive folding into `jamie-nas`. Track the drive
first, per the standing register item, so the merge preview reads from its
store. Then, on titan: validate the merge and record the findings, preview it
and check the summary against a manual sample, run a consuming merge with
policy `Skip`, and verify that the drive retains exactly the reported
conflicts and policy skips, that no destination file changed without a
`Replace` or `KeepBoth` entry saying so, and that skipped duplicates all carry
confirmed integrity hashes. Measure preview time from the index on the largest
source available and confirm neither method made a filesystem write.

## Decisions for James

1. `consume_sources` default. Proposed off in the dialog and required
   explicitly in the CLI, since it is the destructive half.
2. Default conflict policy. Proposed `Skip`: touch nothing that differs,
   report it.
3. Junk handling. Proposed: a small fixed list (`.DS_Store`, `Thumbs.db`,
   `desktop.ini`) is never copied, never a conflict, and counted as `Junk` in
   the plan. Alternative: treat junk like any other file.
4. The drag modifier. Proposed Option/Alt, since Cmd and Shift already carry
   selection meanings in the explorer.
5. Whether a consuming cross-volume merge that would strand assertions should
   warn, refuse without a flag, or proceed and report. Proposed: proceed and
   report counts, with carry-forward as the follow-on.
6. Whether warnings ever gate. Proposed: errors refuse server-side, warnings
   never block, and destructive dialogs simply render warnings prominently.
   The alternative is a `force` style acknowledgment flag for specific codes,
   which adds input surface to every action that wants it.
