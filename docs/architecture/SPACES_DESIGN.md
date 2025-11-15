# Spaces & Composable Sidebar Design

**Status:** RFC / Design Phase
**Author:** @jamiepine
**Created:** 2025-01-11
**Updated:** 2025-01-11

---

## Overview

This document outlines the design for **Spaces** - an Arc-browser-inspired organizational system for Spacedrive's sidebar. Spaces allow users to create custom sidebar layouts with device-aware groups, sortable items, and context-based filtering.

### Goals

1. **Composable Sidebar** - Users can organize any SdPath into custom groups
2. **Device-Aware** - Automatically show devices with their volumes and locations as children
3. **Persistent State** - Sidebar configuration syncs across devices
4. **Drag-and-Drop** - Add any file to the sidebar by dragging it
5. **Spaces** - Multiple sidebar presets (like Arc browser) for different contexts

### Non-Goals

- Tabs/windows management (Arc-style)
- Workspace collaboration features
- Advanced filtering/smart collections (Phase 2)

---

## Mental Model

### What are Spaces?

**Spaces are organizational contexts for your files.** Each Space defines:
- Which sidebar groups to show (virtual or custom)
- Space-level pinned shortcuts (optional)
- How to organize devices/locations
- Visual identity (color, icon)

### Virtual vs Custom Groups

**Virtual Groups** (frontend fetches data):
- `GroupType::Locations` - Fetches all locations dynamically
- `GroupType::Tags` - Fetches all tags dynamically
- `GroupType::Device { device_id }` - Fetches that device's volumes/locations
- No SpaceItems stored - data comes from existing tables

**Custom Groups** (uses SpaceItems):
- `GroupType::Custom` - User-defined group with manual items
- `GroupType::QuickAccess` - Fixed items (Overview, Recents, Favorites)
- Items stored in database as SpaceItems

### Space-Level vs Group-Level Items

**Space-Level Items** (`group_id: None`):
- Pinned shortcuts at top of space
- Quick access to frequently used paths
- Not inside any collapsible group
- Example: "Current Project" folder pinned for quick access

**Group-Level Items** (`group_id: Some(uuid)`):
- Organized within collapsible sections
- Part of a logical grouping
- Example: Overview/Recents/Favorites in Quick Access group

### Arc Browser Inspiration

Arc's Spaces solve a key problem: **context switching overload**. Instead of one messy tab bar for "Work + Personal + Shopping + Research", Arc lets you create separate Spaces for each context.

**Mapping to Spacedrive:**
- Arc manages tabs (ephemeral) → Spacedrive manages files (persistent)
- Arc Spaces filter/organize tabs → Spacedrive Spaces filter/organize sidebar
- Arc switches contexts → Spacedrive switches file organization views

### Common Use Cases

**Device-Based Spaces:**
- "This Mac" - Only show local device
- "All Devices" - Aggregate view of all devices
- "NAS Server" - Focus on network storage

**Project-Based Spaces:**
- "Project Alpha" - Specific locations + tagged files
- "Personal Photos" - Media-focused view
- "Work Documents" - Professional files only

**Content-Based Spaces:**
- "Available Offline" - Only cached/local files
- "Cloud Storage" - Cloud providers only
- "Recent Projects" - Time-based filtering

---

## Data Model

### Core Types

