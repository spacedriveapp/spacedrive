# Search Findings Summary

Complete findings from the search for resource events, location creation, and navigation architecture in the packages/interface directory.

---

## Search Completed: 4 Key Questions

### 1. Where are resource events received/handled in packages/interface?

**Answer: Multiple locations with a clear event flow architecture**

**Event Reception:**
- **Primary entry point:** `/packages/ts-client/src/client.ts` (SpacedriveClient class)
  - Inherits from SimpleEventEmitter
  - `subscribe()` method (lines 200-211) listens to backend events
  - Emits 'spacedrive-event' for all received events

**Event Distribution:**
- All events broadcast via SimpleEventEmitter interface
- `client.on('spacedrive-event', handler)` - Add listener
- `client.off('spacedrive-event', handler)` - Remove listener

**Event Handling Components:**

1. **useNormalizedCache Hook** (ts-client/src/hooks/useNormalizedCache.ts)
   - Automatically listens for ResourceChanged events
   - Lines 149-500: Handles ResourceChanged, ResourceChangedBatch, ResourceDeleted
   - Auto-appends new items to cache when isGlobalList=true
   - No manual event handling needed - fully transparent

2. **useEvent Hook** (interface/src/hooks/useEvent.ts)
   - Manual event listening for specific event types
   - Used in custom event handlers
   - Provides cleanup via client.off()

3. **Direct Component Listeners** (Various components)
   - LocationsSection.tsx uses useNormalizedCache for auto-update
   - AddLocationModal.tsx has callback pattern (could add event listening)
   - LocationsGroup.tsx uses useNormalizedCache

**Current Flow:**
```
Backend → client.emit('spacedrive-event', event)
        → useNormalizedCache listener (auto-appends to locations list)
        → useEvent listener (if registered)
        → Component callbacks
        → UI re-render
```

---

### 2. How are location creation events structured and detected?

**Answer: Two event types available with clear structure**

**Event Type 1: LocationAdded (Legacy - Simpler)**
```typescript
{
  LocationAdded: {
    library_id: string;      // Which library
    location_id: string;     // The new location's ID
    path: string;            // Filesystem path
  }
}
```

File: `/packages/ts-client/src/event-filter.ts` (line 79)
In DEFAULT_EVENT_SUBSCRIPTION, so auto-subscribed.

**Event Type 2: ResourceChanged (Modern - Recommended)**
```typescript
{
  ResourceChanged: {
    resource_type: string;   // "location" for locations
    resource: {
      id: string;
      name: string;
      path: string;
      indexed_at: string;
      ...other LocationInfo fields
    };
    metadata?: {
      no_merge_fields?: string[];
      alternate_ids?: string[];
    };
  }
}
```

File: `/packages/ts-client/src/generated/types.ts` (line 424)

**Event Detection Pattern:**

CORRECT:
```typescript
if ('LocationAdded' in event) {
  const { location_id } = event.LocationAdded;
}

if ('ResourceChanged' in event) {
  const { resource_type, resource } = event.ResourceChanged;
}
```

WRONG:
```typescript
if (event.type === 'LocationAdded') { }  // No .type field!
```

Events are **discriminated unions**, not objects with type fields.

**Where Events are Defined:**
- Type definitions: `/packages/ts-client/src/generated/types.ts` (line 424)
- Subscription list: `/packages/ts-client/src/event-filter.ts` (lines 34-85)
- All include LocationAdded, LocationRemoved, ResourceChanged, ResourceChangedBatch

---

### 3. How is navigation implemented?

**Answer: React Router with useNavigate hook**

**Router Setup:**
File: `/packages/interface/src/router.tsx`

```typescript
const router = createBrowserRouter([
  {
    path: '/',
    element: <ExplorerLayout />,
    children: [
      {
        path: 'location/:locationId',      // ← Location route
        element: <ExplorerView />,
      },
      {
        path: 'location/:locationId/*',    // ← With nested paths
        element: <ExplorerView />,
      },
      // ... other routes
    ],
  },
]);
```

**Navigation Usage:**

File: `/packages/interface/src/components/Explorer/components/LocationsSection.tsx`
```typescript
const navigate = useNavigate();

// Navigate to location
navigate(`/location/${location.id}`);

// Navigate back to overview
navigate('/');
```

**Used In:**
- LocationsSection.tsx (line 29) - When adding a location
- LocationsGroup.tsx (line 44) - When clicking a location
- ExplorerView.tsx - Navigation within explorer
- Explorer.tsx - Main layout navigation

