# Spacedrive Interface: Resource Events & Navigation Architecture

## Overview
This document maps out how resource events (particularly location creation) are received, structured, and how navigation is implemented in the `packages/interface` package.

---

## 1. EVENT SYSTEM ARCHITECTURE

### 1.1 Event Flow
```
Backend (Rust/Core) 
  ↓ (emits ResourceChanged event)
SpacedriveClient.subscribe() [transport.ts]
  ↓ (converts to spacedrive-event)
client.emit("spacedrive-event", event)
  ↓ (SimpleEventEmitter)
useNormalizedCache listener [ts-client/hooks/useNormalizedCache.ts]
  OR
useEvent hook [interface/hooks/useEvent.ts]
  ↓
Application logic/UI updates
```

### 1.2 Event Emission Source
**File:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/client.ts`

```typescript
async subscribe(callback?: (event: Event) => void): Promise<() => void> {
  const unlisten = await this.transport.subscribe((event) => {
    // Emit to SimpleEventEmitter (useNormalizedCache listens to this)
    this.emit("spacedrive-event", event);
    if (callback) {
      callback(event);
    }
  });
  return unlisten;
}
```

The client extends SimpleEventEmitter which provides:
- `on(event: string, listener: Function)` - Add listener
- `off(event: string, listener: Function)` - Remove listener
- `emit(event: string, ...args: any[])` - Emit event
- `once(event: string, listener: Function)` - One-time listener

---

## 2. LOCATION CREATION EVENT STRUCTURE

### 2.1 Event Types
**File:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/event-filter.ts`

Three location-related events are available:

```typescript
// Legacy compatibility events (in DEFAULT_EVENT_SUBSCRIPTION):
"LocationAdded"    // { library_id: string; location_id: string; path: string }
"LocationRemoved"  // { library_id: string; location_id: string }

// Resource change events (NEW - recommended):
"ResourceChanged"      // Single location updated
"ResourceChangedBatch" // Multiple locations updated (batch)
"ResourceDeleted"      // Location deleted
```

### 2.2 ResourceChanged Event Structure
**Source:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/generated/types.ts`

```typescript
{
  ResourceChanged: {
    resource_type: string;      // "location" for location changes
    resource: {                  // The actual location object
      id: string;
      name: string;
      path: string;
      // ... other LocationInfo fields
    };
    metadata?: {
      no_merge_fields?: string[];  // Fields to replace (not merge)
      alternate_ids?: string[];    // Alternate ID fields for matching
    }
  }
}
```

### 2.3 LocationAdded Event Structure (Legacy)
```typescript
{
  LocationAdded: {
    library_id: string;   // Which library the location was added to
    location_id: string;  // The ID of the new location
    path: string;         // The filesystem path
  }
}
```

---

## 3. EVENT HANDLING PATTERNS

### 3.1 Pattern A: Using useEvent Hook (Low-level)
**File:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/hooks/useEvent.ts`

```typescript
export function useEvent(eventType: string, handler: (event: any) => void) {
  const client = useSpacedriveClient();

  useEffect(() => {
    if (!client) return;

    const handleEvent = (event: any) => {
      // Events come as { EventName: { ...data } } not { type: "EventName" }
      if (!eventType || eventType in event) {
        handler(event);
      }
    };

    client.on('spacedrive-event', handleEvent);
    return () => {
      client.off('spacedrive-event', handleEvent);
    };
  }, [eventType, client]);
}
```

**Usage Example:**
```typescript
useEvent('LocationAdded', (event) => {
  const { library_id, location_id, path } = event.LocationAdded;
  // Handle new location
});
```

### 3.2 Pattern B: Using useNormalizedCache Hook (Recommended)
**File:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/hooks/useNormalizedCache.ts`

This hook automatically:
1. Fetches data via TanStack Query
2. Listens for ResourceChanged events
3. Atomically updates the cache
4. Triggers re-render

```typescript
const { data: locationsData } = useNormalizedCache({
  wireMethod: 'query:locations.list',
  input: {},
  resourceType: 'location',          // ← Matches event resource_type
  isGlobalList: true,                // ← Auto-appends new locations
});
```

**How it works:**
- When `ResourceChanged` event arrives with `resource_type: 'location'`
- The hook finds the location in the array (by ID)
- If found: merges the update
- If not found AND `isGlobalList: true`: appends as new item
- Component re-renders instantly with new data

---

## 4. CURRENT LOCATION CREATION FLOW

### 4.1 Add Location Dialog
**File:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/AddLocationModal.tsx`

