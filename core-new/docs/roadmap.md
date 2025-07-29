Of course. Based on the extensive Rust codebase and the list of design documents you've provided, here is a comprehensive roadmap for Spacedrive V2. This roadmap outlines the current state of the project, the features actively in development, and the path toward feature completeness.

---

# Spacedrive V2: Core Engine Roadmap

This document outlines the development roadmap for the Spacedrive V2 core engine (`sd-core-new`). The V2 rewrite establishes a robust, scalable, and extensible foundation designed to overcome the architectural limitations of V1 and fully realize the vision of a unified, intelligent, local-first file system.

### Guiding Principles

- **Rust First**: Leverage Rust's safety, performance, and concurrency for a reliable core.
- **Local-First & P2P**: Data and control remain on user devices. The network enhances, it doesn't centralize.
- **AI-Native**: The architecture is built from the ground up to support intelligent, agentic file management.
- **Modular & Extensible**: A clean separation of concerns allows for independent development and future extension.
- **Test-Driven**: A comprehensive integration test framework ensures stability and reliability for core features.

---

## Phase 1: Foundation & Core Services (Largely Complete ✅)

This phase focused on building the non-negotiable, foundational components of the new architecture. These systems are stable, well-tested, and provide the bedrock for all other features.

| Component                        | Status         | Notes                                                                                                                                                                                         |
| :------------------------------- | :------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Core Engine Lifecycle**        | ✅ Implemented | [cite\_start]The `Core` struct manages the application lifecycle, including startup, configuration, and graceful shutdown. [cite: 3099, 3140]                                                 |
| **Configuration Management**     | ✅ Implemented | [cite\_start]A versioned `AppConfig` handles all application-level settings, with support for migrations. [cite: 3413, 3419]                                                                  |
| **Device Management**            | ✅ Implemented | [cite\_start]The `DeviceManager` provides persistent, cross-session device identity, a prerequisite for all P2P operations. [cite: 3941]                                                      |
| **Library Management**           | ✅ Implemented | [cite\_start]The `LibraryManager` handles the full lifecycle of `.sdlibrary` containers, including creation, locking for safe access, discovery, and closure. [cite: 3312, 3460]              |
| **Extensible Job System**        | ✅ Implemented | A highly modular job system with automatic registration via a `#[derive(Job)]` macro. [cite\_start]Supports resumable, persistent jobs with progress tracking. [cite: 3369, 3171, 4364]       |
| **Event Bus**                    | ✅ Implemented | [cite\_start]A decoupled, publish-subscribe event bus (`EventBus`) for inter-service communication, replacing the brittle patterns of V1. [cite: 3154]                                        |
| **Command Line Interface (CLI)** | ✅ Implemented | [cite\_start]A robust CLI provides user interaction with the core engine via a daemon client, featuring structured, format-agnostic output (human, JSON). [cite: 3411, 4096]                  |
| **Test Framework**               | ✅ Implemented | [cite\_start]A custom multi-process test runner (`CargoTestRunner`) enables realistic, end-to-end testing of distributed scenarios like device pairing and file transfers. [cite: 3099, 3257] |

---

## Phase 2: VDFS, Networking & File Operations (In Progress 🚧)

This is the current focus of active development. This phase brings the Virtual Distributed File System (VDFS) to life by implementing the core indexing logic, networking layer, and fundamental file operations.

| Feature                                | Status         | Notes & Design Docs                                                                                                                                                                                                                                                                                                              |
| :------------------------------------- | :------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Volume-Aware Storage Foundation**    | ✅ Implemented | The `VolumeManager` performs platform-aware detection, classification, and performance testing of storage devices. [cite\_start]This provides the foundation for intelligent, cross-volume file operations. [cite: 3110, 4522] \<br\> _Design: `VOLUME_CLASSIFICATION_DESIGN.md`, `VOLUME_TRACKING_IMPLEMENTATION_PLAN.md`_      |
| **File Type System**                   | ✅ Implemented | [cite\_start]A registry-based system (`FileTypeRegistry`) identifies files using a combination of extensions, magic bytes, and content analysis. [cite: 3376, 3407] \<br\> _Design: `DESIGN_FILE_TYPE_SYSTEM.md`_                                                                                                                |
| **Transactional Action System**        | ✅ Implemented | The `ActionManager` provides a central point for dispatching all user operations. [cite\_start]It ensures validation, audit logging, and consistent execution. [cite: 3496, 3099] \<br\> _Design: `ACTION_SYSTEM_DESIGN.md`_                                                                                                     |
| **File Operations (Copy/Move/Delete)** | ✅ Implemented | Core file operations are implemented as durable jobs. [cite\_start]The copy operation uses a strategy pattern to automatically select the optimal method (e.g., atomic rename for same-volume moves, streaming for cross-volume copies). [cite: 3616, 3546, 3547] \<br\> _Design: `CROSS_PLATFORM_COPY_AND_VOLUME_AWARENESS.md`_ |
| **VDFS Indexing Engine**               | 🚧 In Progress | The multi-phase indexer (`IndexerJob`) is functional, including smart filtering, inode-based change detection, and resumability. [cite\_start]The next step is to integrate the designed Indexer Rules System. [cite: 3173, 3555] \<br\> _Design: `INDEXER_ANALYSIS.md`, `INDEXER_RULES_SYSTEM.md`_                              |
| **P2P Networking (Iroh)**              | 🚧 In Progress | The unified networking layer using **Iroh** is operational. End-to-end tests confirm device pairing and persistence across restarts. [cite\_start]Ongoing work focuses on hardening connections and improving reliability. [cite: 3245, 4619] \<br\> _Design: `NETWORKING_SYSTEM_DESIGN.md`, `IROH_MIGRATION_DESIGN.md`_         |
| **Spacedrop (P2P File Transfer)**      | 🚧 In Progress | The core file transfer protocol is implemented and tested. It supports streaming large files between paired devices. [cite\_start]The next step is to build the ephemeral, AirDrop-like user experience. [cite: 3245, 4821] \<br\> _Design: `SPACEDROP_DESIGN.md`, `SPACEDROP_IMPLEMENTATION_PLAN.md`_                           |

