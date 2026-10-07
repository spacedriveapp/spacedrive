# File Operations on Preflight: Rename, Undo, Mirror, Organize, Archive

> Status: landed 2026-09-23; the acceptance case runs as CI tests
> (`docs/core/acceptance/entries-drop-and-file-operations.md`, 2026-10-07),
> with Linux trash restore failing; the Expansion drive run stays open.
> Captured: 2026-09-23
> Owns: the file operations a file manager is expected to have that
> Spacedrive lacks or has without preflight: delete and duplicate in the
> explorer, rename and batch rename, the operation journal with undo and
> restore, one-way mirror, organize and flatten, archive and extract,
> attributes and links, the trash view
> Register: `PROJECT_STATUS.md`
> Companions: `2026-09-22-action-previews.md` (preflight, `FsPlan`, plan
> handles and overlay listings, which every operation here builds on),
> `docs/core/ops.mdx`, `docs/core/product-direction.mdx` (preview, commit,
> and verification as a product promise)

## Outcome

Every operation a person expects of a file manager exists as a registered
action with validate and preview over its exact input, plans through the one
`FsPlan` shape so the explorer's preview mode browses it, derives its set
from the index when it runs, reads a file before removing it, and records
what it did so it can be undone where the filesystem allows. After this plan
the explorer has no operation that skips preflight, and the list of missing
operations is empty: batch rename, undo and restore, mirror, organize and
flatten, archive and extract, attributes and links, duplicate in place, and
the trash.

## Principle: one plan shape, one journal

The previous plan made the operation the unit: clients render findings and
plans and dispatch the identical input. This plan adds two rules for every
operation in it.

- **It plans as an `FsPlan`.** Rename is a `Move` within a directory,
  extract is a set of `Create` rows read from the archive's directory,
  organize is `Move` rows into `CreateDirectory` rows. The vocabulary in
  `core/src/ops/files/plan.rs` grows only where a change has no existing
  kind. Nothing gets a plan shape of its own.
- **It writes a journal.** Each job records what it did as effects with
  enough to reverse them: created, moved from and to, trashed from and where
  it went, replaced and where the previous bytes went, removed. Undo is an
  action over a job's journal, on preflight like everything else, and it
  refuses where the files changed since.

## Where it stands

What exists is in `docs/plans/2026-09-22-action-previews.md`: copy, move,
merge, delete and dedupe on preflight, compare, the duplicates, at-risk and
compare-volumes views, and the CLI and dialogs that render them. What is
missing or off preflight:

- **Delete in the explorer** goes through `useDeleteFiles`
  (`packages/interface/src/routes/explorer/hooks/useDeleteFiles.ts`), a
  browser `confirm()` that dispatches `files.delete` without validate or
  preview, so the last-copy warning delete's validation makes is never
  shown. `DedupeModal` already renders `files.delete` through preflight for
  any target kind.
- **Duplicate in place** has a keybind, `explorer.duplicate` at
  `packages/interface/src/util/keybinds/registry.ts:59`, and no handler in
  `useExplorerKeyboard`.
- **Rename** (`core/src/ops/files/rename/`) checks the name in `from_input`
  with `validate_filename` and runs as a move through
  `FileCopyJob::new_rename`. It has no preflight, so a case-fold collision
  on a case-insensitive drive, a name a target NTFS or exFAT volume rejects,
  and an existing file at the new name are found by the job. There is no
  batch rename.
- **Trash** is `trash::delete` (crate `trash` 3.3,
  `core/src/ops/files/delete/strategy.rs:216`), which does not report where
  the item went, and fails on volumes without an OS trash, network mounts
  among them. Nothing restores, nothing empties, and nothing records what an
  operation did in a form that can be reversed. `DeleteMode::Secure` exists
  in the strategy and nothing dispatches it.
- **Mirror**, making B match A, is merge with overwrite plus the removal of
  what A lacks. Both halves exist as separate operations and there is no
  single previewable one.
- **Organize and flatten, archive and extract, attributes and links** do
  not exist.

## Design

### Delete and duplicate in the explorer

