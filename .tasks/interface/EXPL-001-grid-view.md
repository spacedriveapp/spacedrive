---
id: EXPL-001
title: File Grid View with Virtual Scrolling
status: To Do
assignee: james
parent: EXPL-000
priority: High
tags: [explorer, views, performance]
whitepaper: N/A
---

## Description

Implement a performant grid view for displaying files and folders with thumbnails. Uses virtual scrolling to handle thousands of items efficiently.

## Implementation Notes

- Use TanStack Virtual for virtualization
- Grid layout with CSS Grid
- Thumbnail generation via backend
- Selection state management
- Drag and drop support

## Acceptance Criteria

- [ ] Grid displays files with thumbnails
- [ ] Virtual scrolling works smoothly with 10k+ items
- [ ] Single-click selection, double-click to open
- [ ] Multi-select with Cmd/Ctrl + click
- [ ] Range select with Shift + click
- [ ] Drag and drop for file operations
- [ ] Keyboard navigation (arrow keys)
- [ ] Responsive grid (adjusts to window size)
