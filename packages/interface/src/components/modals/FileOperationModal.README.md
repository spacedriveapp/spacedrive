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
