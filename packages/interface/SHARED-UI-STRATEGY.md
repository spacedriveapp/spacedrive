# Shared UI Strategy: `spacedriveapp/spaceui`

## The Problem

Three codebases, two diverging UI stacks:

| Layer | Spacebot Portal (`spacebot/interface/`) | Spacedrive (`@sd/ui` + `@sd/interface`) |
|-------|----------------------------------------|----------------------------------------|
| **Primitives** | 27 components in `/src/ui/` (own Radix wrappers, CVA) | ~25 components in `@sd/ui` |
| **Forms** | 8 field wrappers (react-hook-form) | 8 field wrappers (react-hook-form) |
| **Composites** | 30 components in `/src/components/` | ~10 in `src/Spacebot/` |
| **Design tokens** | Own CSS vars (`app-line`, `ink`, `accent`, `sidebar-*`) | Same naming scheme, different impl |

The token names are nearly identical because spacebot's were derived from Spacedrive's. The primitives wrap the same Radix packages with the same patterns. Composite components like `ToolCall.tsx` are copy-pasted between repos. This will only get worse as the Spacedrive Spacebot surface fills out its Tasks, Memories, Schedule, and Autonomy routes — all of which the portal already has versions of.

---

## The Proposal: `spacedriveapp/spaceui`

A standalone repo — `spacedriveapp/spaceui` — that owns the entire shared design system. Both Spacedrive and the Spacebot portal become pure consumers. No UI primitives or shared composites live in either app repo.

### Why a Separate Repo

1. **Clean dependency direction.** Both apps depend on spaceui. Neither depends on the other. No circular references, no "which repo do I put this in" decisions.
2. **Independent release cycle.** UI changes can be versioned, published, and adopted at each app's own pace. A breaking change in a primitive doesn't force simultaneous deploys.
3. **Single design authority.** One place to review, approve, and document the design system. No drift between "the spacebot version" and "the spacedrive version."
4. **Contributor clarity.** A designer or frontend engineer working on shared components works in one repo, not scattered across three.
5. **Future consumers.** The marketing site, docs site, mobile app, or any other surface can import from spaceui without pulling in Spacedrive or Spacebot app code.

### Package Structure

The repo publishes multiple packages from a single monorepo. Domain-specific composites live in scoped packages under `packages/` — `ai/` for agent and AI interaction components, `explorer/` for file management components, with room for more as product surfaces grow.