```typescript
export function useAddLocationDialog(
  onLocationAdded?: (locationId: string) => void,  // ← Callback!
) {
  return dialogManager.create((props) => (
    <AddLocationDialog {...props} onLocationAdded={onLocationAdded} />
  ));
}

// Inside the dialog's submit handler:
const onSubmit = form.handleSubmit(async (data) => {
  const input: LocationAddInput = { /* ... */ };
  
  try {
    const result = await addLocation.mutateAsync(input);
    dialog.state.open = false;

    // Call callback with new location ID
    if (result?.id && props.onLocationAdded) {
      props.onLocationAdded(result.id);
    }
  } catch (error) {
    console.error("Failed to add location:", error);
  }
});
```

### 4.2 Current Usage in LocationsSection
**File:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/LocationsSection.tsx`

```typescript
const handleAddLocation = async () => {
  await useAddLocationDialog((locationId) => {
    navigate(`/location/${locationId}`);  // ← Navigate on add
  });
};
```

**Current behavior:**
1. User clicks "Add Location"
2. Dialog appears with form
3. User submits form
4. Mutation completes and returns location ID
5. `onLocationAdded` callback fires
6. Navigation happens to `/location/{locationId}`

**Issue:** This doesn't wait for the location to be fully indexed/ready. It navigates immediately after mutation, before the backend has emitted ResourceChanged events.

---

## 5. NAVIGATION SYSTEM

### 5.1 Router Setup
**File:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/router.tsx`

```typescript
import { createBrowserRouter } from "react-router-dom";

export function createExplorerRouter() {
  return createBrowserRouter([
    {
      path: "/",
      element: <ExplorerLayout />,
      children: [
        {
          index: true,
          element: <Overview />,
        },
        {
          path: "location/:locationId",           // ← Location route
          element: <ExplorerView />,
        },
        {
          path: "location/:locationId/*",         // ← With nested paths
          element: <ExplorerView />,
        },
        // ... other routes
      ],
    },
  ]);
}
```

### 5.2 Using useNavigate
React Router's `useNavigate` hook is used throughout:

```typescript
import { useNavigate } from "react-router-dom";

function Component() {
  const navigate = useNavigate();
  
  navigate(`/location/${locationId}`);           // Navigate
  navigate("/");                                 // Back to home
}
```

### 5.3 Current Navigation Examples

**LocationsGroup (sidebar):**
```typescript
const navigate = useNavigate();

{locations.map((location) => (
  <button
    onClick={() => navigate(`/location/${location.id}`)}
    // ...
  >
    {location.name}
  </button>
))}
```

**LocationsSection (add location):**
```typescript
const handleAddLocation = async () => {
  await useAddLocationDialog((locationId) => {
    navigate(`/location/${locationId}`);
  });
};
```

---

## 6. ARCHITECTURAL DECISIONS FOR YOUR IMPLEMENTATION

### 6.1 Best Approach: Event-Driven Navigation
**Recommended:** Listen for ResourceChanged event after mutation

```typescript
// In AddLocationModal or a parent component
const onSubmit = form.handleSubmit(async (data) => {
  const input: LocationAddInput = { /* ... */ };
  const result = await addLocation.mutateAsync(input);
  dialog.state.open = false;

  if (result?.id) {
    // Wait for ResourceChanged event before navigating
    const unsubscribe = client.on('spacedrive-event', (event) => {
      if ('ResourceChanged' in event) {
        const { resource_type, resource } = event.ResourceChanged;
        if (resource_type === 'location' && resource.id === result.id) {
          unsubscribe?.();
          props.onLocationAdded?.(result.id);  // Will trigger navigate
        }
      }
    });

    // Safety timeout (don't wait forever)
    setTimeout(() => unsubscribe?.(), 5000);
  }
});
```

### 6.2 Alternative: Using useEvent Hook
```typescript
// In LocationsSection
const navigate = useNavigate();
const [pendingLocationId, setPendingLocationId] = useState<string | null>(null);

useEvent('ResourceChanged', (event) => {
  if ('ResourceChanged' in event && pendingLocationId) {
    const { resource_type, resource } = event.ResourceChanged;
    if (resource_type === 'location' && resource.id === pendingLocationId) {
      navigate(`/location/${pendingLocationId}`);
      setPendingLocationId(null);
    }
  }
});

const handleAddLocation = async () => {
  await useAddLocationDialog((locationId) => {
    setPendingLocationId(locationId);  // Wait for event
  });
};
```

### 6.3 Alternative: Using LocationAdded Event
```typescript
useEvent('LocationAdded', (event) => {
  if ('LocationAdded' in event) {
    const { location_id } = event.LocationAdded;
    navigate(`/location/${location_id}`);
  }
});
```

