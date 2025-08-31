---
id: CORE-003
title: Implement Content Identity System for Deduplication
status: Done
assignee: james
parent: CORE-000
priority: High
tags: [core, vdfs, deduplication, hashing]
whitepaper: Section 4.2
---

## Description

Implemented the Content Identity system, which forms the foundation for data deduplication and redundancy tracking. It uses an adaptive hashing strategy to efficiently fingerprint files.

## Implementation Notes
-   The core logic is in `src/domain/content_identity.rs`.
-   The `ContentHashGenerator` uses a fast, sampled BLAKE3 hash for large files (>100KB) and a full hash for smaller files, as described in the whitepaper.
-   The corresponding `content_identities` table in the database schema stores these hashes.

## Acceptance Criteria
-   [x] `ContentHashGenerator` can produce deterministic hashes for files.
-   [x] The system correctly uses different hashing strategies for small and large files.
-   [x] The database schema supports storing content hashes and linking them to entries.