```rust
// core/src/domain/space.rs

/// A Space defines a sidebar layout and filtering context
/// Note: No library_id - each library has its own database
pub struct Space {
    pub id: Uuid,
    pub name: String,
    pub icon: String,      // Phosphor icon name or emoji
    pub color: String,     // Hex color (#3B82F6)
    pub order: i32,        // For sorting in space switcher
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A SpaceGroup is a collapsible section in the sidebar
pub struct SpaceGroup {
    pub id: Uuid,
    pub space_id: Uuid,
    pub name: String,
    pub group_type: GroupType,
    pub is_collapsed: bool,
    pub order: i32,        // For sorting within space
    pub created_at: DateTime<Utc>,
}

/// Types of groups that can appear in a space
pub enum GroupType {
    /// Fixed quick navigation (Overview, Recents, Favorites)
    QuickAccess,

    /// Device with its volumes and locations as children
    Device { device_id: Uuid },

    /// All locations across all devices
    Locations,

    /// Tag collection
    Tags,

    /// Cloud storage providers
    Cloud,

    /// User-defined custom group
    Custom,
}

/// An item within a space (can be space-level or within a group)
pub struct SpaceItem {
    pub id: Uuid,
    pub space_id: Uuid,              // Always required
    pub group_id: Option<Uuid>,      // None = space-level item
    pub item_type: ItemType,
    pub order: i32,                  // For sorting within space or group
    pub created_at: DateTime<Utc>,
}

/// Types of items that can appear in a group
pub enum ItemType {
    /// Overview screen (fixed)
    Overview,

    /// Recent files (fixed)
    Recents,

    /// Favorited files (fixed)
    Favorites,

    /// Indexed location
    Location { location_id: Uuid },

    /// Storage volume (with locations as children)
    Volume { volume_id: Uuid },

    /// Tag filter
    Tag { tag_id: Uuid },

    /// Any arbitrary path (dragged from explorer)
    Path { sd_path: SdPath },
}
```

### Database Schema

```sql
-- core/prisma/schema.prisma

model Space {
  id         String   @id @default(uuid())
  library_id String
  name       String
  icon       String
  color      String
  order      Int
  created_at DateTime @default(now())
  updated_at DateTime @updatedAt

  library Library @relation(fields: [library_id], references: [id], onDelete: Cascade)
  groups  SpaceGroup[]

  @@index([library_id])
}

model SpaceGroup {
  id           String   @id @default(uuid())
  space_id     String
  name         String
  group_type   Json     // Serialized GroupType enum
  is_collapsed Boolean  @default(false)
  order        Int
  created_at   DateTime @default(now())

  space Space @relation(fields: [space_id], references: [id], onDelete: Cascade)
  items SpaceItem[]

  @@index([space_id])
}

model SpaceItem {
  id         String   @id @default(uuid())
  group_id   String
  item_type  Json     // Serialized ItemType enum
  order      Int
  created_at DateTime @default(now())

  group SpaceGroup @relation(fields: [group_id], references: [id], onDelete: Cascade)

  @@index([group_id])
}
```

### TypeScript Types (Auto-Generated)

```tsx
// packages/ts-client/src/generated/types.ts

type Space = {
  id: string;
  library_id: string;
  name: string;
  icon: string;
  color: string;
  order: number;
  created_at: string;
  updated_at: string;
};

type SpaceGroup = {
  id: string;
  space_id: string;
  name: string;
  group_type: GroupType;
  is_collapsed: boolean;
  order: number;
  created_at: string;
};

type GroupType =
  | "QuickAccess"
  | { Device: { device_id: string } }
  | "Locations"
  | "Tags"
  | "Cloud"
  | "Custom";

type SpaceItem = {
  id: string;
  group_id: string;
  item_type: ItemType;
  order: number;
  created_at: string;
};

type ItemType =
  | "Overview"
  | "Recents"
  | "Favorites"
  | { Location: { location_id: string } }
  | { Volume: { volume_id: string } }
  | { Tag: { tag_id: string } }
  | { Path: { sd_path: SdPath } };

type SpaceLayout = {
  space: Space;
  groups: Array<{
    group: SpaceGroup;
    items: SpaceItem[];
  }>;
};
```

---

## API Design

### Queries

```rust
// All spaces in current library
spaces.list() -> Vec<Space>

// Get specific space
spaces.get(space_id: Uuid) -> Space

// Get full sidebar layout for a space
spaces.get_layout(space_id: Uuid) -> SpaceLayout

// Get default space for library
spaces.get_default() -> Space
```

### Actions

```rust
// Space management
spaces.create(input: CreateSpaceInput) -> Space
spaces.update(space_id: Uuid, input: UpdateSpaceInput) -> Space
spaces.delete(space_id: Uuid) -> ()
spaces.reorder(space_ids: Vec<Uuid>) -> ()

// Group management
spaces.add_group(input: AddGroupInput) -> SpaceGroup
spaces.update_group(group_id: Uuid, input: UpdateGroupInput) -> SpaceGroup
spaces.delete_group(group_id: Uuid) -> ()
spaces.toggle_group(group_id: Uuid) -> SpaceGroup
spaces.reorder_groups(space_id: Uuid, group_ids: Vec<Uuid>) -> ()

// Item management
spaces.add_item(input: AddItemInput) -> SpaceItem
spaces.delete_item(item_id: Uuid) -> ()
spaces.reorder_items(group_id: Uuid, item_ids: Vec<Uuid>) -> ()
```

