# File operation dialogs

Two dialogs run file operations through the daemon's preflight checks: `FileOperationModal`
for copy and move, `MergeFoldersModal` for merging folders. Both ask the
daemon the two questions before an execution, over the exact input they
would dispatch, and render the answers through the shared `PreflightPanel`:

- **Validate** says whether and how the operation would run: findings with a
  stable code and a severity, and the facts of the execution (where it runs,
  the strategy, estimated files and bytes, free space after). It re-runs as
  options change and an error finding disables the confirm button. The daemon
  runs the same validation again at dispatch, so a refused action surfaces as
  a `RefusedError` with the findings.
- **Preview** says what would exist afterward, as an `FsPlan` read from the
  index: counts per kind of change, the conflicts and replacements as rows,
  and the store revisions it read. It is advisory; the job applies the same
  decisions leaf by leaf against the live filesystem.

Neither dialog does conflict math of its own.

## Copy and move

```tsx
const openFileOperation = useFileOperationDialog();

openFileOperation({
  operation: "copy",
  sources: selectedFiles.map((f) => f.sd_path),
  destination: folderPath,
  onComplete: () => clipboard.clearClipboard(),
});
```

The dialog offers Copy and Move (⌘1, ⌘2) and a policy for files that already
exist: skip (S), keep both (K), overwrite (O). When a single folder is copied onto
a folder of its name, copy validation names the collision (`copy.folder_collision`)
and the dialog offers **Merge** (⌘3) as a third choice, which closes it and
opens the merge dialog with that folder as the destination. A move is a
consuming plan: the source tree ends gone and the destination tree grown.

## Merge

```tsx
const openMergeFolders = useMergeFoldersDialog();

openMergeFolders({
  sources: [folderA, folderB],
  destination: existingFolder,
  policy: "skip",      // skip | overwrite | keep_both | keep_newer
  consume: false,      // remove settled leaves from the sources
});
```

Three ways in:

- **Context menu.** With only folders on the clipboard, a folder's context
  menu shows "Merge into '<name>'". Its destination is the right-clicked
  folder, unlike Paste, which targets the current directory. After a cut the
  merge consumes the sources.
- **Drag with Option held.** dnd-kit delivers no modifier state, so
  `hooks/useModifierKeys.ts` tracks the keys at the window; `DndProvider`
  reads them at drop time and opens the merge dialog when every dragged item
  is a folder and the target is a folder.
- **A plain drop that collides.** The operation modal offers Merge as above.

While a drag hovers a folder in the grid, the folder validates the drop and
badges the first error or warning, such as a folder copied into itself.

## Deleting, and removing duplicate copies

`DeleteModal` runs `files.delete` through the same preflight for its three
kinds of target, and is what every delete entry point opens: ⌘⌫, ⌥⌘⌫ and the
context menu for named files, and the duplicate-removal entries for the
rest. Validation carries the warning only an index can give, which of the
files are the last copy of their bytes anywhere in the library; the plan
lists the rows that go, last copies first; the trash or permanent switch is
preset by the key that opened it; and an error finding disables confirm.

```tsx
const openDelete = useDeleteDialog();

openDelete({
  title: "Trash 3 items",
  targets: { kind: "paths", paths: selectedFiles.map((f) => f.sd_path) },
  permanent: false,
});

openDelete({
  title: "Remove duplicate copies inside 'Photos'",
  targets: {
    kind: "duplicates",
    duplicates: { scope: folderPath, keep: { kind: "first" }, min_size: null },
  },
});
```

- **A folder's context menu** has "Remove duplicates inside": of each content
  held more than once under it, the first copy in walk order stays.
- **A file's context menu** has "Remove other copies": that file stays and
  every other copy of its content, anywhere attached, goes.
- **With one folder on the clipboard**, a folder's menu has "Delete what
  '<B>' already holds": the comparison delete by content.
- **Protection, Duplicates** lists the groups the `files.duplicates` query
  finds, with a keeper chosen per group and one dialog for the rest.

For duplicates the plan lists each kept copy beside the copies that go,
confirm is disabled when nothing would go, and the job reads each pair in
full before removing one.

## Duplicate in place

⌘D and the "Duplicate" menu item write a numbered copy beside each selected
file: `files.copy` into the file's own directory, keeping both. There is
nothing to choose, so `routes/explorer/hooks/useDuplicateFiles.ts` validates
the input, shows a refusal, and dispatches the same input without a dialog.
The copy preview plans it as one create at the numbered name.

## Renaming

A single file renames in place; the daemon validates the name at dispatch
(a name the volume does not write, one already there, one the directory
cannot tell from an existing one on a case-insensitive volume), and a
refusal is shown as a toast with the findings. With several files selected,
Enter and "Rename N items…" open `RenameModal`: an ordered list of rules
applied to each name (replace, case, add text, number, format), previewed
as every rule changes, with each old name beside its new one and a conflict
where two files want one name. Confirm dispatches `files.rename_batch`.

```tsx
const openBatchRename = useBatchRenameDialog();
openBatchRename({ targets: selectedFiles.map((f) => f.sd_path) });
```

## Undo

`UndoModal` runs `files.undo` on preflight: the plan is the reverse of a
job's journal and validation says what cannot be reversed and what changed
since. It opens from the job list, where a completed job with something to
reverse shows an undo button, from ⌘Z in the explorer, which undoes the
most recent such job on this device, and from the trash view, scoped to one
trashed item.

```tsx
const openUndo = useUndoDialog();
openUndo({ job: job.id, label: "Copying 'Photos'" });
openUndo({ job, label: "trashing IMG_0041.JPG", effects: [sequence] });
```

## Mirror, organize, flatten, archive, extract, attributes

The merge dialog has "Remove what the sources lack", which makes the merge
a mirror: the plan lists the files only the destination holds as flagged
deletes. `RearrangeModal` holds `useOrganizeDialog` (subfolders by date,
kind or extension) and `useFlattenDialog`; `ArchiveModal` holds
`useArchiveDialog` (a zip or tar.zst beside the selection) and
`useExtractDialog` (into the current folder, planned from the archive's
directory); `AttributesModal` sets the mode, modification time and hidden
flag. Each opens from a folder's or file's context menu, validates and
previews as its options change, and dispatches the previewed input. "Make
link" writes a symlink beside a file after validation, with no dialog.

The trash view at `/trash` lists what Spacedrive trashed with a known
location, restores one item through the undo dialog, and empties the trash.

## Browsing the result

Both dialogs have "Browse the result". A preview's plan is retained by the
daemon under a handle, and the explorer's preview mode
(`routes/explorer/hooks/usePlanPreview.ts`) passes that handle as `overlay`
with every directory listing. Listings then come back as the directory would
look after the operation: rows the plan would create appear ghosted, rows it
would delete are struck, rows it skips are dimmed, and every touched row
carries a badge naming the change. Beneath a source a merge consumes, what
the merge settles is marked as removed and what it does not stays with its reason.
The banner above the files names the operation and exits the mode. A handle
the daemon let lapse is rebuilt from the same input.

## Styling

Semantic Tailwind classes only: `bg-app-box` and `bg-app` for surfaces,
`text-ink`, `text-ink-dull`, `text-ink-faint` for the text hierarchy,
`bg-accent` for the primary action, and `red-500` and `amber-500` tints for
error and warning findings.
