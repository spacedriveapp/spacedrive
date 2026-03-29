# Spacebot–Spacedrive Integration Contract

## Purpose

Define the exact boundary between Spacebot and Spacedrive so both products remain independently functional while gaining real capabilities when paired together.

Both sides need a flag. Spacebot needs to know whether Spacedrive is present. Spacedrive needs to know which device hosts Spacebot. Neither product should break without the other.

---

## Principles

1. **Both products work alone.** Spacebot runs standalone with Discord, Slack, Telegram, webchat. Spacedrive runs standalone as a file manager. Neither requires the other.
2. **Pairing is opt-in.** A configuration flag on each side enables the integration. Disabled by default.
3. **Spacedrive is the device graph.** Spacebot never owns device identity, library membership, or multi-device topology. It receives that information from Spacedrive.
4. **Spacebot is the agent runtime.** Spacedrive never runs LLM processes, manages agent memory, or orchestrates workers. It delegates that to Spacebot.
5. **The library is the boundary.** A Spacebot instance is paired to a library, through a specific device. Every device in that library can access Spacebot through the paired device.
6. **No leader device.** Spacedrive's P2P system is leaderless. There is no master device. But there is exactly one device that hosts Spacebot — that device has the `spacebot_host` capability, and all other devices route through it.

---

## Spacebot Side

### Config: `[spacedrive]`

Add a new top-level section to Spacebot's `config.toml`:

```toml
[spacedrive]
enabled = false
```

That is the minimum. When `enabled = false` (the default), Spacebot operates exactly as it does today. No Spacedrive awareness, no device graph, no remote execution. All tools run locally against the workspace filesystem.

When `enabled = true`, Spacebot expects a Spacedrive node to be reachable. This unlocks:

- **Device graph awareness** — Spacebot can see all devices in the paired library.
- **Remote execution** — workers can target specific devices for shell/file operations.
- **File System Intelligence** — agents receive context and policy when navigating paths through Spacedrive.
- **Proxy chat** — Spacedrive devices in the library can reach Spacebot through the P2P layer without a direct HTTP connection.

### Config Shape

```rust
pub struct SpacedriveIntegrationConfig {
    /// Master switch. When false, Spacebot has no Spacedrive awareness.
    pub enabled: bool,

    /// How to reach the paired Spacedrive node.
    /// Default: "http://127.0.0.1:7872" (local co-located node).
    pub api_url: Option<String>,

    /// Auth token for the Spacedrive API, if required.
    pub api_key: Option<String>,

    /// Library ID this Spacebot instance is paired with.
    /// Set during pairing. Spacebot only operates within this library.
    pub library_id: Option<String>,

    /// Device UUID of the paired Spacedrive node.
    /// This is the device that Spacebot "lives on" in the library graph.
    pub device_id: Option<String>,
}
```

```toml
# Minimal — disabled, standalone Spacebot
[spacedrive]
enabled = false

# Paired — co-located Spacebot and Spacedrive on the same machine
[spacedrive]
enabled = true
api_url = "http://127.0.0.1:7872"
library_id = "a1b2c3d4-..."
device_id = "e5f6g7h8-..."
```

### Where It Plugs In

**Config types** (`src/config/types.rs`):
- Add `spacedrive: SpacedriveIntegrationConfig` to the top-level `Config` struct, alongside `llm`, `defaults`, `agents`, `messaging`, etc.
- Default is `enabled: false`. All other fields are `Option` and only relevant when enabled.

**Agent initialization** (`src/main.rs`):
- When `spacedrive.enabled`, create a `SpacedriveClient` that connects to the paired node.
- Pass the client into `AgentDeps` so branches and workers can access it.
- Query the device graph on startup and refresh periodically.

**Worker tool server** (`src/tools.rs`):
- When Spacedrive is enabled and a worker has an `execution_target` set to a remote device, swap local shell/file tool implementations for Spacedrive-proxied versions.
- When no `execution_target` is set, tools run locally as they do today.
- The tool interface stays identical from the model's perspective. Only the backend changes.

**Runtime behavior when `enabled = true` but Spacedrive is unreachable:**
- Spacebot should start normally and log a warning.
- Local tools continue to work.
- Remote execution tools fail with a clear error if the target device is unreachable.
- Spacebot retries the Spacedrive connection in the background.
- This must not block agent startup or conversation.

---

