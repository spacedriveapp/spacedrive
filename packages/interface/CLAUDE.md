# Spacedrive Interface Development Rules

**Status:** Living Document - Update as architectural decisions are made
**Purpose:** Ensure consistent, clean, and maintainable code across the interface package
**Audience:** AI assistants and developers working on @sd/interface

---

## Core Principles

1. **Platform Agnostic** - This package works on Tauri, Web, and React Native
2. **Clean Separation** - UI components here, state in @sd/ts-client, primitives in @sd/ui
3. **Type Safety First** - Use auto-generated types, no `any`, strict TypeScript
4. **Performance Matters** - Virtual scrolling, code splitting, memoization when needed
5. **Accessible** - Radix primitives, proper ARIA labels, keyboard navigation
6. **Consistent Styling** - Semantic color system, no arbitrary values

---

## Package Architecture

### What Lives Where

**@sd/interface** (this package):
- Route components and layouts
- Feature components (Explorer, Settings, etc.)
- React Query hook wrappers
- UI composition and interactivity
- NO state management (use @sd/ts-client)
- NO primitive components (use @sd/ui)
- NO platform APIs (use platform prop)

**@sd/ts-client**:
- Client implementation
- Transport layer
- Auto-generated types from Rust
- State stores (if needed)

**@sd/ui**:
- Primitive components (Button, Input, DropdownMenu, etc.)
- Reusable, unstyled or minimally styled
- No business logic
- No API calls

### Current Structure

```
packages/interface/
├── src/
│   ├── Shell.tsx                 # App entry point (providers, daemon check)
│   ├── ShellLayout.tsx           # Layout shell (sidebar, inspector, TopBar)
│   ├── router.tsx                # Route configuration
│   ├── DemoWindow.tsx            # Demo/testing window
│   ├── FloatingControls.tsx      # Floating controls UI
│   ├── components/
│   │   ├── DndProvider.tsx       # Drag-and-drop coordinator
│   │   ├── Explorer/
│   │   │   ├── ExplorerView.tsx  # File browser view
│   │   │   ├── context.tsx       # Explorer state/context
│   │   │   └── ...
│   │   ├── QuickPreview/
│   │   │   ├── Controller.tsx    # Preview navigation
│   │   │   ├── Syncer.tsx        # Selection sync
│   │   │   └── ...
│   │   └── ...
│   ├── TopBar/                   # TopBar portal system
│   │   ├── TopBar.tsx
│   │   ├── Context.tsx
│   │   └── Portal.tsx
│   ├── routes/                   # Route components
│   │   └── overview/
│   ├── hooks/                    # React hooks
│   ├── context.tsx               # Client context and hooks
│   ├── styles.css                # Global CSS variables
│   └── index.tsx                 # Public exports
```

### Architecture Layers

The interface is organized into clear separation of concerns:

**Shell Layer** (`Shell.tsx`):
- Root entry point
- Provider setup (Spacedrive, Server, TabManager, Platform)
- Daemon connection management (Tauri-specific)

**Layout Layer** (`ShellLayout.tsx`):
- Chrome/frame (sidebar, inspector, TopBar containers)
- Provider setup (TopBar, Selection, Explorer)
- Tab bar positioning
- QuickPreview coordination

**View Layer** (routes like `ExplorerView.tsx`):
- Actual content rendering
- TopBar button registration (via portal)
- Feature-specific logic

**Coordination Layer**:
- `DndProvider.tsx` - Global drag-and-drop
- `QuickPreview/Controller.tsx` - Preview navigation
- `QuickPreview/Syncer.tsx` - Selection-to-preview sync

**Visual Hierarchy:**
```
Shell (providers) → DndProvider → Router
                                    ↓
                            ShellLayout (chrome)
                                    ↓
                              <Outlet> (routes)
                                    ↓
                  Overview | ExplorerView | Settings | etc.
```

---

## Code Style Rules

### React 19 Standards

### Critical: You Might Not Need an Effect

