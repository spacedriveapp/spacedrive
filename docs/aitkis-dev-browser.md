# Spacedrive browser dev (aitkis fork)

Run the file explorer UI in **Firefox** without the slow Tauri dev loop.

**Branch:** dev must run on `feature/arik103` (long-lived fork branch). See [aitkis-fork-strategy.md](./aitkis-fork-strategy.md).

## Quick start (after reboot)

```bash
cd ~/GitHub/aitkis/spacedrive
git checkout feature/arik103
chmod +x scripts/aitkis-dev-browser.sh   # once
./scripts/aitkis-dev-browser.sh start
./scripts/aitkis-dev-browser.sh open
```

Or manually in **three terminals** (on `feature/arik103`):

| # | Command | Port | Purpose |
|---|---------|------|---------|
| 1 | `DYLD_LIBRARY_PATH=apps/.deps/lib ./target/debug/sd-daemon --data-dir ~/.spacedrive` | 6969 | Rust daemon (backend) |
| 2 | `./target/debug/sd-server` | 8080 | HTTP bridge (`/rpc`, `/events`) |
| 3 | `cd apps/web && bun run dev` | 3000 | React UI (open in browser) |

**Browser URL:** http://localhost:3000

**Health check:** http://localhost:8080/health → should return 200

## Stop

```bash
./scripts/aitkis-dev-browser.sh stop
```

Or `Ctrl+C` in each terminal.

## Do NOT use for browser testing

- `just dev-desktop` — Tauri native app; hangs on cold compile, blank in Firefox
- `apps/tauri` + `bun run dev` on :1420 — Tauri-only; crashes in browser

## First-time / fresh clone setup

```bash
cd ~/GitHub/aitkis/spacedrive
brew install bun just          # if missing
just setup                       # deps + native libs
cargo build --bin sd-daemon
cargo build --bin sd-server
```

## Local patches (aitkis)

Uncommitted fixes required for dev on this machine:

| File | Why |
|------|-----|
| `crates/task-system/src/system.rs` | `try_update` for Rust 1.96+ |
| `apps/tauri/vite.config.ts` + `stubs/spacebot-api-client.ts` | Missing private Spacebot repo |
| `apps/web/vite.config.ts` + `stubs/spacebot-api-client.ts` | Same + `style-to-js` CJS alias |
| `apps/web/src/main.tsx` | ErrorBoundary for visible crashes |

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Blank white page | Check Firefox **Console** (not Inspector) |
| `style-to-js` export error | Restart vite after pulling latest `apps/web/vite.config.ts` |
| `@spacebot/api-client` missing | Stub must exist; `rm -rf apps/web/node_modules/.vite` |
| `Problem loading` on :8080 | Terminal 2 (`sd-server`) not running |
| Port in use | `./scripts/aitkis-dev-browser.sh stop` then restart |

## Data location

Libraries and config persist at `~/.spacedrive` (survives reboot).