```
spacedriveapp/spaceui/
├── packages/
│   ├── primitives/              # @spaceui/primitives
│   │   ├── src/
│   │   │   ├── Button.tsx
│   │   │   ├── Input.tsx
│   │   │   ├── Badge.tsx
│   │   │   ├── Card.tsx
│   │   │   ├── Dialog.tsx
│   │   │   ├── Dropdown.tsx
│   │   │   ├── DropdownMenu.tsx
│   │   │   ├── ContextMenu.tsx
│   │   │   ├── Popover.tsx
│   │   │   ├── Tooltip.tsx
│   │   │   ├── Tabs.tsx
│   │   │   ├── Select.tsx
│   │   │   ├── Checkbox.tsx
│   │   │   ├── RadioGroup.tsx
│   │   │   ├── Switch.tsx
│   │   │   ├── Slider.tsx
│   │   │   ├── SearchBar.tsx
│   │   │   ├── NumberStepper.tsx
│   │   │   ├── ProgressBar.tsx
│   │   │   ├── CircularProgress.tsx
│   │   │   ├── Loader.tsx
│   │   │   ├── Banner.tsx
│   │   │   ├── Toast.tsx
│   │   │   ├── Divider.tsx
│   │   │   ├── FilterButton.tsx
│   │   │   ├── ToggleGroup.tsx
│   │   │   ├── Resizable.tsx
│   │   │   ├── Typography.tsx
│   │   │   ├── Shortcut.tsx
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   ├── forms/                   # @spaceui/forms
│   │   ├── src/
│   │   │   ├── Form.tsx
│   │   │   ├── FormField.tsx
│   │   │   ├── InputField.tsx
│   │   │   ├── TextAreaField.tsx
│   │   │   ├── SelectField.tsx
│   │   │   ├── CheckboxField.tsx
│   │   │   ├── RadioGroupField.tsx
│   │   │   ├── SwitchField.tsx
│   │   │   └── index.ts
│   │   └── package.json         # peer deps: @spaceui/primitives, react-hook-form, zod
│   │
│   ├── ai/                      # @spaceui/ai
│   │   ├── src/
│   │   │   ├── ToolCall.tsx
│   │   │   ├── Markdown.tsx
│   │   │   ├── InlineWorkerCard.tsx
│   │   │   ├── ChatComposer.tsx
│   │   │   ├── TaskBoard.tsx
│   │   │   ├── TaskCard.tsx
│   │   │   ├── MemoryGraph.tsx
│   │   │   ├── MemoryList.tsx
│   │   │   ├── ModelSelect.tsx
│   │   │   ├── ProfileAvatar.tsx
│   │   │   ├── AgentSelector.tsx
│   │   │   ├── ConnectionStatus.tsx
│   │   │   ├── CronJobList.tsx
│   │   │   ├── AutonomyPanel.tsx
│   │   │   ├── types.ts
│   │   │   └── index.ts
│   │   └── package.json         # peer deps: @spaceui/primitives, @spacebot/api-client
│   │
│   ├── explorer/                # @spaceui/explorer
│   │   ├── src/
│   │   │   ├── FileGrid.tsx
│   │   │   ├── FileList.tsx
│   │   │   ├── FileRow.tsx
│   │   │   ├── FileThumb.tsx
│   │   │   ├── PathBar.tsx
│   │   │   ├── Inspector.tsx
│   │   │   ├── InspectorPanel.tsx
│   │   │   ├── TagPill.tsx
│   │   │   ├── KindIcon.tsx
│   │   │   ├── DragOverlay.tsx
│   │   │   ├── QuickPreview.tsx
│   │   │   ├── RenameInput.tsx
│   │   │   ├── types.ts
│   │   │   └── index.ts
│   │   └── package.json         # peer deps: @spaceui/primitives
│   │
│   └── tokens/                  # @spaceui/tokens
│       ├── src/
│       │   ├── colors.ts        # semantic color definitions
│       │   ├── tailwind-preset.ts
│       │   ├── css/
│       │   │   ├── base.css     # CSS custom properties
│       │   │   └── themes/
│       │   │       ├── dark.css
│       │   │       └── light.css
│       │   └── index.ts
│       └── package.json
│
├── turbo.json                   # or bun workspace config
├── tsconfig.base.json
├── tailwind.config.ts           # base config using @spaceui/tokens
└── package.json                 # workspace root
```

### Package Responsibilities

#### `@spaceui/tokens`

Design tokens and Tailwind preset. The foundation everything else builds on.

- Semantic color definitions (`ink`, `ink-dull`, `ink-faint`, `app`, `app-box`, `app-line`, `sidebar-*`, `accent`, `menu-*`)
- CSS custom properties (bare HSL values for Tailwind alpha support)
- Dark and light theme files
- Tailwind preset that both apps and all other packages use
- Spacing, border radius, typography scales if they diverge from Tailwind defaults

Both app `tailwind.config.ts` files use:
```ts
import { spaceUiPreset } from '@spaceui/tokens';
export default { presets: [spaceUiPreset], /* app-specific overrides */ };
```

#### `@spaceui/primitives`

All interactive building blocks. No business logic. No data fetching. No product-specific concepts.

**Migrated from current @sd/ui:**
- Button, Input, Checkbox, Switch, Slider, SearchBar
- Dropdown, DropdownMenu, ContextMenu
- Dialog, Popover, Tooltip, Tabs
- ProgressBar, CircularProgress, Loader
- RadioGroup, Select, Divider, Toast
- Resizable, Shortcut, Icon, TopBarButton, TopBarButtonGroup

**Migrated from spacebot `/src/ui/`:**
- Badge (6 color variants, 2 sizes)
- Card (composable: Header, Title, Description, Content, Footer)
- NumberStepper (inc/dec, min/max, float support, progress bar)
- Banner (5 variants with dot indicator — merges with current InfoBanner)
- FilterButton (small toggle with active state)
- ToggleGroup (radio-like visual toggle with options array)

**Merged (best of both):**
- Typography — unify Spacedrive's set with spacebot's 6 heading/body variants
- Shortcut/Kbd — evaluate and keep one

#### `@spaceui/forms`

Form field wrappers built on `react-hook-form` + `@spaceui/primitives`. Identical pattern in both apps today — Controller wrapper → Label → Primitive → ErrorMessage.