**Effects are an escape hatch** - only use them to sync with external systems (network, DOM, browser APIs).

**DON'T use Effects for:**
- Transforming data for rendering (calculate during render instead)
- Handling user events (use event handlers)
- Updating state based on props (calculate during render or use `key`)
- Chains of state updates (do in event handler)
- Initializing app (use module-level code)
- Notifying parent of changes (pass callback, call in event handler)

**DO use Effects for:**
- Subscribing to external systems (WebSocket, browser events)
- Syncing with non-React widgets
- Network requests with proper cleanup

### Examples

**Wrong - Don't use Effect to transform data:**
```tsx
function TodoList({ todos, filter }) {
  const [visibleTodos, setVisibleTodos] = useState([]);
  useEffect(() => {
    setVisibleTodos(getFilteredTodos(todos, filter));
  }, [todos, filter]);
  // Extra render pass!
}
```

**Correct - Calculate during render:**
```tsx
function TodoList({ todos, filter }) {
  const visibleTodos = getFilteredTodos(todos, filter);
  // Or use useMemo if expensive:
  const visibleTodos = useMemo(
    () => getFilteredTodos(todos, filter),
    [todos, filter]
  );
}
```

**Wrong - Don't use Effect for user events:**
```tsx
function ProductPage({ product, addToCart }) {
  useEffect(() => {
    if (product.isInCart) {
      showNotification('Added to cart!');
    }
  }, [product]);
}
```

**Correct - Use event handler:**
```tsx
function ProductPage({ product, addToCart }) {
  function buyProduct() {
    addToCart(product);
    showNotification('Added to cart!');
  }
}
```

**Wrong - Don't use Effect to update parent:**
```tsx
function Toggle({ onChange }) {
  const [isOn, setIsOn] = useState(false);
  useEffect(() => {
    onChange(isOn); // Too late! Extra render.
  }, [isOn, onChange]);
}
```

**Correct - Call in event handler:**
```tsx
function Toggle({ onChange }) {
  const [isOn, setIsOn] = useState(false);
  function updateToggle(nextIsOn) {
    setIsOn(nextIsOn);
    onChange(nextIsOn); // Same render pass!
  }
}
```

**Wrong - Don't chain Effects:**
```tsx
useEffect(() => {
  if (card.gold) setGoldCardCount(c => c + 1);
}, [card]);

useEffect(() => {
  if (goldCardCount > 3) setRound(r => r + 1);
}, [goldCardCount]);
// Multiple render passes!
```

**Correct - Calculate in event handler:**
```tsx
function handlePlaceCard(nextCard) {
  setCard(nextCard);
  if (nextCard.gold) {
    if (goldCardCount < 3) {
      setGoldCardCount(goldCardCount + 1);
    } else {
      setGoldCardCount(0);
      setRound(round + 1);
    }
  }
  // Single render pass!
}
```

### Function components only:
```tsx
// Correct
function Component({ name }: { name: string }) {
  return <div>{name}</div>;
}

// Wrong
const Component: React.FC<{ name: string }> = ({ name }) => {
  return <div>{name}</div>;
};
```

**Hooks must follow rules:**
```tsx
// Correct - proper cleanup
useEffect(() => {
  const subscription = subscribe();
  return () => subscription.unsubscribe();
}, [dependency]);

// Wrong - missing cleanup
useEffect(() => {
  subscribe();
}, []);
```

**Use TypeScript strictly:**
```tsx
// Correct - explicit types
interface ButtonProps {
  label: string;
  onClick: () => void;
}

function Button({ label, onClick }: ButtonProps) { }

// Wrong - implicit any
function Button(props) { }
```

---

## Color System Rules

### CRITICAL: Always Use Semantic Tailwind Classes

Never use `var()` syntax directly. Always use Tailwind's semantic color classes.

**WRONG:**
```tsx
className="bg-[var(--color-sidebar)]"
className="text-[var(--color-sidebar-ink)]"
className="border-[var(--color-accent)]"
```