### Input Types

```rust
pub struct CreateSpaceInput {
    pub name: String,
    pub icon: String,
    pub color: String,
}

pub struct UpdateSpaceInput {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

pub struct AddGroupInput {
    pub space_id: Uuid,
    pub name: String,
    pub group_type: GroupType,
}

pub struct UpdateGroupInput {
    pub name: Option<String>,
    pub is_collapsed: Option<bool>,
}

pub struct AddItemInput {
    pub group_id: Uuid,
    pub item_type: ItemType,
}
```

---

## State Management

### Server State (Backend)

**Stored in Database:**
- Space definitions
- Group configurations
- Item lists and ordering
- Synced across all devices via library sync

**Real-Time Updates:**
```tsx
// Spaces list updates when created/deleted on another device
const spacesQuery = useNormalizedCache({
  wireMethod: "query:spaces.list",
  input: null,
  resourceType: "space",
  isGlobalList: true,
});

// Space layout updates when groups/items change
const layoutQuery = useNormalizedCache({
  wireMethod: "query:spaces.get_layout",
  input: { space_id: currentSpaceId },
  resourceType: "space_layout",
});
```

### Client State (Frontend)

**Persisted in localStorage:**
- Current active space ID
- User preferences

**Ephemeral (session-only):**
- Collapsed group states (per session)
- Drag-and-drop state
- UI interactions

### Zustand Store

```tsx
// packages/ts-client/src/stores/sidebar.ts
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SidebarStore {
  // Persisted state
  currentSpaceId: string | null;
  setCurrentSpace: (id: string | null) => void;

  // Ephemeral state
  collapsedGroups: Set<string>;
  toggleGroup: (groupId: string) => void;
  collapseAll: () => void;
  expandAll: () => void;

  // Drag state
  draggedItem: DraggedItem | null;
  setDraggedItem: (item: DraggedItem | null) => void;
}

type DraggedItem =
  | { type: 'file'; data: File }
  | { type: 'space-item'; data: SpaceItem }
  | { type: 'space-group'; data: SpaceGroup };

export const useSidebarStore = create<SidebarStore>()(
  persist(
    (set, get) => ({
      // Persisted
      currentSpaceId: null,
      setCurrentSpace: (id) => set({ currentSpaceId: id }),

      // Ephemeral
      collapsedGroups: new Set(),
      toggleGroup: (groupId) => set((state) => {
        const newSet = new Set(state.collapsedGroups);
        if (newSet.has(groupId)) {
          newSet.delete(groupId);
        } else {
          newSet.add(groupId);
        }
        return { collapsedGroups: newSet };
      }),
      collapseAll: () => set((state) => {
        // Get all group IDs from current layout
        const allGroupIds = /* ... */;
        return { collapsedGroups: new Set(allGroupIds) };
      }),
      expandAll: () => set({ collapsedGroups: new Set() }),

      // Drag
      draggedItem: null,
      setDraggedItem: (item) => set({ draggedItem: item }),
    }),
    {
      name: 'spacedrive-sidebar',
      partialize: (state) => ({
        currentSpaceId: state.currentSpaceId,
      }),
    }
  )
);
```

---

## Component Architecture

### Component Tree

```
SpacesSidebar/
├── index.tsx                    # Main sidebar container
├── SpaceSwitcher.tsx            # Dropdown for switching spaces
├── SpaceGroup.tsx               # Collapsible group container
├── SpaceItem.tsx                # Individual sidebar item
├── DeviceGroup.tsx              # Device with children
├── LocationsGroup.tsx           # Virtual locations group
├── TagsGroup.tsx                # Virtual tags group
├── AddGroupButton.tsx           # Plus button to add groups
├── AddGroupModal.tsx            # Modal for selecting group type
├── CreateSpaceModal.tsx         # Modal for creating new space
├── DragDropContext.tsx          # Drag-and-drop wrapper (Phase 5)
└── hooks/
    ├── useSpaces.ts             # Spaces queries with useNormalizedCache
    └── useSpaceDragDrop.ts      # Drag-and-drop logic (Phase 5)
```

