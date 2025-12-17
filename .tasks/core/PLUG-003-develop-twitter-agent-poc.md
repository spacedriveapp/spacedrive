---
id: PLUG-003
title: Develop Production Extension (Photos or Email)
status: To Do
assignee: jamiepine
parent: PLUG-000
priority: Medium
tags: [plugins, wasm, extension, production]
whitepaper: Section 6.8
last_updated: 2025-10-14
related_tasks: [PLUG-001, PLUG-002]
---

## Description

Develop a production-ready extension as a real-world validation of the WASM extension system. This will serve as the canonical example for third-party developers and demonstrate the full capabilities of the extension platform.

**Candidates:**

- **Photos Extension**: AI-powered photo management (face recognition, places, moments) - Currently "In Progress"
- **Email Archive Extension**: Gmail/Outlook ingestion with OCR and classification - Design complete

## Implementation Steps

1.  Complete PLUG-002 (VDFS Plugin API Bridge) first
    - Ensure host_spacedrive_call() is fully functional
    - Add required operations: ai.ocr, ai.classify_text, vdfs.write_sidecar
2.  Develop the extension as a Rust project using spacedrive-sdk
    - Use #[extension], #[job], #[agent] macros
    - Implement core functionality
3.  Create extension-specific models and database tables
    - Define data schema (e.g., Person, Face, Album for Photos)
    - Use #[model] macro for real SQL tables
4.  Implement job pipeline with progress tracking
    - Discovery, processing, and persistence phases
    - Checkpointing for resumability
5.  Add CLI/UI integration
    - Extension installation flow
    - Configuration UI
    - Job dispatch and monitoring

## Acceptance Criteria

- [ ] Extension can be loaded and initialized by the `PluginManager`
- [ ] Extension creates and queries its own database tables
- [ ] Extension can dispatch jobs with full progress tracking
- [ ] Extension integrates with AI operations (OCR, classification, embeddings)
- [ ] Extension data is searchable and accessible in the library
- [ ] Extension can be distributed as a standalone .wasm + manifest.json

## Implementation Files

**Extension Code:**

- extensions/photos/ - Photos extension (in progress)
- extensions/finance/ - Finance extension (planned)

**Supporting Infrastructure:**

- core/src/ops/extension_test/ - Test operations
- workbench/core/extensions/ - Design documents

## Notes

- **Supersedes**: Original PLUG-003 (Twitter Archive) is outdated
- **Current Focus**: Photos extension is partially implemented
- **Reference**: See docs/extensions/ for SDK documentation and examples/
