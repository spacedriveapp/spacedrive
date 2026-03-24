<p align="center">
  <img src=".github/logo.png" alt="Spacedrive" width="120" height="120" />
</p>

<h1 align="center">Spacedrive</h1>

<p align="center">
  <strong>An open source cross-platform file manager.</strong><br/>
  Powered by a virtual distributed filesystem written in Rust.
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

### Data Archival

Beyond files, Spacedrive can index and archive data from external sources via script-based adapters. Gmail, Apple Notes, Chrome bookmarks, Obsidian, Slack, GitHub, calendar events, contacts. Each data source becomes a searchable repository. Search fans out across files and archived data together.

Adapters are simple: a folder with an `adapter.toml` manifest and a sync script in any language. If it can read stdin and print lines, it can be an adapter.

Shipped adapters: Gmail, Apple Notes, Chrome Bookmarks, Chrome History, Safari History, Obsidian, OpenCode, Slack, macOS Contacts, macOS Calendar, GitHub.

### Spacebot

Spacedrive integrates with [Spacebot](https://github.com/spacedriveapp/spacebot), an open source AI agent runtime. Spacebot runs as a separate process alongside Spacedrive, communicating over APIs. Spacedrive provides the data layer. Spacebot provides the intelligence layer. Neither depends on the other. Together, they form an operating surface where humans and agents work side by side.

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
