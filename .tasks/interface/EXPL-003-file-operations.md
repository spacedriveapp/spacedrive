---
id: EXPL-003
title: File Operations UI
status: To Do
assignee: james
parent: EXPL-000
priority: High
tags: [explorer, file-operations]
whitepaper: N/A
---

## Description

Implement UI for core file operations: copy, move, delete, rename. Integrates with backend jobs and shows progress.

## Implementation Notes

- Use useLibraryMutation for all operations
- Show progress toast for long operations
- Subscribe to job progress events
- Handle errors gracefully with user feedback
- Confirmation dialogs for destructive operations

## Acceptance Criteria

- [ ] Copy files via context menu or Cmd+C
- [ ] Move files via drag and drop
- [ ] Delete with confirmation dialog
- [ ] Rename with inline editing
- [ ] Duplicate files
- [ ] Create new folders
- [ ] Progress indicator for long operations
- [ ] Error handling with user-friendly messages
- [ ] Undo for safe operations
