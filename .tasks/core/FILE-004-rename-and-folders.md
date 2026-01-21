---
id: FILE-004
title: "Rename, New Folder, and New Folder with Items"
status: Done
assignee: jamiepine
priority: High
tags: [core, interface, file-ops, keybinds, ux]
parent: FILE-000
last_updated: 2025-12-24
---

## Description

Implement file rename, new folder creation, and new folder with items operations. These features integrate with the existing unified keybind system and context menus, providing inline editing UX similar to macOS Finder.

**Key Features:**

- **Rename**: Press Enter on selected item → inline edit mode → Enter again to save
- **New Folder**: Create empty folder using VolumeBackend (works on cloud storage)
- **New Folder with Items**: Create folder then spawn FileCopyJob to move selected items
- All operations accessible via context menu, menu bar, and keyboard shortcuts

## Background

**Existing Infrastructure:**

- Rename already exists via `FileCopyJob::new_rename()` but needs dedicated action API
- Unified keybind system with `explorer.renameFile` already registered (Enter key)
- Context menu system supports `keybindId` for automatic shortcut display
- VolumeBackend provides abstraction for local/cloud operations but lacks `create_directory()`

## Acceptance Criteria

### Backend

- [x] VolumeBackend trait has `create_directory(path, recursive)` method
- [x] LocalBackend implements `create_directory()` using tokio::fs
- [x] CloudBackend has stub implementation for future cloud support
- [x] `FileRenameAction` exists at `core/src/ops/files/rename/`
  - [x] Input validation: no path separators, empty names, invalid characters
  - [x] Platform-specific validation (Windows reserved names)
  - [x] Wraps `FileCopyJob::new_rename()` for execution
  - [x] Returns `JobReceipt`
- [x] `CreateFolderAction` exists at `core/src/ops/files/create_folder/`
  - [x] Accepts `parent`, `name`, and optional `items` array
  - [x] Uses VolumeBackend to create directory
  - [x] Spawns FileCopyJob if items provided
  - [x] Returns folder path + optional job handle
- [x] Actions registered: `files.rename` and `files.createFolder`

### Frontend State Management

- [x] SelectionContext has rename state:
  - [x] `renamingFileId: string | null`
  - [x] `startRename(fileId: string)`
  - [x] `cancelRename()`
  - [x] `saveRename(newName: string)` using `files.rename` mutation
- [x] Menu items sync correctly (rename enabled when single file selected)

### Frontend UI Components

- [x] `InlineNameEdit` component created at `packages/interface/src/components/Explorer/components/InlineNameEdit.tsx`
  - [x] Auto-focus and select text on mount
  - [x] Split filename into name + extension (only edit name)
  - [x] Handle Enter (save), Escape (cancel), Blur (cancel)
  - [x] Use Input from @sd/ui with transparent variant
  - [x] Match styling of static file name display
- [x] FileCard (GridView) integrates inline editing
  - [x] Conditionally renders InlineNameEdit when `renamingFileId === file.id`
  - [x] Matches positioning and styling
- [x] TableRow (ListView) integrates inline editing in NameCell
  - [x] Same conditional rendering logic
  - [x] Matches inline styling

### Frontend Integration

- [x] Keybind handlers in GridView and ListView:
  - [x] `useKeybind('explorer.renameFile')` triggers rename on Enter
  - [x] Only enabled when single file selected
- [x] Context menu items added to `useFileContextMenu`:
  - [x] "Rename" item with Pencil icon and `keybindId: 'explorer.renameFile'`
  - [x] "New Folder" item with FolderPlus icon
  - [x] "New Folder with Items" item (visible when files selected)
- [x] Menu bar integration works (keybind already registered, menu state synced)

### Edge Cases Handled

- [x] Empty name → Cancel rename, revert to original
- [x] Name unchanged → Accept without mutation call
- [x] Backend validation errors → Show error, keep in edit mode
- [x] Navigation during rename → Cancel rename before navigating
- [x] Selection change during rename → Cancel rename
- [x] Multiple rapid Enter presses → Debounce or disable during save
- [ ] Folder creation failure → Show error toast
- [ ] Copy job failure for "new folder with items" → Folder still created, show job status

## Implementation Plan

### Phase 1: Extend VolumeBackend (Backend)

**File: `core/src/volume/backend/mod.rs`**

Add method to VolumeBackend trait:

