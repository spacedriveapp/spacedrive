---
id: INDEX-000
title: "Epic: Hybrid Indexing Engine"
status: Done
assignee: jamiepine
priority: High
tags: [epic, core, indexing]
whitepaper: Section 4.3
last_updated: 2025-12-16
---

## Description

The hybrid indexing engine is Spacedrive's core filesystem discovery and processing system. It layers an ultra-fast, in-memory ephemeral index over a robust SQLite-backed persistent index, enabling instant browsing of unmanaged locations (like a file manager) while seamlessly upgrading paths to managed libraries (like a DAM) without UI flicker.

## Architecture

- **Ephemeral Layer**: Memory-resident index for instant browsing of external drives and unmanaged paths
- **Persistent Layer**: SQLite-backed index with full change tracking, sync, and content analysis
- **Five-Phase Pipeline**: Discovery → Processing → Aggregation → Content Identification → Finalizing
- **Change Detection**: Dual-mode system with batch ChangeDetector and real-time ChangeHandler trait
- **Database Architecture**: Closure tables for O(1) hierarchy queries and directory path caching

## Key Features

- Instant browsing of millions of files in RAM (~50 bytes per entry)
- Seamless promotion from ephemeral to persistent with UUID preservation
- Multi-phase indexing with resumable jobs
- Real-time filesystem watching via unified ChangeHandler
- Intelligent rules engine with .gitignore integration
- Index verification and integrity checking