**CORRECT:**
```tsx
className="bg-sidebar"
className="text-sidebar-ink"
className="border-accent"
```

**IMPORTANT:** CSS variables must be defined as comma-separated HSL values (not wrapped in `hsl()`):
```css
/* CORRECT - bare values for Tailwind */
--color-sidebar: 235, 15%, 7%;

/* WRONG - wrapped in hsl() */
--color-sidebar: hsl(235, 15%, 7%);
```

This is because Tailwind uses `hsla(var(--color-sidebar), <alpha-value>)` which becomes `hsla(235, 15%, 7%, 0.5)` for opacity support.

### Color Categories

**Accent:** `accent`, `accent-faint`, `accent-deep`
- Use for: Primary actions, selections, focus states

**Text (Ink):** `ink`, `ink-dull`, `ink-faint`
- Use for: Text hierarchy (primary, secondary, tertiary)

**Sidebar:** `sidebar`, `sidebar-box`, `sidebar-line`, `sidebar-ink`, `sidebar-selected`, etc.
- Use for: Sidebar-specific elements

**App:** `app`, `app-box`, `app-line`, `app-hover`, `app-selected`, etc.
- Use for: Main content area elements

**Menu:** `menu`, `menu-line`, `menu-hover`, `menu-ink`, etc.
- Use for: Dropdowns, context menus

### Opacity Modifiers

```tsx
// Use Tailwind opacity
className="bg-accent/10"
className="bg-sidebar/65"

// Don't use manual alpha
className="bg-[var(--color-accent)]/10"
```

---

## Component Rules

### Primitive vs Feature Components

**Primitives** (@sd/ui):
- Generic, reusable
- Minimal styling (or unstyled)
- No business logic
- Example: `DropdownMenu`, `Button`, `Input`

**Feature Components** (@sd/interface):
- Specific to Spacedrive features
- Uses primitives
- Can have business logic
- Example: `Explorer`, `Sidebar`, `LibrariesDropdown`

### Component Structure

```tsx
// Correct structure
import { Primitive } from '@sd/ui';
import { useSomeQuery } from '../context';

interface ComponentProps {
  // Props interface
}

function Component({ prop }: ComponentProps) {
  // Hooks first
  const data = useSomeQuery();

  // Logic
  const derived = useMemo(() => transform(data), [data]);

  // Render
  return (
    <Primitive className="semantic-colors">
      {/* Content */}
    </Primitive>
  );
}

export { Component };
```

### Naming Conventions

- **Files:** `PascalCase.tsx` for components, `camelCase.ts` for utilities
- **Components:** `PascalCase` functions
- **Hooks:** `useCamelCase` pattern
- **Constants:** `SCREAMING_SNAKE_CASE`
- **CSS classes:** Semantic names only (`bg-sidebar`, not `bg-gray-900`)

---

## Styling Rules

### CRITICAL: Never Use Style Tags

**NEVER** use `<style>`, `<style jsx>`, or any inline style tags. Always use Tailwind utility classes.

**WRONG:**
```tsx
<style jsx>{`
  .slider::-webkit-slider-thumb {
    background: var(--color-accent);
  }
`}</style>
```

**CORRECT:**
```tsx
className="[&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:rounded-full"
```

Use Tailwind's arbitrary variant syntax for pseudo-elements and other edge cases.

### Tailwind Class Order

Follow this order for readability:
1. Layout (`flex`, `grid`, `w-full`, `h-screen`)
2. Spacing (`p-4`, `m-2`, `gap-2`)
3. Typography (`text-sm`, `font-medium`)
4. Colors (`bg-sidebar`, `text-ink`)
5. Borders (`border`, `border-sidebar-line`, `rounded-lg`)
6. Effects (`shadow-sm`, `backdrop-blur`)
7. States (`hover:bg-app-hover`, `focus:ring-accent`)
8. Transitions (`transition-colors`)

### Rounding (V2 Style)

