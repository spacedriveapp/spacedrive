# Spacebot in Spacedrive — Interface Vision

This document describes the target experience for the Spacebot surface inside Spacedrive. It is written for developers building this interface. It covers what exists today, what the interface should become, and how to get there without rewriting everything at once.

For architectural context on how Spacebot integrates with Spacedrive at the system level, see `spacedrive/docs/core/design/spacebot-integration.md`. For the broader product direction, see `company/new-direction/2026-03-20-SPACEDRIVE-DIRECTION.md`.

---

## What Exists Today

The current implementation is a first working slice of Spacebot embedded inside Spacedrive. It connects to a local Spacebot instance over HTTP and SSE, and provides a functional chat experience.

### Working

- **SpacebotLayout** — sidebar with nav items (Chat, Tasks, Memories, Autonomy, Schedule), project list, and conversation history. Top bar with agent selector, search, and new-chat button.
- **Chat** — full conversation flow via Spacebot's webchat API. Messages sent through `POST /api/webchat/send`, history fetched via `/api/webchat/history`, live streaming via SSE deltas. Virtualized message list.
- **Conversations** — create, list, switch between conversations. Sidebar shows history with search filtering.
- **InlineWorkerCard** — expandable cards showing worker task, status, tool call count, and live status. Drill into full transcript with paired tool calls and results.
- **ToolCall** — structured display of tool invocations with args, results, shell output formatting, and error detection.
- **Markdown** — rendered assistant responses with GFM, code blocks, tables, links.
- **ChatComposer** — input with project/model selectors, voice overlay trigger, animated expand on focus, send button.
- **SSE event source** — reconnecting EventSource with backoff, typing state, streaming deltas, message completion events.
- **Agent selector** — switch between agents (hardcoded: Star, Operations, Builder).
- **Voice overlay** — triggers a separate Tauri window for voice interaction.

### Placeholder

- **Tasks** — stub route, no implementation.
- **Memories** — stub route, no implementation.
- **Autonomy** — stub route, no implementation.
- **Schedule** — stub route, no implementation.

### Hardcoded

- Server URL is `http://127.0.0.1:19898` (set in SpacebotContext).
- Agents list is static in SpacebotContext.
- Projects list is static in SpacebotContext.
- Model options are static in SpacebotContext.
- EmptyChatHero greets "James" by name.

---

## What This Should Become

The Spacebot surface inside Spacedrive is not a dashboard for monitoring an agent runtime. It is the primary interface where a person works with their AI. The direction doc frames it as the place a person opens in the morning and lives in all day. That means the interface should feel like a workspace, not a control panel.

### The Core Triangle

The product thesis identifies three primitives that define the work experience:

1. **Conversation** — where intent is expressed.
2. **Task** — where responsibility is managed.
3. **File** — where knowledge persists.

The Spacebot surface owns the first two and connects to the third. Chat is the front door but not the whole house. The interface should make it easy to move between talking to the agent, reviewing what work is happening, and seeing what was produced.

### Personal Before Technical

The default experience should start with work, not settings. What matters right now. What the agent is doing. What needs you. What was produced. What you should do next.

Technical depth — agent configuration, model routing, memory inspection, worker transcripts — should exist but live behind the daily operating layer. An employee sees their chat and tasks. An admin also sees agent management, policy, and system health.

### The Daily Surface

When a person opens the Spacebot section of Spacedrive, they should see:

- Their ongoing conversations, with the most recent or active one immediately accessible.
- Active tasks and their current state — who is working on what, what is blocked, what needs approval.
- Recent artifacts produced by the agent — files created, documents written, code committed.
- A fast path to start a new conversation or assign new work.

This is not five separate pages behind sidebar tabs. It is a cohesive surface where these elements are visible together or flow naturally between each other.

---

## Interface Direction

### Chat

Chat is working. The next steps are refinement, not rewrite.

**Immediate improvements:**

- Replace the hardcoded greeting with the user's actual name from the library or platform context.
- Replace hardcoded agents, projects, and model lists with data fetched from Spacebot's API (`/api/agents`, `/api/status`).
- Replace the hardcoded server URL with the Spacebot connection config from `AppConfig` (managed local, external local, or remote — see `spacebot-integration.md`).
- Add connection status indicator. The current SSE hook tracks connection state but it is not surfaced in the UI. Show whether Spacebot is connected, reconnecting, or offline.
- Add a warmup readiness gate. Do not show the chat composer until `/api/agents/warmup` reports the agent is ready. Show a clear loading or warming-up state instead.

**Near-term improvements:**