- Form, FormField (base wrapper)
- InputField, TextAreaField
- SelectField, CheckboxField, RadioGroupField, SwitchField
- Peer deps on `react-hook-form` and `zod`

#### `@spaceui/ai`

Assembled components for AI agent interaction. These understand agent concepts — tool calls, workers, transcripts, tasks, memories, conversations — but are not tied to any specific agent runtime. Tailored to Spacebot today, generic enough that the component vocabulary applies to any agent surface.

Used by: Spacedrive's embedded Spacebot surface, Spacebot portal, and any future AI-facing UI.

**Currently duplicated (move immediately):**

| Component | Status | What It Does |
|-----------|--------|-------------|
| **ToolCall** | Duplicated in both repos | Tool invocation display: name, args, result, status, shell output formatting, error detection. Includes `pairTranscriptSteps()` utility. |
| **Markdown** | Duplicated in both repos | Agent response renderer: react-markdown + remark-gfm + rehype-raw with semantic color styling for all HTML elements. |

**Currently in one repo (extract and share):**

| Component | Currently In | What It Does |
|-----------|-------------|-------------|
| **InlineWorkerCard** | Spacedrive | Collapsible card: task name, status, tool call count, live status. Expands to show full transcript with paired ToolCalls. Copy logs, cancel buttons. |
| **ChatComposer** | Spacedrive | Message input: project/model selectors, voice overlay trigger, animated expand on focus, send button. Portal has equivalent logic split across CortexChatPanel. |
| **ModelSelect** | Portal | LLM model picker: search/filter, grouped by provider, context window display, tool calling + reasoning badges, custom model ID input. |
| **ProfileAvatar** | Portal | Deterministic gradient avatar from seed. Image upload support, initials display, SVG-based. |
| **TaskBoard** | Portal | Kanban board: 5 columns (pending_approval, backlog, ready, in_progress, done). Drag-droppable cards via dnd-kit. |
| **TaskCard** | Portal (inside TaskBoard) | Task card: priority badge, assignee avatar, description, status color. |
| **MemoryGraph** | Portal | Sigma.js graph viz: force-atlas2 layout, color-coded nodes by memory type, relation edges, node detail inspection, hover/click interactions. |
| **AgentSelector** | Both (different impls) | Dropdown for switching agents. Both have the concept, different UI. Unify. |
| **ConnectionStatus** | Portal (ConnectionBanner) | Connection state indicator: connected, connecting, offline, error. VISION Phase 1 requires this in Spacedrive. |

**New components (build here first):**

| Component | VISION Phase | What It Does |
|-----------|-------------|-------------|
| **MemoryList** | Phase 3 | List view of memories with type filtering and search. Detail view with source attribution and graph edges. Delete/edit individual memories. |
| **CronJobList** | Phase 3 | Cron job list: name, schedule expression, last/next run time, status. Create, enable/disable, delete. Execution history per job. |
| **AutonomyPanel** | Phase 4 | Current autonomy level display. Toggle broad presets. Pending approval requests. |

**Shared types** (in `types.ts` or re-exported from `@spacebot/api-client`):

```typescript
// Tool execution
interface ToolCallPair {
  id: string;
  name: string;
  argsRaw: string;
  args: Record<string, unknown> | null;
  resultRaw: string | null;
  result: Record<string, unknown> | null;
  status: ToolCallStatus;
}
type ToolCallStatus = 'running' | 'completed' | 'error';

// Worker transcripts
type TranscriptStep = {
  type: 'action' | 'tool_result';
  call_id: string;
  name: string;
  content: Array<{ type: string; id: string; name: string; args: string }>;
  text: string;
};

// Domain objects
interface TaskInfo { id: string; title: string; status: string; priority: string; assignees: string[]; conversation_id?: string; }
interface MemoryInfo { id: string; type: string; content: string; source?: string; edges?: Array<{ target: string; relation: string }>; }
interface CronJobInfo { id: string; name: string; schedule: string; last_run?: string; next_run?: string; status: string; }
interface AgentInfo { id: string; name: string; detail: string; status?: string; }
interface ModelOption { id: string; name: string; provider: string; context_window?: number; capabilities?: string[]; }
```

#### `@spaceui/explorer`

Assembled components for file browsing and management. These understand file system concepts — paths, thumbnails, kinds, tags, metadata, preview — but are not tied to Spacedrive's specific backend. The Spacedrive app wires them to its core queries; any other file-browsing surface could use them with different data sources.