### Main Sidebar Component

```tsx
// packages/interface/src/components/Sidebar/index.tsx
export function Sidebar() {
  const { currentSpaceId, setCurrentSpace } = useSidebarStore();
  const { data: spaces } = useSpaces();
  const { data: layout } = useSpaceLayout(currentSpaceId);
  const { data: devices } = useDevices();

  // Auto-select first space if none selected
  const currentSpace = spaces?.find(s => s.id === currentSpaceId) ?? spaces?.[0];

  useEffect(() => {
    if (currentSpace && currentSpace.id !== currentSpaceId) {
      setCurrentSpace(currentSpace.id);
    }
  }, [currentSpace, currentSpaceId]);

  return (
    <SpaceDragDropContext>
      <div className="flex h-full w-[220px] min-w-[176px] max-w-[300px] flex-col bg-sidebar/65 backdrop-blur">
        {/* Space Switcher */}
        <SpaceSwitcher
          spaces={spaces}
          currentSpace={currentSpace}
          onSwitch={setCurrentSpace}
        />

        {/* Scrollable Groups */}
        <div className="flex-1 space-y-4 overflow-y-auto px-2 py-4 no-scrollbar">
          {layout?.groups.map(({ group, items }) => (
            <SpaceGroup key={group.id} group={group} items={items} devices={devices} />
          ))}

          {/* Add Group Button */}
          <AddGroupButton spaceId={currentSpace?.id} />
        </div>

        {/* Settings (pinned to bottom) */}
        <div className="border-t border-sidebar-line p-2">
          <SpaceItem icon={Gear} label="Settings" onClick={() => navigate('/settings')} />
        </div>
      </div>
    </SpaceDragDropContext>
  );
}
```

### Space Switcher

```tsx
// packages/interface/src/components/Sidebar/SpaceSwitcher.tsx
interface SpaceSwitcherProps {
  spaces: Space[] | undefined;
  currentSpace: Space | undefined;
  onSwitch: (spaceId: string) => void;
}

export function SpaceSwitcher({ spaces, currentSpace, onSwitch }: SpaceSwitcherProps) {
  const [isCreating, setIsCreating] = useState(false);

  return (
    <>
      <DropdownMenu.Root
        trigger={
          <button className="mx-2 mt-2 flex w-auto items-center gap-2 rounded-lg bg-sidebar-box px-3 py-2 hover:bg-sidebar-selected">
            <div
              className="h-2 w-2 rounded-full"
              style={{ backgroundColor: currentSpace?.color }}
            />
            <span className="flex-1 truncate text-sm font-medium text-sidebar-ink">
              {currentSpace?.name || 'All Files'}
            </span>
            <CaretDown size={12} className="text-sidebar-ink-dull" />
          </button>
        }
        className="w-[200px]"
      >
        {spaces?.map(space => (
          <DropdownMenu.Item
            key={space.id}
            onClick={() => onSwitch(space.id)}
            className={cn(space.id === currentSpace?.id && "bg-sidebar-selected")}
          >
            <div className="flex items-center gap-2">
              <div
                className="h-2 w-2 rounded-full"
                style={{ backgroundColor: space.color }}
              />
              <span>{space.name}</span>
            </div>
          </DropdownMenu.Item>
        ))}

        <DropdownMenu.Separator />

        <DropdownMenu.Item onClick={() => setIsCreating(true)}>
          <Plus size={16} />
          <span>New Space</span>
        </DropdownMenu.Item>
      </DropdownMenu.Root>

      <CreateSpaceModal isOpen={isCreating} onClose={() => setIsCreating(false)} />
    </>
  );
}
```

### Space Group

