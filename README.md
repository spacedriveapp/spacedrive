<p align="center">
  <img src=".github/logo.png" alt="Spacedrive" width="120" height="120" />
</p>

<h1 align="center">Spacedrive</h1>

<p align="center">
  <strong>One file manager for all your devices and clouds.</strong><br/>
	<span>Powered by a Virtual Distributed File System, complete with apps for macOS, Windows, Linux, iOS and Android</span>
</p>

<p align="center">
  <a href="https://fsl.software/">
    <img src="https://img.shields.io/static/v1?label=License&message=FSL-1.1-ALv2&color=000" />
  </a>
  <a href="https://github.com/spacedriveapp/spacedrive">
    <img src="https://img.shields.io/static/v1?label=Core&message=Rust&color=DEA584" />
  </a>
  <a href="https://discord.gg/gTaF2Z44f5">
    <img src="https://img.shields.io/discord/949090953497567312?label=Discord&color=5865F2" />
  </a>
</p>

<p align="center">
  <a href="https://spacedrive.com"><strong>spacedrive.com</strong></a> &bull;
  <a href="https://discord.gg/gTaF2Z44f5">Discord</a> &bull;
  <a href="#getting-started">Getting Started</a>
</p>

---

## What is Spacedrive?

Spacedrive is a cross-device data platform. Index files, emails, notes, and external sources. Search everything. Sync via P2P. Keep AI agents safe with built-in screening.

- **Content identity** — every file gets a BLAKE3 content hash. Same file on two devices produces the same hash. Spacedrive tracks redundancy and deduplication across all your machines.
- **Cross-device** — see all your files across all your devices in one place. Files on disconnected devices stay in the index and appear as offline.
- **P2P sync** — devices connect directly via Iroh/QUIC. No servers, no cloud, no single point of failure. Metadata syncs between devices. Files stay where they are.
- **Cloud volumes** — index S3, Google Drive, Dropbox, OneDrive, Azure, and GCS as first-class volumes alongside local storage.
- **Nine views** — grid, list, columns, media, size, recents, search, knowledge, and splat. QuickPreview for video, audio, code, documents, 3D, and images.
- **Local-first** — everything runs on your machine. No data leaves your device unless you choose to sync between your own devices.

### Is this a replacement for Finder or Explorer?

No. Spacedrive sits above your OS file manager and adds capabilities Finder/Explorer lack:

- **Portal across everything** — search and browse files across local disks, external drives, NAS, cloud storage, and archived data sources from one interface.
- **Operating surface for files** — content identity, sidecars, derivative artifacts, rich metadata, sync, and cross-device awareness built into the core model.
- **Embeddable and shareable** — run it as a desktop app, headless server, hosted file service, or embed the interface and APIs into other products.
- **AI-ready by design** — indexing and analysis pipelines prepare data ahead of time instead of giving agents raw shell access.
- **Safer access model** — route AI and automation through structured APIs, permissions, and processing layers instead of direct file operations.

You still use your OS for low-level file interactions. Spacedrive adds the cross-platform, cross-device, cloud-aware, and automation-friendly layer on top.

### Data Archival

Spacedrive indexes external data sources via script-based adapters: Gmail, Apple Notes, Chrome bookmarks, Obsidian, Slack, GitHub, calendar events, contacts. Each source becomes a searchable repository alongside your files.

Adapters are a folder with an `adapter.toml` manifest and a sync script in any language. If it reads stdin and prints lines, it works.

**Shipped adapters:** Gmail, Apple Notes, Chrome Bookmarks, Chrome History, Safari History, Obsidian, OpenCode, Slack, macOS Contacts, macOS Calendar, GitHub.

### Spacebot

