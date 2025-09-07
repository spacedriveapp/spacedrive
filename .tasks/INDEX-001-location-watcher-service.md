---
id: INDEX-001
title: Location Watcher Service
status: Done
assignee: james
parent: INDEX-000
priority: High
tags: [indexing, watcher, real-time]
whitepaper: Section 4.3.3
---

## Description

The `LocationWatcher` service, which provides real-time monitoring of filesystem events within indexed locations, is implemented. This is crucial for keeping the VDFS index up-to-date without requiring frequent, expensive rescans.

## Implementation Notes
- A cross-platform file system watching library (`notify`) is integrated.
- The `LocationWatcher` service monitors multiple locations simultaneously.
- The service translates raw filesystem events into VDFS-specific events (e.g., `FileCreated`, `FileModified`, `FileDeleted`).
- The service dispatches these events to an `EventBus` for other services to consume.

## Acceptance Criteria
-   [x] The `LocationWatcher` can be started and stopped gracefully.
-   [x] The service correctly detects file creation, modification, and deletion events.
-   [x] The service dispatches VDFS-specific events to the `EventBus`.