```tsx
// packages/interface/src/components/Sidebar/SpaceGroup.tsx
interface SpaceGroupProps {
  group: SpaceGroup;
  items: SpaceItem[];
  devices: Map<string, LibraryDeviceInfo> | undefined;
}

export function SpaceGroup({ group, items, devices }: SpaceGroupProps) {
  const { collapsedGroups, toggleGroup } = useSidebarStore();
  const isCollapsed = collapsedGroups.has(group.id);

  // Render different layouts based on group type
  if ('Device' in group.group_type) {
    return (
      <DeviceGroup
        deviceId={group.group_type.Device.device_id}
        items={items}
        isCollapsed={isCollapsed}
        onToggle={() => toggleGroup(group.id)}
      />
    );
  }

  // Standard collapsible group
  return (
    <div>
      {/* Group Header */}
      <button
        onClick={() => toggleGroup(group.id)}
        className="mb-1 flex w-full items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
      >
        <CaretRight
          className={cn("transition-transform", !isCollapsed && "rotate-90")}
          size={10}
        />
        <span>{group.name}</span>
      </button>

      {/* Items */}
      {!isCollapsed && (
        <div className="space-y-0.5">
          {items.map(item => (
            <SpaceItem key={item.id} item={item} />
          ))}
        </div>
      )}
    </div>
  );
}
```

### Device Group

```tsx
// packages/interface/src/components/Sidebar/DeviceGroup.tsx
interface DeviceGroupProps {
  deviceId: string;
  items: SpaceItem[];
  isCollapsed: boolean;
  onToggle: () => void;
}

export function DeviceGroup({ deviceId, items, isCollapsed, onToggle }: DeviceGroupProps) {
  const { data: device } = useDevice(deviceId);
  const { data: volumes } = useVolumes(deviceId);
  const { data: locations } = useLocations();

  // Filter locations for this device
  const deviceLocations = locations?.filter(loc =>
    getDeviceFromSdPath(loc.sd_path) === deviceId
  );

  return (
    <div>
      {/* Device Header */}
      <button
        onClick={onToggle}
        className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm hover:bg-sidebar-selected/40"
      >
        <CaretRight
          className={cn("transition-transform", !isCollapsed && "rotate-90")}
          size={12}
        />
        <DeviceIcon os={device?.os} size={16} />
        <span className="flex-1 truncate text-sidebar-ink">{device?.name}</span>
        {device?.is_online && (
          <div className="h-1.5 w-1.5 rounded-full bg-green-400" />
        )}
      </button>

      {/* Children (Volumes & Locations) */}
      {!isCollapsed && (
        <div className="ml-4 mt-1 space-y-0.5">
          {/* Volumes */}
          {volumes?.map(volume => (
            <SpaceItem
              key={volume.id}
              icon={getVolumeIcon(volume.volume_type)}
              label={volume.name}
              onClick={() => navigate(`/volume/${volume.id}`)}
            />
          ))}

          {/* Locations on this device */}
          {deviceLocations?.map(location => (
            <SpaceItem
              key={location.id}
              icon={Folder}
              label={location.name || 'Unnamed'}
              onClick={() => navigate(`/location/${location.id}`)}
            />
          ))}

          {/* Custom items added via drag-and-drop */}
          {items.map(item => (
            <SpaceItem key={item.id} item={item} />
          ))}
        </div>
      )}
    </div>
  );
}
```

---

## Drag-and-Drop

### Library Choice

**@dnd-kit/core** - Modern, accessible, performant drag-and-drop for React

**Why not react-dnd?**
- @dnd-kit is more modern and performant
- Better TypeScript support
- Built-in accessibility
- Better touch support

### Implementation

```tsx
// packages/interface/src/components/Sidebar/DragDropContext.tsx
import { DndContext, DragOverlay, PointerSensor, useSensor, useSensors } from '@dnd-kit/core';

export function SpaceDragDropContext({ children }: PropsWithChildren) {
  const { handleDragEnd } = useSpaceDragDrop();
  const { draggedItem } = useSidebarStore();

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // 8px of movement before drag starts
      },
    })
  );

  return (
    <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
      {children}

      <DragOverlay>
        {draggedItem && (
          <div className="rounded-lg bg-sidebar-box px-2 py-1 shadow-lg">
            {renderDragPreview(draggedItem)}
          </div>
        )}
      </DragOverlay>
    </DndContext>
  );
}
```

