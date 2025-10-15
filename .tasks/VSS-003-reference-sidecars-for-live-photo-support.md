---
id: VSS-003
title: "Reference Sidecars for Live Photo Support"
status: To Do
assignee: james
parent: VSS-000
priority: Medium
tags: [vss, feature, photos, indexing]
whitepaper: "Section 4.1.4"
---

## Description

[cite_start]Implement the "Reference Sidecar" feature as described in **REFERENCE_SIDECARS.md** [cite: 5495-5498] [cite_start]and **VIRTUAL_SIDECAR_SYSTEM.md** [cite: 5564-5572]. This allows Spacedrive to track pre-existing files (like the video component of a Live Photo) as virtual sidecars without moving them from their original locations.

## Implementation Notes

- [cite_start]Add the `source_entry_id: Option<i32>` column to the `sidecars` table in the database to link the sidecar record to the original file's `Entry`[cite: 5496].
- [cite_start]During indexing, when an image and its matching video component are detected, create a `sidecar` record for the video file with its `source_entry_id` pointing to its own `Entry` record [cite: 5497-5498].
- The video file must remain in its original location on disk.
- [cite_start]Implement a new `Action` to allow users to "convert" a reference sidecar into a managed sidecar, which will move the file into the `.sdlibrary/sidecars` directory[cite: 5496].

## Acceptance Criteria

- [ ] The indexer correctly identifies Live Photo pairs (image + video).
- [ ] The video component is recorded as a "reference" sidecar for the image's content.
- [ ] The video file is NOT moved from its original location during indexing.
- [ ] A user can successfully trigger an action to convert the reference into a managed sidecar, moving the file into the library.
