# Shared UI Strategy: Spacebot Portal ↔ Spacedrive Embedded

## The Problem

Two parallel UI stacks with growing overlap:

| Layer | Spacebot Portal (`spacebot/interface/`) | Spacedrive (`@sd/ui` + `@sd/interface`) |
|-------|----------------------------------------|----------------------------------------|
| **Primitives** | 27 components in `/src/ui/` (own Radix wrappers, CVA, etc.) | ~25 components in `@sd/ui` |
| **Forms** | 8 field wrappers with react-hook-form | 8 field wrappers with react-hook-form |
| **Composites** | 30 components in `/src/components/` | ~10 in `src/Spacebot/` |
| **Design tokens** | Own CSS vars (`app-line`, `ink`, `accent`, `sidebar-*`) | Same naming scheme, different impl |

The token names are nearly identical because spacebot's were derived from Spacedrive's. The primitives wrap the same Radix packages with the same patterns. And now composite components like `ToolCall.tsx` are copy-pasted between repos.

---

## The Proposal: Three Packages

### 1. `@sd/ui` — Primitives (already exists, expand it)

Every Radix wrapper, base input, layout primitive, and form field that both surfaces need. Single source of truth for interactive building blocks.

**Already in @sd/ui and shared:**

- Button, Input, Checkbox, Switch, Slider, SearchBar
- Dropdown, DropdownMenu, ContextMenu
- Dialog, Popover, Tooltip, Tabs
- ProgressBar, CircularProgress, Loader
- RadioGroup, Select, Divider, Toast
- All form field wrappers

**Migrate from spacebot `/src/ui/` into @sd/ui:**

| Component | Spacebot Has | @sd/ui Has | Action |
|-----------|-------------|-----------|--------|
| Badge | 6 color variants, 2 sizes | No | **Add to @sd/ui** |
| Banner | 5 variants, dot indicator | InfoBanner (simpler) | **Merge** — upgrade InfoBanner to spacebot's richer API |
| Card | Composable: Header/Title/Content/Footer | No | **Add to @sd/ui** |
| NumberStepper | Inc/dec, min/max, float, progress | No | **Add to @sd/ui** |
| FilterButton | Small filter toggle with active state | No | **Add to @sd/ui** |
| ToggleGroup | Radio-like visual toggle | No | **Add to @sd/ui** |
| Typography | 6 heading/body variants | Different set | **Merge** — unify variants |
| Kbd | Keyboard shortcut pill | Shortcut (similar) | **Evaluate** — may be same thing |

**Primitives where spacebot adopts @sd/ui's version:**

All remaining: Button, Input, Checkbox, RadioGroup, Toggle/Switch, Slider, Select, Dropdown, Dialog, Tabs, Tooltip, Popover, ProgressBar, Loader, Divider, and all form fields. The spacebot portal replaces its `/src/ui/` with imports from `@sd/ui`.

### 2. `@sd/spacebot-ui` — Shared Spacebot Composites (new package)

Assembled components specific to the Spacebot product domain but needed by both the portal and the embedded surface. These are not generic primitives — they understand Spacebot concepts (tool calls, workers, transcripts, tasks, memories, markdown).

**Components to extract here:**

| Component | Currently In | Notes |
|-----------|-------------|-------|
| **ToolCall** | Both (duplicated) | Identical logic. Display of tool name, args, result, status. Move here immediately. |
| **InlineWorkerCard** | Spacedrive `src/Spacebot/` | Worker status card with expandable transcript. Portal has similar in CortexChatPanel. Extract shared version. |
| **Markdown** | Both (duplicated) | Agent markdown renderer with semantic styling. Nearly identical. Move here. |
| **TaskBoard** | Spacebot portal | Kanban board with 5 columns. Spacedrive needs this for the Tasks route (VISION Phase 2). Build once here. |
| **TaskCard** | Spacebot portal (inside TaskBoard) | Individual task card with priority, assignee, status. Extract. |
| **MemoryGraph** | Spacebot portal | Sigma.js graph viz. Spacedrive needs this for the Memories route (VISION Phase 3). Build once here. |
| **MemoryList** | Doesn't exist yet | VISION calls for list view of memories with search. Build here. |
| **ModelSelect** | Spacebot portal | LLM model picker with provider grouping, search, capability badges. Both surfaces need it. |
| **ProfileAvatar** | Spacebot portal | Deterministic gradient avatar. Useful in both. |
| **ChatComposer** | Spacedrive `src/Spacebot/` | Message input with project/model selectors. Portal has equivalent in CortexChatPanel. Unify. |
| **ConnectionStatus** | Spacebot portal (ConnectionBanner) | Connection state indicator. VISION Phase 1 calls for this in Spacedrive. Build once here. |
| **CronJobList** | Doesn't exist yet | VISION Phase 3 — schedule display. Build here. |
| **AutonomyPanel** | Doesn't exist yet | VISION Phase 4 — permission display/toggles. Build here. |
| **AgentSelector** | Both (different impls) | Dropdown for switching agents. Unify. |