Used by: Spacedrive desktop and mobile, and potentially the Spacebot portal if it ever needs a file picker or artifact browser.

**Candidates to extract from Spacedrive's current Explorer:**

| Component | Currently In | What It Does |
|-----------|-------------|-------------|
| **FileGrid** | `@sd/interface` Explorer | Grid layout of file thumbnails with selection, drag, context menu. |
| **FileList** | `@sd/interface` Explorer | Table/list layout with sortable columns. |
| **FileRow** | `@sd/interface` Explorer | Single row in list view: icon, name, size, modified date, kind. |
| **FileThumb** | `@sd/interface` Explorer | Thumbnail renderer: images, video previews, kind icons, loading states. |
| **PathBar** | `@sd/interface` TopBar | Breadcrumb path navigation with clickable segments and dropdown overflow. |
| **Inspector** | `@sd/interface` | File metadata panel: EXIF, tags, notes, hash, location. |
| **InspectorPanel** | `@sd/interface` | Collapsible section within the inspector. |
| **TagPill** | `@sd/interface` | Colored pill for file tags with optional remove button. |
| **KindIcon** | `@sd/interface` | Icon mapped to file kind (document, image, video, audio, etc.). |
| **DragOverlay** | `@sd/interface` | Visual overlay during file drag with count badge and preview stack. |
| **QuickPreview** | `@sd/interface` | Spacebar-triggered preview modal for images, video, audio, text, PDF. |
| **RenameInput** | `@sd/interface` | Inline rename field with validation and extension awareness. |

**New components (build as Spacedrive Explorer matures):**

| Component | What It Does |
|-----------|-------------|
| **LocationCard** | Summary card for a storage location: name, path, free/used space, online status. |
| **StorageBar** | Visual bar showing space usage breakdown by kind (images, video, documents, etc.). |
| **JobProgress** | Active job display: indexing, thumbnailing, identifying — with progress bar and cancel. |

The explorer package starts smaller than ai — many of these components are deeply integrated into Spacedrive's Explorer context today. Extract incrementally as the interfaces stabilize, starting with the most self-contained pieces (TagPill, KindIcon, FileThumb, PathBar) and working toward the more stateful ones (FileGrid, QuickPreview).

---

## Dependency Strategy

### `@spaceui/primitives` Dependencies

Direct:
- `@radix-ui/*` (checkbox, dialog, dropdown-menu, popover, radio-group, select, slider, switch, tabs, tooltip)
- `class-variance-authority`
- `clsx`
- `framer-motion`
- `@phosphor-icons/react`

Peer:
- `react`, `react-dom`
- `tailwindcss` (build-time)

### `@spaceui/ai` Dependencies

Direct:
- `react-markdown`, `remark-gfm`, `rehype-raw` (for Markdown)

Peer:
- `@spaceui/primitives`
- `@spacebot/api-client` (for types)
- `@tanstack/react-query` (consumers provide)
- `@tanstack/react-virtual` (consumers provide)
- `react`, `react-dom`

Optional / lazy-loaded:
- `@react-sigma/core`, `sigma`, `graphology` (MemoryGraph — code-split)
- `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities` (TaskBoard — code-split)

### `@spaceui/explorer` Dependencies

Peer:
- `@spaceui/primitives`
- `@tanstack/react-virtual` (consumers provide)
- `react`, `react-dom`

Optional / lazy-loaded:
- Media preview libraries (video, PDF) for QuickPreview — code-split

### What Stays in App Repos

| Dependency | Stays In |
|-----------|----------|
| `@xyflow/react` | Portal (TopologyGraph) |
| `@codemirror/*` | Portal (OpenCodeEmbed) |
| `recharts` | Portal (charts) |
| `@lobehub/icons` | Portal (provider icons) |
| `@fortawesome/*` | Portal (legacy icons) |
| `sonner` | Both apps individually (toast wiring is app-specific) |

---

## What Stays in Each App

### Spacedrive `packages/interface/`

App-specific layout, routing, Spacedrive integration, and anything that calls `useCoreQuery` / `useLibraryQuery`:

**Spacebot surface (`src/Spacebot/`):**
- SpacebotLayout (sidebar + chrome)
- SpacebotContext / SpacebotProvider (state management)
- useSpacebotEventSource (SSE hook — will evolve to use core proxy)
- EmptyChatHero
- Route components: ChatRoute, ConversationRoute, TasksRoute, MemoriesRoute, AutonomyRoute, ScheduleRoute
- Router config