`useDeleteFiles` is replaced by the dialog `DedupeModal` already is: a
`files.delete` dialog over any `DeleteTargets`, renamed `DeleteModal` and
opened by the context menu, ⌘⌫ and ⌥⌘⌫. It shows the findings, the last-copy
warning with its count, the plan's removed rows, and the trash or permanent
switch preset by the shortcut. Confirm is gated on an error finding, and the
keyboard path and the menu path share one hook.

Duplicate in place is `files.copy` with the destination set to the file's own
directory and `on_conflict` set to keep both, so the copy is written beside the
original with a numbered name and its plan shows the one `Create`. ⌘D and a
"Duplicate" menu item dispatch it after the same preflight the copy modal
runs, without the modal, since there is nothing to choose.

### Rename on preflight, and batch rename

`files.rename` gains both methods. Validation reads the target volume's
filesystem from the volume registry (`Volume::file_system`) and answers:

- `rename.illegal_name`: a character or name the filesystem rejects, NTFS
  and exFAT reserved names (`CON`, `NUL`, trailing dots and spaces), and
  length past the filesystem's limit.
- `rename.exists`: a file already at the new name.
- `rename.case_collision`: on a case-insensitive volume, an existing name
  that differs only in case, which a naive rename silently eats.
- `rename.case_only`: info, a rename that changes only case, which the job
  does through a temporary name on a case-insensitive volume.

The preview is one `Move { from }` row, so the overlay shows the renamed row
ghosted at its new name and struck at the old.

Batch rename is a new action, `files.rename_batch`, over a list of paths and
an ordered list of rules applied to each name:

- `Replace { find, with, regex: bool }`, over the stem or the whole name.
- `Case { stem: Lower | Upper | Title, extension: Lower | Keep }`.
- `Affix { prefix, suffix }`.
- `Sequence { start, step, pad }`, substituting `{n}` in a template.
- `Template { pattern }` with tokens `{name}`, `{ext}`, `{n}`, `{parent}`,
  `{date:%Y-%m-%d}` from the modification time and, where the store holds
  it, the captured time.

Validation runs the single-rename checks for every target and adds
`rename.collision` for two targets that want one name. The preview is a
`Move` row per changed name and a `Conflict { kind: Sources }` row per
collision, so the listing shows every new name before anything moves. The
job renames in an order that never overwrites, using temporary names where a
chain of renames would, and journals each rename. The dialog lists the rules,
edits them in place, and re-previews on every change; the CLI is
`sd file rename <paths>... --replace a b --regex --case lower --sequence
"IMG_{n:04}" --prefix --suffix --template`.

### The operation journal, undo, and restore

Every job that changes the filesystem writes effects as it goes, stored with
the job record it already has:

- `Created { path }`
- `Moved { from, to }`
- `Trashed { from, to: Option<PathBuf> }`, `to` known where the platform
  reports it
- `Replaced { path, previous: Option<PathBuf> }`, `previous` where the old
  bytes were stashed
- `Removed { path }`, permanent, not reversible

Merge's per-leaf outcomes become effects; copy, move, rename, delete, dedupe,
organize, flatten, extract and archive write theirs. The journal is the
job's output, not new durable file state, so it lives with jobs in the
library database.

`files.undo { job }` is an action on preflight. Its preview is the reverse
as an `FsPlan`: a `Delete` for each `Created`, a `Move` back for each
`Moved`, a `Move` from the trash location for each `Trashed` whose location
is known, a `Move` back of the stashed bytes for each `Replaced` with a
`previous`. Validation refuses an effect whose subject changed since the job
ran (`undo.changed`: size, modification time, or hash no longer match what
the journal recorded) and reports what cannot be reversed
(`undo.irreversible`: permanent removals, replacements without a stash,
trashed items whose location is unknown). An undo job writes its own
journal, so undoing an undo is the same action again. The job list gets
"Undo" on a completed job, and ⌘Z in the explorer undoes the most recent
reversible job on this device.

Trash locations: on Windows and Linux the `trash` crate lists and restores
(`trash::os_limited`). On macOS the crate reports nothing, so the daemon
calls `NSFileManager trashItemAtURL:resultingItemURL:` through
`objc2-foundation` and records the resulting location. On a volume with no
OS trash, today's failure becomes a Spacedrive trash directory at the
volume's root (`.spacedrive/trash/<job>/`), an atomic rename on the same
volume, with the original path in the journal. Replacements stash the
previous file the same way the trash does, so an overwrite is undoable until
the trash is emptied and costs a rename rather than a copy.

