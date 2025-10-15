# Welcome to the Spacedrive Contributing Guide

Thank you for investing your time in contributing to our project!

Please read our [Code of Conduct](./CODE_OF_CONDUCT.md) to keep our community approachable and respectable.

This guide will provide an overview of the contribution workflow, including opening an issue, creating a pull request (PR), and the review and merge process.

> **Important: Spacedrive V2 Rewrite**
>
> Spacedrive underwent a complete architectural rewrite in early 2025. If you contributed to V1, please read the [Migration Guide for V1 Contributors](#migrating-from-v1) section below to understand the major changes.
>
> **TL;DR:** V1 used Tauri + React + Prisma. V2 is Rust-first with native Swift apps, SeaORM, and a CLI. Most of the stack was replaced to address fundamental architectural issues detailed in our [history document](docs/overview/history.mdx).

## New Contributor Guide

To familiarize yourself with the project, please read the [README](README.md). Here are some resources to help you get started with open-source contributions:

- [Finding ways to contribute to open-source on GitHub](https://docs.github.com/en/get-started/exploring-projects-on-github/finding-ways-to-contribute-to-open-source-on-github)
- [Setting up Git](https://docs.github.com/en/get-started/quickstart/set-up-git)
- [GitHub flow](https://docs.github.com/en/get-started/quickstart/github-flow)
- [Collaborating with pull requests](https://docs.github.com/en/github/collaborating-with-pull-requests)
- [Spacedrive Architecture Documentation](docs/core/architecture.mdx)
- [V1 to V2 Migration Guide](#migrating-from-v1) (for returning contributors)

## Getting Started

### Issues

#### Creating a New Issue

If you come across an issue or have a feature request for Spacedrive, please [search if a related issue has already been reported](https://docs.github.com/en/github/searching-for-information-on-github/searching-on-github/searching-issues-and-pull-requests#search-by-the-title-body-or-comments). If no relevant issue exists, you can open a new issue using the appropriate [issue form](https://github.com/spacedriveapp/spacedrive/issues/new/choose).

#### Solving an Issue

To find an issue that interests you, you can browse through our [existing issues](https://github.com/spacedriveapp/spacedrive/issues) and use the available `labels` to narrow down your search (See [Labels](https://github.com/spacedriveapp/spacedrive/labels) for more information). As a general rule, if you find an issue you want to work on, you are welcome to open a PR with a fix.

## Development Setup

Spacedrive V2 is built with a **Rust-first architecture**. The core Virtual Distributed File System (VDFS) is pure Rust, with native Swift apps for iOS/macOS maintained as separate submodules to keep Spacedrive recognized as a Rust project on GitHub.

### Prerequisites

Before you begin, ensure you have the following installed:

| Tool  | Version                        | Required For                 |
| ----- | ------------------------------ | ---------------------------- |
| Rust  | [`1.81+`](rust-toolchain.toml) | Core development             |
| Xcode | Latest                         | iOS/macOS development        |
| Git   | Any recent version             | Version control & submodules |

**Note:** Node.js and pnpm are only needed if you're working on TypeScript client generation or shared UI packages.

[`rustup`](https://rustup.rs/) should automatically pick up the correct Rust version from the project's `rust-toolchain.toml`.

### Clone the Repository

```bash
git clone https://github.com/spacedriveapp/spacedrive
cd spacedrive
```

If you plan to work on GUI applications, initialize the submodules:

Some submodules are private, such as extensions.

```bash
git submodule update --init --recursive
```

### System Dependencies

Run the setup script to install required system dependencies:

**Unix (Linux / macOS):**

```bash
./scripts/setup.sh
```

**Windows:**

```powershell
.\scripts\setup.ps1
```

This script will install platform-specific dependencies required for building Spacedrive.

For mobile development, run:

```bash
./scripts/setup.sh mobile
```

This installs additional Rust targets for iOS and Android cross-compilation.

## Core Development

The heart of Spacedrive is the Rust core (`core/`). Most contributions will involve working with this codebase.

### Quick Start with CLI

The fastest way to start developing is with the CLI:

```bash
# Create a library and start exploring
cargo run -p sd-cli -- library create "Dev Library"

# Add a location to index
cargo run -p sd-cli -- location add ~/Documents

# Search for files
cargo run -p sd-cli -- search .
```

You can set up a bashprofile or alias

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific package
cargo test -p sd-core

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run all checks
cargo fmt && cargo clippy && cargo test
```

### Working with Examples

The `core/examples/` directory contains working demonstrations of core features:

```bash
# Run the indexing demo
cargo run --example indexing_demo

# Run the file type detection demo
cargo run --example file_type_demo

# See all available examples
ls core/examples/
```

Examples are a great way to understand how different parts of the system work together.

## GUI Application Development

GUI applications (iOS, macOS, and the upcoming Desktop app) are maintained as **separate Git submodules**. This approach keeps the main repository focused on Rust code, ensuring Spacedrive is properly recognized as a Rust project on GitHub.

### Why Submodules?

With 35k+ stars, Spacedrive is one of the top 30 largest Rust projects globally, yet GitHub's language detection can misclassify it as TypeScript/Swift based on lines of code. Submodules solve this by keeping frontend code in separate repositories.

### Working with Submodules

#### Initialize All Submodules

```bash
git submodule update --init --recursive
```

#### Update Submodules to Latest

```bash
git submodule update --remote
```

#### Check Submodule Status

```bash
git submodule status
```

### iOS Development

The iOS app embeds the full Spacedrive core as a native library, enabling offline-first operation and direct P2P networking.

```bash
# Navigate to iOS submodule
cd apps/ios

# Open in Xcode
open Spacedrive.xcodeproj
```

For detailed iOS development instructions, see [`apps/ios/README.md`](apps/ios/README.md).

**Key Points:**

- Core is embedded as `sd-ios-core` static library
- Uses shared `packages/swift-client` for type-safe Swift API
- No external daemon required
- Full documentation in submodule README

### macOS Development

The macOS app connects to the Spacedrive daemon via Unix socket.

```bash
# Navigate to macOS submodule
cd apps/macos

# Open in Xcode
open Spacedrive.xcodeproj
```

For macOS-specific development instructions, see `apps/macos/README.md`.

### Desktop Development (Coming Soon)

The cross platform desktop app will use Tauri and will also be maintained as a submodule.

```bash
# Future workflow (when desktop submodule exists)
cd apps/desktop
pnpm install
pnpm tauri dev
```

## Extension Development

Spacedrive supports WASM-based extensions for adding custom functionality. Extensions run in sandboxed environments with full access to the Spacedrive SDK.

### Getting Started with Extensions

```bash
# Navigate to extensions directory
cd extensions/

# Create a new extension from template
cargo generate --path template

# Build an extension
cd your-extension
cargo build --target wasm32-unknown-unknown
```

For comprehensive extension development documentation, see [`docs/extensions/introduction.mdx`](docs/extensions/introduction.mdx).

## TypeScript Client Development

The TypeScript client (`packages/ts-client`) provides a type-safe interface to the Spacedrive daemon. Types are automatically generated from Rust definitions.

### Generate TypeScript Types

```bash
# From the ts-client directory
cd packages/ts-client
pnpm run generate-types

# Or run directly from core
cargo run --bin generate_typescript_types --manifest-path core/Cargo.toml
```

### Build the Client

```bash
cd packages/ts-client
pnpm install
pnpm build
```

The TypeScript client is primarily used by the desktop GUI (future) and can be used to build custom interfaces.

## Architecture Overview

Spacedrive V2 introduces several architectural improvements over V1:

- **Entry-Centric Model**: Files and directories unified as Entries with optional content identity
- **SdPath Addressing**: Universal file addressing across devices and storage types
- **Event-Driven**: EventBus eliminates coupling between core subsystems
- **CQRS Pattern**: Actions (mutations) and Queries (reads) with preview-commit-verify flow
- **Durable Jobs**: Long-running operations survive app restarts via MessagePack serialization
- **Domain-Separated Sync**: Leaderless P2P sync with HLC timestamps

For deep-dive architecture documentation, see [`docs/core/architecture.mdx`](docs/core/architecture.mdx).

## Submitting a Pull Request

Once you have finished making your changes, create a pull request (PR) to submit them.

### Before Submitting

1. **Run all checks:**

   ```bash
   cargo fmt && cargo clippy && cargo test
   ```

2. **Update documentation** if you've changed public APIs

3. **Add tests** for new functionality

4. **Test on relevant platforms** (especially for iOS/macOS changes)

### Creating the PR

- Fill out the PR template to help reviewers understand your changes
- [Link your PR to an issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue) if addressing an existing issue
- Enable the checkbox to [allow maintainer edits](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/allowing-changes-to-a-pull-request-branch-created-from-a-fork)
- A team member will review your proposal and may request changes
- Mark conversations as [resolved](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/commenting-on-a-pull-request#resolving-conversations) as you address feedback

## Your PR is Merged!

Congratulations! The Spacedrive team thanks you for your contribution!

Once your PR is merged, your changes will be included in the next release of the application.

## Troubleshooting

### Rust Compilation Issues

**Error: Could not compile `sd-core`**

- Ensure you're using Rust 1.81+ (`rustup update`)
- Clean build artifacts: `cargo clean`
- Check that all dependencies are installed via setup script

**Error: Linking failed**

- On Linux, ensure all system dependencies are installed
- Run `./scripts/setup.sh` again to verify dependencies

### Xcode Issues

#### `xcrun: error: unable to find utility "xctest"`

This occurs when Xcode command line tools are not properly configured.

**Solution:**

```bash
# Install Xcode from App Store
# Then configure command line tools
xcode-select -s /Applications/Xcode.app/Contents/Developer
```

#### `unable to lookup item 'PlatformPath'`

This typically indicates outdated command line tools.

**Solution:**

```bash
# Ensure macOS is fully updated
# Install/update command line tools
xcode-select --install

# Install Rosetta (required for some dependencies on Apple Silicon)
softwareupdate --install-rosetta --agree-to-license
```

### Submodule Issues

#### Submodules not initialized

**Error:** `apps/ios` directory is empty or missing files

**Solution:**

```bash
git submodule update --init --recursive
```

#### Submodule pointing to wrong commit

After pulling latest changes, submodules may be out of sync.

**Solution:**

```bash
git submodule update --remote
```

#### Changes in submodule not showing

If you've made changes inside a submodule directory:

```bash
# Commit within the submodule
cd apps/ios
git add .
git commit -m "Your changes"

# Then commit the submodule reference in main repo
cd ../..
git add apps/ios
git commit -m "Update iOS submodule"
```

### iOS/macOS Build Issues

#### Core library not found

**Error:** `sd-ios-core` framework not found

**Solution:**

```bash
# From the project root
cargo ios
# Or the full command:
cargo xtask build-ios
```

#### Swift client compilation errors

If you've updated Rust types, regenerate Swift bindings:

```bash
# The swift-client uses generated types from specta
# Rebuild the core to regenerate types
cargo build -p sd-core
```

### Test Failures

If tests fail locally:

```bash
# Run specific test with output
cargo test test_name -- --nocapture

# Check for database issues
rm -rf ~/.local/share/spacedrive  # Remove test databases
cargo test
```

## Migrating from V1

If you contributed to Spacedrive V1 (the 35k+ star version visible on GitHub from 2022-2024), welcome back! V2 is a ground-up rewrite that addresses every architectural flaw that made V1 unmaintainable. This section helps you understand what changed and how your V1 knowledge maps to V2.

### Why the Rewrite?

V2 wasn't perfectionism—V1 became fundamentally broken:

- **Prisma deprecated** - No migration path for our database layer
- **libp2p unreliable** - P2P transfers constantly failed
- **No extensibility** - Community couldn't build on top
- **Dual file systems** - Incompatible indexed vs ephemeral files
- **Development paralysis** - Simple features required 1000+ lines of boilerplate

Read the full analysis in [docs/overview/history.mdx](docs/overview/history.mdx).

### Architecture Changes at a Glance

| Aspect              | V1 (PRRTT Stack)                  | V2 (Rust-First)                                                                        |
| ------------------- | --------------------------------- | -------------------------------------------------------------------------------------- |
| **Database**        | Prisma (deprecated)               | SeaORM                                                                                 |
| **Type Generation** | rspc                              | Specta (TypeScript + Swift)                                                            |
| **Desktop**         | Tauri + React (in repo)           | Swift (native macOS submodule) / Tauri + React (cross platform submodule, coming soon) |
| **Mobile**          | React Native                      | Native Swift (iOS/macOS submodules)                                                    |
| **P2P Networking**  | libp2p                            | Iroh (QUIC-based)                                                                      |
| **File Model**      | Dual system (indexed + ephemeral) | Unified Entry + SdPath                                                                 |
| **RPC**             | rspc procedures                   | Specta-generated types                                                                 |
| **Extensibility**   | None                              | WASM SDK                                                                               |
| **CLI**             | Planned                           | Production-ready (`sd-cli`)                                                            |
| **Job System**      | 1000+ lines boilerplate           | ~50 lines with macros                                                                  |
| **Sync**            | Custom CRDT (incomplete)          | HLC timestamps (works)                                                                 |

### Development Workflow Changes

**V1 Workflow:**

```bash
pnpm i
pnpm prep  # Generate Prisma client + rspc types
pnpm tauri dev  # Desktop
pnpm mobile ios  # React Native mobile
cargo run -p sd-server  # Backend server
```

**V2 Workflow:**

```bash
cargo run -p sd-cli -- library create "My Library"  # CLI-first
open apps/ios/Spacedrive.xcodeproj  # Native iOS (submodule)
# Desktop: cd apps/desktop && pnpm tauri dev (when submodule exists)
```

### File Structure Comparison

**V1 Monorepo:**

```
apps/
  desktop/        # Tauri app (main repo)
  mobile/         # React Native
  web/            # React SPA
  landing/        # Next.js
  server/         # Rust backend
  storybook/      # Component docs
interface/        # Shared React components
packages/
  client/         # rspc client (@sd/client)
  ui/             # Shared UI components
  config/         # ESLint/TS configs
core/
  prisma/         # Prisma schema
  src/            # Rust core
```

**V2 Structure:**

```
core/             # Pure Rust (VDFS implementation)
  src/
    domain/       # Core models
    ops/          # CQRS operations
    infra/        # DB, jobs, events, sync
    service/      # High-level services
apps/
  cli/            # Production CLI
  ios/            # Native Swift app (SUBMODULE)
  macos/          # Native Swift app (SUBMODULE)
  desktop/        # Future: Tauri app (SUBMODULE)
extensions/       # WASM extensions
packages/
  ts-client/      # Minimal TypeScript client
  swift-client/   # Shared Swift client
  ui/             # Minimal shared UI
```

### What Happened to My Favorite V1 Component?

**Desktop GUI (`apps/desktop`):**

- **Status:** Will return as a submodule (Tauri + React, like V1)
- **Why submodule:** Keep Spacedrive categorized as Rust on GitHub
- **Migration:** Desktop contributors should wait for submodule or contribute to native apps

**React Native Mobile (`apps/mobile`):**

- **Status:** Replaced with native Swift apps in `apps/ios` and `apps/macos` submodules
- **Why:** Better performance, full iOS API access, offline-first with embedded core
- **Migration:** Learn SwiftUI or contribute to core Rust

**Interface Package (`interface/`):**

- **Status:** Removed; UI now lives in individual app submodules
- **Migration:** Desktop UI will be in `apps/desktop` submodule when added

**Prisma Client (`core/prisma`):**

- **Status:** Replaced with SeaORM
- **Migration:** Learn SeaORM migration system and entity definitions

**rspc (`packages/client`):**

- **Status:** Replaced with Specta for type generation
- **Migration:** Types now generated from Rust using Specta for both TypeScript and Swift

**libp2p Networking:**

- **Status:** Replaced with Iroh
- **Why:** More reliable P2P with better NAT traversal
- **Migration:** Iroh API is different; review networking code in `core/src/service/network/`

### Breaking Changes for Contributors

#### Database Layer

- **V1:** Prisma schema → `prisma generate` → Rust client
- **V2:** SeaORM entities → Migrations in `core/src/infra/db/migration/`
- **Learn:** [SeaORM docs](https://www.sea-ql.org/SeaORM/)

#### Type Safety Across Boundaries

- **V1:** rspc procedures with TypeScript codegen
- **V2:** Specta generates both TypeScript and Swift types
- **Example:**

  ```rust
  // V1 (rspc)
  .query("getLibrary", |t| t(|ctx, input: String| async move { ... }))

  // V2 (Specta)
  #[derive(Serialize, Deserialize, Type)]
  pub struct Library { ... }
  // Types auto-generated for TS and Swift
  ```

#### Job System

- **V1:** Manual boilerplate (500-1000 lines per job)
- **V2:** Derive macro (~50 lines)
- **Example:**
  ```rust
  // V2
  #[derive(Job)]
  pub struct IndexerJob {
      location_id: Uuid,
  }
  ```

#### File Operations

- **V1:** Dual system (indexed `FilePath` vs ephemeral)
- **V2:** Unified `Entry` model with `SdPath` addressing
- **Learn:** Read `core/src/domain/entry.rs` and SdPath in docs

### Where Should V1 Contributors Focus?

**If you worked on:**

**Core/Rust:**

- Your knowledge transfers well
- Learn SeaORM, Specta, and the new domain-driven structure
- Focus on `core/src/domain/` and `core/src/ops/`

**Desktop GUI:**

- Wait for `apps/desktop` submodule or contribute to native apps
- React/TypeScript skills will transfer when desktop submodule is added

**Mobile:**

- iOS/macOS are now native Swift apps
- Learn SwiftUI or contribute to core Rust (embedded in apps)
- Check `apps/ios/README.md` for architecture

**Database/Migrations:**

- Completely different system (SeaORM vs Prisma)
- Learn SeaORM migrations
- Focus on `core/src/infra/db/`

**Networking/P2P:**

- Iroh replaced libp2p entirely
- Learn Iroh's QUIC-based approach
- Focus on `core/src/service/network/`

**Search:**

- Now using FTS5 with semantic re-ranking
- Focus on `core/src/infra/search/`

**Extensions/SDK:**

- **New in V2!** WASM-based extension system
- Check `extensions/` and `docs/extensions/`

### Quick Reference: Command Mapping

| V1 Command               | V2 Equivalent                                        |
| ------------------------ | ---------------------------------------------------- |
| `pnpm i`                 | Not needed (unless working on ts-client)             |
| `pnpm prep`              | Not needed (Specta generates on build)               |
| `pnpm tauri dev`         | Will be `cd apps/desktop && pnpm tauri dev` (future) |
| `pnpm mobile ios`        | `open apps/ios/Spacedrive.xcodeproj`                 |
| `cargo run -p sd-server` | `cargo run -p sd-cli`                                |
| `pnpm dev:web`           | Not yet available (web in progress)                  |

### Getting Help with Migration

- **Read the history:** [docs/overview/history.mdx](docs/overview/history.mdx) explains every V1 failure
- **Architecture docs:** [docs/core/architecture.mdx](docs/core/architecture.mdx) explains V2 design
- **Discord:** Ask in #development channel
- **Examples:** Run `cargo run --example <name>` to see V2 patterns

### Will V1 Knowledge Help?

**Yes for:**

- General Rust development
- Core concepts (VDFS, CAS, file indexing)
- Understanding the problem space

**No for:**

- Specific API calls (completely redesigned)
- Database queries (different ORM)
- File operation patterns (unified model now)
- P2P code (different library)

## Additional Resources

- [Spacedrive Architecture](docs/core/architecture.mdx) - Deep dive into V2 architecture
- [Extension Development](docs/extensions/introduction.mdx) - Build WASM extensions
- [Whitepaper](docs/overview/whitepaper.mdx) - Spacedrive's vision and technical design
- [Discord Community](https://discord.gg/gTaF2Z44f5) - Get help and discuss development

## Credits

This CONTRIBUTING.md file was inspired by the [github/docs CONTRIBUTING.md](https://github.com/github/docs/blob/main/.github/CONTRIBUTING.md) file, and we extend our gratitude to the original author.
