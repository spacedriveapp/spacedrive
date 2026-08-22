<p align="center">
  <img src=".github/logo.png" alt="Spacedrive" width="120" height="120" />
</p>

<h1 align="center">Spacedrive</h1>

<p align="center">
  <strong>More space, from the drives you already have.</strong><br/>
</p>

<p align="center">
  <a href="https://www.apache.org/licenses/LICENSE-2.0">
    <img src="https://img.shields.io/static/v1?label=License&message=Apache-2.0&color=000" />
  </a>
  <a href="https://github.com/spacedriveapp/spacedrive">
    <img src="https://img.shields.io/static/v1?label=Core&message=Rust%20%2B%20TypeScript&color=DEA584" />
  </a>
  <a href="https://discord.gg/gTaF2Z44f5">
    <img src="https://img.shields.io/discord/949090953497567312?label=Discord&color=5865F2" />
  </a>
</p>

<p align="center">
  <a href="https://docs.spacedrive.com"><strong>docs.spacedrive.com</strong></a> &bull;
  <a href="https://discord.gg/gTaF2Z44f5">Discord</a> &bull;
  <a href="#files">Files</a> &bull;
  <a href="#getting-started">Getting Started</a> &bull;
  <a href="#status">Status</a>
</p>

---

> [!NOTE]
> August 2026: Version v2.0.0-alpha.4 is out now!

Spacedrive is a lightweight, Rust native, file manager that connects your devices,
drives and clouds into one virtual file system. Pool your storage capacity together and browse and search drives that aren't even plugged in.

Backed by a continiously updating, modular index. Spacedrive can walk all major file systems, extracting metadata and snapshotting drive state without touching the files.

Mount anything to your Mac or PC. Like screenshots shared between several computers, or a project with 10TB of raw footage shared with a team. File content streams from one device to another on-demand.

## The mission

The file system has been the single most important part of computing from the start, but software has become increasingly dependent on cloud services at the cost of data ownership.

This is a software problem, Spacedrive is here to solve it.

## Files

The file manager of that computer, built on an index that Spacedrive maintains
continuously. The watcher feeds it as files change, so it is already current
when you look — nothing is built on demand while you wait.

Every surface is a view over that one index: grid, list, columns, media, size,
recents, search, knowledge. A size analysis of a folder and a photo grid of the
same folder are the same data through a different lens, and each view opens
current because the index already is.

- **One address for everything.** Every file has an `SdPath`, whether it lives on
  this machine, another of your devices, or a cloud volume. Operations work the
  same across all of them.
- **Content identity.** BLAKE3 hashing gives the same file on two machines the
  same fingerprint — the basis for deduplication and redundancy tracking across
  your machines.
- **Cloud volumes as first-class storage.** S3, Google Drive, Dropbox, OneDrive,
  Azure Blob and GCS get indexed alongside local disks.
- **P2P sync.** Devices connect directly over Iroh and QUIC with no server in the
  middle. Metadata syncs, files stay where they are, and devices that are offline
  remain in the index.
- **Search that works.** SQLite FTS5 over files and archived records, roughly 55ms
  on a million entries, with indexing at around 8,500 files per second.
- **Preview before commit.** Copies, moves and deletes are simulated first, so you
  see conflicts and space changes before anything runs, and the operation becomes
  a durable job that survives restarts.

Files runs standalone: a desktop app on macOS, Windows and Linux, iOS and Android
clients, a browser client, a headless server and a CLI. The macOS install is
44.4 MB with no bundled codecs and nothing to download. V2 is in alpha; the last
stable download is v1.

It is machine-scoped by design. A host has one filesystem, so it gets one index,
and everything else on the machine declares against that index as a capability
instead of running an indexer of its own.

### Search everything, not just files

Archive indexes external sources through script adapters, which puts email,
notes, bookmarks and chat history in the same index as your files. Shipped
adapters: Gmail, Apple Notes, Chrome Bookmarks, Chrome History, Safari History,
Obsidian, OpenCode, Slack, macOS Contacts, macOS Calendar and GitHub.

Writing one means a directory with an `adapter.toml` and a script in any language
that reads stdin and prints lines. See
[`docs/archive/README.md`](docs/archive/README.md).

### Screening

When enabled, every record passes through a safety pipeline before becoming
searchable: a local classifier (Prompt Guard 2) checks external content for
prompt injection, trust tiers apply stricter screening to content you did not
author, flagged records are quarantined out of agent queries, and search results
carry trust metadata so an agent knows what is untrusted.

---

## Where V2 came from

**2021.** A personal project: one interface over files scattered across devices
and cloud accounts, with content-addressed identity so the same file on two
machines is the same file.

**May 2022.** Open sourced. Number one on GitHub Trending for three days, 10,000
stars in the first week, a $2M seed round, a team of around twelve.

**Early 2025.** 35,000 stars, 600,000 installs, and a codebase that could no
longer ship. The Rust Prisma client was deprecated with no migration path, libp2p
transfers hung in ways nobody could debug, and two incompatible file models meant
every operation needed writing twice.
[`docs/overview/history.mdx`](docs/overview/history.mdx) is the full post-mortem.

**2025.** V2, rebuilt solo against that list. SeaORM for Prisma, Iroh for libp2p,
one `SdPath` for every file, jobs down to about 50 lines each, and search that
actually searches.