**What does NOT go here** (stays in respective apps):

- Layout shells (SpacebotLayout, Sidebar, TopBar) — app-specific chrome
- Route components — app-specific routing
- CortexChatPanel — portal-only system chat
- ChannelCard/ChannelEditModal — portal-only channel management
- CreateAgentDialog/DeleteAgentDialog — portal-only admin
- TopologyGraph — portal-only viz
- Orb — portal-only decorative
- OpenCodeEmbed — portal-only embed

### 3. `@spacebot/api-client` — API Client (already exists, keep it)

Already shared between both surfaces. New API methods (tasks, memories, cron, autonomy) get added here as the VISION phases roll out. No change to ownership.

---

## Shared Types

Co-locate in `@spacebot/api-client` or create `@sd/spacebot-types`:

```typescript
// Already duplicated — need single source:
interface ToolCallPair { id, name, argsRaw, args, resultRaw, result, status }
type ToolCallStatus = 'running' | 'completed' | 'error'
type TranscriptStep = { type, call_id, name, content, text }

// Types needed for new shared components:
interface Task { id, title, status, priority, assignees, conversation_id }
interface Memory { id, type, content, source, edges }
interface CronJob { id, name, schedule, last_run, next_run, status }
interface AgentInfo { id, name, detail, status }
interface ModelOption { id, name, provider, context_window, capabilities }
```

---

## Design Token Alignment

Both surfaces use the same semantic names (`ink`, `ink-dull`, `ink-faint`, `app`, `app-box`, `app-line`, `sidebar-*`, `accent`). The spacebot portal derived these from Spacedrive.

**Action:** Extract Spacedrive's color token config into a shared Tailwind preset in `@sd/ui`. The portal imports it rather than maintaining its own copy.

- Extract `@sd/ui/tailwind-preset.ts` with the shared color/token config
- Spacebot portal's `tailwind.config.ts` uses `presets: [require('@sd/ui/tailwind-preset')]`
- Both apps get identical token resolution

---

## Dependency Alignment

For `@sd/spacebot-ui`, these are the dependencies to consider:

| Dependency | Used By | In Shared Pkg? |
|-----------|---------|----------------|
| `react-markdown` + `remark-gfm` + `rehype-raw` | Both (Markdown) | Yes |
| `@phosphor-icons/react` | Both | Yes |
| `framer-motion` | Both | Yes |
| `clsx` | Both | Yes |
| `@tanstack/react-query` | Both | Peer dep |
| `@tanstack/react-virtual` | Both | Peer dep |
| `@react-sigma/core` + `sigma` + `graphology` | Portal only (MemoryGraph) | Yes — lazy loaded |
| `@dnd-kit/*` | Portal only (TaskBoard) | Yes — lazy loaded |
| `recharts` | Portal only | No — stays in portal |
| `@xyflow/react` | Portal only | No — stays in portal |
| `@codemirror/*` | Portal only | No — stays in portal |

Heavy viz deps (sigma, dnd-kit) should be lazy-loaded from `@sd/spacebot-ui` so they don't bloat the Spacedrive bundle when those routes aren't active.

---

## Implementation Order

### Phase 0 — Foundation

1. Extract Tailwind preset from `@sd/ui` for shared tokens
2. Spacebot portal adopts the preset
3. Create `@sd/spacebot-ui` package with build config

### Phase 1 — Stop the Bleeding

