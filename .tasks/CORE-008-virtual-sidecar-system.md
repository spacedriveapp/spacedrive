---
id: CORE-008
title: Virtual Sidecar System
status: In Progress
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, sidecars, derivatives]
whitepaper: Section 4.1
---

## Description

Implement the Virtual Sidecar System for managing derivative data (e.g., thumbnails, OCR results, transcoded videos). This system will be responsible for creating, storing, and managing the lifecycle of these derivative files.

## Implementation Steps

1.  Design the database schema for storing information about sidecar files.
2.  Implement a `SidecarManager` service that can create and manage sidecar files.
3.  Integrate the `SidecarManager` with the Job System to allow for asynchronous generation of sidecars.
4.  Develop a system for associating sidecar files with their parent entries.

## Acceptance Criteria

- [ ] The system can generate thumbnails for images.
- [ ] The system can perform OCR on documents.
- [ ] The system can transcode videos into different formats.
- [ ] Sidecar files are correctly associated with their parent entries.
