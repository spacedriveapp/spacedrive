# Spacedrive V2 Whitepaper

### Executive Summary

[cite_start]Spacedrive is a **local-first Virtual Distributed File System (VDFS)** designed to solve the problem of data fragmentation across multiple devices and cloud services[cite: 4]. [cite_start]Unlike cloud-centric services like Dropbox or Google Drive, Spacedrive creates a unified, content-aware view of a user's files without requiring them to be moved from their original locations[cite: 4, 14]. [cite_start]It operates offline, minimizes reliance on central servers, and is built on a peer-to-peer (P2P) architecture that scales from single users to large organizations[cite: 5, 32].

The architecture is founded on four key principles:
1.  [cite_start]**A Unified Data Model**: A virtual layer that treats all files as content-addressable objects with rich metadata[cite: 17].
2.  [cite_start]**Safe & Predictable Operations**: A transactional system where all file operations can be simulated and previewed before execution[cite: 18, 19].
3.  [cite_start]**Resilient Synchronization**: A leaderless, hybrid P2P model that avoids distributed consensus by separating data into different ownership domains[cite: 20, 21, 22].
4.  [cite_start]**An AI-Native Agent Architecture**: A design where an AI agent can observe the file system index and propose organizational actions using the same safe, transactional model as human users[cite: 23, 24].

[cite_start]The reference implementation is written in Rust and demonstrates high performance, with sub-100ms semantic search latency and an approximate memory footprint of 150 MB for libraries containing over one million files[cite: 26].

---

### Core Architectural Components

#### 1. The VDFS (Virtual Distributed File System) Model

[cite_start]At the heart of Spacedrive is the VDFS, an abstraction layer that unifies a user's data[cite: 94].

* [cite_start]**The Library**: All of a user's metadata, configurations, and thumbnails are stored in a self-contained, portable directory called a `.sdlibrary`[cite: 103]. [cite_start]This makes backup and migration as simple as copying a single directory[cite: 104, 106].
* [cite_start]**The Entry-Centric Model**: Every file and directory is represented as a universal object called an **Entry**[cite: 108]. [cite_start]A key innovation is the "metadata-first" approach: an Entry is created almost instantly during file discovery, allowing it to be tagged or organized by the user immediately, while slower content analysis (like hashing) happens asynchronously in the background[cite: 110, 111, 112].
* [cite_start]**SdPath (Universal File Addressing)**: Spacedrive uses a universal addressing scheme, `SdPath`, that makes device boundaries transparent[cite: 129]. It supports:
    * [cite_start]**Physical Addressing**: `sd://<device_id>/path/to/file` points to a specific file on a specific device[cite: 142].
    * [cite_start]**Content-Aware Addressing**: `sd://content/<content_id>` acts as a handle for file content, regardless of location[cite: 130]. [cite_start]When a content-aware path is used, Spacedrive automatically performs **optimal path resolution**, finding the best available copy of the file based on locality, network speed, and device availability[cite: 134, 136].
* [cite_start]**Virtual Sidecar System**: Spacedrive never modifies original user files[cite: 146]. [cite_start]Instead, derivative data like thumbnails, OCR text, or AI-generated transcripts are stored in "sidecar" files within the Library, linked to the original Entry[cite: 147, 148].
* [cite_start]**Advanced File Type System**: A multi-method system accurately identifies files using a combination of file extensions, "magic byte" pattern matching, and content analysis, grouping them into 17 semantic categories (e.g., Image, Video, Code, Document) for intuitive organization[cite: 152, 153].

#### 2. Content Identity System: Deduplication and Redundancy

[cite_start]This system serves two purposes: reducing storage usage and tracking file redundancy for data protection[cite: 168].

* **Adaptive Hashing**: To balance performance and accuracy, Spacedrive uses a size-based hashing strategy. [cite_start]Small files (<100KB) are fully hashed using BLAKE3[cite: 171, 204]. [cite_start]Large files undergo **strategic sampling**, where only the header, footer, and four evenly-spaced chunks from the body are hashed[cite: 172]. [cite_start]This reduces the time to hash a 10GB file from over 30 seconds to under 100ms while maintaining over 99.9% deduplication accuracy[cite: 173].
* [cite_start]**Redundancy Analysis**: The system tracks how many copies of a file exist and where they are located[cite: 177]. [cite_start]This allows it to identify at-risk data (e.g., critical files with only one copy) and enables the AI to proactively suggest backups[cite: 180, 187].

#### 3. The Indexing Engine

The indexer is a multi-phase pipeline that builds and maintains the VDFS.

* [cite_start]**Five-Phase Pipeline**: The process is broken into five resumable stages: Discovery, Processing, Aggregation, Content Identification, and Analysis Queueing[cite: 195, 196, 197]. [cite_start]This ensures that if the process is interrupted, it can continue from where it left off[cite: 213].
* [cite_start]**Real-Time Monitoring**: Spacedrive uses platform-native file system watchers (e.g., FSEvents on macOS, inotify on Linux) to keep the index synchronized with on-disk changes in real-time[cite: 225, 230].
* [cite_start]**Offline Recovery**: When Spacedrive starts after being offline, it efficiently detects changes by comparing directory modification times against the "offline window," avoiding the need for a full, slow re-scan of the entire file system[cite: 243, 249, 250].
* [cite_start]**Remote Volume Support**: Through integration with the OpenDAL library, Spacedrive can index remote storage (like Amazon S3 or FTP servers) as if they were local drives, using efficient ranged reads to perform adaptive hashing without downloading the entire file[cite: 258, 262, 265].

