# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Read `AGENTS.md` first.** It is the detailed development guide: code standards, comment/documentation style, logging rules, the extension SDK, and the "current direction" constraints for the entries-to-record-table migration. This file is a quick map; AGENTS.md is authoritative where they overlap.

## Commands

First-time setup: `just setup` (bun install + `cargo xtask setup`), or `just setup-native-deps` to also fetch the prebuilt FFmpeg/libheif/Pdfium bundle.

```bash
just dev-daemon                          # Run the daemon (default dev workflow with dev-desktop)
just dev-desktop                         # Tauri desktop app in dev mode (no Rust watch)
just dev-server                          # Headless server with web UI
just cli <command>                       # Run the CLI (binary is sd-cli, not spacedrive)
just test                                # cargo test --workspace
cargo test <test_name>                   # Run a single test
just check                               # cargo fmt --check + cargo clippy --workspace
just fmt                                 # cargo fmt (tabs for indentation)
```

After changing any Rust type with a `Type` derive that the frontend sees, regenerate the TypeScript client:

```bash
cargo run --bin generate_typescript_types   # -> packages/ts-client/src/generated/types.ts
```

After rebuilding the daemon, restart it so clients use the new code: `just cli restart`.

The Tauri app and `apps/native` (gpui) are excluded from default cargo builds. Run Tauri via `just dev-desktop`; run the native app with `cargo run --release -p sd-native`.

Never run `cargo clean` without asking.

## Architecture

Daemon-client: one `sd-daemon` process owns the core; the CLI, Tauri desktop app, and web/server clients connect over Unix domain sockets (WebSockets for web) speaking JSON-RPC with Wire method strings.

- `core/` — the core crate (`sd_core`). Domain types in `src/domain/`, operations in `src/ops/` split into actions (writes) and queries (reads), each feature a module with `input.rs`/`output.rs`/`action.rs` (and `job.rs` for long-running work).
- `crates/` — supporting crates. `crates/store` is the source store (one SQLite file per source, `record`/facet/`content`/`edge` tables) that is replacing the old `entry`/`location` schema. `crates/sdk` + `sdk-macros` are the WASM extension SDK.
- `adapters/` — non-filesystem sources (gmail, slack, obsidian, etc.); an adapter source differs from a filesystem source only in ingest.
- `apps/` — cli, server, tauri (primary desktop app), mobile, native (gpui prototype), web.
- `packages/` — TypeScript: `interface` (React UI), `ts-client` (generated from Rust via Specta), `ui`, plus `swift-client` for the iOS/macOS prototypes.
- `extensions/` — WASM extensions built against the SDK (excluded from the workspace; built with `--target wasm32-unknown-unknown`).

Operations register through `register_query!` / `register_library_action!` / `register_core_action!` macros; the `inventory` crate collects them into global registries at startup (`core/src/infra/wire/registry.rs`). Never implement `Wire` manually.

Frontend code must stay type-safe against the generated `ts-client` types. Never cast to `any` or redefine backend types by hand.

Use `tracing` macros, never `println!`; inside jobs use `ctx.log()` so entries get the job id.

## Current direction

The entries schema (`entry`, `location`) is being torn down in favor of the record table in `crates/store`. Do not add columns to `entry`/`location` or grow the persistent indexing path; new durable state belongs in the record table. The status table at the top of `docs/plans/2026-08-20-entries-teardown-execution.md` is the only task register and gets updated in the same commit as the work (`/.tasks/` is frozen). See "Current direction" in AGENTS.md for the full constraints.

Docs are kept current: `.mdx` files under `/docs` (core architecture in `/docs/core/`, design docs in `/docs/core/design/`, active plans in `/docs/plans/`).