**Explorer surface (`src/components/Explorer/`, etc.):**
- Explorer context and state management
- Route components (locations, tags, spaces, overview)
- Data wiring (queries, mutations, subscriptions to sd-core)
- DnD coordination
- Selection management
- Platform-specific behavior (Tauri commands, native menus)

Spacedrive's `@sd/ui` package **goes away** — its contents migrate to `@spaceui/primitives`. During transition, `@sd/ui` becomes a thin re-export wrapper, then is removed.

### Spacebot Portal `spacebot/interface/src/`

App-specific chrome, admin features, and portal-only surfaces:

- Sidebar, TopBar, AgentTabs (portal navigation)
- CortexChatPanel (system-level LLM chat — portal only)
- WebChatPanel (channel chat display)
- ChannelCard, ChannelEditModal, ChannelSettingCard (channel management)
- CreateAgentDialog, DeleteAgentDialog (admin dialogs)
- TopologyGraph (portal-only viz)
- Orb (decorative)
- OpenCodeEmbed (IDE embed)
- All route/page components
- Portal-only hooks: useCortexChat, useWebChat, useChannelLiveState, useAgentOrder, useAudioRecorder, useTtsPlayback

The portal's `/src/ui/` directory **is deleted entirely** once migration is complete.

---

## Migration Plan

### Phase 0 — Bootstrap the Repo

1. Create `spacedriveapp/spaceui` repo with monorepo tooling (Turbo or Bun workspaces).
2. Set up `@spaceui/tokens` with the shared Tailwind preset, extracting color definitions from Spacedrive's current `styles.css` and tailwind config.
3. Set up `@spaceui/primitives`, `@spaceui/forms`, `@spaceui/ai`, `@spaceui/explorer` as empty packages with build config (tsup or unbuild, Tailwind, TypeScript).
4. Both app repos add spaceui as a workspace dependency (git submodule, npm link, or published packages — your call on linking strategy).

### Phase 1 — Stop the Bleeding (week 1)

Move the actively-duplicated components first so no more copy-paste happens:

1. Move `ToolCall.tsx` + `pairTranscriptSteps` + types → `@spaceui/ai`
2. Move `Markdown.tsx` → `@spaceui/ai`
3. Both apps update imports. Delete the duplicates.

### Phase 2 — Primitives Migration (weeks 2–3)

Move all primitives out of both repos into `@spaceui/primitives`:

1. Start with the intersection — components that exist in both @sd/ui and spacebot `/src/ui/` with identical APIs: Button, Input, Checkbox, Switch, Slider, Dialog, Tabs, Tooltip, Popover, Select, RadioGroup, Dropdown, ProgressBar, Loader, Divider.
2. For each: take the better implementation, move to spaceui, update imports in both apps.
3. Add spacebot-only primitives that are genuinely reusable: Badge, Card, NumberStepper, FilterButton, ToggleGroup.
4. Merge divergent implementations: Banner/InfoBanner, Typography, Shortcut/Kbd.
5. Move form fields → `@spaceui/forms`.
6. Spacedrive's `@sd/ui` becomes a thin `index.ts` that re-exports from `@spaceui/primitives` (backwards compat shim).

### Phase 3 — AI Composite Extraction (weeks 3–5)

Extract shared AI composites into `@spaceui/ai`:

1. `InlineWorkerCard` — extract from Spacedrive, parameterize the API calls (accept data via props, not internal fetching).
2. `ChatComposer` — extract from Spacedrive, make project/model selectors pluggable.
3. `ModelSelect` — extract from portal.
4. `ProfileAvatar` — extract from portal.
5. `AgentSelector` — unify both implementations.
6. `ConnectionStatus` — extract from portal's ConnectionBanner.

### Phase 4 — New Shared Components (aligned with VISION)

Build new components directly in `@spaceui/ai`:

1. `TaskBoard` + `TaskCard` — refactor from portal's version, serve VISION Phase 2.
2. `MemoryGraph` + `MemoryList` — refactor from portal's MemoryGraph, add list view, serve VISION Phase 3.
3. `CronJobList` — build for VISION Phase 3.
4. `AutonomyPanel` — build for VISION Phase 4.

### Phase 5 — Explorer Extraction (parallel track)

Extract file management components into `@spaceui/explorer` incrementally:

