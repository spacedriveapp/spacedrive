<p align="center">
  <img width="150" height="150" src="packages/assets/images/AppLogoV2.png" alt="Spacedrive Logo">
  <h1 align="center">Spacedrive</h1>
  <p align="center">
  	The operating system for your personal data
    <br />
    <a href="https://spacedrive.com"><strong>spacedrive.com</strong></a>
    ·
    <a href="https://discord.gg/gTaF2Z44f5"><strong>Discord</strong></a>
    ·
    <a href="https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md"><strong>Read the Whitepaper</strong></a>
  </p>
  <p align="center">
    <a href="https://discord.gg/gTaF2Z44f5">
      <img src="https://img.shields.io/discord/949090953497567312?label=Discord&color=5865F2" />
    </a>
    <a href="https://www.gnu.org/licenses/agpl-3.0">
      <img src="https://img.shields.io/static/v1?label=Licence&message=AGPL%20v3&color=000" />
    </a>
    <a href="https://github.com/spacedriveapp/spacedrive">
      <img src="https://img.shields.io/static/v1?label=Core&message=Rust&color=DEA584" />
    </a>
    <a href="https://github.com/spacedriveapp/spacedrive/tree/main/extensions">
      <img src="https://img.shields.io/static/v1?label=Ecosystem&message=WASM&color=63B17A" />
    </a>
  </p>
</p>

Spacedrive is an open source file manager that unifies your files across all your devices. Built on a **Virtual Distributed File System (VDFS)** written in Rust, it turns a scattered collection of files into a single, organized library you can access from anywhere.

---

## Key Architectural Principles

Spacedrive is built on four foundational principles:

- **Unified Data Model**: A content-aware VDFS that treats files as first-class objects with rich metadata, enabling deduplication and redundancy tracking.
- **Safe Operations**: Transactional actions that simulate and preview changes before execution, ensuring predictability.
- **Resilient Sync**: Leaderless P2P synchronization with domain separation for conflict-free replication.
- **AI-Native Design**: Extension-based agents for semantic search, automation, and natural language queries.
  These enable offline-first operation, sub-100ms semantic search, and efficient management of libraries with over a million files.

## Features

| Feature                 | Description                                                                                                                                   | Status      |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| **Filesystem Indexing** | A multi-phase, resumable pipeline that discovers and processes file metadata. Uses real-time watchers and efficient offline change detection. | Done        |
| **Durable Jobs**        | A resilient system that executes long-running tasks as durable jobs that survive restarts and network interruptions.                          | Done        |
| **Actions**             | A transactional system where all operations are validated and can be previewed before execution, ensuring safe and predictable outcomes.      | Done        |
| **Storage Volumes**     | Automatically discovers, classifies, and monitors all storage locations, from internal drives to network shares.                              | Done        |
| **Device Sync**         | Leaderless, offline-first library (metadata) synchronization between peers.                                                                   | Done        |
| **Networking**          | Secure peer-to-peer device pairing using Iroh, local first with E2E encrypted cloud relay fallback.                                           | Done        |
| **Semantic Tags**       | Graph-based tagging with contextual disambiguation, hierarchies, aliases, and compositional attributes for nuanced organization.              | In Progress |
| **Spacedrop**           | AirDrop-style P2P file sharing between devices with automatic protocol selection and consent-based transfers.                                 | In Progress |
| **Content Identity**    | Blake3-based content addressing with adaptive hashing (sampling for large files) enabling cross-device deduplication.                         | Done        |
| **File Type Detection** | Extension and magic byte matching with priority-based disambiguation across 100+ file types and MIME mappings.                                | Done        |
| **Search**              | Combines high-speed keyword filtering (FTS5) with semantic re-ranking for natural language queries.                                           | In Progress |
| **Extensions**          | Extend core functionality into domain-specific use cases with sandboxed WASM extensions.                                                      | In Progress |
| **Third-Party Cloud**   | Connect S3, Google Drive, Dropbox as cloud volumes, cloud indexing.                                                                           | In Progress |
| **Virtual Sidecars**    | Manage derivitive data automatically; thumbnails, proxy media, OCR text extraction, Live Photos and more.                                     | Done        |
| **Library Encryption**  | At-rest encryption using OS keychain for key storage (SQLCipher integration pending).                                                         | In Progress |
| **AI & Intelligence**   | An observe-orient-act event loop for autonomous agents to perform tasks like file organization and analysis.                                  | Planned     |

## How it Works

The heart of Spacedrive is the **Virtual Distributed File System (VDFS)**. It indexes your files in a local-first database, creating a unified view of your data. It doesn't matter if a file is on `C:\Users\...` or `~/Documents`—Spacedrive makes it accessible from any of your connected devices.

The true power of Spacedrive is its extensibility. A sandboxed **WASM-based extension system** allows for the creation of powerful plugins that can introduce new features, data models, and AI agents. With a comprehensive Rust SDK, developers can build first-class extensions that are indistinguishable from core functionality.

## Platform Support

| Platform    | Core (Rust) | CLI       | GUI         |
| ----------- | ----------- | --------- | ----------- |
| **macOS**   | Available   | Available | Available   |
| **Windows** | Available   | Available | In Progress |
| **Linux**   | Available   | Available | In Progress |
| **iOS**     | Available   | N/A       | Available   |
| **Android** | Available   | N/A       | In Progress |
| **Web**     | N/A         | N/A       | In Progress |

## Extensions

## Privacy & Security First

Your privacy is paramount. Spacedrive is a **local-first** application. Your data and metadata live on your devices.

- **End-to-End Encryption:** All network traffic is encrypted using modern protocols.
- **Encryption-at-Rest:** Libraries can be encrypted on disk with SQLCipher, protecting your data if a device is lost or stolen.
- **No Central Servers:** Your files are your own. Optional cloud integration is available for backup and remote access, but it's never required.

## Get Involved

- **⭐ Star the repo** to show your support.
- **💬 Join the [Discord](https://discord.gg/gTaF2Z44f5)** to chat with the developers and community.
- **📖 Read the [Whitepaper](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)** to understand the full vision.
- **🧩 Build an Extension:** Check out the [SDK documentation](docs/sdk.md) and create your own extensions.

---

<p align="center">
  <em>Your files, unified. Your data, private. Your experience, limitless.</em>
</p>