Spacedrive integrates with [Spacebot](https://github.com/spacedriveapp/spacebot), an open source AI agent runtime. Spacebot runs as an optional separate process. Spacedrive provides the data, permission, and execution layer. Spacebot provides the intelligence.

Each Spacebot instance pairs with one Spacedrive node as its home device. That node authenticates the agent, maintains the device graph, resolves permissions, and forwards operations to peer devices. Every device in your library can reach Spacebot through the paired node over P2P (Iroh/QUIC) without direct network access. One agent runtime serves your entire device fleet.

When Spacebot spawns a worker, that worker can target any device in the library. File reads, shell commands, and operations proxy through Spacedrive to the target device. Talk to the agent from your phone while work executes on a server. Read files from a NAS, run commands on a workstation, report to a laptop — all in one task.

Every operation passes through Spacedrive's permission system: which devices the agent can access, which paths are readable or writable, which operations are allowed, and which require human confirmation. The paired node resolves effective policy before forwarding. One security model, one audit surface across all devices and clouds.

### File System Intelligence

Spacedrive adds intelligence to your filesystem by combining three layers:

- **File intelligence** — derivative data like OCR, transcripts, extracted metadata, thumbnails, previews, classifications, and sidecars.
- **Directory intelligence** — contextual knowledge attached to folders and subtrees ("active projects", "dormant archives", etc).
- **Access intelligence** — permissions and policy that apply across devices and clouds, routing agents through structured access instead of raw shell commands.

When an agent navigates through Spacedrive, it receives the file listing, subtree context, effective permissions, and summaries. Users can explain how they organize their system. Agents can add attributed notes. Jobs generate summaries from structure and activity. The intelligence stays attached to the filesystem, not buried in temporary session memory.

### Safety Screening

When enabled, every record passes through a safety pipeline before becoming searchable:

- **Prompt Guard 2** — local classifier detects prompt injection in emails, messages, and documents before they enter the index.
- **Trust tiers** — authored content (your notes) gets balanced screening, external content (email inbox) gets strict screening.
- **Quarantine system** — flagged records excluded from AI agent queries, reviewable in desktop app.
- **Content fencing** — search results include trust metadata so agents know what's safe vs untrusted.

No other local data tool screens indexed content before exposing it to AI.

---

## Architecture

The core is built on four principles:

1. **Virtual Distributed Filesystem (VDFS)** — files and folders become first-class objects with rich metadata, independent of physical location. Every file gets a universal address (`SdPath`) that works across devices.

2. **Content Identity System** — adaptive hashing (BLAKE3 with strategic sampling for large files) creates a unique fingerprint for every piece of content. Enables deduplication, redundancy tracking, and content-based operations.

3. **Transactional Actions** — every file operation can be previewed before execution. See space savings, conflicts, and estimated time, then approve or cancel. Operations become durable jobs that survive network interruptions and device restarts.

4. **Leaderless Sync** — peer-to-peer synchronization without central coordinators. Device-specific data uses state replication. Shared metadata uses an HLC-ordered log with deterministic conflict resolution.

The implementation is a single Rust crate with CQRS/DDD architecture. Every operation (file copy, tag create, search query) is a registered action or query with type-safe input/output that auto-generates TypeScript types for the frontend.

| Component       | Technology                                   |
| --------------- | -------------------------------------------- |
| Language        | Rust                                         |
| Async runtime   | Tokio                                        |
| Database        | SQLite (SeaORM + sqlx)                       |
| P2P             | Iroh (QUIC, hole-punching, local discovery)  |
| Content hashing | BLAKE3                                       |
| Vector search   | LanceDB + FastEmbed                          |
| Cloud storage   | OpenDAL                                      |
| Cryptography    | Ed25519, X25519, ChaCha20-Poly1305, AES-GCM  |
| Media           | FFmpeg, libheif, Pdfium, Whisper             |
| Desktop         | Tauri 2                                      |
| Mobile          | React Native + Expo                          |
| Frontend        | React 19, Vite, TanStack Query, Tailwind CSS |
| Type generation | Specta                                       |

```
spacedrive/
├── core/                  # Rust engine (CQRS/DDD)
├── apps/
│   ├── tauri/             # Desktop app (macOS, Windows, Linux)
│   ├── mobile/            # React Native (iOS, Android)
│   ├── cli/               # CLI and daemon
│   ├── server/            # Headless server
│   └── web/               # Browser client
├── packages/
│   ├── interface/         # Shared React UI
│   ├── ts-client/         # Auto-generated TypeScript client
│   ├── ui/                # Component library
│   └── assets/            # Icons, images, SVGs
├── crates/                # Standalone Rust crates (ffmpeg, crypto, etc.)
├── adapters/              # Script-based data source adapters
└── schemas/               # TOML data type schemas
```

---

## Getting Started

Requires [Rust](https://rustup.rs/) 1.81+, [Bun](https://bun.sh) 1.3+, [just](https://github.com/casey/just), and Python 3.9+ (for adapters).

```bash
git clone https://github.com/spacedriveapp/spacedrive
cd spacedrive

just setup        # bun install + native deps + cargo config
just dev-desktop  # launch the desktop app (auto-starts daemon)
just test         # run all workspace tests
```

---

## Privacy & Security

Spacedrive is local-first. Your data stays on your devices.

- **End-to-End Encryption** — all P2P traffic encrypted via QUIC/TLS
- **At-Rest Encryption** — libraries can be encrypted on disk (SQLCipher)
- **No Telemetry** — zero tracking or analytics
- **Self-Hostable** — run your own relay servers
- **Data Sovereignty** — you control where your data lives

Optional cloud integration is available for backup and remote access, but it's never required. The cloud service runs unmodified Spacedrive core as a standard P2P device—no special privileges.

---

## Contributing

- **Join [Discord](https://discord.gg/gTaF2Z44f5)** to chat with developers and community
- **[Contributing Guide](CONTRIBUTING.md)**
- **[Adapter Guide](docs/ADAPTERS.md)** — write a data source adapter

---

## License

FSL-1.1-ALv2 — [Functional Source License](https://fsl.software/), converting to Apache 2.0 after two years.