### Mirror

Mirror is merge with the destination made to match: a `remove_extras`
option on `files.merge` rather than a new action, so one dialog and one CLI
command carry it. With the option on, the plan adds a `Delete` row for every
path the destination holds that no source does, from the compare engine with
the sides reversed, each flagged `last_copy` from the same lookup delete uses,
and validation adds `merge.last_copies` with the count. The job runs the
merge, then the comparison delete over the extras, reading nothing it does
not have to. The dialog shows the switch under consume; the CLI takes
`--remove-extras`. Mirror with `KeepNewer` and `--remove-extras` is the one
way sync of a working folder to a backup, previewed.

### Organize and flatten

`files.organize { scope, rule, recursive }` moves the files directly under a
folder, or beneath it with `recursive`, into subfolders named by a rule:

- `ByDate { field: Modified | Created | Captured, granularity: Year |
  YearMonth | YearMonthDay }`, captured from the store's media data where it
  has it and the modification time otherwise.
- `ByKind`, the content kind the indexer assigned.
- `ByExtension`.

The plan is a `CreateDirectory` per new folder and a `Move` per file, with
`Conflict { kind: Sources }` where two files would meet at one name. Every
move stays inside one source, so the job renames in place and records keep
their identity, which validation says as `move.atomic` does today.

`files.flatten { scope, on_conflict }` moves every file beneath a folder to
its root, resolves name collisions by the merge policy vocabulary
(`KeepBoth` numbers them, `Skip` leaves them where they are), skips junk, and
prunes emptied folders. Both preview through the overlay: the scope's listing
shows the new folders ghosted with the files inside them, or the root grown
by everything beneath.

### Archive and extract

`files.archive { sources, destination, format, remove_sources }` writes one
archive, `Zip` or `TarZstd`, to a temporary name in the destination and
renames it into place when complete, so an interrupted job leaves nothing
half-written. Its plan is one `Create` with the arena's byte estimate and,
with `remove_sources`, the `Delete` rows. Validation checks the destination
volume's free space against the estimate and, for zip, names paths the
format cannot carry.

`files.extract { archive, destination, on_conflict, strip_components }`
reads the archive's directory, the zip central directory or a pass over the
tar headers, and plans a `Create`, `Replace` or `Skip` per entry against the
index, with `Conflict { kind: FileVsDirectory }` where an entry meets a
folder. Validation refuses an entry that escapes the destination and warns
of free space. The job extracts entry by entry with byte progress and
resumes at the entry index it checkpointed. Both take the merge conflict
policy for collisions.

### Attributes, links, and the trash

`files.set_attributes { paths, mode, modified, hidden }` changes what the
filesystem lets it, planned as a new `ChangeKind::SetAttributes` row per
file naming what changes, refusing what the filesystem cannot express
(`attributes.unsupported`, a mode on exFAT) and what permission forbids.
`files.link { at, target, kind: Symlink | Hardlink }` plans one `Create`,
and validation refuses a hard link across volumes.

The trash view lists the journal's `Trashed` effects with a location, newest
first, restores one or all through `files.undo` scoped to the effect, and
empties the trash: `trash::os_limited::purge_all` where the crate has it,
`NSWorkspace` on macOS, and the Spacedrive trash directories on volumes that
have one.

### Out of scope

- Undo across devices. A job journals on the device that ran it.
- Redo as a separate history; undoing an undo covers it.
- Media conversion and resizing.
- Batch rename from content, such as titles read from tags or EXIF, beyond
  the captured date token.

## Phases

