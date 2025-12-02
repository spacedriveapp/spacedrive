---
id: NAV-000
title: Multi-Window System
status: To Do
assignee: james
parent: UI-000
priority: Medium
tags: [navigation, windows, architecture]
whitepaper: N/A
---

## Description

Implement support for multiple windows, each with independent navigation state. Users can open multiple Explorer windows, media viewers, or settings panels.

## Implementation Notes

- Each window has independent router state
- Tauri: use multi-window API
- Web: use browser windows or tabs
- Window state persists (size, position, route)
- Cross-window communication via events

## Acceptance Criteria

- [ ] Open new Explorer window from menu
- [ ] Each window has independent navigation
- [ ] Window position/size persists
- [ ] Cross-window drag and drop
- [ ] Windows share library state
- [ ] Close all windows command
- [ ] Platform-specific window controls (traffic lights on macOS)