1. Start with self-contained pieces: TagPill, KindIcon, FileThumb, PathBar.
2. Then stateful but bounded: RenameInput, DragOverlay, InspectorPanel.
3. Then the larger views: FileGrid, FileList, Inspector, QuickPreview.
4. Each extraction follows the same principle: data via props, events via callbacks, no internal queries.

### Phase 6 — Cleanup

1. Delete spacebot portal's `/src/ui/` entirely.
2. Remove Spacedrive's `@sd/ui` re-export shim (update all imports to `@spaceui/primitives`).
3. Audit both apps for any remaining duplicated UI code.
4. Portal's `/src/components/` should only contain portal-specific components (channels, cortex, orchestration, admin).

---

## Linking Strategy Options

How both app repos consume spaceui:

| Approach | Pros | Cons |
|----------|------|------|
| **npm publish** | Clean versioning, standard consumption, works in CI | Publish step adds friction for rapid iteration |
| **Git submodule** | Always latest, no publish step | Submodule pain (everyone knows), version pinning is awkward |
| **Bun/npm workspace link** | Zero friction during dev, instant feedback | Only works locally, CI needs a different strategy |
| **Hybrid: workspace link for dev, publish for CI/prod** | Best of both | Slightly more config |

Recommendation: **Hybrid.** During active development, both repos use workspace linking to spaceui (or a shared parent workspace). For CI and production builds, spaceui publishes to a private npm registry (or GitHub Packages). This gives instant local iteration without publish friction, and reproducible CI builds with pinned versions.

---

## Component Design Principles for spaceui

### Primitives

- **No business logic.** A Button doesn't know about agents or files.
- **Styling via className + CVA variants.** Consumers can override or extend.
- **Semantic color classes only.** All components use `@spaceui/tokens` colors. No hardcoded hex, no `var()` references in className.
- **Radix for accessibility.** Every interactive primitive wraps a Radix component.
- **Composable over configurable.** Card has Card.Header, Card.Content, Card.Footer — not a single Card with 15 props.

### AI Composites

- **Data via props, not internal fetching.** Components accept typed data. The app decides where data comes from (direct HTTP, core proxy, mock). No `useQuery` inside shared components — the app wraps them.
- **Events via callbacks.** `onSend`, `onCancel`, `onApprove` — not internal mutations.
- **Layout-agnostic.** No assumptions about being full-width, in a sidebar, or in a split pane. Use flex/grid and let the container constrain.
- **Lazy-loadable.** Heavy components (MemoryGraph, TaskBoard) export lazy wrappers. The app decides when to load them.
- **Types co-located.** Each component exports its prop interface. Domain types live in `types.ts`.

### Explorer Composites

