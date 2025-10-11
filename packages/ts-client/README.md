# @spacedrive/client

Type-safe TypeScript client for the Spacedrive daemon, automatically generated from Rust core types using Specta.

## Features

- **Fully Type-Safe** - All types generated directly from Rust core
- **Real-time Events** - Subscribe to daemon events with full type safety
- **Unix Domain Sockets** - Direct communication with daemon
- **Auto-Generated** - Types stay in sync with Rust core automatically
- **Rich Documentation** - JSDoc comments from Rust code

## Installation

```bash
npm install @spacedrive/client
```

## Quick Start

```typescript
import { SpacedriveClient } from '@spacedrive/client';

const client = new SpacedriveClient();

// Test connection
await client.ping();

// Get libraries (fully typed)
const libraries = await client.getLibraries();
console.log('Libraries:', libraries);

// Get jobs with status filtering
const jobs = await client.getJobs('running');
console.log('Running jobs:', jobs);

// Create a new library
const newLibrary = await client.createLibrary('My Photos', '/Users/me/Photos');
console.log('Created library:', newLibrary);
```

## Event Subscription

```typescript
// Subscribe to specific event types
await client.subscribe(['JobStarted', 'JobProgress', 'JobCompleted']);

// Listen for events (fully typed)
client.on('event', (event) => {
  switch (event) {
    case 'CoreStarted':
      console.log('Daemon started');
      break;
    default:
      if ('JobStarted' in event) {
        console.log(`Job started: ${event.JobStarted.job_type} (${event.JobStarted.job_id})`);
      } else if ('JobProgress' in event) {
        console.log(`Job progress: ${event.JobProgress.progress * 100}%`);
      } else if ('JobCompleted' in event) {
        console.log(`Job completed: ${event.JobCompleted.job_type}`);
        console.log('Output:', event.JobCompleted.output);
      }
      break;
  }
});

client.on('error', (error) => {
  console.error('Client error:', error);
});

client.on('disconnected', () => {
  console.log('Disconnected from daemon');
});
```

## Type Generation

Types are automatically generated from the Rust core using Specta. To regenerate types:

```bash
npm run generate-types
```

## API Reference

### Core Methods

- `ping()` - Test daemon connectivity
- `executeQuery<Q, R>(query: Q, method: string): Promise<R>` - Execute a query operation
- `executeAction<A, R>(action: A, method: string): Promise<R>` - Execute an action operation
- `subscribe(eventTypes?: string[]): Promise<void>` - Subscribe to daemon events

### Convenience Methods

- `getLibraries(includeStats?: boolean): Promise<LibraryInfo[]>` - Get all libraries
- `createLibrary(name: string, path?: string): Promise<LibraryCreateOutput>` - Create a new library
- `getJobs(status?: JobStatus): Promise<JobListOutput>` - Get job list with optional filtering

### Event Types

All event types are fully typed TypeScript unions:

- `Event` - Main event union type
- `JobOutput` - Job completion output (adjacently tagged)
- `JobStatus` - Job status enum (`"queued" | "running" | "completed" | ...`)
- `FileOperation` - File operation types
- `Progress` - Progress information with multiple formats

## Architecture

This client uses the same architecture as the Swift client:

1. **JSON API Layer** - Communicates via JSON instead of bincode for external clients
2. **Unix Domain Sockets** - Direct, efficient communication with daemon
3. **Type Generation** - Rust types → TypeScript via Specta
4. **Event Streaming** - Real-time event subscription with line-delimited JSON

## Development

```bash
# Install dependencies
npm install

# Build the client
npm run build

# Run tests
npm test

# Watch mode for development
npm run dev
```