- File attachments in the composer. Spacebot's webchat API supports them.
- Better empty state when Spacebot is not running or not configured. Guide the user to settings instead of showing a broken chat.
- Conversation titles that update from the agent's first response, not just "Untitled".
- Keyboard shortcuts: focus composer, navigate conversations, copy last response.

### Tasks

Tasks are the most important missing surface. The direction doc makes the task the primary unit of business work — shared between humans and agents, not scoped to agent sessions.

**What a task looks like in this interface:**

- Title, status, priority, assignees (human and agent).
- Conversation thread linked to the task.
- Linked files and documents produced during the task.
- Execution runs with expandable worker transcripts (reuse InlineWorkerCard).
- Approval state — whether the task is waiting on human input.

**Data source:** Spacebot currently tracks tasks internally. The first slice should query Spacebot's API for task data and render it here. The longer-term model moves toward org-scoped tasks owned by Spacedrive with Spacebot as one executor, but the interface can start with what Spacebot already provides.

**First slice:**

- List active and recent tasks for the current agent.
- Show task detail with status, linked conversation, and worker runs.
- Allow marking tasks as approved or completed from the UI.

### Memories

The memory surface should make the agent's knowledge visible and inspectable without requiring the user to understand the internal memory graph.

**What matters to the user:**

- What does the agent know about me?
- What has it learned from our conversations?
- What decisions has it recorded?
- Can I correct something it got wrong?

**First slice:**

- List memories by type (Fact, Preference, Decision, Identity, Goal).
- Search memories by content.
- View memory detail with source attribution and graph edges.
- Delete or edit individual memories.

**Data source:** Spacebot's memory API.

### Autonomy

Autonomy controls what the agent is allowed to do without asking. This maps to the confirmation policy and operation policy described in the remote execution design.

**What matters to the user:**

- What can the agent do on its own?
- What requires my approval?
- Which devices and paths is it allowed to access?
- What are the current safety boundaries?

**First slice:**

- Display current autonomy level (e.g., ask before destructive actions, allow reads, require approval for shell).
- Toggle broad autonomy presets.
- Show pending approval requests if any exist.

**Longer-term:** This surface merges with Spacedrive's File System Intelligence policy UI, where subtree permissions and agent access rules are managed per-location. The Spacebot autonomy view becomes one lens into the broader permission model.

### Schedule

Schedule maps directly to Spacebot's cron job system.

**First slice:**

- List active cron jobs for the current agent.
- Show job name, schedule expression, last run time, next run time, status.
- Create a new scheduled job from conversation or from this UI.
- Enable, disable, or delete jobs.
- Show recent execution history per job.

**Data source:** Spacebot's cron API.

---

## Layout Evolution

The current layout is a fixed sidebar plus a single content area. This works for chat but will feel cramped as tasks, files, and activity need to coexist.

### Near-term

Keep the sidebar. Add content to the existing routes. The sidebar already has the right nav structure — Chat, Tasks, Memories, Autonomy, Schedule. Fill in the placeholder routes with real implementations.

### Medium-term

Consider a split-view or panel model where chat and tasks can be visible simultaneously. A person should be able to talk to the agent while reviewing the task list or watching a worker run, without switching routes.

This does not mean a complex multi-pane layout on day one. It means designing components so they can exist side by side later. Keep chat and task views self-contained enough that they can be composed into a split layout without refactoring their internals.

### Agent Context Bar

The sidebar currently shows projects and conversation history. Over time it should also surface:

- Active workers and their live status.
- Pending approvals that need attention.
- Connected devices (when remote execution exists).
- The agent's current "focus" or active task.

This makes the sidebar a peripheral awareness surface — you can glance at it and know what is happening without interrupting your current view.

---

## Connection Model

The current implementation hardcodes `http://127.0.0.1:19898`. The integration design defines three connection modes: Managed Local, External Local, and Remote.

### What the interface needs

- Read connection config from `AppConfig` (`spacebot.mode`, `spacebot.base_url`, `spacebot.auth_token`).
- Pass auth token as a Bearer header when configured.
- Show connection state prominently — connected, connecting, offline, error.
- In Managed Local mode, show process state — starting, running, stopped, crashed.
- Settings page for configuring the connection (mode selector, URL input, token input, connection test).

### Migration path

1. Replace the hardcoded `setServerUrl` call with a config-driven value.
2. Add a connection status component to the top bar or sidebar.
3. Build the settings page (likely under the existing Services settings pattern).
4. Add managed local process controls (start/stop) once the Tauri platform commands exist.

---

## Data Flow

The current data flow is clean and should be preserved:

```
SpacebotContext (provider)
  → TanStack Query for conversations, history, workers
  → SSE EventSource for live updates
  → Mutations for sending messages, creating conversations
  → Routes consume context via useSpacebot()
```