V2 is more rounded than V1. Use:
- `rounded-lg` for most containers (8px)
- `rounded-md` for smaller elements (6px)
- `rounded-full` for pills/badges
- `rounded-[10px]` for window frame

### Animation

Use framer-motion for complex animations:
```tsx
import { motion, AnimatePresence } from 'framer-motion';

<AnimatePresence>
  {isOpen && (
    <motion.div
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: 'auto', opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
    >
      {content}
    </motion.div>
  )}
</AnimatePresence>
```

---

## Data Fetching Rules

### Use Type-Safe Hooks

**Core queries** (no library required):
```tsx
import { useCoreQuery } from '../context';

const { data: libraries } = useCoreQuery({
  type: 'libraries.list',
  input: { include_stats: false },
});
```

**Library queries** (requires library context):
```tsx
import { useLibraryQuery } from '../context';

const { data: files } = useLibraryQuery({
  type: 'files.directory_listing',
  input: { path: '/' },
});
```

**Mutations:**
```tsx
import { useCoreMutation, useLibraryMutation } from '../context';

const createLib = useCoreMutation('libraries.create');
const applyTags = useLibraryMutation('tags.apply');
const copyFiles = useLibraryMutation('files.copy');
const deleteFiles = useLibraryMutation('files.delete');

// Use mutations, not client.execute()
createLib.mutate({ name: 'New Library', path: null });
await copyFiles.mutateAsync({
  sources: { paths: [path1, path2] },
  destination: destPath,
  overwrite: false,
  verify_checksum: false,
  preserve_timestamps: true,
  move_files: false,
  copy_method: "Auto"
});
```

### Never Fetch Manually

**Wrong:**
```tsx
const [data, setData] = useState();
useEffect(() => {
  fetchData().then(setData);
}, []);
```

**Correct:**
```tsx
const { data } = useCoreQuery({ type: 'operation', input: {} });
```

---

## Performance Rules

### Virtual Scrolling

Use for lists > 100 items:
```tsx
import { useVirtualizer } from '@tanstack/react-virtual';

const virtualizer = useVirtualizer({
  count: items.length,
  getScrollElement: () => parentRef.current,
  estimateSize: () => 50,
});
```

### Code Splitting

Lazy load routes:
```tsx
const SettingsPage = lazy(() => import('./Settings'));

<Suspense fallback={<Spinner />}>
  <SettingsPage />
</Suspense>
```

### Memoization

Only when actually needed:
```tsx
// Expensive computation
const sorted = useMemo(
  () => items.sort(expensiveCompare),
  [items]
);

// Premature optimization
const greeting = useMemo(() => `Hello ${name}`, [name]);
```

---

## Component Composition Rules

### Dropdown Example (Current Implementation)

The `DropdownMenu` primitive provides minimal base functionality. Explorer customizes it:

```tsx
// Primitive (in @sd/ui/DropdownMenu.tsx)
export const DropdownMenu = {
  Root: ({ trigger, children, className }) => (
    // Minimal expanding container with motion
  ),
  Item: ({ children, onClick, className }) => (
    // Basic button with flex layout
  ),
  Separator: ({ className }) => (
    // Simple divider
  ),
};

// Usage (in Explorer.tsx)
<DropdownMenu.Root
  trigger={
    <button className="w-full bg-sidebar-box border-sidebar-line rounded-lg">
      {currentLibrary?.name}
    </button>
  }
  className="bg-sidebar-box border-sidebar-line rounded-lg"
>
  <DropdownMenu.Item
    className="px-2 py-1 rounded-md hover:bg-sidebar-selected"
    onClick={() => switchLibrary(lib.id)}
  >
    {lib.name}
  </DropdownMenu.Item>
</DropdownMenu.Root>
```

**Key principles:**
1. Primitive has minimal/no styling
2. All visual styling applied via className prop
3. Business logic (filtering, selecting) in parent component
4. Semantic color classes only

---

## Type Safety Rules

### Use Generated Types

