---
id: EXPL-002
title: File List View with Sortable Columns
status: To Do
assignee: james
parent: EXPL-000
priority: High
tags: [explorer, views, performance]
whitepaper: N/A
---

## Description

Implement a list view for files with sortable columns showing name, size, date modified, kind, and tags.

## Implementation Notes

- Use TanStack Table for column management
- TanStack Virtual for row virtualization
- Sortable columns with multi-column sort
- Resizable columns with drag handles
- Icon + text for file type

## Acceptance Criteria

- [ ] List shows files with columns: Name, Size, Modified, Kind, Tags
- [ ] Click column header to sort
- [ ] Multi-column sort with Shift + click
- [ ] Drag column edges to resize
- [ ] Virtual scrolling for large lists
- [ ] Selection works same as grid view
- [ ] Keyboard navigation (up/down arrows)