### What to add

- **Agent list query** — fetch from `/api/agents` instead of hardcoding.
- **Status query** — fetch from `/api/status` for version, uptime, warmup state.
- **Task queries** — fetch task list and detail from Spacebot's API.
- **Memory queries** — fetch memories with search and filtering.
- **Cron queries** — fetch scheduled jobs and execution history.
- **Config integration** — read Spacebot connection settings from Spacedrive's AppConfig via the existing core query system.

Each new data source should follow the same pattern: TanStack Query for fetching, mutations for writes, SSE for live updates where available.

### API Client

The `@spacebot/api-client` package already exists and is used throughout. New API methods should be added there as Spacebot's API surface is consumed. Keep it as the single point of contact with the Spacebot HTTP API.

---

## Component Principles

These apply to everything built in this directory.

### Reuse what exists

- **InlineWorkerCard** and **ToolCall** are solid. Use them in task detail views, not just conversation timelines.
- **ChatComposer** is well-built. It should remain the single input component for all conversation contexts.
- **Markdown** renderer is complete. Use it for any agent-generated text.
- **useSpacebotEventSource** handles reconnection well. Extend its handler map for new event types rather than creating parallel SSE connections.

### Follow Spacedrive conventions

- Semantic color classes only. No `var()` references. No arbitrary hex values.
- Radix and @sd/ui primitives for interactive elements.
- TanStack Query for all data fetching. No manual `useEffect` + `fetch`.
- Function components. Explicit TypeScript interfaces. No `any`.
- See the interface `CLAUDE.md` for the full rulebook.

### Keep components composable

Every major view component should work both as a full route and as a panel that could be embedded in a split layout. This means:

- No assumptions about being the only thing on screen.
- No hardcoded widths that break at smaller sizes.
- Accept data through props or context, not by owning the data fetching internally when it could be shared.

### Performance

- Virtual scrolling for any list that could exceed 50 items (conversations, tasks, memories, tool calls).
- Lazy load route components. Tasks, Memories, Autonomy, and Schedule are independent features that do not need to be in the initial bundle.
- Memoize expensive derived data (tool call pairing, memory filtering, task grouping).

---

## Phases

### Phase 1 — Unblock the hardcodes

- Replace hardcoded agents, projects, models with API-driven data.
- Replace hardcoded server URL with config-driven connection.
- Replace hardcoded greeting with user's actual name.
- Add connection status indicator.
- Add warmup readiness gate before showing chat.

### Phase 2 — Tasks

- Implement the Tasks route with list and detail views.
- Connect to Spacebot's task API.
- Show task status, linked conversation, worker runs, approval state.
- Reuse InlineWorkerCard for execution display.

### Phase 3 — Schedule and Memories

- Implement the Schedule route with cron job list, create, enable/disable.
- Implement the Memories route with list, search, detail, delete.
- Connect both to their respective Spacebot APIs.

### Phase 4 — Autonomy and Settings

- Implement the Autonomy route with current permission display and toggles.
- Build the Spacebot connection settings page.
- Add managed local process controls (start/stop/restart) once platform commands exist.

### Phase 5 — Layout and integration depth

- Add split-view or panel model for simultaneous chat + tasks.
- Enrich the sidebar with live worker status, pending approvals, device state.
- Cross-link Spacebot artifacts with Spacedrive file views.
- Surface File System Intelligence context when the agent references paths.

---

## Multi-Device Agent Access

The desktop Spacebot surface connects directly to a Spacebot instance over HTTP and SSE. That works when Spacebot is running on the same machine or reachable over the network. But mobile devices, remote laptops, and other Spacedrive nodes in the library cannot always reach Spacebot directly. Spacedrive's P2P system solves this.

### The Architecture

Spacebot always pairs with exactly one Spacedrive node. That node is Spacebot's home device inside the library. Every other Spacedrive device in the same library can reach Spacebot through that paired node using the existing P2P transport (Iroh/QUIC, hole-punching, local discovery).

```
Mobile phone (Spacedrive)
    → P2P connection to library
        → Paired Spacedrive node
            → Spacebot instance (localhost HTTP)
                → Agent runtime
```

The mobile app does not need to know Spacebot's HTTP address. It does not need a direct network path. It talks to Spacedrive, and Spacedrive routes the conversation to whichever node is paired with Spacebot. If the paired node is a server in the office, a NAS on the home network, or a hosted instance — the mobile device reaches it through the library graph.

This is important because it means one Spacebot instance serves the entire device fleet through Spacedrive's existing infrastructure. No separate mobile SDK, no separate authentication flow, no separate API surface.