All types are auto-generated from Rust:
```tsx
import type { LibraryInfo, CoreQuery, LibraryAction } from '@sd/ts-client';
```

**Never:**
- Define manual type interfaces that duplicate Rust types
- Use `any` (use `unknown` with type guards if needed)
- Ignore TypeScript errors

### Query Type Safety

The hooks automatically infer types:
```tsx
// TypeScript knows data is LibraryInfo[]
const { data } = useCoreQuery({
  type: 'libraries.list',
  input: { include_stats: false },
});

// data is automatically typed based on the operation!
```

---

## File Organization Rules

### Component Co-location

```
Explorer/
├── index.tsx           # Main component
├── Sidebar.tsx         # Sub-component
├── TopBar.tsx          # Sub-component
└── hooks/
    └── useExplorer.ts  # Feature-specific hooks
```

### Exports

Only export what's needed:
```tsx
// index.tsx
export { Shell } from './Shell';
export { DemoWindow } from './DemoWindow';
// Don't export everything
```

---

## macOS-Specific Rules

### Native Traffic Lights

The window uses **native** macOS traffic lights positioned by Swift code:
- Traffic lights are real, functional native controls
- Content must have `pt-[52px]` to avoid overlap
- No fake CSS traffic lights
- Transparent titlebar + invisible toolbar trick (see sd-desktop-macos crate)

### Window Styling

```tsx
// Correct - accounts for native traffic lights
<nav className="pt-[52px] ...">
  {/* Content starts below traffic lights */}
</nav>

// Correct - window frame with rounded corners
<div className="rounded-[10px] border-transparent frame">
  {/* App content */}
</div>
```

### Blur Effects

Use backdrop blur for macOS native feel:
```tsx
className="backdrop-blur-lg bg-sidebar/65"
```

---

## Current Architectural Decisions

### 1. Expanding Dropdowns (Not Overlays)

Decision: Dropdowns should expand inline and push content down, not overlay it.

Implementation:
- Use `framer-motion` for smooth height animation
- No Radix Portal (renders inline in DOM)
- Pushes surrounding content naturally

### 2. Library Switcher Logic

Decision: Show/hide current library based on count.

Rules:
- **1 library:** Hide current from dropdown (no point showing it)
- **2+ libraries:** Show all including current (with highlight)
- Always show "New Library" and "Library Settings"

### 3. Color System

Decision: Use Tailwind semantic classes, never `var()` directly.

```tsx
// Correct
className="bg-sidebar-box text-sidebar-ink border-sidebar-line"

// Wrong
className="bg-[var(--color-sidebar-box)]"
```

### 4. Rounded Style (V2)

Decision: V2 is more rounded than V1.

- Containers: `rounded-lg` (8px)
- Small elements: `rounded-md` (6px)
- Window: `rounded-[10px]`
- Pills/badges: `rounded-full`

---

## Development Workflow

### Before Writing Code

1. Check if primitive exists in @sd/ui
2. Check if types are auto-generated (they probably are)
3. Plan component composition (primitive + styling)
4. Use semantic color classes

### When Adding Features

1. Create minimal primitive in @sd/ui if needed
2. Use primitive in @sd/interface with styling
3. Use type-safe queries/mutations
4. Add to this document if architectural decision made

### When Styling

1. Use semantic colors (`bg-sidebar`, not `bg-gray-900`)
2. Follow V2 rounded style
3. Use opacity modifiers (`bg-accent/10`)
4. Maintain color context (sidebar colors in sidebar, app colors in main area)

---

## Common Patterns

### Shell Entry Point Pattern

The app entry point follows a clean provider hierarchy:

```tsx
// Shell.tsx
export function Shell({ client }: { client: SpacedriveClient }) {
  const platform = usePlatform();

  return (
    <SpacedriveProvider client={client}>
      <ServerProvider>
        <TabManagerProvider routes={explorerRoutes}>
          <TabKeyboardHandler />
          <DndProvider>
            <RouterProvider router={router} />
          </DndProvider>
        </TabManagerProvider>
      </ServerProvider>
    </SpacedriveProvider>
  );
}

// ShellLayout renders inside router, provides layout chrome
// Routes (ExplorerView, Overview, etc.) render inside <Outlet />
```