```tsx
// packages/interface/src/components/Sidebar/hooks/useSpaceDragDrop.ts
import { useDndMonitor } from '@dnd-kit/core';

export function useSpaceDragDrop() {
  const addItem = useLibraryMutation('spaces.add_item');
  const reorderItems = useLibraryMutation('spaces.reorder_items');

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    if (!over) return;

    // Case 1: Dragging file from Explorer onto sidebar
    if (active.data.current?.type === 'file' && over.data.current?.type === 'space-group') {
      const file = active.data.current.file as File;
      const groupId = over.data.current.groupId as string;

      addItem.mutate({
        group_id: groupId,
        item_type: { Path: { sd_path: file.sd_path } },
      });
    }

    // Case 2: Reordering items within a group
    if (active.data.current?.type === 'space-item' && over.data.current?.type === 'space-item') {
      const activeItem = active.data.current.item as SpaceItem;
      const overItem = over.data.current.item as SpaceItem;

      if (activeItem.group_id === overItem.group_id) {
        // Reorder within same group
        // Calculate new order...
        reorderItems.mutate({ group_id: activeItem.group_id, item_ids: newOrder });
      }
    }

    // Case 3: Moving item to different group
    if (active.data.current?.type === 'space-item' && over.data.current?.type === 'space-group') {
      // Move item to new group...
    }
  };

  return { handleDragEnd };
}
```

### Draggable File Item (Explorer)

```tsx
// packages/interface/src/components/Explorer/FileItem.tsx
import { useDraggable } from '@dnd-kit/core';

export function FileItem({ file }: { file: File }) {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: file.id,
    data: {
      type: 'file',
      file,
    },
  });

  return (
    <div
      ref={setNodeRef}
      {...attributes}
      {...listeners}
      className={cn(
        "file-item",
        isDragging && "opacity-50"
      )}
    >
      {/* File content */}
    </div>
  );
}
```

### Droppable Space Group

```tsx
// packages/interface/src/components/Sidebar/SpaceGroup.tsx
import { useDroppable } from '@dnd-kit/core';

export function SpaceGroup({ group, items }: SpaceGroupProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: group.id,
    data: {
      type: 'space-group',
      groupId: group.id,
    },
  });

  return (
    <div
      ref={setNodeRef}
      className={cn(
        "space-group",
        isOver && "ring-2 ring-accent"
      )}
    >
      {/* Group content */}
    </div>
  );
}
```

---

## Default Space Creation

When a library is created, automatically create a default "All Devices" space:

```rust
// core/src/ops/libraries/create.rs

pub async fn create_library(ctx: &CoreContext, name: String) -> Result<Library> {
    // Create library...
    let library = Library::create(name).await?;

    // Create default space
    let default_space = Space::create(CreateSpaceInput {
        library_id: library.id,
        name: "All Devices".to_string(),
        icon: "Planet".to_string(),
        color: "#3B82F6".to_string(),
    }).await?;

    // Create default groups
    let quick_access = SpaceGroup::create(AddGroupInput {
        space_id: default_space.id,
        name: "Quick Access".to_string(),
        group_type: GroupType::QuickAccess,
    }).await?;

    // Add fixed items
    SpaceItem::create_batch(vec![
        AddItemInput {
            group_id: quick_access.id,
            item_type: ItemType::Overview,
        },
        AddItemInput {
            group_id: quick_access.id,
            item_type: ItemType::Recents,
        },
        AddItemInput {
            group_id: quick_access.id,
            item_type: ItemType::Favorites,
        },
    ]).await?;

    // Create device groups for each device
    for device in ctx.devices().await? {
        SpaceGroup::create(AddGroupInput {
            space_id: default_space.id,
            name: device.name.clone(),
            group_type: GroupType::Device { device_id: device.id },
        }).await?;
    }

    Ok(library)
}
```

---

## Device Discovery

When a new device joins the library, automatically add it to all spaces:

```rust
// core/src/ops/devices/pair.rs

pub async fn pair_device(ctx: &CoreContext, device: Device) -> Result<()> {
    // Pair device...

    // Add to all spaces
    let spaces = Space::list_all(ctx).await?;

    for space in spaces {
        SpaceGroup::create(AddGroupInput {
            space_id: space.id,
            name: device.name.clone(),
            group_type: GroupType::Device { device_id: device.id },
        }).await?;
    }

    Ok(())
}
```

