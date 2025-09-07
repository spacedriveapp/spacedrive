# Spacedrive V2 Project Roadmap

This document provides a complete overview of the Spacedrive V2 project, tracking its status against the vision outlined in the official whitepaper.

**Status Legend:**

- ✅ **Completed** - Feature is fully implemented and stable.
- ⏳ **In Progress** - Foundational work is complete, but the feature is not yet fully realized.
- 📝 **To Do** - Feature is planned but not yet started.

---

## VDFS Core

The foundational architecture for the Virtual Distributed File System. This layer is largely complete and forms the stable base for all other features.

- ✅ **Core Data Model**
  - ✅ Implement Entry-centric model for all files and directories.
  - ✅ Implement SdPath for universal, cross-device addressing.
  - ✅ Implement the Virtual Sidecar System schema for managing derivative data (thumbnails, OCR, etc.).
- ✅ **Content & Storage Abstraction**
  - ✅ Implement Content Identity system with adaptive hashing for global deduplication.
  - ✅ Implement Volume Management for detecting and classifying storage devices across platforms (macOS, Windows, Linux).
  - 📝 Implement StorageClass logic for intelligent data tiering between hot and cold storage.
- ✅ **Database & Performance**
  - ✅ Implement initial database schema using SeaORM with SQLite.
  - ✅ Implement Closure Table pattern for high-performance hierarchical queries.
  - ✅ Implement a sophisticated, multi-method File Type System with TOML definitions.
- ⏳ **Advanced Features**
  - ⏳ Implement the full Virtual Sidecar System with background jobs for generating derivative data.
  - 📝 Implement an Intelligent Undo system based on the audit_log to safely reverse operations.

---

## Indexing & File Management

The engine for discovering, processing, and managing user data. This system is robust and production-ready.

- ✅ **Indexing Engine**
  - ✅ Implement the resilient, multi-phase indexing pipeline (Discovery, Processing, Aggregation, Content ID).
  - ✅ Implement change detection using inode tracking for efficient incremental updates.
  - ✅ Implement support for both persistent (managed locations) and ephemeral (on-the-fly browsing) indexing modes.
- ✅ **Operations Engine**
  - ✅ Implement the Transactional Action System as the safe, user-facing entry point for all operations.
  - ✅ Implement the Durable Job System for background execution of long-running tasks like indexing and file transfers.
  - ✅ Implement remote action dispatch, allowing actions initiated on one device to execute on another.

---

## Networking

The P2P communication layer powered by Iroh. The foundation for device pairing and direct data transfer is complete.

- ✅ Integrate Iroh as the core P2P networking stack.
- ✅ Implement secure, multi-device pairing protocol.
- ✅ Implement cross-device file transfer for trusted, paired devices.
- 📝 Implement Spacedrop for ephemeral, AirDrop-style sharing between non-paired devices.
- 📝 Package and document the self-hosted relay infrastructure for private networks and enterprise deployments.
- 📝 Implement practical conflict resolution strategies for metadata synchronization.

---

## Security & Privacy

Architectural components designed to ensure user data remains private and secure. Foundational elements are in place, but advanced features are pending.

- ✅ Ensure End-to-End Encryption for all data in transit via the Iroh stack.
- ✅ Implement secure storage for device keys using the OS keychain.
- 📝 Implement at-rest encryption for Library databases using SQLCipher.
- 📝 Implement a cryptographically chained Audit Log to create a tamper-proof record of all operations.
- 📝 Implement Role-Based Access Control (RBAC) for enterprise and team collaboration features.
- 📝 Implement Certificate Pinning for all connections to third-party cloud storage providers.

---

## AI & Intelligence

Features that transform the VDFS into an intelligent, proactive data management system. The architectural groundwork is laid, with implementation of the AI agent pending.

- ⏳ **Temporal-Semantic Search**
  - ✅ FTS5 keyword search is implemented for high-speed temporal filtering.
  - ⏳ Implement the Unified Vector Repository system for efficient, distributed semantic search.
- 📝 Implement background jobs for OCR, image analysis, and transcription to enrich the VDFS index.
- 📝 **Develop the AI Agent for proactive assistance.**
  - 📝 Implement a service to analyze the audit_log for user organization patterns.
  - 📝 Create a mechanism for the agent to generate and propose Action previews to the user.
- 📝 Implement AI-Driven Storage Tiering suggestions based on file access patterns.

---

## Resource Management & Mobile

Ensures Spacedrive runs efficiently on all devices, from powerful desktops to battery-constrained mobile phones.

- 📝 Implement Adaptive Throttling for background jobs based on device power source (AC vs. battery) and thermal status.
- 📝 Implement network-aware synchronization to limit large data transfers on cellular networks.
- 📝 Develop mobile-specific background processing strategies for iOS and Android to comply with OS limitations.

---

## User Interface (UI)

The primary interfaces for user interaction with Spacedrive.

- ✅ Develop a robust Command-Line Interface (CLI) for all core operations.
- 📝 Develop a cross-platform Desktop GUI.
- 📝 Develop a Web Interface for remote access and management.