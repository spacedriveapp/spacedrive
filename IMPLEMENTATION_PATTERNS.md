# Quick Reference: Implementation Patterns

## Core Event Mechanics

### How Events Work
```typescript
// 1. Backend emits event
// 2. Client receives via transport
client.emit('spacedrive-event', event)
// 3. Listeners receive event
client.on('spacedrive-event', handler)
// 4. Event structure: { EventName: {...data} }
if ('ResourceChanged' in event) {
  const { resource_type, resource } = event.ResourceChanged;
}
```

### Event Detection
Events come as **discriminated unions**, not objects with type field:
```typescript
// CORRECT - Check if property exists
if ('LocationAdded' in event) {
  event.LocationAdded.location_id  // TypeScript knows type
}

// CORRECT - Using optional chaining
event.ResourceChanged?.resource?.id  // Safe

// WRONG
if (event.type === 'LocationAdded') { }  // Events don't have .type
```

---

## Pattern 1: Listen for Event in Component

**Use when:** You need to react to a specific event globally

```typescript
// hooks/useLocationCreated.ts
import { useEffect } from 'react';
import { useSpacedriveClient } from '../context';

export function useOnLocationCreated(
  callback: (locationId: string) => void
) {
  const client = useSpacedriveClient();

  useEffect(() => {
    if (!client) return;

    const handleEvent = (event: any) => {
      if ('ResourceChanged' in event) {
        const { resource_type, resource } = event.ResourceChanged;
        if (resource_type === 'location') {
          callback(resource.id);
        }
      }
    };

    client.on('spacedrive-event', handleEvent);
    return () => {
      client.off('spacedrive-event', handleEvent);
    };
  }, [callback, client]);
}

// Usage in LocationsSection
function LocationsSection() {
  const navigate = useNavigate();
  
  useOnLocationCreated((locationId) => {
    navigate(`/location/${locationId}`);
  });

  return (/* ... */);
}
```

---

## Pattern 2: Wait for Event After Mutation

**Use when:** You need to wait for confirmation before proceeding

```typescript
// components/AddLocationModal.tsx
const onSubmit = form.handleSubmit(async (data) => {
  const result = await addLocation.mutateAsync(input);
  
  if (result?.id) {
    // Setup listener for the specific location
    let unsubscribe: (() => void) | undefined;
    const timeout = setTimeout(() => {
      unsubscribe?.();
      // Fallback: navigate anyway after timeout
      props.onLocationAdded?.(result.id);
    }, 5000);

    unsubscribe = client.on('spacedrive-event', (event) => {
      if ('ResourceChanged' in event) {
        const { resource_type, resource } = event.ResourceChanged;
        if (resource_type === 'location' && resource.id === result.id) {
          clearTimeout(timeout);
          unsubscribe?.();
          // Location confirmed by event - safe to navigate
          props.onLocationAdded?.(result.id);
        }
      }
    });
  }
});
```

---

## Pattern 3: Auto-Update List from Events (Recommended)

**Use when:** You want automatic cache updates

```typescript
// components/LocationsList.tsx
import { useNormalizedCache } from '@sd/ts-client';
import { useNavigate } from 'react-router-dom';

function LocationsList() {
  const navigate = useNavigate();

  // This hook automatically:
  // 1. Fetches locations
  // 2. Listens for ResourceChanged events
  // 3. Adds new locations to the list
  // 4. Updates existing locations
  const { data: locationsData, isLoading } = useNormalizedCache({
    wireMethod: 'query:locations.list',
    input: {},
    resourceType: 'location',
    isGlobalList: true,  // ← Key! Auto-appends new items
  });

  const locations = locationsData?.locations || [];

  return (
    <div>
      {locations.map((loc) => (
        <button
          key={loc.id}
          onClick={() => navigate(`/location/${loc.id}`)}
        >
          {loc.name}
        </button>
      ))}
    </div>
  );
}
```

**Why this works:**
- When ResourceChanged arrives with `resource_type: 'location'`
- Hook checks if location already in list (by ID)
- If not found AND `isGlobalList: true`: appends it
- Component re-renders instantly with new location

---

## Pattern 4: useEvent Hook (Manual)

**Use when:** You need low-level control

```typescript
// hooks/useLocationAdded.ts
import { useEvent } from './useEvent';

export function useLocationAdded(
  callback: (locationId: string) => void
) {
  useEvent('LocationAdded', (event) => {
    if ('LocationAdded' in event) {
      callback(event.LocationAdded.location_id);
    }
  });
}

// Usage
function MyComponent() {
  useLocationAdded((locationId) => {
    console.log('Location added:', locationId);
  });
}
```

---

## Pattern 5: Event Filtering

**Use when:** You need specific location changes

```typescript
// Respond only to a specific location's changes
function useLocationUpdates(locationId: string, callback: () => void) {
  const client = useSpacedriveClient();

  useEffect(() => {
    if (!client) return;

    const handleEvent = (event: any) => {
      if ('ResourceChanged' in event) {
        const { resource_type, resource } = event.ResourceChanged;
        if (
          resource_type === 'location' &&
          resource.id === locationId
        ) {
          callback();
        }
      }
    };

    client.on('spacedrive-event', handleEvent);
    return () => {
      client.off('spacedrive-event', handleEvent);
    };
  }, [locationId, callback, client]);
}
```

---

## Pattern 6: Batch Events

**Use when:** Multiple resources change together