### Chat on Mobile

The mobile app (`apps/mobile/`) is an Expo/React Native app with native tabs (Overview, Browse, Settings) and an embedded Spacedrive core communicating over a JSON-RPC transport (`SDMobileCore.sendMessage`). There is currently no Spacebot surface.

Adding chat to mobile means:

1. Spacedrive core gains a Spacebot proxy capability — it can forward webchat messages and SSE events between a local client and the paired Spacebot node over the P2P layer.
2. The mobile app adds a chat tab or modal that uses Spacedrive core queries and actions (not direct HTTP to Spacebot) to send messages and receive responses.
3. The proxy handles the transport. The mobile UI handles the conversation experience.

The mobile chat surface should be simpler than desktop. It is the continuation of the same relationship in a smaller form — check what the agent is working on, ask quick questions, review approvals, read documents produced during the day. The direction doc frames it as a first-class portal into the same living system, not a secondary companion.

**First mobile slice:**

- A chat screen accessible from the tab bar or a floating action button.
- Send messages to the current agent through Spacedrive core.
- Receive streaming responses proxied from Spacebot.
- View active tasks and pending approvals.
- Voice input using the device microphone.

**What mobile does not need initially:**

- Full worker transcript inspection (too dense for mobile).
- Memory management (desktop concern).
- Schedule management (desktop concern).
- Agent configuration (desktop/admin concern).

### Remote Execution Across Devices

The same P2P proxy that enables mobile chat also enables Spacebot to operate across the entire device fleet. This is described in detail in `spacedrive/docs/core/design/spacebot-remote-execution.md`.

The core idea: when Spacebot spawns a worker, that worker can target any device in the library. The worker's shell and file tools proxy through Spacedrive to the target device. The tool interface stays identical from the model's perspective — it still calls `shell` and `file_read` — but the execution happens on a different machine.

```
User asks Spacebot to work on a repo on the MacBook
    → Spacebot spawns worker with execution_target = MacBook
        → Worker calls shell tool
            → Proxy tool sends request to paired Spacedrive node
                → Paired node checks policy (principal + device + path + operation)
                    → Forwards typed execution request to MacBook's Spacedrive
                        → MacBook executes locally
                            → Result returns through the chain
```

Every operation passes through Spacedrive's permission system. The paired node resolves effective policy for the agent principal, target device, target path, and operation kind before forwarding anything. The target device can enforce a second check for defense in depth.

### What This Means for the Interface

The Spacebot surface in Spacedrive should eventually expose the device dimension:

- **Device awareness in workers** — when a worker targets a remote device, the InlineWorkerCard should show which device it is running on.
- **Device picker in conversation** — when the agent asks which machine to work on, the UI can present the library's device list with online/offline status instead of requiring the user to type a device name.
- **Device-scoped permissions in Autonomy** — the autonomy view should show which devices the agent can access, which paths are readable/writable, and which operations require confirmation, per device.
- **Mobile approvals** — when a worker on a remote device needs human confirmation for a destructive action, that approval request should be pushable to the mobile app. Pull out your phone, review the action, approve or deny, and the worker continues.

### The Spacedrive Proxy Layer

For all of this to work, Spacedrive core needs a proxy service that:

1. Accepts Spacebot webchat operations (send, history, events) as typed core actions and queries.
2. Routes them to the paired Spacebot node over the P2P transport.
3. Proxies SSE events back as Spacedrive core subscription events.
4. Handles connection lifecycle — what happens when the paired node goes offline, when the Spacebot instance restarts, when the library topology changes.

This proxy lives in `sd-core`, not in the interface layer. The interface consumes it through the same `useCoreQuery` / `useCoreAction` / `useLibraryQuery` hooks that power the rest of the mobile and desktop apps. The `@spacebot/api-client` package remains useful for desktop direct connections, but mobile and remote devices go through the core proxy.

The proxy also enables a clean answer to the connection model question on desktop: instead of three explicit modes (managed local, external local, remote), the desktop app could also route through the core proxy when Spacebot is paired at the library level. Direct HTTP remains available as an optimization for the co-located case.

---

## What This Is Not

- This is not a clone of the Spacebot standalone dashboard. That dashboard exposes runtime internals for developers. This interface exposes work for users.
- This is not a settings-first experience. Configuration lives in settings. The main surface is about work.
- This is not a separate app. It is a section of Spacedrive that feels native to the rest of the product — same design language, same color system, same interaction patterns.
- This is not the final architecture. The direction doc describes a future where tasks are org-scoped and agents are employees in a shared workspace. This interface should evolve toward that without requiring a ground-up rewrite.