### TopBar Portal Pattern

Views register their TopBar buttons via portal:

```tsx
// ExplorerView.tsx
import { TopBarPortal } from '../../TopBar';

function ExplorerView() {
  return (
    <>
      <TopBarPortal
        left={<BackButton />}
        center={<PathBar />}
        right={<ViewControls />}
      />
      <div>{/* View content */}</div>
    </>
  );
}
```

### Library Switcher Pattern

```tsx
const client = useSpacedriveClient();
const { data: libraries } = useLibraries();
const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(null);

// Auto-select first library
useEffect(() => {
  if (libraries && libraries.length > 0 && !currentLibraryId) {
    client.setCurrentLibrary(libraries[0].id);
    setCurrentLibraryId(libraries[0].id);
  }
}, [libraries, currentLibraryId, client]);

// Switch library
const handleSwitch = (id: string) => {
  client.setCurrentLibrary(id);
  setCurrentLibraryId(id);
};
```

### Sidebar Item Pattern

```tsx
function SidebarItem({ icon: Icon, label, active }: Props) {
  return (
    <button
      className={clsx(
        "flex items-center gap-2 px-2 py-1 rounded-md text-sm font-medium",
        active
          ? "bg-sidebar-selected text-sidebar-ink"
          : "text-sidebar-inkDull hover:text-sidebar-ink"
      )}
    >
      <Icon className="size-4" weight={active ? "fill" : "bold"} />
      <span className="truncate">{label}</span>
    </button>
  );
}
```

### Dropdown Pattern

```tsx
<DropdownMenu.Root
  trigger={<button className="...">Trigger</button>}
  className="bg-sidebar-box border-sidebar-line rounded-lg"
>
  <DropdownMenu.Item
    className="px-2 py-1 hover:bg-sidebar-selected"
    onClick={() => action()}
  >
    Item content
  </DropdownMenu.Item>
  <DropdownMenu.Separator className="border-sidebar-line" />
</DropdownMenu.Root>
```

### Context Menu Pattern

Use `useContextMenu` hook for platform-agnostic context menus:

```tsx
import { useContextMenu } from '../hooks/useContextMenu';
import { Copy, Trash } from '@phosphor-icons/react';

const { selectedFiles } = useExplorer();
const copyFiles = useLibraryMutation('files.copy');
const deleteFiles = useLibraryMutation('files.delete');

const contextMenu = useContextMenu({
  items: [
    {
      icon: Copy,
      label: selectedFiles.length > 1 ? `Copy ${selectedFiles.length} items` : "Copy",
      onClick: async () => {
        await copyFiles.mutateAsync({
          sources: { paths: selectedFiles.map(f => f.sd_path) },
          destination: currentPath,
          overwrite: false,
          verify_checksum: false,
          preserve_timestamps: true,
          move_files: false,
          copy_method: "Auto"
        });
      },
      keybind: "⌘C",
      condition: () => selectedFiles.length > 0, // Only show if files selected
    },
    { type: "separator" },
    {
      icon: Trash,
      label: "Delete",
      onClick: async () => {
        await deleteFiles.mutateAsync({
          targets: { paths: selectedFiles.map(f => f.sd_path) },
          permanent: false,
          recursive: true
        });
      },
      keybind: "⌘⌫",
      variant: "danger"
    }
  ]
});

return <div onContextMenu={contextMenu.show}>Content</div>;
```

**Key features:**
- Platform-agnostic (native on Tauri, Radix on web)
- Conditional items via `condition` callback
- Smart labels that update based on state
- Supports icons, keybinds, variants, submenus, separators
- Use `useLibraryMutation` for actions, not `client.execute()`

---