| Phase | Scope | Exit proof |
|---|---|---|
| F1 | Delete and duplicate in the explorer | Landed 2026-09-23. ⌘⌫, ⌥⌘⌫ and the menu open `DeleteModal` on preflight with the last-copy warning and the removed rows; `useDeleteFiles` and its `confirm()` are gone. ⌘D and "Duplicate" write a numbered copy beside the file after validation, and the copy preview plans a keep-both copy at its numbered name |
| F2 | Rename on preflight, batch rename | Landed 2026-09-23. `validate:files.rename` answers every code against the live directory, probing case sensitivity rather than trusting the filesystem's name, with NTFS rules on those volumes and SMB shares; `files.rename_batch` previews new names and collisions per rule, the job renames a chain and a swap without overwriting, and `RenameModal` re-previews as rules change |
| F3 | Journal, undo, restore | Landed 2026-09-23. Every mutating job writes effects; the trash reports locations on macOS through `NSFileManager` and on Windows and Linux through the `trash` crate's listing, with a Spacedrive trash directory on volumes without one; overwrites stash the previous file. Undoing a rename, a copy, a trash and a merge with replacements is tested; a file changed since is `undo.changed` and left; ⌘Z, the job list and the trash view reach it. Trash restore on Windows and Linux is written against the crate and not yet run there |
| F4 | Mirror | Landed 2026-09-23. `files.merge` with `remove_extras` previews the extras as flagged deletes, validation counts the last copies (`merge.last_copies`), the job trashes only what no source holds and prunes the emptied folders, and `sd file merge --remove-extras` and the merge dialog's switch render it |
| F5 | Organize and flatten | Landed 2026-09-23. Organizing by month previews the new folders and moves and each move is a rename, so the inode survives; flattening numbers collisions under keep both, leaves them under skip, and prunes the emptied folders. `sd file organize`, `sd file flatten`, and the two dialogs |
| F6 | Archive and extract | Landed 2026-09-23. Zip and tar.zst round trip with a folder collision and a replacement, an escaping entry refuses the extract, an interrupted extract resumes at its entry, and no temporary file is left behind. `sd file archive`, `sd file extract`, and the two dialogs |
| F7 | Attributes, links, trash | Landed 2026-09-23. Attribute and link plans preview and undo, exFAT refuses a mode, a hard link across volumes or to a directory is refused, and the trash view at `/trash` restores through undo and empties; emptying the platform's trash and restoring from it are written for Windows and Linux and run so far on macOS |

F1 first since it is the one gap in the explorer that skips preflight today.
F2 before F3 because batch rename produces the chains the journal has to
reverse. F3 before F4 through F6 so every new operation writes its journal
from the start. F7 last; it is the smallest and depends on F3's trash
locations.

## Acceptance

On the Expansion drive folder from the previous plan's acceptance case: batch
rename one day's photos to a dated sequence and undo it; mirror the folder to
`jamie-nas` with extras removed, confirm the plan's deletes match `file
compare` reversed, and undo the mirror; flatten a downloads folder with known
collisions; trash a file on the Mac, on titan, and on a network mount, and
restore each from the trash view. Measure preview time for a batch rename
over the largest folder available, and confirm no operation left a file the
journal does not account for.

## Decisions taken

The seven decisions below were taken as proposed, since the phases were
built in one pass: the OS trash with recorded locations and a Spacedrive
trash directory only where a volume has none; stashing on replace, on; the
five rename rules with regex inside replace; captured dates from the store
with the modification time as the fallback; mirror as a merge option; undo
on both the job list and ⌘Z, plus the trash view; zip and tar with zstd.
Each is a switch of code rather than of design if reversed.

## Decisions for James

1. Trash mechanism. Proposed: the OS trash with the resulting location
   recorded (the `objc2-foundation` call on macOS, `os_limited` elsewhere),
   and a Spacedrive trash directory only on volumes without one. The
   alternative is a Spacedrive trash directory everywhere, which is simpler
   and works on every volume but keeps deleted files out of Finder's trash.
2. Stashing on replace. Proposed on: an overwrite moves the previous file to
   the trash instead of truncating it, so replacements are undoable. The
   alternative keeps today's truncation and marks replacements irreversible.
3. Batch rename rules. Proposed the five rules above with a small template
   token set; regex only inside `Replace`. The alternative is templates
   alone.
4. Captured dates in organize. Proposed: use the store's media data where
   present and fall back to the modification time, with the plan saying
   which field each file used. The alternative is filesystem dates only for
   the first version.
5. Mirror as a merge option rather than its own action. Proposed the option,
   for one dialog and one command.
6. Undo surface. Proposed both the job list and ⌘Z on the most recent
   reversible job. The alternative is the job list alone.
7. Archive formats. Proposed zip and tar with zstd. The alternative adds 7z,
   which needs a native library.