**2026.** The subtraction. V2 fixed V1's engineering and kept its appetite:
every install compiled in a WASM extension runtime and shipped a bundled media
stack, on the theory that a file manager should carry its whole environment. It
should not, and both became opt-in:

|                      |                                                  |
| -------------------- | ------------------------------------------------ |
| macOS payload before | ~156 MB (73.2 MB binary plus an 83 MB framework) |
| Default install now  | **44.4 MB, one file, nothing to download**       |

Nothing was removed from the product. The heavy subsystems became opt-in
features, macOS thumbnails route through ImageIO and QuickLook instead of bundled
FFmpeg and libheif, and half the old binary turned out to be data rather than
code. Every number is measured; the method is in
[`docs/plans/2026-07-29-install-size.md`](docs/plans/2026-07-29-install-size.md).

---

## Status

Under active development, and in alpha. V2 is a rebuild rather than a patch, so
treat everything below as moving.

|                                                                       |             |
| --------------------------------------------------------------------- | ----------- |
| Files: indexing, sync, cloud volumes, desktop, mobile and web clients | working     |
| Archive: record spine, durable overlay, FTS5 search, 11 adapters      | working     |
| Machine-scoped daemon: supervision, health, lifecycle, port leases    | working     |
| Mounts: read-only share on loopback, byte-range peer protocol         | in progress |
| Per-source stores and the in-memory read tier                         | in progress |

---

## Architecture

```
spacedrive/
├── core/                  # Rust engine (CQRS/DDD): indexing, sync, jobs, volumes
├── crates/                # archive, supervisor, task-system, crypto, imageio, images
├── apps/
│   ├── tauri/  mobile/    # Desktop (macOS, Windows, Linux), React Native
│   ├── cli/  server/      # CLI, daemon, headless server
│   ├── native/            # GPUI client prototype
│   └── web/               # Browser client
├── packages/
│   ├── interface/         # Files' React UI
│   └── ts-client/  swift-client/  assets/
└── adapters/              # Script-based data source adapters
```

Every core operation — a file copy, a tag create, a search query — is a
registered action or query with type-safe input and output, and Specta generates
the TypeScript and Swift clients from those definitions.

| Component       | Technology                                                      |
| --------------- | --------------------------------------------------------------- |
| Engine          | Rust, Tokio                                                     |
| Database        | SQLite (SeaORM + sqlx)                                          |
| Search          | SQLite FTS5                                                     |
| Content hashing | BLAKE3                                                          |
| P2P             | Iroh (QUIC, hole-punching, local discovery)                     |
| Cloud storage   | OpenDAL (S3, Google Drive, Dropbox, OneDrive, Azure Blob, GCS)  |
| Cryptography    | Ed25519, X25519, ChaCha20-Poly1305, AES-GCM                     |
| Media           | ImageIO and QuickLook on macOS, FFmpeg/libheif/Pdfium elsewhere |
| Desktop         | Tauri 2                                                         |
| Mobile          | React Native + Expo                                             |
| Frontend        | React 19, Vite, TanStack Query, Tailwind CSS v4                 |
| Design system   | [SpaceUI](https://github.com/spacedriveapp/spaceui)             |
| Type generation | Specta (TypeScript + Swift)                                     |

---

## Getting Started

Requires [Rust](https://rustup.rs/) (stable, MSRV 1.81), [Bun](https://bun.sh) 1.3+,
Node 20.x, and [just](https://github.com/casey/just).

```bash
git clone https://github.com/spacedriveapp/spacedrive
cd spacedrive

# System dependencies: cmake on macOS, GTK/WebKit/gstreamer on Linux.
# Pass `mobile` to also add the iOS/Android Rust targets.
./scripts/setup.sh              # .\scripts\setup.ps1 on Windows

bun install
cargo run -p xtask -- setup
```

`xtask setup` writes `.cargo/config.toml`, builds the release daemon, and registers
the `cargo xtask`, `cargo daemon` and `cargo cli` aliases, so after the first run
`just setup` does the same thing in one command. It downloads nothing.

On macOS that is everything you need. Thumbnails for video, HEIC, RAW and PDF come
from ImageIO and QuickLook, the same system codecs Finder uses. Elsewhere those
formats need the prebuilt codec bundle (FFmpeg, libheif, Pdfium), which is a 91 MB
download and enables the `ffmpeg` and `heif` features:

```bash
cargo run -p xtask -- setup --native-deps
```

### Run Files

```bash
just dev-desktop  # desktop app (builds and starts the daemon for you)
just dev-server   # headless server + web UI
just dev-mobile   # Expo dev server
```

### Test and check

```bash
just test         # cargo test --workspace
just check        # cargo fmt --check + clippy
```

Python 3.9+ is needed at runtime by the bundled adapters, not to build Spacedrive.

## Contributing

- **Join [Discord](https://discord.gg/gTaF2Z44f5)** to chat with developers and community
- **[Contributing Guide](CONTRIBUTING.md)**
- **[Archive & Adapters](docs/archive/README.md)**, how data source adapters work
- **[SpaceUI](https://github.com/spacedriveapp/spaceui)**, shared design system

---

## License

[Apache-2.0](LICENSE).

Releases before 2026-03-24 were published under AGPL-3.0 and remain available
under that license.
