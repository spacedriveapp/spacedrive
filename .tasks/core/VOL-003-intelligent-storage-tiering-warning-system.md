---
id: VOL-003
title: Intelligent Storage Tiering Warning System
status: To Do
assignee: james
parent: VOL-000
priority: Medium
tags: [volume, storage-tiering, warnings, ai]
whitepaper: Section 4.8
---

## Description

Implement the intelligent warning system that alerts the user when there is a mismatch between a Location's `LogicalClass` and its underlying Volume's `PhysicalClass`.

## Implementation Steps

1.  Develop a service that periodically checks for mismatches between `LogicalClass` and `PhysicalClass`.
2.  Implement the logic to generate a warning when a mismatch is detected (e.g., a "Hot" Location on an "HDD" Volume).
3.  The warning should explain the potential performance implications and suggest a solution (e.g., moving the Location to a faster Volume).
4.  Integrate the warning system with the UI to display the warnings to the user.

## Acceptance Criteria

- [ ] The system can detect mismatches between `LogicalClass` and `PhysicalClass`.
- [ ] The system generates a clear and helpful warning message for the user.
- [ ] The user is notified of the warning through the UI.
