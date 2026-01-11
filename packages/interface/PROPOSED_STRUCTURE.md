# Proposed Interface Structure

## Goals
1. Clear separation between routes, components, and windows
2. Related files colocated
3. Intuitive naming and hierarchy
4. No confusion about where files belong

## Proposed Structure

```
src/
├── Shell.tsx                       # App entry point
├── ShellLayout.tsx                 # Layout chrome
├── router.tsx                      # Route configuration
├── index.tsx                       # Public exports
├── styles.css                      # Global styles
│
├── contexts/                       # React contexts
│   ├── SpacedriveContext.tsx      # Main client context (rename from context.tsx)
│   ├── ServerContext.tsx
│   └── PlatformContext.tsx        # (rename from platform.tsx)
│
├── routes/                         # Route components (what renders in <Outlet />)
│   ├── overview/
│   │   ├── index.tsx
│   │   ├── OverviewTopBar.tsx
│   │   └── ...
│   ├── explorer/                  # Move from components/Explorer/
│   │   ├── ExplorerView.tsx       # Main view
│   │   ├── context.tsx            # Explorer state
│   │   ├── views/                 # Grid, List, Column, etc.
│   │   ├── components/            # ExplorerView-specific
│   │   └── hooks/
│   ├── tag/
│   ├── file-kinds/
│   ├── settings/                  # Move from Settings/
│   └── daemon/                    # Rename DaemonManager
│
├── components/                     # Reusable feature components
│   ├── DndProvider.tsx
│   ├── ErrorBoundary.tsx          # Move from root
│   │
│   ├── Inspector/                 # Consolidate all inspector code
│   │   ├── Inspector.tsx          # Main container (move from root)
│   │   ├── variants/              # Inspector implementations
│   │   │   ├── FileInspector.tsx
│   │   │   ├── LocationInspector.tsx
│   │   │   ├── MultiFileInspector.tsx
│   │   │   └── KnowledgeInspector.tsx
│   │   └── primitives/            # UI components (current Inspector/ folder)
│   │       ├── Tabs.tsx
│   │       ├── Section.tsx
│   │       ├── InfoRow.tsx
│   │       └── ...
│   │
│   ├── SpacesSidebar/
│   ├── QuickPreview/
│   ├── JobManager/
│   ├── SyncMonitor/
│   ├── TabManager/
│   ├── Tags/
│   │
│   ├── modals/                    # Consolidate modals
│   │   ├── CreateLibraryModal.tsx
│   │   ├── PairingModal.tsx
│   │   ├── SyncSetupModal.tsx
│   │   └── FileOperationModal.tsx
│   │
│   └── overlays/                  # Overlays
│       ├── DaemonDisconnectedOverlay.tsx
│       └── DaemonStartupOverlay.tsx
│
├── windows/                        # Special purpose windows
│   ├── FloatingControls.tsx
│   ├── DemoWindow.tsx
│   └── Spacedrop.tsx
│
├── demo/                           # Demo/testing components
│   ├── LocationCacheDemo.tsx
│   └── SpacedropDemo.tsx
│
├── hooks/                          # Global hooks
│   ├── useKeybind.ts
│   ├── useContextMenu.ts
│   ├── useClipboard.ts
│   └── ...
│
├── util/                           # Utilities
│   └── keybinds/
│
└── TopBar/                         # TopBar portal system
    ├── TopBar.tsx
    ├── Context.tsx
    └── Portal.tsx
```

## Migration Priority

### Phase 1: Quick Wins (Low Risk)
1. ✅ Move `ErrorBoundary.tsx` to `components/`
2. ✅ Create `windows/` and move demo/special windows
3. ✅ Create `demo/` and move demo components
4. ✅ Create `components/modals/` and move modals
5. ✅ Create `components/overlays/` and move overlays

### Phase 2: Consolidations (Medium Risk)
1. ✅ Consolidate Inspector structure
2. ✅ Rename context files and move to `contexts/`
3. ✅ Move Settings to routes

### Phase 3: Major Restructure (Higher Risk - needs testing)
1. ✅ Move ExplorerView to routes
2. ✅ Update all imports

## Completion Status

**All phases complete!** The refactor has been fully implemented:

- All files moved to their target locations
- All imports updated across the codebase
- TypeScript validation passing (no interface-related errors)
- Public exports updated in `index.tsx` and `components/index.ts`

## Benefits Achieved

1. **Clarity**: Clear where each type of file belongs
2. **Scalability**: Easy to add new routes, components, windows
3. **Maintainability**: Related code colocated
4. **Onboarding**: New developers can understand structure quickly
5. **IDE Navigation**: Better autocomplete and file search

## Resolved Decisions

1. ✅ `ExplorerView` moved to `routes/explorer/` (primary location is routes, can be re-exported from components if needed)
2. ✅ `platform.tsx` → `contexts/PlatformContext.tsx`
3. ✅ Modals and overlays use dedicated subfolders (`components/modals/`, `components/overlays/`)