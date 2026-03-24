# Spacebot Integration Design

## Purpose

Add first-class Spacebot support to Spacedrive without collapsing the two products into one process model.

Spacedrive should be able to:

1. manage a local Spacebot instance for the user
2. connect to an already running local Spacebot instance
3. connect to a remote Spacebot instance

This keeps Spacebot as a separate runtime while making it feel native inside Spacedrive.

## Decision

Spacedrive will treat Spacebot as a companion service, not as an embedded subsystem inside the VDFS daemon.

The integration boundary is HTTP plus SSE, using Spacebot's existing API.

Spacedrive will support three connection modes:

1. **Managed Local** — Spacedrive launches and supervises a foreground Spacebot child process.
2. **External Local** — Spacedrive connects to an existing localhost Spacebot instance.
3. **Remote** — Spacedrive connects to a Spacebot instance over HTTPS with bearer auth.

## Why This Shape

- Spacebot already has a real control plane: HTTP API, health endpoints, status endpoints, SSE, and a stable instance directory model.
- Spacedrive already treats Spacebot as a separate process in the README, which is the right long-term boundary.
- Embedding Spacebot directly into `sd-core` would couple two daemon models too early.
- Spacebot works cleanly as a child process because it has explicit foreground mode and local file-backed state.
- The same client model can serve local managed, local external, and remote connections.

## Non-Goals

- Do not merge Spacebot into the Spacedrive daemon process.
- Do not proxy every Spacebot API through Spacedrive in v1.
- Do not require Spacedrive core to understand Spacebot internals like channels, branches, workers, or memory schemas.
- Do not design a brand-new agent API when Spacebot already has one.

## Existing Spacebot Capabilities

Spacebot already exposes the pieces Spacedrive needs.

### Runtime

- single binary
- foreground mode for supervised child-process execution
- daemon mode with PID file and Unix socket for native CLI control
- configurable instance directory

Relevant files:

- `spacebot/src/main.rs`
- `spacebot/src/daemon.rs`
- `spacebot/src/config/types.rs`

### HTTP API

Default API behavior:

- bind: `127.0.0.1`
- port: `19898`
- optional bearer token auth

Relevant files:

- `spacebot/src/api/server.rs`
- `spacebot/src/api/system.rs`
- `spacebot/docs/docker.md`

### Minimal endpoints Spacedrive can rely on

- `GET /api/health` — liveness
- `GET /api/status` — version, pid, uptime
- `GET /api/idle` — worker and branch activity
- `GET /api/agents/warmup` — work readiness
- `POST /api/webchat/send` — inject a message
- `GET /api/webchat/history` — fetch conversation history
- `GET /api/events` — global SSE event stream

## Integration Modes

### 1. Managed Local

Spacedrive starts Spacebot as a child process in foreground mode and talks to it over localhost HTTP.

Recommended command shape:

```text
spacebot start --foreground --config <path>
```

Recommended ownership:

- process lifecycle owned by the desktop shell layer, not by `sd-core`
- status mirrored into Spacedrive config and UI
- health and warmup polled over HTTP

Why this is the recommended default:

- easiest onboarding
- strongest first-class user experience
- least invasive to Spacedrive core
- preserves Spacebot as a separate product and runtime

### 2. External Local

Spacedrive connects to an already running local Spacebot instance.

Expected user inputs:

- base URL, usually `http://127.0.0.1:19898`
- optional bearer token

This mode is important for:

- developers already running Spacebot manually
- advanced users with custom instance directories or configs
- system-service installs managed outside Spacedrive

### 3. Remote

Spacedrive connects to a remote Spacebot instance over HTTPS.

Expected user inputs:

- base URL
- bearer token
- optional instance label

This mode is important for:

- self-hosted NAS or server deployments
- hosted Spacebot instances
- team or shared deployments

## Recommended V1 Scope

The smallest honest first-class integration is:

1. support Managed Local and External Local first
2. design the client so Remote works with the same abstraction
3. use the existing Spacebot webchat and SSE APIs instead of inventing a new protocol
4. keep Spacebot lifecycle in the app layer
5. keep Spacebot connection metadata in app config

## Architecture Boundary

### Spacedrive Core responsibilities

- persist Spacebot connection settings in app config
- expose typed config get/update operations
- expose lightweight status and health queries if the UI should stay transport-agnostic
- optionally publish Spacebot connection events onto Spacedrive's own event system later

### Desktop shell responsibilities

- spawn and stop managed local Spacebot processes
- supervise child process lifecycle
- detect existing local process connectivity
- surface launch and crash diagnostics

### Interface responsibilities

- Spacebot settings page
- connection mode selection
- status display and diagnostics
- embedded chat and activity surfaces

### Spacebot responsibilities

- own agent runtime, messaging, memory, tools, and control API
- remain independently deployable and independently upgradeable

## Why Not Manage Spacebot in `sd-core`

`sd-core` is the VDFS daemon. Spacebot is its own daemon-like runtime with its own process lifecycle, logs, warmup state, secrets, agent graph, and HTTP UI model.

Putting child-process management directly into `sd-core` would:

- blur product boundaries
- complicate server and mobile targets unnecessarily
- make local-only process concerns leak into the core library

The right split is:

- config lives in core
- process supervision lives in the platform shell

## Proposed Spacedrive Config Shape

Add a new block to `AppConfig` in `spacedrive/core/src/config/app_config.rs`.

Suggested shape:

```text
spacebot:
  enabled
  mode                 # managed_local | external_local | remote
  base_url
  auth_token
  manage_process
  binary_path
  config_path
  instance_dir
  auto_start
  connect_on_launch
  last_known_status
```