---

## UI/UX Details

### Visual Design

**Space Switcher:**
- 2px colored dot indicator (space color)
- Space name in sidebar-ink
- CaretDown icon
- Hover: bg-sidebar-selected

**Space Groups:**
- Uppercase section title (sidebar-ink-faint)
- 10px caret for collapse indicator
- Smooth rotate-90 animation

**Device Groups:**
- Device icon (based on OS)
- Online indicator (green dot, 1.5px)
- Nested items with 16px left margin
- Shows volumes AND locations

**Drag Preview:**
- Follows cursor
- Shows item icon + name
- Shadow-lg
- Slightly transparent

### Animations

**Space Switch:**
```tsx
<motion.div
  key={currentSpace.id}
  initial={{ opacity: 0, x: -20 }}
  animate={{ opacity: 1, x: 0 }}
  exit={{ opacity: 0, x: 20 }}
  transition={{ duration: 0.15 }}
>
  {/* Sidebar content */}
</motion.div>
```

**Group Collapse:**
```tsx
<motion.div
  initial={false}
  animate={{ height: isCollapsed ? 0 : 'auto' }}
  transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
>
  {/* Group items */}
</motion.div>
```

**Drag Highlight:**
```tsx
className={cn(
  "space-group",
  isOver && "ring-2 ring-accent ring-opacity-50"
)}
```

### Keyboard Shortcuts

- `Cmd/Ctrl + 1-9` - Switch to space 1-9
- `Cmd/Ctrl + Shift + N` - New space
- `Cmd/Ctrl + Shift + E` - Expand all groups
- `Cmd/Ctrl + Shift + C` - Collapse all groups
- Arrow keys - Navigate sidebar items
- Enter - Activate selected item

---

## Edge Cases

### Device Offline

**Scenario:** Device goes offline while its group is visible

**Solution:**
- Show offline indicator (gray dot)
- Keep group visible but grayed out
- Show tooltip: "Device offline"
- Items still clickable (show offline message)

### Space Deleted on Another Device

**Scenario:** User deletes space on Device A, Device B has it as current space

**Solution:**
- useNormalizedCache detects space deletion
- Auto-switch to first available space
- Show toast: "Space 'Work Files' was deleted"

### Circular Nesting

**Scenario:** User tries to add a parent folder as a child of its subfolder

**Solution:** Not possible with current design - items are flat, groups don't nest

### Empty Space

**Scenario:** User deletes all groups from a space

**Solution:**
- Show empty state with "Add Group" button
- Suggest presets (devices, locations, tags)

### Duplicate Items

**Scenario:** User drags same file to sidebar twice

**Solution:**
- Backend checks for duplicate sd_path before creating
- Show toast: "This path is already in the sidebar"

---

## Performance Considerations

### Query Optimization

**Problem:** Loading all devices + volumes + locations on every space switch

**Solution:**
```tsx
// Prefetch device data once
const devicesQuery = useNormalizedCache({
  wireMethod: "query:devices.list",
  input: null,
  resourceType: "device",
  staleTime: 5 * 60 * 1000, // 5 minutes
});

// Lazy-load volumes when device group expands
const volumesQuery = useQuery({
  queryKey: ['volumes', deviceId],
  queryFn: () => client.execute('query:volumes.list', { device_id: deviceId }),
  enabled: !isCollapsed,
});
```

### Virtualization

