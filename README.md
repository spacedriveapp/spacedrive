<p align="center">
  <img src=".github/logo.png" alt="Spacedrive" width="120" height="120" />
</p>

<h1 align="center">Spacedrive</h1>

<p align="center">
  <strong>A virtual distributed filesystem written in Rust.</strong><br/>
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

Spacedrive is a file manager that treats files as first-class objects with content identity, not paths. A photo on your laptop and the same photo on your NAS are recognized as one piece of content. Organize files across multiple devices, clouds, and platforms from a single interface.

- **Content identity** — every file gets a BLAKE3 content hash. Same file on two devices produces the same hash. Spacedrive tracks redundancy and deduplication across all your machines.
- **Cross-device** — see all your files across all your devices in one place. Files on disconnected devices stay in the index and appear as offline.
- **P2P sync** — devices connect directly via Iroh/QUIC. No servers, no cloud, no single point of failure. Metadata syncs between devices. Files stay where they are.
- **Cloud volumes** — index S3, Google Drive, Dropbox, OneDrive, Azure, and GCS as first-class volumes alongside local storage.
- **Nine views** — grid, list, columns, media, size, recents, search, knowledge, and splat. QuickPreview for video, audio, code, documents, 3D, and images.
- **Local-first** — everything runs on your machine. No data leaves your device unless you choose to sync between your own devices.

### Is this a replacement for Finder or Explorer?

Not exactly.

Spacedrive is not trying to replace Finder on macOS or Explorer on Windows as the default system file manager. That is not the goal, and it is not where the product is strongest.

Spacedrive sits on top of your operating system and adds capabilities the stock file manager does not have:

- **Portal across everything** — one place to search and browse files across local disks, external drives, NAS, cloud storage, and archived data sources.
- **Operating surface for files** — content identity, sidecars, derivative artifacts, rich metadata, sync, and cross-device awareness built into the core model.
- **Embeddable and shareable** — run it as a desktop app, a headless server, a hosted file service, or embed the interface and APIs into other products.
- **AI-ready by design** — prepare data ahead of time through indexing and analysis pipelines instead of giving agents raw shell access to your filesystem.
- **Safer access model** — route AI and automation through Spacedrive's structured APIs, permissions, and processing layers instead of direct file reads and shell commands.

You still use your operating system for low-level file interactions. Spacedrive adds the cross-platform, cross-device, cloud-aware, shareable, and automation-friendly layer on top.

If Finder or Explorer is the street-level view of your files, Spacedrive is the map, index, archive, and control plane.

### Data Archival

Beyond files, Spacedrive can index and archive data from external sources via script-based adapters. Gmail, Apple Notes, Chrome bookmarks, Obsidian, Slack, GitHub, calendar events, contacts. Each data source becomes a searchable repository. Search fans out across files and archived data together.

Adapters are simple: a folder with an `adapter.toml` manifest and a sync script in any language. If it can read stdin and print lines, it can be an adapter.

Shipped adapters: Gmail, Apple Notes, Chrome Bookmarks, Chrome History, Safari History, Obsidian, OpenCode, Slack, macOS Contacts, macOS Calendar, GitHub.

### Spacebot