## Type-Safe Query Pattern

### Query Keys

Use descriptive, hierarchical keys:
```tsx
// Good
queryKey: ['libraries', 'list']
queryKey: ['files', 'directory', libraryId, path]

// Bad
queryKey: ['getLibraries']
queryKey: ['data']
```

### Using Queries

```tsx
const { data, isLoading, error } = useCoreQuery({
  type: 'libraries.list',
  input: { include_stats: true },
});

// data is automatically typed as LibraryInfo[]!
```

---

## Testing Requirements

### Critical Paths Must Be Tested

- Explorer file operations
- Library switching
- Settings mutations
- Search functionality

### Test Pattern

```tsx
import { render, screen } from '@testing-library/react';
import { Shell } from './Shell';

test('switches libraries', async () => {
  const user = userEvent.setup();
  render(<Shell client={mockClient} />);

  await user.click(screen.getByText('Switch Library'));
  // ...
});
```

---

## Migration from V1

When porting V1 components:

1. **Update colors:** `bg-gray-900` → `bg-app`, `text-gray-400` → `text-ink-dull`
2. **Use primitives:** Extract reusable parts to @sd/ui
3. **Remove state:** Move to @sd/ts-client if global, use local state if component-specific
4. **Update queries:** Use new type-safe hooks
5. **Add rounding:** V1 used `rounded-md`, V2 uses `rounded-lg`

---

## Checklist Before PR

- [ ] All colors use semantic classes (no `var()` directly)
- [ ] Component uses primitives from @sd/ui where applicable
- [ ] Type-safe queries/mutations (no manual fetch)
- [ ] Follows V2 rounded style
- [ ] No `any` types
- [ ] Proper cleanup in useEffect
- [ ] Accessible (keyboard nav, ARIA labels)
- [ ] Tested critical paths

---

## Quick Reference

### Import Order

```tsx
// 1. External libraries
import { useState } from 'react';
import { motion } from 'framer-motion';

// 2. @sd packages
import { Button, DropdownMenu } from '@sd/ui';
import { useCoreQuery } from '@sd/ts-client';

// 3. Local imports
import { useLibraries } from './hooks/useLibraries';
import clsx from 'clsx';
```

### Common Mistakes

`<style>` or `<style jsx>` tags → Use Tailwind arbitrary variants
`className="bg-[var(--color-sidebar)]"` → `className="bg-sidebar"`
`bg-gray-900` → `bg-app`
`rounded-md` everywhere → `rounded-lg` for V2
Manual fetch → Use type-safe hooks
State in component → Use @sd/ts-client or local state

---

## Questions to Ask

Before writing code:

1. **Is this a primitive?** → Should it be in @sd/ui?
2. **Is this state global?** → Should it be in @sd/ts-client?
3. **Are the types auto-generated?** → Don't duplicate them!
4. **Can I use a semantic color?** → Yes, always!
5. **Is this accessible?** → Keyboard nav? ARIA labels?

---

## Resources

- **Type Generation:** `cargo run --bin generate_typescript_types`
- **Color System:** `/docs/react/ui/colors.mdx`
- **Workbench Docs:** `/workbench/interface/`
- **V1 Reference:** `/Users/jamespine/Projects/spacedrive_v1`

---

## Status: Current Implementation

**Complete:**
- Type-safe client with auto-generated types
- Native macOS traffic lights
- V1 color system as CSS variables
- Expanding dropdown (DropdownMenu primitive)
- Explorer with sidebar and library switcher
- TanStack Query integration
- Clean architecture refactor (Shell → ShellLayout → Views)
- Extracted DndProvider for drag-and-drop coordination
- QuickPreview components (Controller + Syncer)
- TopBar portal system for view-specific controls

**In Progress:**
- Port remaining V1 components
- Build complete Explorer (file grid/list views)
- Settings pages
- Multi-window system

---

**Remember:** This is a living document. Update it when architectural decisions are made. This is our rulebook for building a world-class file manager interface!