**Not Needed Yet:**
- Most users have less than 10 devices
- Sidebar items are lightweight
- Only render visible groups (collapsed groups don't render children)

**Future:** If users have 50+ sidebar items, use `@tanstack/react-virtual`

### Real-Time Updates

**Efficient Event Handling:**
```tsx
// Only update if event affects current space
useEvent('ResourceChanged', (event) => {
  if (event.ResourceChanged.resource_type === 'space') {
    const space = event.ResourceChanged.resource;
    if (space.id === currentSpaceId) {
      queryClient.setQueryData(['space-layout', currentSpaceId], ...);
    }
  }
});
```

---

## Implementation Timeline

### Phase 1: Foundation (Week 1-2)
- [ ] Add Rust types (Space, SpaceGroup, SpaceItem)
- [ ] Database schema + migrations
- [ ] Backend CRUD operations
- [ ] Generate TypeScript types
- [ ] Add Zustand store with persistence

### Phase 2: Basic Spaces (Week 3-4)
- [ ] Create SpaceSwitcher component
- [ ] Add spaces CRUD UI
- [ ] Implement space activation
- [ ] Create default space on library creation
- [ ] Space creation modal

### Phase 3: Composable Groups (Week 5-6)
- [ ] Create SpaceGroup component
- [ ] Implement collapsible sections
- [ ] Add AddGroupModal
- [ ] Group CRUD operations
- [ ] Reordering logic

### Phase 4: Device-Aware Layout (Week 7-8)
- [ ] Create DeviceGroup component
- [ ] Show volumes under devices
- [ ] Show locations under devices
- [ ] Online/offline indicators
- [ ] Auto-add new devices to spaces

### Phase 5: Drag-and-Drop (Week 9-10)
- [ ] Install @dnd-kit/core
- [ ] Make file items draggable
- [ ] Make sidebar groups droppable
- [ ] Implement item reordering
- [ ] Visual feedback and animations

### Phase 6: Polish (Week 11-12)
- [ ] Keyboard shortcuts (Cmd+1-9, etc.)
- [ ] Framer Motion animations
- [ ] Context menus (right-click)
- [ ] Empty states
- [ ] Error states
- [ ] Performance optimization
- [ ] Documentation

---

## Open Questions

1. **Space Sync:** Should spaces be library-scoped (sync across devices) or device-local?
   - **Proposal:** Library-scoped by default, with option to create device-local spaces

2. **Smart Collections:** Should Phase 2 include dynamic filtering (tags, content types, date ranges)?
   - **Proposal:** Start simple, add smart collections in Phase 2 after user feedback

3. **Volume Navigation:** Should clicking a volume show its contents or its locations?
   - **Proposal:** Show volume contents (all files on that volume)

4. **Space Limits:** Should we limit the number of spaces per library?
   - **Proposal:** No hard limit, but warn if more than 20 spaces

5. **Default Space:** Can users delete the default space?
   - **Proposal:** Yes, but warn "Are you sure? This is your default space"

6. **Group Icons:** Should groups support custom icons?
   - **Proposal:** Not in Phase 1, add in Phase 2 if requested

---

## Success Metrics

**Adoption:**
- 70%+ of users create at least one custom space within first week
- 40%+ of users use 2+ spaces regularly

**Engagement:**
- Average 3-5 groups per space
- Average 5-10 items per group
- 30%+ of sidebar items are custom paths (dragged from explorer)

**Performance:**
- Space switch less than 100ms
- Sidebar render less than 50ms
- Drag-and-drop latency less than 16ms (1 frame)

**Feedback:**
- NPS score improvement after Spaces launch
- Positive feedback on device-aware organization
- Reduced "can't find my files" support requests

---

## Alternatives Considered

### 1. Tags Instead of Spaces

**Pros:** Simpler data model, users already understand tags

**Cons:** Tags are for files, not sidebar organization. Doesn't solve device-aware problem.

**Decision:** Spaces are orthogonal to tags. Tags filter files, Spaces organize navigation.

### 2. Saved Searches Instead of Spaces

**Pros:** More powerful filtering, dynamic results

**Cons:** Too complex for sidebar use case, not device-aware, requires query language.

**Decision:** Saved searches could be a future feature, but Spaces solve the organizational problem better.

### 3. Workspaces (like VSCode)

**Pros:** Familiar to developers, powerful

**Cons:** Too heavy for file management, doesn't match Spacedrive's cross-device model.

**Decision:** Workspaces are project-oriented, Spaces are context-oriented. Different mental models.

---

## References

- [Arc Browser Spaces Documentation](https://resources.arc.net/en/articles/6155972-spaces)
- [Spacedrive V1 Sidebar](https://github.com/spacedriveapp/spacedrive/tree/v1/packages/interface/src/components/sidebar)
- [@dnd-kit Documentation](https://docs.dndkit.com/)
- [Zustand Persist Middleware](https://github.com/pmndrs/zustand#persist-middleware)

---

## Changelog

- 2025-01-11: Initial design document
