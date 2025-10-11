<p align="center">
  <img width="150" height="150" src="packages/assets/images/AppLogoV2.png" alt="Spacedrive Logo">
  <h1 align="center">Spacedrive</h1>
  <p align="center">
    The file explorer for your entire digital life.
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

Your files are scattered across devices, cloud accounts, and external drives. Spacedrive unifies them into a single, breathtakingly fast library, no matter where they are. It's a file manager from the future, built on a **Virtual Distributed File System (VDFS)** and made infinitely powerful by a sandboxed **WASM extension system**.

Browse, search, and organize everything from one place. Device boundaries disappear.

---

## Core Features

Spacedrive is built on a set of foundational pillars designed for performance, privacy, and scalability. The entire core is implemented in **Rust**, with a clean architecture based on CQRS and Domain-Driven Design principles.

#### 1. The Virtual Distributed File System (VDFS)

The VDFS is the heart of Spacedrive. It creates a unified namespace for all your data using a universal addressing system called `SdPath`. It doesn't matter if a file is on `C:\Users\...` or `~/Documents`—Spacedrive gives it a stable, virtual address, allowing you to interact with it from any connected device.

#### 2. AI-Powered Semantic Search

Go beyond filename matching. Spacedrive's search engine indexes the content of your documents, images, and media. Find files with natural language queries like _"tax documents from last year"_ or _"sunset photos from my Hawaii vacation."_ It combines full-text search, semantic re-ranking, and vector search to deliver instant, intelligent results.

#### 3. Leaderless, Offline-First Sync

Spacedrive uses a novel, **leaderless synchronization model** that makes it faster and more resilient than traditional cloud services. Changes are efficiently transferred directly between your devices using the **Iroh** P2P networking library. It works perfectly offline, and your data is never stored on a central server unless you want it to be.

#### 4. Transactional, Verifiable Actions

All file operations (copy, move, delete) are treated as **resumable, transactional jobs**. You can preview the outcome of an operation before it executes, preventing errors. The system guarantees that operations will eventually complete, even if a device goes offline midway through a transfer.

## Flagship Extensions

The true power of Spacedrive is realized through its extension system. These powerful add-ons can deeply integrate with the VDFS, introducing new data models, AI agents, and UI components.

- **Data Guardian:** An essential utility that monitors the health of your library. It identifies data rot, finds duplicate files, and alerts you to at-risk files (e.g., important documents that only exist on one device), suggesting automated backups.

- **Chronicle:** A complete research and knowledge management assistant. It automatically analyzes documents, extracts key concepts, builds a knowledge graph of your library, and helps you find gaps in your research.

- **Ledger:** Turns your file system into a financial intelligence hub. It finds and parses receipts, invoices, and tax documents, automatically categorizing spending and helping you manage budgets.

- **Studio:** A digital asset manager for creators. It organizes creative projects, versions assets, and adds powerful features like video scene detection, transcript generation, and topic analysis.

- **Archives (Email, Browser, Spotify):** A suite of open-source extensions that import your digital life from other platforms, making your Spacedrive library a truly complete archive of your personal data.

## A Powerful SDK for a Limitless Ecosystem

Spacedrive provides a beautiful, comprehensive Rust SDK to create first-class extensions that are indistinguishable from core functionality. Extensions run in a secure **WASM sandbox**.

The SDK makes it trivial to:

- **Define Models:** Create new database schemas with a simple `#[model]` macro.
- **Create Jobs:** Define long-running background tasks with `#[job]`.
- **Build AI:** Give your extension a 'mind' with the `#[agent]` macro, enabling it to react to events in the VDFS.
- **Add Actions:** Expose new capabilities to the user with `#[action]`.
- **Integrate UIs:** Add custom views, sidebar sections, and components to the Spacedrive apps with a simple `ui_manifest.json`.

## Privacy & Security First

Your privacy is paramount. Spacedrive is a **local-first** application. Your data and metadata live on your devices.

- **End-to-End Encryption:** All network traffic is encrypted using modern protocols.
- **Encryption-at-Rest:** Libraries can be encrypted on disk with SQLCipher, protecting your data if a device is lost or stolen.
- **No Central Servers:** Your files are your own. Optional cloud integration is available for backup and remote access, but it's never required.

## Available Everywhere

Access your entire digital life, from anywhere.

- **Desktop:** A powerful desktop app for macOS, Windows, and Linux serves as your command center.
- **Mobile:** Native apps for iOS and Android provide full functionality on the go.
- **CLI:** A comprehensive command-line interface for power users and server administration.
- **Web:** Access your library from any browser with a self-hosted web interface.

## Get Involved

- **⭐ Star the repo** to show your support.
- **💬 Join the [Discord](https://discord.gg/gTaF2Z44f5)** to chat with the developers and community.
- **📖 Read the [Whitepaper](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)** to understand the full vision.
- **🧩 Build an Extension:** Check out the [SDK documentation](docs/sdk.md) and create your own extensions.

---

<p align="center">
  <em>Your files, unified. Your data, private. Your experience, limitless.</em>
</p>