```rust
async fn create_directory(&self, path: &Path, recursive: bool) -> Result<(), VolumeError>;
```

**File: `core/src/volume/backend/local.rs`**

Implement for LocalBackend:

```rust
async fn create_directory(&self, path: &Path, recursive: bool) -> Result<(), VolumeError> {
    let full_path = self.resolve_path(path);
    if recursive {
        fs::create_dir_all(&full_path).await.map_err(VolumeError::Io)?;
    } else {
        fs::create_dir(&full_path).await.map_err(VolumeError::Io)?;
    }
    Ok(())
}
```

**File: `core/src/volume/backend/cloud.rs`** (if exists)

Add stub for future implementation.

### Phase 2: Create Rename Action (Backend)

**Directory Structure:**

```
core/src/ops/files/rename/
├── mod.rs          # Module exports
├── input.rs        # FileRenameInput
├── action.rs       # FileRenameAction
└── validation.rs   # Filename validation
```

**`input.rs`:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileRenameInput {
    pub target: SdPath,
    pub new_name: String,
}
```

**`validation.rs`:**

- Validate filename: no path separators, not empty, valid characters
- Platform-specific validation (Windows reserved names like CON, PRN, AUX)

**`action.rs`:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRenameAction {
    pub target: SdPath,
    pub new_name: String,
}

impl LibraryAction for FileRenameAction {
    type Input = FileRenameInput;
    type Output = JobReceipt;

    async fn validate(...) -> Result<ValidationResult, ActionError> {
        // 1. Validate target exists
        // 2. Validate new_name (no path separators, not empty, valid chars)
        // 3. Check if destination already exists (conflict detection)
        // 4. Validate target is not Content/Sidecar path
    }

    async fn execute(...) -> Result<JobReceipt, ActionError> {
        // Dispatch FileCopyJob::new_rename(target, new_name)
        // Return job receipt for tracking
    }
}

register_library_action!(FileRenameAction, "files.rename");
```

**File: `core/src/ops/files/mod.rs`**

Add: `pub mod rename;`

### Phase 3: Create Folder Operations (Backend)

**Directory Structure:**

```
core/src/ops/files/create_folder/
├── mod.rs          # Module exports
├── input.rs        # CreateFolderInput
├── output.rs       # CreateFolderOutput
└── action.rs       # CreateFolderAction
```

**`input.rs`:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateFolderInput {
    pub parent: SdPath,
    pub name: String,
    #[serde(default)]
    pub items: Vec<SdPath>,  // Optional items to move into folder
}
```

**`output.rs`:**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFolderOutput {
    pub folder_path: SdPath,
    pub job_handle: Option<JobReceipt>,  // Present if items were provided
}
```

**`action.rs`:**

```rust
impl LibraryAction for CreateFolderAction {
    type Input = CreateFolderInput;
    type Output = CreateFolderOutput;

    async fn validate(...) -> Result<ValidationResult, ActionError> {
        // 1. Validate parent exists and is a directory
        // 2. Validate folder name
        // 3. Check if folder already exists
        // 4. Validate items (if provided) exist
    }

    async fn execute(...) -> Result<CreateFolderOutput, ActionError> {
        // 1. Construct destination path: parent.join(name)
        // 2. Create directory using VolumeBackend
        let volume_manager = library.volumes();
        let backend = volume_manager.get_backend_for_path(&parent)?;
        backend.create_directory(&folder_path, false).await?;

        // 3. If items provided, dispatch FileCopyJob
        let job_handle = if !self.items.is_empty() {
            let job = FileCopyJob::new(
                SdPathBatch::new(self.items),
                folder_path.clone()
            );
            Some(library.jobs().dispatch(job).await?)
        } else {
            None
        };

        Ok(CreateFolderOutput {
            folder_path,
            job_handle: job_handle.map(|h| h.into()),
        })
    }
}

register_library_action!(CreateFolderAction, "files.createFolder");
```

**File: `core/src/ops/files/mod.rs`**

Add: `pub mod create_folder;`

### Phase 4: Add Rename State Management (Frontend)

**File: `packages/interface/src/components/Explorer/SelectionContext.tsx`**

Add to interface (around line 6):

```typescript
interface SelectionContextValue {
	// ... existing fields
	renamingFileId: string | null;
	startRename: (fileId: string) => void;
	cancelRename: () => void;
	saveRename: (newName: string) => Promise<void>;
}
```