**Navigation Flow for Location Creation:**
1. User clicks "Add Location"
2. AddLocationModal opens (routes don't change)
3. User fills form and submits
4. Mutation completes, returns location.id
5. onLocationAdded callback fires
6. navigate(`/location/{id}`) executes
7. Router loads ExplorerView component
8. Route params updated to /:locationId
9. ExplorerView mounts and fetches location data

---

### 4. What existing event listeners or handlers exist?

**Answer: Complete event listener system with multiple patterns**

**Available Event Listeners:**

**1. SimpleEventEmitter (Core)**
Location: `/packages/ts-client/src/client.ts` (lines 8-39)

```typescript
class SimpleEventEmitter {
  on(event: string, listener: Function) { }          // Add listener
  off(event: string, listener: Function) { }         // Remove listener
  emit(event: string, ...args: any[]) { }           // Emit event
  once(event: string, listener: Function) { }        // One-time listener
}
```

**2. useEvent Hook**
Location: `/packages/interface/src/hooks/useEvent.ts`

```typescript
export function useEvent(eventType: string, handler: (event: any) => void) {
  const client = useSpacedriveClient();
  useEffect(() => {
    const handleEvent = (event: any) => {
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

Usage:
```typescript
useEvent('LocationAdded', (event) => {
  const { location_id } = event.LocationAdded;
  // Handle event
});
```

**3. useAllEvents Hook**
Location: `/packages/interface/src/hooks/useEvent.ts` (line 35-37)

```typescript
export function useAllEvents(handler: (event: any) => void) {
  return useEvent('', handler);  // Empty string matches all
}
```

**4. useNormalizedCache Hook (Auto-listener)**
Location: `/packages/ts-client/src/hooks/useNormalizedCache.ts`

Auto-handles:
- ResourceChanged events (lines 157-250)
- ResourceChangedBatch events (lines 251-467)
- ResourceDeleted events (lines 468-499)
- Automatically updates TanStack Query cache
- No manual event handling needed

**5. Component-Level Listeners**

**LocationsSection.tsx:**
- Uses useNormalizedCache (auto-updates locations list)
- Has onLocationAdded callback from dialog
- Calls navigate() when callback fires

**AddLocationModal.tsx:**
- Has onLocationAdded callback parameter
- Calls it on mutation success (line 241-242)
- Could add event listening here

**LocationsGroup.tsx:**
- Uses useNormalizedCache (auto-updates sidebar)
- Responds to location changes via cache updates

---

## Current Architecture Summary

**Event Flow:**
```
Backend → Transport → client.emit('spacedrive-event', event)
        ↓
        useNormalizedCache (auto-append if isGlobalList=true)
        ↓
        useEvent / custom listeners (if registered)
        ↓
        Component callbacks
        ↓
        UI re-render
```

**For Location Creation:**
```
User clicks "Add Location"
        ↓
Dialog opens
        ↓
User submits form
        ↓
addLocation.mutateAsync(input)
        ↓
Backend creates location, emits ResourceChanged
        ↓
useNormalizedCache listener receives event
        ↓
Locations list updated in TanStack Query cache
        ↓
Mutation returns result with location.id
        ↓
onLocationAdded(location.id) callback fires
        ↓
navigate(`/location/${location.id}`)
        ↓
Router loads ExplorerView
        ↓
ExplorerView fetches location data
        ↓
User sees location
```

---

## Key Files Found

### Core Event System
- `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/client.ts` (226 lines)
- `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/event-filter.ts` (94 lines)
- `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/generated/types.ts` (auto-generated)

### Event Hooks
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/hooks/useEvent.ts` (38 lines)
- `/Users/jamespine/Projects/spacedrive/packages/ts-client/src/hooks/useNormalizedCache.ts` (512 lines)

### Location Components
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/AddLocationModal.tsx` (441 lines)
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/LocationsSection.tsx` (74 lines)
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/components/SpacesSidebar/LocationsGroup.tsx` (56 lines)

### Router
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/router.tsx` (52 lines)

### Context
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/context.tsx` (28 lines)
- `/Users/jamespine/Projects/spacedrive/packages/interface/src/Explorer.tsx` (158 lines)

---

## Additional Documentation Created

The following documents have been created in the project root to support your implementation:

1. **ARCHITECTURE_EVENT_NAVIGATION.md** - Complete 10-section architecture guide
2. **IMPLEMENTATION_PATTERNS.md** - 6 specific code patterns with full examples
3. **CODE_MAP.md** - Line-by-line reference of all key files
4. **EVENT_SYSTEM_SUMMARY.txt** - Visual ASCII summary
5. **SEARCH_FINDINGS_SUMMARY.md** - This document

---

## Ready to Implement?

All necessary information is now available to implement:

1. Event-driven navigation on location creation
2. Custom event listeners for specific events
3. Cache updates from ResourceChanged events
4. Timeout-safe event waiting patterns

See IMPLEMENTATION_PATTERNS.md for complete working examples.