#### 4. The Transactional Action System

[cite_start]This system fundamentally changes file management by treating all operations as transactions that can be previewed before execution[cite: 288].

* [cite_start]**Preview, Commit, Verify**: Before any file is moved, copied, or deleted, Spacedrive uses its complete index to run an in-memory simulation of the operation[cite: 289, 290]. [cite_start]It generates a detailed preview for the user, showing outcomes like storage changes, deduplication savings, and potential conflicts (e.g., insufficient space)[cite: 321].
* [cite_start]**Durable Execution**: Once approved by the user, the operation becomes a durable, resumable job that will continue to execute in the background, even across device disconnections and restarts[cite: 294, 295].

#### 5. Library Sync and Networking

[cite_start]Spacedrive uses a **leaderless, P2P hybrid model** for synchronization, which is simpler and more resilient than traditional consensus algorithms[cite: 359].

* **Domain Separation**: The key is to separate data by ownership:
    * **Device-Authoritative Data** (e.g., the index of files on a specific device): This data is synced using simple **state-based replication**. [cite_start]Since only one device can be the "source of truth" for its own files, write conflicts are impossible[cite: 361, 362].
    * **Shared Metadata** (e.g., tags, ratings): This data can be modified by any device. Conflicts are resolved deterministically using a lightweight, per-device change log where each change is timestamped with a **Hybrid Logical Clock (HLC)**. [cite_start]The change with the higher HLC timestamp wins (last-writer-wins)[cite: 364, 366, 367].
* [cite_start]**Iroh-Powered Networking**: All P2P communication (sync, file transfers, device discovery) is handled by a unified networking layer built on Iroh[cite: 370]. [cite_start]This provides superior NAT traversal, QUIC-based encryption, and reliable connectivity across consumer networks[cite: 371, 374].

#### 6. AI-Native Architecture

[cite_start]Spacedrive is designed with AI as a core component, not an add-on[cite: 434]. [cite_start]The VDFS index serves as a "world model" that an AI agent can reason about[cite: 435].

* **Agentic Loop (Observe, Orient, Act)**:
    1.  [cite_start]**Observe**: The AI observes the file index, which is enriched with semantic information (OCR text, image tags, video transcripts) generated by background analysis jobs[cite: 444, 445].
    2.  [cite_start]**Orient**: It analyzes this data and user history (from an audit log) to understand context and identify organizational patterns[cite: 448].
    3.  [cite_start]**Act**: The AI proposes actions (e.g., `FileCopyAction`, `BatchTagAction`) to the user[cite: 449]. [cite_start]Crucially, it uses the same safe, previewable Transactional Action System, ensuring the user always has final approval[cite: 450, 456].
* [cite_start]**Natural Language & Proactive Assistance**: This allows users to issue commands like "organize my tax documents from last year"[cite: 453]. [cite_start]The AI can also proactively suggest actions, such as offering to back up newly imported photos that lack redundancy or automatically sorting downloaded invoices into the correct folder[cite: 458, 462, 472].
* [cite_start]**Privacy-First AI**: The architecture is model-agnostic and supports local AI models via services like **Ollama**, ensuring user data never has to leave their device for analysis[cite: 438, 496].

#### 7. Temporal-Semantic Search

Spacedrive's search is designed to be fast, smart, and non-blocking.

* [cite_start]**Asynchronous Jobs**: All searches are executed as durable background jobs, so the UI is never blocked[cite: 501, 506].
* **Two-Stage Hybrid Process**:
    1.  [cite_start]**Fast Filtering**: A high-speed keyword search using SQLite's FTS5 index rapidly narrows down millions of files to a small set of relevant candidates[cite: 519].
    2.  [cite_start]**Semantic Re-ranking**: A lightweight local AI model then re-ranks only this small candidate set based on semantic similarity to the user's query[cite: 521, 522]. [cite_start]This delivers the power of semantic search with the speed of traditional search, achieving sub-100ms latency[cite: 523].

---

### Cloud Service Integration

The whitepaper outlines how this P2P architecture enables a unique cloud service. [cite_start]Instead of a traditional client-server model, a user's cloud instance is simply a managed, containerized version of the standard Spacedrive core engine[cite: 567, 569].

* [cite_start]**The Cloud as a Peer**: This "Cloud Core" has a regular device ID and is paired with the user's other devices using the same secure P2P protocol[cite: 571, 579]. [cite_start]There is no custom cloud API[cite: 573].
* [cite_start]**Seamless Operations**: Backing up files to the cloud becomes a standard `FileCopyAction` to another peer device[cite: 581, 582]. [cite_start]This elegant design provides cloud availability and backup while preserving the local-first security model and architecture[cite: 35].

### Security and Privacy

Security is a core tenet, addressed through a defense-in-depth strategy.

* [cite_start]**Data at Rest**: The Library database is transparently encrypted using SQLCipher, with keys derived from the user's password via PBKDF2[cite: 698, 699].
* [cite_start]**Data in Transit**: All network traffic is end-to-end encrypted using QUIC (with TLS 1.3) and features perfect forward secrecy[cite: 706, 707].
* [cite_start]**Credential Management**: Cloud service API keys are stored in a secure vault, encrypted with a master key derived from the user's password, and can be integrated with native OS credential stores like the macOS Keychain[cite: 709, 710].
* [cite_start]**Threat Model**: The design explicitly mitigates threats such as a stolen laptop, network eavesdropping, and a compromised cloud service[cite: 711, 712, 713].
