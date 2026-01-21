---
id: EXPL-004
title: File Opening UI Integration
status: To Do
assignee: jamiepine
parent: EXPL-000
priority: High
tags: [explorer, file-operations, ui]
whitepaper: DESIGN-open-with.md
last_updated: 2025-12-24
related_tasks: [CORE-014]
---

## Description

Integrate file opening functionality into Explorer UI. When users double-click a file, it should open with the system's default application. The context menu should provide "Open" and "Open With" options showing available applications.

## Dependencies

Requires CORE-014 (backend file opening system) to be completed first.

## Implementation Notes

- Extend platform interface with file opening methods
- Create `useOpenWith` React hook for querying apps and opening files
- Update context menu to show "Open With" submenu
- Wire up double-click handlers in all view components
- Handle multi-file selection (show intersection of compatible apps)
- Use toast notifications for errors

Currently there's a TODO at `packages/interface/src/components/Explorer/hooks/useFileContextMenu.ts:82-83` that needs to be replaced with actual implementation.

See `DESIGN-open-with.md` for complete architecture details.

## Acceptance Criteria

- [ ] Platform interface extended with:
  - [ ] `getAppsForPaths(paths: string[]): Promise<OpenWithApp[]>`
  - [ ] `openPathDefault(path: string): Promise<OpenResult>`
  - [ ] `openPathWithApp(path, appId): Promise<OpenResult>`
  - [ ] `openPathsWithApp(paths, appId): Promise<OpenResult[]>`
- [ ] `useOpenWith` hook created with:
  - [ ] React Query integration for fetching apps
  - [ ] `openWithDefault(path)` function
  - [ ] `openWithApp(path, appId)` function
  - [ ] `openMultipleWithApp(paths, appId)` function
  - [ ] Proper error handling with toast notifications
- [ ] Context menu integration:
  - [ ] "Open" menu item works for files and folders
  - [ ] "Open With" submenu appears for files
  - [ ] Shows list of compatible applications
  - [ ] For multi-select, only shows apps that can open ALL files
  - [ ] Apps are sorted alphabetically
- [ ] Double-click handlers updated in:
  - [ ] GridView (`FileCard.tsx`)
  - [ ] ListView (`TableRow.tsx`)
  - [ ] ColumnView (`Column.tsx`)
  - [ ] MediaView (if applicable)
- [ ] Error handling:
  - [ ] File not found → toast error
  - [ ] App not found → toast error
  - [ ] Permission denied → toast error
  - [ ] Platform errors → toast with message
- [ ] Loading states shown while querying apps
- [ ] TODO at `useFileContextMenu.ts:82-83` is removed

## Implementation Files

To be created:
- `packages/interface/src/hooks/useOpenWith.ts`

To be modified:
- `apps/tauri/src/platform.ts` (extend interface)
- `packages/interface/src/components/Explorer/hooks/useFileContextMenu.ts`
- `packages/interface/src/components/Explorer/views/GridView/FileCard.tsx`
- `packages/interface/src/components/Explorer/views/ListView/TableRow.tsx`
- `packages/interface/src/components/Explorer/views/ColumnView/Column.tsx`

## User Experience

**Before:**
- Double-clicking files does nothing
- No "Open" or "Open With" in context menu
- Users can only use "Quick Preview"

**After:**
- Double-clicking files opens them in default app
- Double-clicking folders still navigates (unchanged)
- Context menu has "Open" option (⌘O)
- Context menu has "Open With" submenu showing available apps
- Multi-select shows only apps compatible with all selected files
- Proper error messages if opening fails

## Testing

- Test double-click on various file types (.txt, .pdf, .jpg, .mp4)
- Test double-click on folders still navigates
- Test "Open" in context menu
- Test "Open With" shows correct apps
- Test multi-select intersection logic
- Test error cases: missing file, no apps, permission denied
- Test on all platforms: macOS, Windows, Linux