## Spacedrive Side

### AppConfig: `spacebot`

Spacedrive already has a `SpacebotConfig` in `AppConfig` (`core/src/config/app_config.rs`):

```rust
pub struct SpacebotConfig {
    pub enabled: bool,
    pub base_url: String,
    pub auth_token: Option<String>,
    pub default_agent_id: String,
    pub default_sender_name: String,
}
```

This is currently a direct-connection config (HTTP URL + token). It needs to evolve to support the paired-node model where Spacebot is reached through the P2P layer, not just through a direct HTTP URL.

### Proposed Evolution

```rust
pub struct SpacebotConfig {
    /// Master switch. When false, no Spacebot UI or functionality.
    pub enabled: bool,

    /// Connection mode.
    pub mode: SpacebotConnectionMode,

    /// HTTP base URL (used in ManagedLocal and ExternalLocal modes).
    pub base_url: String,

    /// Bearer auth token for direct HTTP connections.
    pub auth_token: Option<String>,

    /// Path to the Spacebot binary (Managed Local mode).
    pub binary_path: Option<PathBuf>,

    /// Path to the Spacebot config.toml to use (Managed Local mode).
    pub config_path: Option<PathBuf>,

    /// Instance directory for managed Spacebot data.
    pub instance_dir: Option<PathBuf>,

    /// Auto-start Spacebot when Spacedrive launches (Managed Local mode).
    pub auto_start: bool,

    /// Default agent to target from the embedded chat.
    pub default_agent_id: String,

    /// Default sender name used by the embedded chat.
    pub default_sender_name: String,
}

pub enum SpacebotConnectionMode {
    /// Spacedrive launches and supervises Spacebot as a child process.
    ManagedLocal,
    /// Spacedrive connects to an already-running local Spacebot.
    ExternalLocal,
    /// Spacedrive connects to a remote Spacebot via the P2P layer,
    /// routing through the device that has `spacebot_host` capability.
    Library,
}
```

The `Library` mode is the new one. In this mode, Spacedrive does not connect to Spacebot over HTTP directly. Instead, it routes messages through the P2P system to whichever device in the library has the `spacebot_host` capability. That device proxies to its local Spacebot instance.

### Device Table: `spacebot_host` Capability

The device table already has a `capabilities` JSON field that syncs across all devices in the library:

```json
{"indexing": true, "p2p": true, "volume_detection": true}
```

Add `spacebot_host`:

```json
{"indexing": true, "p2p": true, "volume_detection": true, "spacebot_host": true}
```

This is a boolean flag on the device record. It is set on exactly one device in the library — the device that runs Spacebot. It syncs automatically to all other devices via the existing shared-resource sync protocol (HLC-ordered, last-write-wins on the device record).

**Rules:**

- At most one device in a library may have `spacebot_host: true`. If a second device claims it, the UI should warn and the user should resolve the conflict.
- The flag is set when Spacebot is paired to this device (either through managed local startup or manual configuration).
- The flag is cleared when Spacebot is unpaired or the device is removed from the library.
- Any device in the library can query the device list and find the `spacebot_host` device.

**Why this lives in capabilities, not a separate table:**

- It syncs automatically. Every device in the library sees it without any new sync protocol work.
- It is queryable alongside other device metadata.
- It does not require a migration to add a new column — `capabilities` is already a JSON blob.
- It follows the existing pattern for device feature flags.

### What the `spacebot_host` Flag Enables

When a Spacedrive device sees another device in the library with `spacebot_host: true`:

1. **It knows Spacebot exists in this library.** The UI can show Spacebot features even if the local device is not the host.
2. **It knows where to route.** Chat messages, approvals, and status queries route to the host device over P2P.
3. **It knows the host's online status.** If the host device is offline, the UI can show "Spacebot offline" instead of a broken connection.

When the local device itself has `spacebot_host: true`:

1. **It runs the Spacebot proxy.** It accepts forwarded messages from other devices and relays them to the local Spacebot HTTP API.
2. **It sets the capability on its own device record.** This propagates to all other devices automatically.
3. **It manages the Spacebot lifecycle** (in managed local mode).

---

## The Proxy

### Raw HTTP Proxy, Not a Typed Protocol