---

## 7. KEY FILES & PATHS

### Core Event System
- **Client with event emitter:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/client.ts`
- **Event filter/types:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/event-filter.ts`
- **Generated event types:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/generated/types.ts`
- **SimpleEventEmitter class:** Lines 8-39 in client.ts

### Event Listeners
- **useEvent hook:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/hooks/useEvent.ts`
- **useNormalizedCache hook:** `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/hooks/useNormalizedCache.ts`
- **useAllEvents hook:** Lines 35-37 in useEvent.ts

### Location Components
- **Add Location Dialog:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/AddLocationModal.tsx`
- **Locations Section:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/LocationsSection.tsx`
- **Locations Group (sidebar):** `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/SpacesSidebar/LocationsGroup.tsx`

### Navigation
- **Router definition:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/router.tsx`
- **Explorer layout:** `/Users/jamespine/Projects/spacedrive/packages/interface/src/Explorer.tsx`

---

## 8. EVENT STRUCTURE EXAMPLES

### ResourceChanged Event (Preferred)
```json
{
  "ResourceChanged": {
    "resource_type": "location",
    "resource": {
      "id": "abc123",
      "name": "My Photos",
      "path": "/Users/james/Pictures",
      "indexed_at": "2025-11-13T10:30:00Z"
    },
    "metadata": {
      "no_merge_fields": ["indexed_at"],
      "alternate_ids": []
    }
  }
}
```

### LocationAdded Event (Legacy)
```json
{
  "LocationAdded": {
    "library_id": "lib-123",
    "location_id": "loc-456",
    "path": "/Users/james/Pictures"
  }
}
```

### ResourceChangedBatch Event
```json
{
  "ResourceChangedBatch": {
    "resource_type": "file",
    "resources": [
      { "id": "file1", "name": "photo1.jpg", ... },
      { "id": "file2", "name": "photo2.jpg", ... }
    ],
    "metadata": { ... }
  }
}
```

---

## 9. CODE PATTERNS TO FOLLOW

### Pattern: Event + Navigation
```typescript
function AddLocationDialog(props: { onLocationAdded?: (id: string) => void }) {
  const client = useSpacedriveClient();
  const navigate = useNavigate();
  
  const handleSuccess = async (result: LocationAddInput) => {
    // Method 1: Wait for ResourceChanged event
    let unsubscribe: (() => void) | null = null;
    
    unsubscribe = client.on('spacedrive-event', (event) => {
      if ('ResourceChanged' in event) {
        const { resource_type, resource } = event.ResourceChanged;
        if (resource_type === 'location' && resource.id === result.id) {
          unsubscribe?.();
          navigate(`/location/${result.id}`);
        }
      }
    });

    // Safety: timeout after 5 seconds
    const timeout = setTimeout(() => {
      unsubscribe?.();
      navigate(`/location/${result.id}`);
    }, 5000);
  };
}
```

### Pattern: Auto-list Updates via useNormalizedCache
```typescript
function LocationsList() {
  const { data: locationsData } = useNormalizedCache({
    wireMethod: 'query:locations.list',
    input: {},
    resourceType: 'location',
    isGlobalList: true,  // ← Auto-appends new locations from events
  });

  // When user adds a location:
  // 1. Mutation completes
  // 2. ResourceChanged event fires
  // 3. useNormalizedCache adds to cache
  // 4. Component re-renders with new location
  // 5. Sidebar shows it automatically
  
  return locationsData?.locations.map(loc => (
    <button key={loc.id} onClick={() => navigate(`/location/${loc.id}`)}>
      {loc.name}
    </button>
  ));
}
```

---

## 10. SUMMARY

**Current Architecture:**
- Events are emitted by backend via `client.subscribe()`
- SimpleEventEmitter broadcasts all events as 'spacedrive-event'
- Two main consumption patterns: `useEvent` (manual) and `useNormalizedCache` (auto)
- Navigation uses React Router's `useNavigate` hook
- Location creation currently navigates immediately on mutation

**What's Available:**
- LocationAdded and LocationRemoved events (legacy)
- ResourceChanged, ResourceChangedBatch, ResourceDeleted events (recommended)
- Event listeners via client.on() / client.off()
- Auto-cache updates via useNormalizedCache

**For Your Implementation:**
1. Hook into the ResourceChanged event after location mutation
2. Wait for the event confirming the location was created
3. Then navigate to the new location
4. Add safety timeout in case event doesn't arrive

This ensures the location is actually created and indexed before the user sees the empty location view.
