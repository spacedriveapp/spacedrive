---
id: EXPL-000
title: "Epic: Explorer Interface"
status: In Progress
assignee: james
parent: UI-000
priority: High
tags: [epic, explorer, interface]
whitepaper: N/A
last_updated: 2025-12-02
---

## Description

Build the complete Explorer interface for browsing and managing files across all devices and locations. The Explorer is the primary interface for interacting with files in Spacedrive.

## Components

- Sidebar with library switcher, locations, tags, devices
- Top bar with breadcrumbs, search, view controls
- Main view with grid/list/media layouts
- Inspector panel for file details
- Context menus for file operations

## Implementation Notes

- Uses virtual scrolling for large file lists (TanStack Virtual)
- Supports multiple selection modes
- Real-time updates via event subscriptions
- Keyboard shortcuts for all operations

## Acceptance Criteria

- [x] Sidebar with library switcher
- [x] Expanding dropdown menus
- [ ] Grid view with thumbnails
- [ ] List view with columns
- [ ] Media view for photos/videos
- [ ] File operations (copy, move, delete)
- [ ] Multi-select with keyboard/mouse
- [ ] Context menus
- [ ] Search integration
- [ ] Inspector panel