Add state and handlers in SelectionProvider:

```typescript
const [renamingFileId, setRenamingFileId] = useState<string | null>(null);
const renameFile = useLibraryMutation("files.rename");

const startRename = useCallback(
	(fileId: string) => {
		if (selectedFiles.length === 1) {
			setRenamingFileId(fileId);
		}
	},
	[selectedFiles],
);

const cancelRename = useCallback(() => {
	setRenamingFileId(null);
}, []);

const saveRename = useCallback(
	async (newName: string) => {
		if (!renamingFileId) return;

		const file = selectedFiles.find((f) => f.id === renamingFileId);
		if (!file) return;

		try {
			await renameFile.mutateAsync({
				target: file.sd_path,
				new_name: newName,
			});
			setRenamingFileId(null);
		} catch (error) {
			// Keep in edit mode, show error
			console.error("Rename failed:", error);
			throw error;
		}
	},
	[renamingFileId, selectedFiles, renameFile],
);
```

### Phase 5: Create Inline Edit Component (Frontend)

**New File: `packages/interface/src/components/Explorer/components/InlineNameEdit.tsx`**

```typescript
import { useState, useEffect, useRef } from 'react';
import { Input } from '@sd/ui';
import type { File } from '@sd/ts-client';

interface InlineNameEditProps {
  file: File;
  onSave: (newName: string) => void;
  onCancel: () => void;
  className?: string;
}

export function InlineNameEdit({ file, onSave, onCancel, className }: InlineNameEditProps) {
  // Split name and extension
  const nameWithoutExtension = file.extension
    ? file.name
    : file.name;

  const [value, setValue] = useState(nameWithoutExtension);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-focus and select on mount
  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      if (value.trim()) {
        onSave(file.extension ? `${value}.${file.extension}` : value);
      } else {
        onCancel();
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    }
  };

  const handleBlur = () => {
    onCancel();
  };

  return (
    <Input
      ref={inputRef}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={handleKeyDown}
      onBlur={handleBlur}
      variant="transparent"
      size="sm"
      className={className}
    />
  );
}
```

### Phase 6: Integrate Inline Editing (Frontend)

**File: `packages/interface/src/components/Explorer/views/GridView/FileCard.tsx`**

Around line 160, replace file name rendering:

```typescript
import { InlineNameEdit } from '../../components/InlineNameEdit';
import { useSelection } from '../../SelectionContext';

// Inside FileCard component:
const { renamingFileId, saveRename, cancelRename } = useSelection();

// In render (around line 160):
{renamingFileId === file.id ? (
  <InlineNameEdit
    file={file}
    onSave={saveRename}
    onCancel={cancelRename}
    className="text-sm truncate px-2 py-0.5"
  />
) : (
  <div className="text-sm truncate px-2 py-0.5">
    {file.name}
  </div>
)}
```

**File: `packages/interface/src/components/Explorer/views/ListView/TableRow.tsx`**

Modify NameCell component (around line 196):

```typescript
import { InlineNameEdit } from '../../components/InlineNameEdit';
import { useSelection } from '../../SelectionContext';

// Inside NameCell:
const { renamingFileId, saveRename, cancelRename } = useSelection();

{renamingFileId === file.id ? (
  <InlineNameEdit
    file={file}
    onSave={saveRename}
    onCancel={cancelRename}
    className="truncate text-sm text-ink"
  />
) : (
  <span className="truncate text-sm text-ink">
    {file.name}
  </span>
)}
```

### Phase 7: Wire Up Keybinds (Frontend)

**File: `packages/interface/src/components/Explorer/views/GridView/GridView.tsx`**

After existing useEffect for keyboard nav (around line 204):

```typescript
import { useKeybind } from "../../../hooks/useKeybind";

useKeybind(
	"explorer.renameFile",
	() => {
		if (selectedFiles.length === 1) {
			startRename(selectedFiles[0].id);
		}
	},
	{ enabled: selectedFiles.length === 1 },
);
```

**File: `packages/interface/src/components/Explorer/views/ListView/ListView.tsx`**

Same keybind handler after existing keyboard nav.

### Phase 8: Add Context Menu Items (Frontend)

**File: `packages/interface/src/components/Explorer/hooks/useFileContextMenu.ts`**

