<!--CREATED: 2025-07-30-->
Of course. Based on the V2 whitepaper, your design documents, and the current state of the codebase, here is a clear development roadmap to align the implementation with the full architectural vision.

This roadmap is sequenced to build foundational layers first, ensuring that complex features like AI and Sync are built on a stable and complete core.

Phase 1: Solidify the Core VDFS Foundation
This phase focuses on critical refactoring and completing the core data models. These changes are foundational and will impact almost every other part of the system, so they must be done first to avoid significant rework later.

1. Implement Closure Table Indexing:

Action: Refactor the database schema to replace the current materialized path storage with a closure table for hierarchical data.

Reasoning: This is a major architectural change that will dramatically improve the performance of all hierarchical queries (e.g., directory listings, subtree traversals, aggregate calculations), transforming them from O(N) string matches to O(1) indexed lookups. This is a prerequisite for a scalable system.

2. Finalize At-Rest Library Encryption:

Action: Implement the full library database encryption using SQLCipher. Derive keys from user passwords via PBKDF2 with unique per-library salts, as detailed in the design document.

Reasoning: Security must be built-in, not bolted on. Completing this now ensures all subsequent features operate on an encrypted-by-default storage layer.

3. Implement Native Storage Tiering Model:

Action: Enhance the Volume and Location data models to include PhysicalClass and LogicalClass properties, respectively. Implement the logic to determine the EffectiveStorageClass.

Reasoning: This provides the core system (Action System, Path Resolver) with a crucial understanding of storage capabilities, enabling intelligent warnings and performance optimizations.

4. Enhance the Indexing and Job Systems:

Action: Extend the existing indexing pipeline to fully realize the five phases described in the whitepaper: Discovery, Processing, Aggregation, Content ID, and Intelligence Queueing.

Reasoning: The Intelligence Queueing phase is the critical integration point for the future AI layer. It decouples core indexing from slower, AI-powered analysis jobs (like OCR or transcription), making the system more modular and resilient.

Phase 2: Implement Core Distributed Capabilities
With the local foundation solidified, this phase focuses on making Spacedrive a true distributed system by building the networking and synchronization layers from the ground up.

1. Build the Library Sync Module:

Action: Develop the Library Sync module based on the principles in SYNC_DESIGN.md. Implement the domain separation strategy: Index Sync (device authority), User Metadata Sync (union-merge), and File Operations (explicit actions).

Reasoning: This pragmatic approach avoids the "analysis paralysis" of overly complex CRDTs and provides tailored, effective conflict resolution for different data types. It is the heart of multi-device consistency.

2. Establish Robust P2P Networking with Iroh:

Action: Fully leverage the Iroh stack to handle all P2P communication. This includes implementing device discovery, achieving high-success-rate NAT traversal, and securing all transport with QUIC/TLS 1.3.

Reasoning: A single, unified networking layer is more reliable and maintainable than fragmented solutions. This provides the stable connections that Library Sync relies upon.

3. Develop Spacedrop for Ephemeral Sharing:

Action: Build the Spacedrop ephemeral file-sharing protocol on top of the Iroh networking layer, ensuring each transfer uses ephemeral keys for perfect forward secrecy.

Reasoning: This feature leverages the P2P foundation to provide a key user-facing capability (similar to AirDrop) and validates the flexibility of the networking stack.

Phase 3: Build the Intelligence Layer (AI-Native)
Now that data is reliably indexed and synchronized, you can build the intelligence features that make Spacedrive truly unique.

1. Implement Temporal-Semantic Search:

Action: Build the two-stage search architecture. First, implement fast temporal filtering using SQLite's FTS5. Second, integrate a lightweight embedding model (e.g., all-MiniLM-L6-v2) to create and query vector embeddings for semantic re-ranking.

Reasoning: This hybrid approach provides the speed of keyword search with the power of semantic understanding, achieving sub-100ms queries on consumer hardware as specified in the whitepaper.

2. Implement Extension-Based Agent System:

Action: Build the WASM extension runtime and SDK that enables specialized AI agents. This includes the agent context, memory systems (Temporal, Associative, Working), event subscription mechanism, and integration with the job system.

Reasoning: This provides the foundation for domain-specific intelligence through secure, sandboxed extensions. Each agent (Photos, Finance, Storage, etc.) can maintain its own knowledge base and react to VDFS events while using the same safe, transactional primitives as human users.

3. Implement the Virtual Sidecar System:

Action: Create the mechanism for generating and managing derivative data (thumbnails, OCR text, transcripts) within the .sdlibrary package, linking them to the original Entry without modifying the source file.

Reasoning: This system is the foundation for file intelligence. It provides the raw material (e.g., extracted text) that the search and AI agents need to function, while preserving the integrity of user files.

4. Integrate Local and Cloud AI Providers:

Action: Build a flexible AI provider interface. Prioritize integration with Ollama for local, privacy-first processing. Then, add support for cloud-based AI services with clear user consent and data handling policies.

Reasoning: This fulfills the whitepaper's promise of a privacy-first AI architecture, giving users complete control over where their data is processed.

Phase 4: Enhance User-Facing Features & Extensibility
With the core, distributed, and AI layers in place, this phase focuses on delivering the advanced capabilities and ecosystem integrations promised in the whitepaper.

1. Enhance the Transactional Action System:

Action: Fully implement the "preview-before-commit" simulation engine. Ensure every action can be pre-visualized, showing the exact outcome (space savings, conflicts, etc.) before it is committed to the durable job queue.

Reasoning: This is a cornerstone of Spacedrive's user experience, providing safety, transparency, and control over all file operations.

2. Build the Native Cloud Service Architecture:

Action: Develop the deployment model where a "Cloud Core" runs as a standard, containerized Spacedrive peer. All interactions should use the existing P2P protocols, requiring no custom cloud API.

Reasoning: This elegant architecture provides cloud convenience without sacrificing the local-first security model, demonstrating the power and flexibility of the VDFS design.

3. Implement the WASM Plugin System:

Action: Develop the WebAssembly-based plugin host. Expose a secure, capability-based VDFS API to the WASM sandbox, allowing for extensions like custom content type handlers and third-party cloud storage integrations.

Reasoning: This provides a safe and portable way to extend Spacedrive's functionality, fostering a community ecosystem without compromising the stability of the core system.

Phase 5: Harden for Production and Enterprise
The final phase focuses on the security, management, and scalability features required for a robust, multi-user production environment.

1. Implement Role-Based Access Control (RBAC):

Action: Build the RBAC system on top of the centralized Action System, enabling granular permissions for team and enterprise collaboration.

Reasoning: This is essential for any multi-user or enterprise deployment and relies on the Action System being complete.

2. Create a Cryptographically Immutable Audit Trail:

Action: Enhance the audit logging system to be cryptographically chained (e.g., using hashes of previous entries), making it tamper-proof.

Reasoning: This provides the strong security and compliance guarantees required for enterprise use cases.

3. Performance Tuning and Benchmarking:

Action: Conduct comprehensive performance testing to ensure the implementation meets or exceeds the benchmarks laid out in the whitepaper (e.g., indexing throughput, search latency, memory usage).

Reasoning: This validates that the architectural goals have been met in practice and ensures a smooth user experience at scale.