Spacedrive integrates with [Spacebot](https://github.com/spacedriveapp/spacebot), an open source AI agent runtime. Spacebot runs as a separate process alongside Spacedrive, communicating over APIs. Spacedrive provides the data, permission, and execution layer. Spacebot provides the intelligence layer. Neither depends on the other. Together, they form an operating surface where humans and agents work side by side.

**Paired node model** — every Spacebot instance pairs with exactly one Spacedrive node. That node is Spacebot's home device inside the library. It authenticates Spacebot, maintains the device graph, resolves permissions, and forwards operations to peer devices. Spacebot never owns the multi-device graph directly — Spacedrive is the source of truth for device identity, library membership, and access policy.

**Multi-device agent access** — every Spacedrive device in the library can reach Spacebot through the paired node over the existing P2P transport (Iroh/QUIC). A phone, a laptop, a NAS — all talk to the same Spacebot instance without needing a direct network path to it. Spacedrive proxies conversations, events, and approvals through the library graph. One agent runtime serves the entire device fleet.

**Remote execution** — when Spacebot spawns a worker, that worker can target any device in the library. The worker's shell, file, and execution tools proxy through Spacedrive to the target device. From the model's perspective the tools are identical — it still calls `shell` and `file_read` — but the actual execution happens on a different machine, governed by policy. A founder can talk to an agent from their phone while the real work runs on an office server. An agent can read files from a NAS, run commands on a workstation, and report results to a laptop — all in one task.

**Permission enforcement** — every Spacebot operation passes through Spacedrive's permission system before anything executes. Permissions are library-scoped and layered: which devices the agent may access, which paths and subtrees are readable or writable, which operations are allowed, and which actions require live human confirmation. The paired Spacedrive node resolves effective policy before forwarding, and the target device can enforce a second check for defense in depth. One security model, one permission UX, one audit surface across every device and cloud.

**Chat everywhere** — the desktop app embeds a full Spacebot chat surface with conversations, streaming responses, inline worker cards, and tool call inspection. The mobile app reaches the same agent through the P2P proxy — check what the agent is working on, ask questions, review approvals, and speak naturally while the runtime continues on your own infrastructure. Same agent, same memory, same tasks, any device.

### File System Intelligence

Spacedrive adds a layer of file system intelligence on top of the native filesystem. It does not just expose files and folders. It understands what they are, why they exist, how they are organized, and what agents are allowed to do with them.

File System Intelligence combines three things:

- **File intelligence** — derivative data for individual files such as OCR, transcripts, extracted metadata, thumbnails, previews, classifications, and sidecars.
- **Directory intelligence** — contextual knowledge attached to folders and subtrees, like "this is where I keep active projects" or "this archive contains dormant repositories".
- **Access intelligence** — universal permissions and policy that apply across devices and clouds, so agents can be granted structured access through Spacedrive instead of raw shell access.

This makes the filesystem legible to AI. When an agent navigates a path through Spacedrive, it does not walk blind. It receives the listing, the relevant context for that subtree, the effective permissions, and summaries of what lives there. A projects folder is not just a folder. It is an active workspace. An archive is not just another directory. It carries historical meaning and policy.

That context evolves over time. Users can explain how they organize their system. Agents can add notes and observations with attribution. Jobs can generate summaries from structure and activity. Spacedrive keeps that intelligence attached to the filesystem itself instead of burying it inside temporary session memory.

The result is a file system that feels native to both humans and agents. Finder and Explorer show you where files are. Spacedrive adds the intelligence layer that explains what they are, how they relate, and how automation should interact with them.

---

## Architecture

The core is a single Rust crate with CQRS/DDD architecture. Every operation (file copy, tag create, search query) is a registered action or query with type-safe input/output that auto-generates TypeScript types for the frontend.

| Component           | Technology                                                        |
| ------------------- | ----------------------------------------------------------------- |
| Language            | Rust                                                              |
| Async runtime       | Tokio                                                             |
| Database            | SQLite (SeaORM + sqlx)                                            |
| P2P                 | Iroh (QUIC, hole-punching, local discovery)                       |
| Content hashing     | BLAKE3                                                            |
| Vector search       | LanceDB + FastEmbed                                               |
| Cloud storage       | OpenDAL                                                           |
| Cryptography        | Ed25519, X25519, ChaCha20-Poly1305, AES-GCM                      |
| Media               | FFmpeg, libheif, Pdfium, Whisper                                  |
| Desktop             | Tauri 2                                                           |
| Mobile              | React Native + Expo                                               |
| Frontend            | React 19, Vite, TanStack Query, Tailwind CSS                     |
| Type generation     | Specta                                                            |

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
just dev-daemon   # start the daemon
just dev-desktop  # launch the desktop app (connects to daemon)
just dev-server   # headless server (alternative to desktop)
just test         # run all workspace tests
just cli -- help  # run the CLI
```

---

## Contributing

- **Join [Discord](https://discord.gg/gTaF2Z44f5)** to chat with developers and community
- **[Contributing Guide](CONTRIBUTING.md)**
- **[Adapter Guide](docs/ADAPTERS.md)** — write a data source adapter

---

## License

FSL-1.1-ALv2 — [Functional Source License](https://fsl.software/), converting to Apache 2.0 after two years.
