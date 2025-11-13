# Spacedrive Event System - Code Map

Complete reference of all relevant files with line numbers and key sections.

---

## ts-client Package

### `/packages/ts-client/src/client.ts`
**SpacedriveClient with Event System**

```
Lines 8-39:     SimpleEventEmitter class implementation
                - on(event, listener)
                - off(event, listener)
                - emit(event, ...args)
                - once(event, listener)

Lines 50-226:   SpacedriveClient class
                extends SimpleEventEmitter

Lines 200-211:  subscribe() method
                - Calls transport.subscribe()
                - Emits 'spacedrive-event' for each event
                - Returns unsubscribe function
```

**Key Code:**
```typescript
async subscribe(callback?: (event: Event) => void): Promise<() => void> {
  const unlisten = await this.transport.subscribe((event) => {
    this.emit("spacedrive-event", event);
    if (callback) {
      callback(event);
    }
  });
  return unlisten;
}
```

---

### `/packages/ts-client/src/event-filter.ts`
**Event Type Definitions and Filters**

```
Lines 1-25:     Type definitions
                - ExtractEventVariant<T>
                - EventVariant type

Lines 34-85:    DEFAULT_EVENT_SUBSCRIPTION
                - LocationAdded (line 79)
                - LocationRemoved (line 80)
                - ResourceChanged (line 75)
                - ResourceChangedBatch (line 76)
                - ResourceDeleted (line 77)
                - ... and many others

Lines 90-93:    NOISY_EVENTS
                - LogMessage
                - IndexingProgress
```

**Important:**
LocationAdded is in DEFAULT_EVENT_SUBSCRIPTION, so it will be subscribed automatically.

---

### `/packages/ts-client/src/generated/types.ts`
**Auto-Generated Type Definitions**

```
Line 424:       Event union type definition
                Spans multiple lines - contains all event types including:
                - LocationAdded: { library_id, location_id, path }
                - LocationRemoved: { library_id, location_id }
                - ResourceChanged: { resource_type, resource, metadata }
                - ResourceChangedBatch: { resource_type, resources, metadata }
                - ResourceDeleted: { resource_type, resource_id }

Lines 1-423:    Type definitions for all resources
                - LocationInfo (location with fields)
                - LocationAddInput (for creating locations)
                - And hundreds of other types
```

---

### `/packages/ts-client/src/hooks/useNormalizedCache.ts`
**Auto-Updating Cache from Events**

```
Lines 1-51:     deepMerge() function
                - Merges incoming resource with cached resource
                - Respects no_merge_fields metadata

Lines 57-75:    resourceMatches() function
                - Matches resources by ID or alternate IDs

Lines 77-90:    UseNormalizedCacheOptions interface

Lines 120-147:  useNormalizedCache() hook
                - Setup TanStack Query
                - Create query key including libraryId

Lines 149-500:  useEffect hook that listens for events

    Lines 150-167:  Handle JobProgress/IndexingProgress (ignore)

    Lines 157-250:  Handle ResourceChanged event
                    - Match resource_type
                    - Find existing item in array
                    - Deep merge or append new item
                    - Handle wrapped responses { items: [...] }
                    - Handle single object responses

    Lines 251-467:  Handle ResourceChangedBatch event
                    - Similar to ResourceChanged but for multiple items
                    - Smart matching by ID or content UUID
                    - Preserves existing data
                    - Appends new items if isGlobalList=true

    Lines 468-499:  Handle ResourceDeleted event
                    - Remove item from array
                    - Handle wrapped responses

Lines 502-511:  Subscribe to events
                - client.on('spacedrive-event', handleEvent)
                - Cleanup: client.off(...)
```

**Key Code:**
```typescript
if ('ResourceChanged' in event) {
  const { resource_type, resource } = event.ResourceChanged;
  if (resource_type === resourceType) {
    // Update cache atomically
    queryClient.setQueryData<O>(queryKey, (oldData) => {
      // Merge or append logic here
    });
  }
}
```

