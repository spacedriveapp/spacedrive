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

**Now in Version 2.0:** (Nov 2025) After 3 years of learning from V1's architectural challenges, Spacedrive has been reimagined from the ground up. V2 introduces universal file addressing with **SdPath** that makes device boundaries transparent, an entry-centric model for instant organization, domain-separated sync that actually works, and an event-driven architecture that eliminates coupling. Every fatal flaw from V1 has been systematically addressed, resulting in a production-ready foundation that delivers on the original vision.

---

## Architectural Principles

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

Spacedrive's WASM-based extension system enables both professional tools and data archival capabilities. Extensions can share models and build on each other's data.

### Professional Extensions

Our lineup of industry extensions.

| Extension     | Purpose                         | Key Features                                                                | Status      |
| ------------- | ------------------------------- | --------------------------------------------------------------------------- | ----------- |
| **Guardian**  | Backup & redundancy monitoring  | Content identity tracking, zero-redundancy alerts, smart backup suggestions | Planned     |
| **Photos**    | AI-powered photo management     | Face recognition, place identification, moments, scene classification       | In Progress |
| **Chronicle** | Research & knowledge management | Document analysis, knowledge graphs, AI summaries                           | In Progress |
| **Ledger**    | Financial intelligence          | Receipt OCR, expense tracking, tax preparation                              | Planned     |
| **Atlas**     | Dynamic CRM & team knowledge    | Runtime schemas, contact tracking, deal pipelines                           | In Progress |
| **Cipher**    | Security & encryption           | Password manager, file encryption, breach alerts                            | Planned     |
| **Studio**    | Digital asset management        | Scene detection, transcription, proxy generation                            | Planned     |

### Open Source Archive Extensions

| Extension           | Purpose                 | Provides Data For        | Status  |
| ------------------- | ----------------------- | ------------------------ | ------- |
| **Email Archive**   | Gmail/Outlook backup    | Atlas, Ledger, Chronicle | Planned |
| **Chrome History**  | Browsing history backup | Chronicle                | Planned |
| **Spotify Archive** | Listening history       | Analytics                | Planned |
| **GPS Tracker**     | Location timeline       | Photos, Analytics        | Planned |
| **Tweet Archive**   | Twitter backup          | Chronicle, Analytics     | Planned |
| **GitHub Tracker**  | Repository tracking     | Chronicle                | Planned |

## Privacy & Security First

Your privacy is paramount. Spacedrive is a **local-first** application. Your data and metadata live on your devices.

- **End-to-End Encryption:** All network traffic is encrypted using modern protocols.
- **Encryption-at-Rest:** Libraries can be encrypted on disk with SQLCipher, protecting your data if a device is lost or stolen.
- **No Central Servers:** Your files are your own. Optional cloud integration is available for backup and remote access, but it's never required.

## Tech Stack & Architecture

### Core Technologies

- **Rust** - High-performance core with async/await throughout (Tokio runtime)
- **TypeScript** - Type-safe interfaces and UI logic
- **React** - Cross-platform UI with Tauri (desktop) and React Native (mobile)
- **SQLite** - Local-first database with SeaORM for type-safe queries
- **Iroh** - P2P networking with QUIC transport and hole-punching

### Project Structure

```
spacedrive/
├── core/              # Rust VDFS implementation
│   ├── src/
│   │   ├── domain/    # Core models (Entry, Library, Device)
│   │   ├── ops/       # CQRS operations (actions & queries)
│   │   ├── infra/     # Infrastructure (DB, events, jobs)
│   │   └── service/   # High-level services (network, sessions)
│   └── bin/           # CLI and daemon binaries
├── apps/
│   ├── cli/           # Command-line interface
│   ├── desktop/       # Tauri desktop app (macOS, Windows, Linux)
│   ├── mobile/        # React Native mobile app (iOS, Android)
│   └── web/           # Web interface (connects to daemon/cloud)
├── extensions/        # WASM extensions (Photos, Chronicle, etc.)
├── packages/          # Shared TypeScript packages
├── crates/            # Shared Rust crates
└── docs/              # Architecture docs and whitepaper
```

### Architecture Highlights

- **Entry-Centric Model**: Every file/directory is an Entry with optional content identity
- **SdPath Addressing**: Universal file addressing that works across devices (`sd://device/{id}/path` or `sd://content/{cas_id}`)
- **Event-Driven**: EventBus eliminates coupling between subsystems
- **CQRS Pattern**: Separate Actions (mutations) and Queries (reads) with preview-commit-verify
- **Durable Jobs**: Long-running operations survive app restarts with MessagePack serialization
- **Domain-Separated Sync**: Leaderless P2P sync with clear boundaries between local and shared data

## Getting Started

### Prerequisites

- **Rust** 1.81+ ([rustup](https://rustup.rs/))
- **Node.js** 18.18+ ([nvm](https://github.com/nvm-sh/nvm) recommended)
- **pnpm** 9.4.0+ (`npm install -g pnpm`)

### Quick Start

```bash
# Clone the repository
git clone https://github.com/spacedriveapp/spacedrive
cd spacedrive

# Run setup script (installs dependencies)
./scripts/setup.sh  # macOS/Linux
# or
.\scripts\setup.ps1  # Windows (PowerShell)

# Install Node dependencies
pnpm i

# Build dependencies and run codegen
pnpm prep

# Run the desktop app
pnpm tauri dev

# Or run the CLI
cargo run -p sd-cli -- --help
```

### Development Workflow

**Desktop App**

```bash
pnpm tauri dev  # Runs core + React UI in Tauri window
```

**Web Interface**

```bash
cargo run -p sd-server  # Start backend daemon
pnpm web dev            # Start web dev server (localhost:3000)
```

**CLI Development**

```bash
cargo run -p sd-cli -- library create "My Library"
cargo run -p sd-cli -- location add ~/Documents
cargo run -p sd-cli -- index ~/Documents
```

**Mobile Development**

```bash
./scripts/setup.sh mobile  # Install mobile deps
pnpm mobile android        # Android development
pnpm mobile ios            # iOS development (macOS only)
```

### Useful Commands

- `pnpm autoformat` - Format code (Rust + TS)
- `pnpm clean` - Remove build artifacts
- `pnpm test-data` - Download sample files for testing (macOS/Linux)
- `cargo test` - Run Rust tests
- `pnpm test:e2e` - Run end-to-end tests

For detailed contribution guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Get Involved

- **⭐ Star the repo** to show your support.
- **💬 Join the [Discord](https://discord.gg/gTaF2Z44f5)** to chat with the developers and community.
- **📖 Read the [Whitepaper](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)** to understand the full vision.
- **🧩 Build an Extension:** Check out the [SDK documentation](docs/sdk.md) and create your own extensions.

---

<p align="center">
  <em>Your files, unified. Your data, private. Your experience, limitless.</em>
</p>