- **Same data-via-props principle.** FileGrid receives items and selection state; it doesn't query for them. The app owns the data layer.
- **Platform-agnostic rendering.** Components render to standard React DOM. Platform behaviors (native context menus, drag-to-Finder, Tauri commands) are injected by the app via callbacks or a platform adapter prop.
- **Virtual-scroll ready.** Grid and list components accept a virtualizer or integrate with `@tanstack/react-virtual` as a peer dep. No assumptions about total item count fitting in DOM.
- **Thumbnail contract.** FileThumb accepts a thumbnail URL or a kind identifier. How thumbnails are generated (Spacedrive's jobsystem, a CDN, a local path) is the app's concern.

### Tokens

- **CSS custom properties as bare HSL values** — `235, 15%, 7%` not `hsl(235, 15%, 7%)` — for Tailwind alpha support.
- **Semantic names only.** `ink`, `ink-dull`, `app-box`, not `gray-900`, `slate-400`.
- **Theme-switchable.** Dark and light themes swap the custom properties. Components don't know which theme is active.
- **Tailwind preset is the single integration point.** Apps import the preset; they don't duplicate color definitions.

---

## Audit: Full Component Inventory

### Primitives — Final Merged Set for `@spaceui/primitives`

| Component | Source | Notes |
|-----------|--------|-------|
| Button | Both | Take @sd/ui's, add spacebot's loading + icon props if missing |
| Input | Both | Merge — spacebot has SearchInput and PasswordInput variants |
| Checkbox | Both | Take @sd/ui's Radix wrapper |
| RadioGroup | Both | Take @sd/ui's |
| Switch | Both | Spacebot has 3 sizes (sm/md/lg), merge if @sd/ui doesn't |
| Slider | Both | Spacebot adds marks support — merge |
| Select | Both | Take @sd/ui's |
| Dropdown | Both | Take @sd/ui's |
| DropdownMenu | @sd/ui | Keep |
| ContextMenu | @sd/ui | Keep |
| Dialog | Both | Take @sd/ui's |
| Popover | Both | Take @sd/ui's (has usePopover hook) |
| Tooltip | Both | Take @sd/ui's |
| Tabs | Both | Take @sd/ui's |
| Badge | Spacebot | **New** — 6 color variants, 2 sizes |
| Card | Spacebot | **New** — composable (Header/Title/Description/Content/Footer) |
| Banner | Both | **Merge** — spacebot's 5 variants + @sd/ui's InfoBanner |
| NumberStepper | Spacebot | **New** — inc/dec with min/max/step/float |
| FilterButton | Spacebot | **New** — small toggle with active state |
| ToggleGroup | Spacebot | **New** — radio-like visual toggle |
| ProgressBar | Both | Merge — spacebot adds variants (success/warning/error) |
| CircularProgress | @sd/ui | Keep |
| Loader | Both | Take @sd/ui's |
| Toast | @sd/ui | Keep (sonner wiring stays in apps) |
| Divider | Both | Take @sd/ui's |
| Resizable | @sd/ui | Keep |
| SearchBar | @sd/ui | Keep |
| Typography | Both | **Merge** — unify heading/body variant sets |
| Shortcut | @sd/ui / Spacebot Kbd | **Evaluate** — keep one |
| TopBarButton | @sd/ui | Keep |
| TopBarButtonGroup | @sd/ui | Keep |
| Icon | @sd/ui | Keep |

### AI Composites — Full Set for `@spaceui/ai`

| Component | Source | Priority |
|-----------|--------|----------|
| ToolCall | Both (duplicated) | **Immediate** — stop the bleeding |
| Markdown | Both (duplicated) | **Immediate** — stop the bleeding |
| InlineWorkerCard | Spacedrive | Phase 3 |
| ChatComposer | Spacedrive | Phase 3 |
| TaskBoard | Portal | Phase 4 (VISION P2) |
| TaskCard | Portal | Phase 4 (VISION P2) |
| MemoryGraph | Portal | Phase 4 (VISION P3) |
| MemoryList | New | Phase 4 (VISION P3) |
| ModelSelect | Portal | Phase 3 |
| ProfileAvatar | Portal | Phase 3 |
| AgentSelector | Both | Phase 3 |
| ConnectionStatus | Portal | Phase 3 |
| CronJobList | New | Phase 4 (VISION P3) |
| AutonomyPanel | New | Phase 4 (VISION P4) |

### Explorer Composites — Initial Set for `@spaceui/explorer`

| Component | Source | Priority |
|-----------|--------|----------|
| TagPill | Spacedrive | Phase 5 (early) |
| KindIcon | Spacedrive | Phase 5 (early) |
| FileThumb | Spacedrive | Phase 5 (early) |
| PathBar | Spacedrive | Phase 5 (early) |
| RenameInput | Spacedrive | Phase 5 (mid) |
| DragOverlay | Spacedrive | Phase 5 (mid) |
| InspectorPanel | Spacedrive | Phase 5 (mid) |
| FileRow | Spacedrive | Phase 5 (late) |
| FileGrid | Spacedrive | Phase 5 (late) |
| FileList | Spacedrive | Phase 5 (late) |
| Inspector | Spacedrive | Phase 5 (late) |
| QuickPreview | Spacedrive | Phase 5 (late) |

### Portal-Only Components (stay in `spacebot/interface/`)

- CortexChatPanel
- WebChatPanel
- ChannelCard, ChannelEditModal, ChannelSettingCard
- CreateAgentDialog, DeleteAgentDialog
- TopologyGraph
- Orb
- OpenCodeEmbed
- ConnectionScreen, SetupBanner, UpdatePill
- LiveDuration
- ErrorBoundary

### Spacedrive-Only Components (stay in `spacedrive/packages/interface/`)

- SpacebotLayout, SpacebotContext, SpacebotProvider
- EmptyChatHero
- useSpacebotEventSource
- All Spacebot route components
- Explorer context, state management, data wiring
- All route components (overview, locations, tags, spaces, settings)
- Shell, ShellLayout, DndProvider
- Platform-specific code