Add after "Open" item (around line 90):

```typescript
import { Pencil, FolderPlus } from '@phosphor-icons/react';

{
  icon: Pencil,
  label: "Rename",
  onClick: () => {
    startRename(file.id);
  },
  keybindId: "explorer.renameFile",
  condition: () => selected && selectedFiles.length === 1,
},
{ type: "separator" },
{
  icon: FolderPlus,
  label: "New Folder",
  onClick: () => createFolder(),
  // keybindId: "explorer.newFolder", // Add to registry if needed
},
{
  icon: FolderPlus,
  label: "New Folder with Items",
  onClick: () => createFolderWithItems(),
  condition: () => selectedFiles.length > 0,
}
```

Implement createFolder and createFolderWithItems:

```typescript
const createFolderMutation = useLibraryMutation("files.createFolder");

const createFolder = async () => {
	// Create with default name, then enter rename mode
	const result = await createFolderMutation.mutateAsync({
		parent: currentPath,
		name: "Untitled Folder",
		items: [],
	});
	// TODO: Select new folder and enter rename mode
};

const createFolderWithItems = async () => {
	const result = await createFolderMutation.mutateAsync({
		parent: currentPath,
		name: "New Folder",
		items: selectedFiles.map((f) => f.sd_path),
	});
	// result.job_handle tracks copy progress
};
```

## Implementation Files

### Backend

- `core/src/volume/backend/mod.rs`
- `core/src/volume/backend/local.rs`
- `core/src/volume/backend/cloud.rs`
- `core/src/ops/files/rename/mod.rs` (new)
- `core/src/ops/files/rename/input.rs` (new)
- `core/src/ops/files/rename/action.rs` (new)
- `core/src/ops/files/rename/validation.rs` (new)
- `core/src/ops/files/create_folder/mod.rs` (new)
- `core/src/ops/files/create_folder/input.rs` (new)
- `core/src/ops/files/create_folder/output.rs` (new)
- `core/src/ops/files/create_folder/action.rs` (new)
- `core/src/ops/files/mod.rs`

### Frontend

- `packages/interface/src/components/Explorer/SelectionContext.tsx`
- `packages/interface/src/components/Explorer/components/InlineNameEdit.tsx` (new)
- `packages/interface/src/components/Explorer/views/GridView/FileCard.tsx`
- `packages/interface/src/components/Explorer/views/ListView/TableRow.tsx`
- `packages/interface/src/components/Explorer/views/GridView/GridView.tsx`
- `packages/interface/src/components/Explorer/views/ListView/ListView.tsx`
- `packages/interface/src/components/Explorer/hooks/useFileContextMenu.ts`

## Testing Plan

### Manual Testing

1. **Rename Flow:**
   - Select file, press Enter → Input appears with text selected
   - Type new name, press Enter → Name updates in UI and database
   - Type new name, press Escape → Reverts to original
   - Type new name, click outside → Reverts to original
   - Test in both GridView and ListView
   - Context menu "Rename" works
   - Extension not included in editable text

2. **New Folder:**
   - Context menu → New Folder → Folder created
   - Test on local filesystem
   - Test validation (duplicate names, invalid characters)

3. **New Folder with Items:**
   - Select files → Context menu → New Folder with Items
   - Folder created immediately, copy job starts
   - Items move into new folder with progress tracking

4. **Edge Cases:**
   - Empty name cancels rename
   - Navigation during rename cancels it
   - Selection change during rename cancels it
   - Backend validation errors handled gracefully

### Integration Tests (Future)

- `core/tests/test_rename_action.rs` - Test rename validation and execution
- `core/tests/test_create_folder_action.rs` - Test folder creation with/without items
- `core/tests/test_volume_backend.rs` - Test create_directory() implementation

## Notes

- Rename wraps existing `FileCopyJob::new_rename()` for cleaner API semantics
- New folder uses VolumeBackend for cloud storage compatibility
- "New folder with items" is efficient: folder created instantly, items move with progress
- Keybind system already has infrastructure; just needs handlers wired up
- Context menus automatically show correct shortcuts via `keybindId`
- Menu bar integration works automatically (keybind registered, state synced)

## Related Tasks

- FILE-000 (parent epic)
- Depends on unified keybind system (already implemented)
- Depends on VolumeBackend architecture (already implemented)
- Depends on FileCopyJob (already implemented)