The proxy is not a typed message contract. It is a raw HTTP proxy. A Spacedrive device sends an HTTP request over the P2P connection to the host device, and the host device forwards it to `127.0.0.1:19898` (or whatever Spacebot's API is bound to) and returns the response verbatim.

This means:

- No typed message definitions to maintain on the Spacedrive side.
- No translation layer between Spacedrive's internal types and Spacebot's API.
- Spacebot's API evolves freely — new endpoints, new fields, new event types — and the proxy carries them without changes.
- The desktop interface can use the `@spacebot/api-client` package directly against the proxy URL the same way it uses it against a local Spacebot instance.
- Mobile uses the same proxy through Spacedrive core operations that tunnel HTTP over P2P.

The proxy is transparent. From the client's perspective, it is hitting a Spacebot HTTP API. The only difference is the transport — P2P instead of TCP.

### SSE Relay

SSE is the one part that is not a simple request-response proxy. The host device maintains a single SSE subscription to Spacebot's `/api/events` endpoint and relays events to connected peers over the P2P connection as they arrive.

This is still untyped relay — the host device does not parse or filter the SSE events. It forwards the raw event stream. The receiving device's client code parses and handles events the same way it would with a direct SSE connection.

If multiple devices are connected, the host fans out the same event stream to each. If no devices are connected, it can drop the SSE subscription and re-establish it when a device connects.

### What the Host Device Runs

The Spacebot host device runs a `SpacebotProxy` inside `sd-core` that:

1. Accepts inbound HTTP-over-P2P requests from peer devices.
2. Forwards them to the local Spacebot instance and returns the response.
3. Maintains one SSE subscription to Spacebot and relays events to connected peers.

That is the entire proxy. No caching, no typed messages, no operation-level logic. If Spacebot adds a new endpoint tomorrow, it works through the proxy immediately.

This service only runs on the device with `spacebot_host: true`. Other devices do not run it.

---

## Remote Execution

When Spacebot has `[spacedrive] enabled = true`, workers gain the ability to target specific devices.

### How It Works

1. The agent (channel or branch) decides which device should perform a task. It has access to the device graph — a list of all library devices with their names, slugs, online status, and capabilities.

2. The agent spawns a worker with an `execution_target`:
   ```
   spawn_worker(task: "run tests on the MacBook", execution_target: "jamies-macbook-pro")
   ```

3. The worker's tool server detects that `execution_target` is set to a remote device. Instead of registering local shell/file tools, it registers proxy versions:
   - `ShellTool` → `RemoteShellTool` (sends shell commands to the target device through Spacedrive)
   - `FileReadTool` → `RemoteFileReadTool` (reads files on the target device)
   - `FileWriteTool` → `RemoteFileWriteTool` (writes files on the target device)
   - `FileListTool` → `RemoteFileListTool` (lists files on the target device)

4. The proxy tools send typed execution requests to the paired Spacedrive node, which:
   - Resolves effective policy for the agent principal + target device + path + operation
   - If allowed, forwards the request to the target Spacedrive device over P2P
   - The target device executes locally and returns the result
   - The result returns to the worker through the proxy chain

5. From the model's perspective, the tools are identical. It calls `shell` with a command and gets output back. It does not need to know the command ran on a different machine.

### Policy Enforcement

Every remote operation passes through Spacedrive's permission system:

- **Device access policy** — which devices can Spacebot target?
- **Subtree policy** — which paths are readable/writable on those devices?
- **Operation policy** — which operations are allowed (list, read, write, shell, delete)?
- **Confirmation policy** — which operations require live user approval?

Policy is resolved on the paired Spacedrive node before forwarding. The target device may enforce a second check.

### When `execution_target` Is Not Set

When a worker has no `execution_target`, it runs locally on the Spacebot host machine using standard local tools. This is the current behavior and remains the default. The Spacedrive integration adds remote execution as an opt-in capability per worker, not a global replacement.

---

## Pairing Flow

### First-Time Setup

The pairing flow connects a Spacebot instance to a Spacedrive library:

1. **User enables Spacebot in Spacedrive settings.** Sets connection mode to Managed Local or External Local.

2. **Spacedrive detects or starts Spacebot.** In managed local mode, Spacedrive launches the Spacebot binary. In external local mode, Spacedrive connects to the configured URL.

3. **Spacedrive sets `spacebot_host: true` on the local device record.** This propagates to all devices in the library via sync.

4. **Spacedrive writes the pairing info to Spacebot's config.** Sets `[spacedrive] enabled = true`, `library_id`, and `device_id` in Spacebot's `config.toml`. If managed local, Spacedrive owns this config file. If external, the user configures it manually or Spacedrive writes it via Spacebot's settings API.

5. **Spacebot reads its config and connects to the Spacedrive API.** It queries the device graph and becomes aware of all devices in the library.

6. **Other devices in the library see the `spacebot_host` flag.** Their UI shows Spacebot as available. They can open the chat surface and route messages through the P2P layer to the host device.

### Unpairing

1. User disables Spacebot in Spacedrive settings, or removes the host device from the library.
2. Spacedrive clears `spacebot_host: true` from the device capabilities.
3. Spacedrive stops the managed Spacebot process (if managed local).
4. Other devices see the flag disappear and remove the Spacebot UI.
5. Spacebot continues running but loses Spacedrive awareness (reverts to standalone).

---

## Mobile

The mobile app (`apps/mobile/`) reaches Spacebot through the same P2P proxy that desktop devices use. It does not need a direct HTTP connection to Spacebot.

### How Mobile Finds Spacebot

1. The mobile app is a Spacedrive device in the library. It has its own device UUID and is registered in the library database.
2. When the library syncs, the mobile device receives all device records, including the one with `spacebot_host: true`.
3. The mobile app establishes a P2P connection to the host device (or routes through another connected device via proxy pairing).
4. HTTP requests to Spacebot go through the `SpacebotProxy` on the host device, which forwards them to the local Spacebot API and returns the response.

### Mobile Chat Surface

The mobile app sends HTTP requests to Spacebot through Spacedrive core, which tunnels them over P2P to the host device. From the mobile code's perspective, it is calling a Spacebot API — it does not need to know whether the request traveled over localhost or across the planet.

Spacedrive core handles the routing internally:
- If the local device is the Spacebot host → direct HTTP call to `localhost:19898`
- If another device is the host → HTTP-over-P2P to that device → proxy to Spacebot

The mobile app does not need to know which path was taken.

### Mobile Scope

First slice for mobile:
- Chat screen (send messages, receive streaming responses)
- Active tasks and status
- Approval requests (push from host device when a worker needs confirmation)

Not needed on mobile initially:
- Worker transcript inspection
- Memory management
- Agent configuration
- Schedule management

---

## What Each Side Exposes

### Spacedrive Exposes to Spacebot

When Spacebot queries the Spacedrive API:

- **Device graph** — all devices in the library, with name, slug, form factor, OS, online status, capabilities.
- **Location list** — indexed locations per device, with paths and metadata.
- **File System Intelligence** — context nodes, policies, and summaries for paths the agent navigates.
- **Remote execution** — typed shell/file operations forwarded to target devices with policy enforcement.
- **Audit trail** — every remote operation is logged with agent principal, target device, path, operation, and result.

### Spacebot Exposes to Spacedrive

When Spacedrive queries the Spacebot API (directly or through the proxy):

- **Agent list** — available agents with id, name, role, warmup status.
- **Webchat** — send messages, fetch history, create conversations.
- **SSE events** — streaming deltas, typing state, worker events.
- **Task list** — active and recent tasks with status, assignees, linked conversations.
- **Status** — version, uptime, health, warmup readiness.

---

## Summary of Changes

### Spacebot Changes

| Change | Location | Description |
|---|---|---|
| `SpacedriveIntegrationConfig` struct | `src/config/types.rs` | New config section with `enabled`, `api_url`, `library_id`, `device_id` |
| TOML parsing for `[spacedrive]` | `src/config/load.rs` | Parse the new section, all fields optional when disabled |
| `SpacedriveClient` | new module | HTTP client for Spacedrive API (device graph, FSI, remote exec) |
| Device graph query | agent init | Fetch and cache library device list on startup |
| `execution_target` on workers | `src/agent/worker.rs` | Optional device slug/UUID that routes tools to a remote device |
| Remote tool variants | `src/tools.rs` | `RemoteShellTool`, `RemoteFileReadTool`, etc. that proxy through Spacedrive |
| Graceful degradation | agent init | Warn and continue if Spacedrive is unreachable |

### Spacedrive Changes

| Change | Location | Description |
|---|---|---|
| Evolve `SpacebotConfig` | `core/src/config/app_config.rs` | Add `mode`, `binary_path`, `config_path`, `instance_dir`, `auto_start` |
| `SpacebotConnectionMode` enum | `core/src/config/app_config.rs` | `ManagedLocal`, `ExternalLocal`, `Library` |
| `spacebot_host` capability | device `capabilities` JSON | Boolean flag on device record, syncs automatically |
| `SpacebotProxy` | new service in `core/src/service/` | Runs on host device, forwards HTTP-over-P2P to local Spacebot |
| P2P HTTP tunnel | `core/src/service/network/` | Carries raw HTTP requests/responses and SSE relay over QUIC |
| Core operations | `core/src/ops/spacebot/` | Thin wrappers that route HTTP to Spacebot (local or via P2P proxy) |
| Agent principal model | `core/src/domain/` | New principal type representing a Spacebot instance in the library |
| Policy model | `core/src/domain/` | Device, subtree, operation, and confirmation policies for agent access |
| Settings UI | `packages/interface/` | Connection mode selector, managed local controls, status display |
| Mobile chat | `apps/mobile/` | Chat screen using core operations routed through P2P proxy |

---

## Implementation Order

### Phase 1: Flags and Config

Both sides get their flags. No runtime behavior changes yet.

**Spacebot:**
- Add `SpacedriveIntegrationConfig` to config types
- Parse `[spacedrive]` section in config loader
- Default `enabled = false`, no behavior change

**Spacedrive:**
- Evolve `SpacebotConfig` with connection mode and new fields
- Add `spacebot_host` to `DeviceCapabilities` (the typed struct in `session.rs`)
- Add `spacebot_host` to the capabilities JSON set during device registration
- Core operations to get/update spacebot config

### Phase 2: Direct Connection (Desktop)

Desktop Spacedrive connects to a local Spacebot instance. No P2P proxy yet.

**Spacedrive:**
- Managed local: Tauri spawns Spacebot as a child process
- External local: connect to existing Spacebot at configured URL
- Settings UI for connection mode and status
- Replace hardcoded values in the existing Spacebot interface components

**Spacebot:**
- No changes needed. The existing webchat API already works.

### Phase 3: P2P Proxy

Non-host devices reach Spacebot through the host device.

**Spacedrive:**
- `SpacebotProxy` on host device — raw HTTP proxy + SSE relay over P2P
- Core operations that route HTTP to Spacebot transparently (local or via proxy)
- The host device sets `spacebot_host: true` on its device record

### Phase 4: Mobile Chat

Mobile devices use the same P2P proxy.

**Spacedrive mobile:**
- Chat screen using `spacebot.send_message` core action
- Streaming responses via core subscription events
- Task and approval display

### Phase 5: Remote Execution

Spacebot workers can target remote devices.

**Spacebot:**
- `SpacedriveClient` queries device graph
- `execution_target` on worker spawn
- Remote tool variants that proxy through Spacedrive

**Spacedrive:**
- Agent principal model
- Policy resolution (device + subtree + operation + confirmation)
- Remote execution protocol (typed operations forwarded to target devices)
- Audit logging

### Phase 6: File System Intelligence

Agents receive context and policy when navigating paths.

**Spacedrive:**
- Context node storage and queries
- Policy resolution API
- Agent-readable context surfaced during navigation

**Spacebot:**
- Query FSI when listing or reading files through Spacedrive
- Surface context in worker prompts
- Write observations back with attribution

---

## Open Questions

1. **Config ownership in managed local mode.** Should Spacedrive generate Spacebot's `config.toml` entirely, or should it only inject the `[spacedrive]` section and leave the rest to the user?

2. **Multiple libraries.** Can one Spacebot instance pair with multiple libraries, or is it strictly one-to-one? The current design assumes one library per Spacebot instance. Multiple libraries would require a library selector in the Spacebot config.

3. **Host migration.** What happens when the user wants to move Spacebot from one device to another? The `spacebot_host` flag on the old device needs to be cleared and set on the new one. Should this be a UI action or automatic?

4. **Offline host.** When the host device is offline, should other devices show a "Spacebot unavailable" state, or should they try to reach Spacebot through relay? The answer depends on whether Spacebot can be reached via Iroh relay when the host device is on a different network.

5. **Auth between Spacebot and Spacedrive.** When both run on the same machine, auth may be unnecessary (loopback only). When remote, they need mutual authentication. Should this use the existing Spacedrive session keys from pairing, or a separate shared secret?
