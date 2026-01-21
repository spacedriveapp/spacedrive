# File Operation Modal - Usage Guide

## Overview

The File Operation Modal provides a clean, interactive UI for copying and moving files with validation-based confirmations. It's designed to work with the core validation system and can be extended to show simulation previews in the future.

## Features

### Phase 1: Validation & Confirmation (Current)
- ✅ Interactive conflict resolution (overwrite/rename/skip)
- ✅ Real-time validation feedback
- ✅ Progress tracking during execution
- ✅ Error handling with clear messages
- ✅ Drag-and-drop support

### Phase 2: Simulation (Future)
- 🔜 Detailed operation preview (file count, size, time estimate)
- 🔜 Deduplication analysis (bytes saved)
- 🔜 Space availability checking
- 🔜 Network transfer estimates for cross-device operations

## Basic Usage

### Programmatic

```tsx
import { useFileOperationDialog } from '@sd/interface';

function MyComponent() {
  const openFileOperation = useFileOperationDialog();

  const handleCopyFiles = () => {
    openFileOperation({
      operation: "copy",
      sources: [
        { Physical: { device_slug: "", path: "/source/file1.txt" } },
        { Physical: { device_slug: "", path: "/source/file2.txt" } },
      ],
      destination: { Physical: { device_slug: "", path: "/destination/" } },
      onComplete: () => {
        console.log("Operation completed!");
      },
    });
  };

  return <button onClick={handleCopyFiles}>Copy Files</button>;
}
```

### Drag and Drop

#### In Sidebar Locations

The `LocationsSection` component already has drag-drop enabled:

```tsx
// Just drop files onto a location in the sidebar
// The modal will automatically open with validation
```

#### In Explorer Folders

Use `DropZoneFile` for folders that can receive drops:

```tsx
import { DropZoneFile, useFileOperationDialog } from '@sd/interface';

function FolderGrid({ folders, selectedFiles }) {
  const openFileOperation = useFileOperationDialog();

  const handleFilesDropped = (
    sources: SdPath[],
    destination: SdPath,
    operation: "copy" | "move"
  ) => {
    openFileOperation({
      operation,
      sources,
      destination,
    });
  };

  return (
    <div className="grid grid-cols-4 gap-4">
      {folders.map((folder) => (
        <DropZoneFile
          key={folder.id}
          file={folder}
          selectedFiles={selectedFiles}
          onFilesDropped={handleFilesDropped}
        >
          <File.Thumb file={folder} />
          <File.Title file={folder} />
        </DropZoneFile>
      ))}
    </div>
  );
}
```

#### Making Files Draggable

The `File` component is already draggable by default:

```tsx
import { File } from '@sd/interface';

// Files are draggable by default
<File file={file} selectedFiles={allSelectedFiles}>
  <File.Thumb file={file} />
  <File.Title file={file} />
</File>

// Disable dragging if needed
<File file={file} draggable={false}>
  ...
</File>
```

**Drag behavior:**
- Default: Copy operation
- Hold Alt/Option: Move operation
- Dragging single file: Just that file
- Dragging selected file when multiple selected: All selected files

## Modal Phases

### 1. Validating
Shows a spinner while checking for conflicts and validating the operation.

### 2. Requires Confirmation
Displays conflict resolution options when destination files exist:
- Overwrite existing files
- Keep both (auto-rename with counter)
- Skip conflicting files

### 3. Ready (with optional Simulation)
When simulation is implemented, this phase will show:
- File/folder counts
- Total size and bytes to transfer
- Deduplication savings
- Space availability
- Time estimate

### 4. Executing
Shows progress bar and current operation status.

### 5. Completed
Brief success message before auto-closing.

### 6. Error
Displays error message with option to close.

## Validation System Integration

The modal integrates with the core validation system:

1. **Modal opens** → Shows validating state
2. **Core validates** → Returns `ValidationResult`
3. **If conflicts** → Shows confirmation choices
4. **User selects** → Calls `resolve_confirmation(choice_index)`
5. **Execute** → Runs the operation with resolved conflicts

## Future: Simulation Integration

When simulation is added, the flow will be:

1. **Validate** (fast, required)
2. **Simulate** (optional, for large operations)
3. **Show preview** with detailed stats
4. **Execute** on user confirmation

The modal is designed to smoothly add simulation data without breaking the current flow.

## Styling

The modal uses semantic Tailwind classes:
- `bg-app-box` / `bg-app` for backgrounds
- `text-ink` / `text-ink-dull` / `text-ink-faint` for text hierarchy
- `border-app-line` for borders
- `bg-accent` / `text-accent` for primary actions
- Status colors: `bg-yellow-500/10`, `bg-green-500`, `bg-red-500/10`

## Accessibility

- Keyboard navigation supported
- Focus management with Radix Dialog
- Clear visual states (hover, active, dragging)
- Screen reader friendly labels

## Examples

### Copy to Location
```tsx
// Drag files from explorer onto a location in sidebar
// Modal opens automatically with validation
```

### Move to Folder
```tsx
// Hold Alt/Option and drag files onto a folder
// Modal opens with move operation
```

### Programmatic Copy
```tsx
const openFileOp = useFileOperationDialog();

openFileOp({
  operation: "copy",
  sources: selectedFiles.map(f => f.sd_path),
  destination: folderPath,
});
```

## Architecture

```
FileOperationModal
├── Uses: useLibraryMutation('files.copy')
├── State: DialogPhase (validating → confirmation → ready → executing → completed)
├── Validation: Integrates with core ValidationResult
└── Future: Will display SimulationResult when available

DropZoneFile (for folders)
├── Uses: useGlobalFileDrag hook
├── Detects: Folder kind only
├── Visual: Ring/border on drag over
└── Triggers: FileOperationModal on drop

DropZoneSidebarItem (for locations)
├── Uses: useGlobalFileDrag hook
├── Accepts: Any file drops
├── Visual: Ring/border on drag over
└── Triggers: FileOperationModal on drop

File (base component)
├── Draggable: By default
├── Multi-select: Drags all selected files
└── Operation: Copy (default) or Move (Alt/Option)
```

## Notes

- The modal currently simulates validation with a timeout - replace with actual daemon `validate` endpoint when available
- Conflict detection is currently random (50%) - will use real validation once daemon protocol is updated
- Progress tracking will connect to job system events in future
- Simulation preview UI is ready but needs backend data