Notes:

- `auth_token` should not stay in plain app config long-term if we already have a stronger secret storage primitive available.
- v1 can store token in config only if necessary, but the preferred direction is secure storage.

## Proposed Core Operations

Add config-backed operations for Spacebot.

### Core Queries

- `spacebot.config.get`
- `spacebot.status.get`
- `spacebot.health.get`

### Core Actions

- `spacebot.config.update`
- `spacebot.connect`
- `spacebot.disconnect`
- `spacebot.start_managed`
- `spacebot.stop_managed`

These can begin as thin wrappers around app config and platform commands.

## Proposed Desktop Platform Commands

The Tauri layer already manages `sd-daemon`. Reuse that pattern for Spacebot.

Recommended commands:

- `spacebot_start`
- `spacebot_stop`
- `spacebot_restart`
- `spacebot_status`
- `spacebot_logs_path`

Managed Local should:

- launch Spacebot in foreground mode
- inject or point to a dedicated config path
- set `SPACEBOT_DIR` or equivalent instance path
- wait for `GET /api/health`
- then wait for `GET /api/agents/warmup` if chat UI depends on readiness

## Connection Client Abstraction

Add a lightweight Spacebot client in the app layer or shared TypeScript layer.

Recommended methods:

- `health()`
- `status()`
- `warmupStatus()`
- `sendWebchatMessage(agentId, sessionId, senderName, message)`
- `getWebchatHistory(agentId, sessionId)`
- `subscribeEvents()`

This should be HTTP plus SSE based, independent of whether the instance is local or remote.

## UI Placement

### Settings

Best fit:

- extend `spacedrive/packages/interface/src/Settings/pages/ServicesSettings.tsx`

Add a Spacebot section with:

- mode selector
- managed-local start on launch toggle
- local URL / remote URL
- auth token input
- connection test button
- health, status, and warmup indicators

### Chat Surface

Recommended first placement:

- a dedicated Spacebot route or panel in the interface

The first slice does not need to fully replicate the Spacebot dashboard. It only needs a clean embedded chat surface plus basic runtime status.

## Data and Security Model

### Local managed instance

Recommended default:

- store Spacebot instance data under Spacedrive's data root but in a separate subtree

Example:

```text
<spacedrive-data-dir>/spacebot/
  instance/
  config.toml
  logs/
```

This keeps ownership clear while preserving process separation.

### Auth

- local managed can run without auth if strictly loopback bound
- local external should support optional bearer token
- remote should require bearer token in practice

### Secret storage

Preferred direction:

- store remote bearer tokens outside plain JSON config when possible

## Event Model

Spacebot already emits a global SSE stream from `/api/events`.

V1 recommendation:

- consume it directly in the UI client
- filter by `agent_id` and `channel_id` client-side
- do not mirror all Spacebot events into Spacedrive core yet

Why:

- less duplication
- less coupling
- fewer translation bugs

Future:

- if the rest of Spacedrive needs Spacebot events, add a narrow translated event layer later

## Session Model

Use Spacebot's webchat model as the first integration path.

Suggested mapping:

- one Spacedrive user session or panel maps to one `session_id`
- one chosen Spacebot agent maps to `agent_id`
- user input goes to `/api/webchat/send`
- UI state is hydrated from `/api/webchat/history`
- live output comes from `/api/events`

This is enough to ship first-class chat without adopting the full Spacebot dashboard API surface.

## Risks

### Mode complexity

Supporting managed local, external local, and remote is correct, but the UX can get confusing fast.

Mitigation:

- make Managed Local the recommended default
- place External Local and Remote behind an explicit advanced setup flow

### Readiness mismatch

`/api/health` only means the HTTP server is up. It does not mean the agent is ready.

Mitigation:

- gate chat UX on warmup status, not liveness alone

### Secrets in app config

Remote bearer tokens should not live forever in plain JSON.

Mitigation:

- v1 can be pragmatic
- v2 should move tokens to secure storage

### Tight product coupling

If Spacedrive starts depending on too much of Spacebot's internal API surface, upgrades get harder.

Mitigation:

- define a narrow Spacedrive-facing client contract
- start with webchat, status, health, and SSE only

## Phased Plan

### Phase 1: Config and Discovery

- add Spacebot config block to app config
- add settings UI for connection mode and endpoint
- add lightweight client for health/status/warmup

### Phase 2: Managed Local

- add Tauri platform commands to start and stop Spacebot
- add supervised child-process support
- create dedicated Spacebot instance directory under Spacedrive data

### Phase 3: Embedded Chat

- add Spacebot panel or route
- send messages via `/api/webchat/send`
- show history via `/api/webchat/history`
- stream updates via `/api/events`

The first prototype can ship with a config-gated chat route, handwritten request types for the narrow webchat surface, and polling for history before the SSE layer is wired in.

### Phase 4: Deeper Integration

- agent picker
- worker status and live activity
- memory and task views if useful
- cross-link Spacebot with Spacedrive repository and file contexts

### Phase 5: Remote Hardening

- secure token storage
- richer diagnostics
- better reconnect behavior

## Recommendation

Ship first-class Spacebot support as a companion-runtime integration.

Start with:

- **Managed Local** as the default
- **External Local** as the easy advanced path
- **Remote** as the same client abstraction with a different base URL

Keep the boundary at HTTP plus SSE. Keep process supervision in the desktop shell. Keep settings in Spacedrive core config. Use Spacebot's webchat model first.

That gives Spacedrive deep native Spacebot support without pretending the two runtimes should already be one.
