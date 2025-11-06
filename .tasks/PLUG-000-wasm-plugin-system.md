---
id: PLUG-000
title: "Epic: WASM Extension System"
status: In Progress
assignee: james
priority: High
tags: [epic, plugins, wasm, extensibility, extensions]
whitepaper: Section 6.7
last_updated: 2025-11-05
---

## Description

This epic covers the implementation of the WebAssembly (WASM) based extension system. Extensions can define custom data models, create AI agents, and integrate seamlessly with Spacedrive's sync, search, and action systems.

**Architecture Clarification (Nov 5, 2025):** Extensions get BOTH:
- Their own database tables for domain models (managed by core, auto-sync)
- Location declarations (user chooses where extension files go)
- Files written to locations are automatically indexed by core VDFS
- Query via core entries table with sidecar filters (no custom query caches)

## Current Status

**Infrastructure:** Core WASM runtime is integrated and compiling. Extension SDK with `#[extension]` and `#[job]` macros is functional. Test extensions exist and compile to WASM.

**In Progress:**
- Location declaration and management system
- Core VDFS query API for extensions (sidecar filters)
- Complete host function bridge
- Production extensions (Photos, Ledger, Email Archive)

**Reference:**
- `core/src/infra/extension/README.md` - Implementation details
- `workbench/sdk/EXTENSION_SDK_SPECIFICATION_V2.md` - Complete SDK spec
- `workbench/core/extensions/EXTENSION_DATA_STORAGE_PHILOSOPHY.md` - Storage model