---

## Phase 3: Intelligence & User Experience (Next Up 📝)

With the foundation and VDFS in place, this phase will focus on building the intelligent features and synchronization logic that define the Spacedrive user experience. These features have been designed and are ready for implementation.

| Feature                         | Status      | Notes & Design Docs                                                                                                                                                                                                                  |
| :------------------------------ | :---------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Library Sync**                | 📝 Designed | The core design uses a domain-separated model to avoid the complexity of CRDTs. Implementation will begin once the P2P networking layer is fully stabilized. \<br\> _Design: `SYNC_DESIGN.md`, `SYNC_INTEGRATION_NOTES.md`_          |
| **AI Agent Manager**            | 📝 Designed | A framework for AI agents to observe the VDFS state (via the Event Bus) and propose actions. This will power all proactive and intelligent features. \<br\> _Design: `AGENT_MANAGER_DESIGN.md`_                                      |
| **Lightning Search**            | 📝 Designed | A hybrid search architecture combining fast full-text search (FTS5) with semantic vector search for re-ranking. This enables natural language queries without sacrificing performance. \<br\> _Design: `LIGHTNING_SEARCH_DESIGN.md`_ |
| **Thumbnail Generation System** | 📝 Designed | A dedicated, resumable job for generating thumbnails for various media types (images, videos via ffmpeg feature flag, PDFs). \<br\> _Design: `THUMBNAIL_SYSTEM_DESIGN.md`_                                                           |
| **GUI & Mobile Clients**        | 💡 Planned  | Development of the Tauri (desktop) and React Native (mobile) frontends will begin, connecting to the core engine via a GraphQL API.                                                                                                  |

---

## Phase 4: Ecosystem & Enterprise (Future 💡)

This phase extends Spacedrive from a personal tool into a comprehensive platform for teams and organizations, introducing cloud services, collaboration, and enterprise-grade features.

| Feature                            | Status     | Notes                                                                                                                                                                                                   |
| :--------------------------------- | :--------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Native Cloud Service**           | 💡 Planned | As outlined in the whitepaper, a cloud backend will be offered where each user gets a dedicated, isolated `sd-core-new` instance that acts as a native P2P peer in their network.                       |
| **Team Libraries & Collaboration** | 💡 Planned | Extend the VDFS model to support shared Libraries with Role-Based Access Control (RBAC), building on the foundation of the Action System.                                                               |
| **Third-Party Integrations**       | 💡 Planned | Develop a stable GraphQL API and an extension system to allow integration with other tools and services (e.g., cloud storage providers, productivity apps).                                             |
| **Advanced Storage Tiering**       | 💡 Planned | Leverage the AI Agent and Volume-Aware Storage Foundation to automatically migrate "cold" data to slower, cheaper storage (like a NAS or cloud archive) while keeping "hot" data on fast local storage. |
| **Federated Learning**             | 💡 Planned | Explore privacy-preserving federated learning models to improve the AI agent's organizational suggestions based on anonymized, aggregate user patterns without compromising individual data.            |

### Future Optimizations & Research

- **Closure Table for Hierarchical Queries**: A planned database optimization to replace materialized path queries for directory hierarchies. This will provide O(1) lookups for subtree operations, dramatically improving UI responsiveness for very large directories. (_Design: `CLOSURE_TABLE_INDEXING_PROPOSAL.md`_)
- **Content-Defined Chunking**: Investigate integrating content-defined chunking (e.g., using rolling hashes) to enable block-level deduplication and more efficient P2P file transfers, especially for large, frequently modified files like virtual machine images.