---

### `/packages/ts-client/src/hooks/useClient.tsx`
**Client Provider and Hooks**

```
Exports:
- SpacedriveProvider component
- useSpacedriveClient() hook
- And other hooks...
```

---

## interface Package

### `/packages/interface/src/hooks/useEvent.ts`
**Manual Event Listening Hook**

```
Lines 1-30:     useEvent() hook
                - Takes eventType and handler
                - Uses useSpacedriveClient()
                - Calls client.on('spacedrive-event', handleEvent)
                - Checks: if (!eventType || eventType in event)
                - Cleanup: client.off()

Lines 35-37:    useAllEvents() hook
                - Calls useEvent('', handler)
                - Empty string matches all events
```

**Usage:**
```typescript
useEvent('LocationAdded', (event) => {
  const { location_id } = event.LocationAdded;
  // Handle location creation
});
```

---

### `/packages/interface/src/components/Explorer/components/AddLocationModal.tsx`
**Dialog for Adding Locations**

```
Lines 40-60:    IndexMode definitions
                - Shallow, Content, Deep

Lines 62-119:   JobOption definitions
                - Various indexing jobs

Lines 121-127:  useAddLocationDialog() function
                - Creates dialog with callback
                - onLocationAdded?: (locationId: string) => void

Lines 129-252:  AddLocationDialog component

    Lines 138-142:  useAddLocationDialog setup
                    - useLibraryMutation for adding location
                    - useLibraryQuery for suggested locations

    Lines 223-252:  onSubmit handler
                    Line 236:  const result = await addLocation.mutateAsync(input)
                    Line 238:  dialog.state.open = false
                    Lines 241-242: Call props.onLocationAdded(result.id)
                    ISSUE: No event waiting here!

Lines 254-317:  Form picker step (pick location)

Lines 320-439:  Form settings step (configure indexing)
```

**Current Behavior:**
- Line 236: Mutation completes and returns location
- Line 241-242: Immediately calls onLocationAdded callback
- NO waiting for ResourceChanged event

---

### `/packages/interface/src/components/Explorer/components/LocationsSection.tsx`
**Locations List Component**

```
Lines 1-8:      Imports
                - useNavigate from react-router-dom
                - useAddLocationDialog from AddLocationModal
                - useNormalizedCache from context

Lines 10-72:    LocationsSection component

    Lines 11-12:    const navigate = useNavigate()
                    const { locationId } = useParams()

    Lines 14-19:    useNormalizedCache for locations list
                    wireMethod: 'query:locations.list'
                    resourceType: 'location'
                    isGlobalList: true

    Lines 21:       Extract locations from response

    Lines 23-25:    handleLocationClick()
                    navigate(`/location/${location.id}`)

    Lines 27-31:    handleAddLocation()
                    Line 28: await useAddLocationDialog((locationId) => {
                    Line 29:   navigate(`/location/${locationId}`);
                    })
                    Callback fires when AddLocationModal calls onLocationAdded

    Lines 55-63:    Render location buttons
                    Click: navigate to /location/{locationId}

    Lines 65-70:    Add Location button
                    Click: handleAddLocation()
```

---

### `/packages/interface/src/components/SpacesSidebar/LocationsGroup.tsx`
**Sidebar Locations Display**

```
Lines 1-8:      Imports
                - useNavigate from react-router-dom
                - useNormalizedCache from @sd/ts-client

Lines 11-19:    useNormalizedCache for locations
                wireMethod: 'query:locations.list'
                resourceType: 'location'
                isGlobalList: true

Lines 26-35:    Header with collapse toggle

Lines 39-52:    Locations list
                Line 44: onClick={() => navigate(`/location/${location.id}`)}
```

---

### `/packages/interface/src/hooks/useLibraries.ts`
**Locations/Libraries Hook**

Simple hook that uses useNormalizedCache for locations.

---