```typescript
// Handle multiple locations being updated
const handleEvent = (event: any) => {
  if ('ResourceChangedBatch' in event) {
    const { resource_type, resources } = event.ResourceChangedBatch;
    
    if (resource_type === 'location') {
      // Process all locations at once
      resources.forEach(location => {
        console.log(`Location updated: ${location.name}`);
      });
    }
  }
};

client.on('spacedrive-event', handleEvent);
```

---

## Complete Example: Add Location with Event-Driven Navigation

```typescript
// AddLocationModal.tsx
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useSpacedriveClient, useLibraryMutation } from '../context';
import { Dialog } from '@sd/ui';

export function useAddLocationDialog(
  onLocationAdded?: (locationId: string) => void,
) {
  return dialogManager.create((props) => (
    <AddLocationDialog {...props} onLocationAdded={onLocationAdded} />
  ));
}

function AddLocationDialog(props: {
  id: number;
  onLocationAdded?: (locationId: string) => void;
}) {
  const client = useSpacedriveClient();
  const dialog = useDialog(props);
  const form = useForm<AddLocationFormData>({ /* ... */ });
  const addLocation = useLibraryMutation('locations.add');

  const onSubmit = form.handleSubmit(async (data) => {
    const input: LocationAddInput = {
      path: { Physical: { device_slug: 'local', path: data.path } },
      name: data.name || null,
      mode: data.mode,
    };

    try {
      const result = await addLocation.mutateAsync(input);
      dialog.state.open = false;

      if (result?.id) {
        // Wait for event confirmation
        let unsubscribe: (() => void) | undefined;
        const timeout = setTimeout(() => {
          // Fallback after 5 seconds
          unsubscribe?.();
          props.onLocationAdded?.(result.id);
        }, 5000);

        unsubscribe = client.on('spacedrive-event', (event) => {
          if ('ResourceChanged' in event) {
            const { resource_type, resource } = event.ResourceChanged;
            // Confirm this is our location and it's ready
            if (resource_type === 'location' && resource.id === result.id) {
              clearTimeout(timeout);
              unsubscribe?.();
              // Safe to navigate now
              props.onLocationAdded?.(result.id);
            }
          }
        });
      }
    } catch (error) {
      console.error('Failed to add location:', error);
    }
  });

  return (
    <Dialog
      dialog={dialog}
      form={form}
      onSubmit={onSubmit}
      loading={addLocation.isPending}
    >
      {/* Dialog content */}
    </Dialog>
  );
}

// LocationsSection.tsx
function LocationsSection() {
  const navigate = useNavigate();
  
  const { data: locationsData } = useNormalizedCache({
    wireMethod: 'query:locations.list',
    input: {},
    resourceType: 'location',
    isGlobalList: true,
  });

  const locations = locationsData?.locations || [];

  const handleAddLocation = async () => {
    // This callback will be called when location is confirmed by event
    await useAddLocationDialog((locationId) => {
      navigate(`/location/${locationId}`);
    });
  };

  return (
    <div>
      {locations.map(loc => (
        <SidebarItem
          key={loc.id}
          label={loc.name}
          onClick={() => navigate(`/location/${loc.id}`)}
        />
      ))}
      
      <SidebarItem
        label="Add Location"
        onClick={handleAddLocation}
      />
    </div>
  );
}
```

---

## Debugging Events

```typescript
// Listen to ALL events to see structure
const handleAllEvents = (event: any) => {
  console.log('Event received:', event);
  
  // See which events are coming
  if ('LocationAdded' in event) console.log('LocationAdded event');
  if ('ResourceChanged' in event) console.log('ResourceChanged event');
  if ('ResourceChangedBatch' in event) console.log('ResourceChangedBatch event');
  // etc.
};

client.on('spacedrive-event', handleAllEvents);
```

---

## Files to Modify/Create

### If implementing event-driven navigation:
1. **Modify:** `/packages/interface/src/components/Explorer/components/AddLocationModal.tsx`
   - Add event listener before calling `onLocationAdded`
   - Implement timeout fallback

2. **Create:** `/packages/interface/src/hooks/useLocationCreated.ts`
   - Reusable hook for listening to location creation
   - Can be used in any component

3. **Reference:** `/packages/interface/src/components/Explorer/components/LocationsSection.tsx`
   - Already uses callback pattern
   - No changes needed if you handle events in AddLocationModal

### If using useNormalizedCache improvements:
1. **Reference:** `/packages/ts-client/src/hooks/useNormalizedCache.ts`
   - Already handles ResourceChanged events
   - Already auto-appends with `isGlobalList: true`
   - No changes needed

---

## Key Points to Remember

1. **Events are discriminated unions:**
   ```typescript
   // Check with 'in' operator
   if ('LocationAdded' in event) { }
   // NOT: if (event.type === 'LocationAdded') { }
   ```

2. **Resource type matching:**
   ```typescript
   // Events have resource_type field
   event.ResourceChanged.resource_type === 'location'
   ```

3. **Always cleanup listeners:**
   ```typescript
   useEffect(() => {
     const unsubscribe = client.on('event', handler);
     return () => unsubscribe?.();  // ← Don't forget!
   }, []);
   ```

4. **useNormalizedCache auto-handles events:**
   ```typescript
   // No extra event handling needed!
   const { data } = useNormalizedCache({
     wireMethod: 'query:locations.list',
     resourceType: 'location',
     isGlobalList: true,  // ← Auto-appends
   });
   ```

5. **Safety timeouts prevent hanging:**
   ```typescript
   const timeout = setTimeout(() => {
     unsubscribe?.();
     navigateAnyway();  // Fallback
   }, 5000);
   ```
