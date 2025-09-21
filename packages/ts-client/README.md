# Spacedrive TypeScript Client

A type-safe TypeScript client for interacting with the Spacedrive daemon.

## Features

- **Type Safety**: Full compile-time type checking with generated TypeScript types
- **Automatic Generation**: Types are automatically generated from Rust definitions
- **Simple API**: Three core methods: `executeQuery`, `executeAction`, and `subscribe`
- **Modern TypeScript**: Uses async/await and AsyncGenerator for clean asynchronous code

## Installation

### npm

```bash
npm install @spacedrive/client
```

### yarn

```bash
yarn add @spacedrive/client
```

## Usage

```typescript
import { SpacedriveClient } from '@spacedrive/client';

// Initialize the client
const client = new SpacedriveClient('/path/to/daemon.sock');

// Execute a query
const status = await client.executeQuery(
    {}, // CoreStatusQuery has no fields
    "query:core.status.v1"
);

// Execute an action
const result = await client.executeAction(
    { name: "My Library", path: null },
    "action:libraries.create.input.v1"
);

// Subscribe to events
for await (const event of client.subscribe(["JobProgress", "JobCompleted"])) {
    console.log("Received event:", event);
}
```

## Development

### Building

```bash
npm run build
```

### Testing

```bash
npm test
```

### Regenerating Types

After making changes to Rust types in the core:

1. Build the core to generate the schema:
   ```bash
   cd core && cargo build
   ```

2. Regenerate the TypeScript client:
   ```bash
   cd packages/ts-client
   ./generate_client.sh
   ```

### Requirements

- Node.js 18+
- TypeScript 5.0+
- quicktype (for type generation): `npm install -g quicktype`

## Architecture

The client uses a clean, modular architecture:

1. **Generated Types**: `types.ts` contains all the generated types from quicktype
2. **Client API**: `client.ts` provides the main SpacedriveClient class
3. **Transport Layer**: `transport.ts` handles communication with the daemon
4. **Index**: `index.ts` provides the public API exports

This separation ensures that the generated types don't pollute the main API and can be regenerated without affecting user code.

## Error Handling

The client provides structured error handling with the `SpacedriveError` class:

```typescript
try {
    const result = await client.executeQuery(query, method);
} catch (error) {
    if (error instanceof SpacedriveError) {
        console.error(`${error.type}: ${error.message}`);
    }
}
```

Error types include:
- `connection`: Connection to daemon failed
- `serialization`: Failed to serialize/deserialize data
- `daemon`: Error from the daemon itself
- `invalid_response`: Unexpected response format