### `/packages/interface/src/router.tsx`
**Router Definition**

```
Lines 1-6:      Imports
                - createBrowserRouter from react-router-dom

Lines 10-51:    createExplorerRouter() function

    Lines 11-12:    createBrowserRouter([...])

    Lines 13-49:    Route definitions

    Lines 21-27:    Location route
                    path: 'location/:locationId'
                    element: <ExplorerView />
                    Also: 'location/:locationId/*' for nested paths
```

---

### `/packages/interface/src/Explorer.tsx`
**Main Layout Component**

```
Lines 1-12:     Imports
                - useLocation from react-router-dom
                - useEffect for Tauri event listening

Lines 18-75:    ExplorerLayout component
                Main layout structure

Lines 78-140:   UI structure with sidebar, main content, inspector

Lines 143-157:  Explorer component (root)
                Sets up SpacedriveProvider and RouterProvider
```

---

### `/packages/interface/src/context.tsx`
**Context Re-exports**

```
Lines 1-28:     Re-exports from @sd/ts-client/hooks
                - SpacedriveProvider
                - useSpacedriveClient
                - useCoreQuery
                - useLibraryQuery
                - useCoreMutation
                - useLibraryMutation
                - useNormalizedCache
```

---

## Summary of Key Lines

| File | Lines | Purpose |
|------|-------|---------|
| `client.ts` | 8-39 | SimpleEventEmitter implementation |
| `client.ts` | 200-211 | subscribe() method |
| `event-filter.ts` | 34-85 | Event subscription list |
| `generated/types.ts` | 424 | Event union type |
| `useNormalizedCache.ts` | 150-500 | Event handling logic |
| `useEvent.ts` | 1-30 | Manual event hook |
| `AddLocationModal.tsx` | 236-242 | Location creation (needs modification) |
| `LocationsSection.tsx` | 27-30 | Add location callback |
| `LocationsGroup.tsx` | 44 | Navigate on location click |
| `router.tsx` | 21-27 | Location route definition |

---

## Files That Need Modification

For event-driven navigation on location creation:

1. **AddLocationModal.tsx** (Lines 236-242)
   - Add event listener after mutation completes
   - Wait for ResourceChanged event
   - Then call onLocationAdded

2. **OR create new file:** `useOnLocationCreated.ts`
   - Reusable hook for location creation events
   - Use in LocationsSection or other components

3. **Optional:** Create `useLocationUpdates.ts`
   - Listen for updates to specific location
   - Use in ExplorerView when viewing a location

---

## Event Processing Flow

```
AddLocationModal.tsx (line 236)
  ↓
  User submits form
  ↓
addLocation.mutateAsync(input)
  ↓
Backend processes, creates location
  ↓
Backend emits ResourceChanged event
  ↓
client.emit('spacedrive-event', event)  [client.ts]
  ↓
useNormalizedCache listener [useNormalizedCache.ts]
  ↓
queryClient.setQueryData() - updates cache
  ↓
Component re-renders with new location
  ↓
AddLocationModal.tsx (line 241)
  ↓
onLocationAdded(result.id) callback fires
  ↓
LocationsSection.tsx (line 29)
  ↓
navigate(`/location/${locationId}`)
  ↓
Router navigates to ExplorerView
  ↓
User sees new location
```

---

## Testing Points

1. Add location through dialog
2. Observe whether navigate happens before ResourceChanged event
3. Check browser console for event logs
4. Verify location appears in sidebar immediately
5. Verify ExplorerView displays location data

---

## Related Files (Reference Only)

- `/packages/interface/src/components/Explorer/ExplorerView.tsx` - Displays location contents
- `/packages/interface/src/components/explorer/Sidebar.tsx` - Shows locations in sidebar
- `/packages/interface/src/components/JobManager/` - Job progress tracking
- `/packages/interface/src/Inspector.tsx` - File inspector panel
- `/packages/interface/src/components/QuickPreview/` - Quick preview modal