1. Move `ToolCall` + `pairTranscriptSteps` + types into `@sd/spacebot-ui`
2. Move `Markdown` renderer into `@sd/spacebot-ui`
3. Move `ToolCallPair`/`ToolCallStatus`/`TranscriptStep` types into shared types
4. Both apps import from `@sd/spacebot-ui` — delete duplicates

### Phase 2 — Primitive Convergence

1. Add Badge, Card, NumberStepper to `@sd/ui`
2. Merge Banner/InfoBanner
3. Spacebot portal starts replacing its `/src/ui/` imports with `@sd/ui`
4. Track remaining portal-only primitives — either move them or accept they're portal-specific

### Phase 3 — Composite Extraction (aligned with VISION phases)

1. Extract `InlineWorkerCard` into `@sd/spacebot-ui`
2. Extract `ChatComposer` into `@sd/spacebot-ui`
3. Extract `ModelSelect`, `ProfileAvatar`, `AgentSelector`
4. Build `TaskBoard`/`TaskCard` in `@sd/spacebot-ui` (serves VISION Phase 2)
5. Build `MemoryGraph`/`MemoryList` in `@sd/spacebot-ui` (serves VISION Phase 3)
6. Build `CronJobList`, `ConnectionStatus`, `AutonomyPanel` (serves VISION Phases 3–4)

### Phase 4 — Portal Migration

1. Spacebot portal replaces all `/src/ui/` with `@sd/ui`
2. Portal replaces duplicated composites with `@sd/spacebot-ui`
3. Delete `/src/ui/` directory from portal
4. Portal's component directory shrinks to portal-only features (channels, orchestration, cortex, admin)

---

## Package Structure

```
spacedrive/packages/
├── ui/                          # @sd/ui — primitives
│   ├── src/
│   │   ├── Button.tsx
│   │   ├── Badge.tsx            ← new
│   │   ├── Card.tsx             ← new
│   │   ├── ...
│   │   └── tailwind-preset.ts   ← new (shared tokens)
│   └── package.json
│
├── spacebot-ui/                 # @sd/spacebot-ui — shared composites (new)
│   ├── src/
│   │   ├── ToolCall.tsx
│   │   ├── Markdown.tsx
│   │   ├── InlineWorkerCard.tsx
│   │   ├── ChatComposer.tsx
│   │   ├── TaskBoard.tsx
│   │   ├── MemoryGraph.tsx
│   │   ├── ModelSelect.tsx
│   │   ├── ProfileAvatar.tsx
│   │   ├── AgentSelector.tsx
│   │   ├── ConnectionStatus.tsx
│   │   ├── types.ts
│   │   └── index.ts
│   └── package.json
│
└── interface/
    └── src/Spacebot/            # App-specific layout, routes, context
```

---

## What Stays Where

### Spacedrive `src/Spacebot/` (app-specific)

- SpacebotLayout, SpacebotContext, SpacebotProvider
- Route components (Chat, Tasks, Memories, Autonomy, Schedule)
- useSpacebotEventSource (Spacedrive-specific SSE with core proxy path)
- EmptyChatHero
- Router config

### Spacebot Portal `src/` (app-specific)

- Sidebar, TopBar, AgentTabs (portal chrome)
- CortexChatPanel (system-level chat — portal only)
- ChannelCard, ChannelEditModal, ChannelSettingCard (channel management)
- CreateAgentDialog, DeleteAgentDialog (admin)
- TopologyGraph, Orb (portal-only viz)
- OpenCodeEmbed (portal-only embed)
- All route/page components
- Hooks: useCortexChat, useWebChat, useChannelLiveState, useAgentOrder, useAudioRecorder, useTtsPlayback

---

## Why Build in Spacedrive First

1. **`@sd/ui` already exists** with a mature primitive set and conventions. Extending it is natural.
2. **The embedded surface is the primary consumer** going forward — per VISION, "the place a person opens in the morning."
3. **The portal is the admin/developer surface.** It should consume shared components, not define them.
4. **Monorepo build system.** Building composites in `@sd/spacebot-ui` inside the Spacedrive monorepo means they inherit the Spacedrive build system, linting, and type checking.
5. **The portal becomes a consumer** that installs or workspace-links these packages.

The exception: components that only the portal needs (channels, orchestration, cortex) still get built in the portal. But anything that touches the core Spacebot experience — chat, tools, workers, tasks, memories — gets built in Spacedrive packages.
