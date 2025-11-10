# Type-Safe Spacedrive Client Usage

The TypeScript client now mirrors the Swift client with full type safety via auto-generated types.

## Architecture

```
Auto-Generated Types (Specta)
  ↓
SpacedriveClient (low-level execution)
  ↓
React Query Hooks (useCoreQuery, useLibraryQuery)
  ↓
React Components (fully type-safe!)
```

## Setup

```typescript
import { SpacedriveClient, SpacedriveProvider } from '@sd/ts-client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Create client
const client = SpacedriveClient.fromTauri(invoke, listen);

// Wrap your app
function App() {
  return (
    <SpacedriveProvider client={client}>
      <YourApp />
    </SpacedriveProvider>
  );
}
```

## Usage in Components

### Core Queries (no library required)

```typescript
import { useCoreQuery } from '@sd/ts-client';

function LibraryList() {
  // Fully type-safe! Input and output types are inferred
  const { data: libraries, isLoading } = useCoreQuery({
    type: 'libraries.list',
    input: {}, // TypeScript validates this matches ListLibrariesInput
  });

  const { data: status } = useCoreQuery({
    type: 'core.status',
    input: {}, // Empty type
  });

  if (isLoading) return <div>Loading...</div>;

  return (
    <div>
      <h1>Spacedrive v{status?.version}</h1>
      <ul>
        {libraries?.map(lib => (
          <li key={lib.id}>{lib.name}</li>
          // lib is fully typed as LibraryInfo!
        ))}
      </ul>
    </div>
  );
}
```

### Library Queries (requires library context)

```typescript
import { useLibraryQuery, useSpacedriveClient } from '@sd/ts-client';
import { useEffect } from 'react';

function FileExplorer() {
  const client = useSpacedriveClient();

  // Set current library
  useEffect(() => {
    client.switchToLibrary('some-library-id');
  }, []);

  // Library-scoped query (auto uses current library)
  const { data: files } = useLibraryQuery({
    type: 'files.directory_listing',
    input: {
      path: '/',
      // TypeScript validates this matches DirectoryListingInput
    },
  });

  const { data: jobs } = useLibraryQuery({
    type: 'jobs.list',
    input: {},
  });

  return (
    <div>
      {files?.entries.map(file => (
        <div key={file.id}>{file.name}</div>
        // file is fully typed as File!
      ))}
    </div>
  );
}
```

### Mutations

```typescript
import { useCoreMutation, useLibraryMutation } from '@sd/ts-client';

function CreateLibraryButton() {
  const createLibrary = useCoreMutation('libraries.create');

  const handleCreate = () => {
    createLibrary.mutate({
      name: 'My New Library',
      path: null,
      // TypeScript validates this matches LibraryCreateInput
    }, {
      onSuccess: (result) => {
        console.log('Created library:', result.id);
        // result is typed as LibraryCreateOutput!
      }
    });
  };

  return <button onClick={handleCreate}>Create Library</button>;
}

function ApplyTagsButton({ entryIds }: { entryIds: number[] }) {
  const applyTags = useLibraryMutation('tags.apply');

  const handleApply = () => {
    applyTags.mutate({
      entry_ids: entryIds,
      tag_ids: ['tag-uuid-1', 'tag-uuid-2'],
      source: null,
      confidence: null,
      applied_context: null,
      instance_attributes: null,
      // TypeScript validates all fields!
    });
  };

  return <button onClick={handleApply}>Apply Tags</button>;
}
```

## Type Safety Benefits

### Input Validation
```typescript
// This works
useCoreQuery({
  type: 'libraries.list',
  input: {}
});

// TypeScript error: input must match ListLibrariesInput
useCoreQuery({
  type: 'libraries.list',
  input: { invalid_field: true }
});
```

### Output Types
```typescript
const { data: files } = useLibraryQuery({
  type: 'files.directory_listing',
  input: { path: '/' }
});

// TypeScript knows the exact type!
files?.entries  // File[]
files?.total_count  // number
files?.cursor  // string | null
```

### Wire Methods (Auto-Generated)
```typescript
import { WIRE_METHODS } from '@sd/ts-client';

// All wire methods are in the WIRE_METHODS constant
WIRE_METHODS.coreQueries['libraries.list']  // => 'query:libraries.list'
WIRE_METHODS.libraryActions['files.copy']  // => 'action:files.copy.input'
```

## Comparison: Swift vs TypeScript

### Swift (Auto-Generated API)
```swift
let client = SpacedriveClient(socketPath: "/tmp/sd.sock")

// Auto-generated methods
let libraries = try await client.libraries.list()
let files = try await client.files.directoryListing(input)
```

### TypeScript (React Query + Auto-Generated Types)
```typescript
const client = SpacedriveClient.fromSocket('/tmp/sd.sock');

// React Query hooks with auto-generated types
const { data: libraries } = useCoreQuery({ type: 'libraries.list', input: {} });
const { data: files } = useLibraryQuery({ type: 'files.directory_listing', input: { path: '/' } });
```

**Both are fully type-safe and auto-generated from the same Rust types!**

## Low-Level API (if needed)

```typescript
import { useSpacedriveClient } from '@sd/ts-client';

function CustomComponent() {
  const client = useSpacedriveClient();

  const handleCustomOperation = async () => {
    // Direct execute method if you need more control
    const result = await client.execute<LibraryCreateInput, LibraryCreateOutput>(
      'action:libraries.create.input',
      { name: 'Test', path: null }
    );
  };

  return <button onClick={handleCustomOperation}>Custom Op</button>;
}
```

## Event Subscription

```typescript
import { useSpacedriveClient } from '@sd/ts-client';
import { useEffect } from 'react';

function EventListener() {
  const client = useSpacedriveClient();

  useEffect(() => {
    const unlisten = client.subscribe((event) => {
      // event is fully typed as Event union!
      console.log('Received event:', event);
    });

    return () => unlisten.then(fn => fn());
  }, [client]);

  return null;
}
```

## Next Steps

1. Type generation working (cargo run --bin generate_typescript_types)
2. Client with simple execute method
3. React Query hooks (useCoreQuery, useLibraryQuery, mutations)
4. → Use in interface components
5. → Test with real Tauri app

All types are auto-generated - no manual maintenance needed!
