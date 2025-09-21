# Spacedrive Swift Client

A type-safe Swift client for interacting with the Spacedrive daemon.

## Features

- **Type Safety**: Full compile-time type checking with generated Swift types
- **Automatic Generation**: Types are automatically generated from Rust definitions
- **Simple API**: Three core methods: `executeQuery`, `executeAction`, and `subscribe`
- **Modern Swift**: Uses async/await and AsyncStream for clean asynchronous code

## Installation

### Swift Package Manager

Add this to your `Package.swift`:

```swift
dependencies: [
    .package(path: "path/to/spacedrive/packages/swift-client")
]
```

## Usage

```swift
import SpacedriveClient

// Initialize the client
let client = SpacedriveClient(socketPath: "/path/to/daemon.sock")

// Execute a query
let status = try await client.executeQuery(
    CoreStatusQuery(),
    method: "query:core.status.v1",
    responseType: CoreStatus.self
)

// Execute an action
let result = try await client.executeAction(
    LibraryCreateInput(name: "My Library"),
    method: "action:libraries.create.input.v1",
    responseType: LibraryCreateOutput.self
)

// Subscribe to events
for await event in client.subscribe(to: ["JobProgress", "JobCompleted"]) {
    print("Received event: \(event)")
}
```

## Development

### Regenerating Types

After making changes to Rust types in the core:

1. Build the core to generate the schema:
   ```bash
   cd core && cargo build
   ```

2. Regenerate the Swift client:
   ```bash
   cd packages/swift-client
   ./generate_client.sh
   ```

### Requirements

- Swift 5.9+
- macOS 13+ or iOS 16+
- quicktype (for type generation): `npm install -g quicktype`

## Architecture

The client uses a two-layer architecture:

1. **Generated Types Layer**: `types.swift` contains all the generated types from quicktype
2. **Client API Layer**: `SpacedriveClient.swift` provides the clean, user-facing API

This separation ensures that the generated types don't pollute the main API and can be regenerated without affecting user code.
