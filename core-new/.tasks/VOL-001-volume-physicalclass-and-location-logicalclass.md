---
id: VOL-001
title: Volume PhysicalClass and Location LogicalClass
status: To Do
assignee: unassigned
parent: VOL-000
priority: High
tags: [volume, storage-tiering, classification]
whitepaper: Section 4.8
---

## Description

Implement the `PhysicalClass` for Volumes and `LogicalClass` for Locations. This is the foundation of the intelligent storage tiering system, allowing Spacedrive to understand the physical characteristics of storage devices and the user's intent for the data stored on them.

## Implementation Steps

1.  Define the `PhysicalClass` enum (e.g., `SSD`, `HDD`, `Cloud`) for Volumes.
2.  Define the `LogicalClass` enum (e.g., `Hot`, `Warm`, `Cold`) for Locations.
3.  Implement the logic to associate a `PhysicalClass` with each `Volume`.
4.  Implement the logic to allow users to assign a `LogicalClass` to each `Location`.

## Acceptance Criteria
-   [ ] The `PhysicalClass` and `LogicalClass` enums are defined.
-   [ ] The system can correctly identify the `PhysicalClass` of a Volume.
-   [ ] A user can assign a `LogicalClass` to a Location.
