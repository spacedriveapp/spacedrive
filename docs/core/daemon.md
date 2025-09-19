# Daemon Architecture

Spacedrive uses a **daemon-client architecture** where a single daemon process manages the core functionality and multiple client applications (CLI, GraphQL server, desktop app) connect to it via Unix domain sockets.

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   CLI Client    │    │  GraphQL Server │    │  Desktop App    │
│                 │    │                 │    │                 │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          │         Unix Domain Sockets (UDS)          │
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │    Spacedrive Daemon    │
                    │                         │
                    │  ┌─────────────────┐    │
                    │  │   RPC Server    │    │
                    │  │  (JSON-over-UDS)│    │
                    │  └─────────────────┘    │
                    │                         │
                    │  ┌─────────────────┐    │
                    │  │ Core Instance   │    │
                    │  │   Manager       │    │
                    │  └─────────────────┘    │
                    │                         │
                    │  ┌─────────────────┐    │
                    │  │  Event System   │    │
                    │  │   (Streaming)   │    │
                    │  └─────────────────┘    │
                    └─────────────────────────┘
```

## Daemon Process

### Entry Point
- **Location**: `core/src/bin/daemon.rs`
- **Purpose**: Starts the default daemon server
- **Key Function**: `start_default_server(socket_path, data_dir, enable_networking)`

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = sd_core::config::default_data_dir()?;
    let socket_path = data_dir.join("daemon/daemon.sock");

    sd_core::infra::daemon::bootstrap::start_default_server(socket_path, data_dir, false).await
}
```

### Bootstrap Process
- **Location**: `core/src/infra/daemon/bootstrap.rs`
- **Responsibilities**:
  - Initialize tracing with file logging to `{data_dir}/logs/daemon.log`
  - Create `CoreInstanceManager` for managing multiple core instances
  - Set up event streaming system
  - Start the RPC server

### RPC Server
- **Location**: `core/src/infra/daemon/rpc.rs`
- **Protocol**: JSON-over-Unix Domain Sockets
- **Message Types**: `DaemonRequest` and `DaemonResponse`
- **Features**:
  - Request/response handling for actions and queries
  - Real-time event streaming
  - Connection management for multiple clients

### Socket Management
- **Default Socket**: `{data_dir}/daemon/daemon.sock`
- **Named Instances**: `{data_dir}/daemon/daemon-{instance}.sock`
- **Security**: Unix domain sockets provide local-only access
- **Cleanup**: Automatic socket cleanup on daemon shutdown

### Instance Management
- **Component**: `CoreInstanceManager`
- **Purpose**: Manages multiple core instances within a single daemon
- **Benefits**: Resource sharing, centralized management, instance isolation

## Client Applications

### CoreClient
All client applications use `CoreClient` from `core/src/client/mod.rs` to communicate with the daemon.

```rust
use sd_core::client::CoreClient;

let socket_path = data_dir.join("daemon/daemon.sock");
let client = CoreClient::new(socket_path);
```

### CLI Application
- **Location**: `apps/cli/src/main.rs`
- **Connection**: Direct socket connection for command execution
- **Features**: Start/stop daemon, execute operations, stream logs

```rust
// CLI connection example
let socket_path = if let Some(inst) = &instance {
    data_dir.join("daemon").join(format!("daemon-{}.sock", inst))
} else {
    data_dir.join("daemon/daemon.sock")
};
let client = CoreClient::new(socket_path);
```

### GraphQL Server
- **Location**: `apps/graphql/src/main.rs`
- **Connection**: Persistent connection for serving GraphQL API
- **Features**: Query/mutation handling, real-time subscriptions

```rust
// GraphQL connection example
let socket = data_dir.join("daemon/daemon.sock");
let state = Arc::new(AppState {
    core: CoreClient::new(socket),
});
```

### Desktop Application
- **Integration**: Via Tauri with Rust backend
- **Connection**: Similar to CLI/GraphQL but with desktop-specific features
- **Features**: Full GUI, file system integration, system tray

## Communication Protocol

### Transport Layer
- **Protocol**: Unix Domain Sockets (UDS)
- **Benefits**:
  - High performance (no network overhead)
  - Security (local-only access)
  - Reliability (kernel-managed)
  - Cross-platform support

### Message Format
- **Protocol**: JSON-over-UDS for message framing
- **Serialization**: `bincode` for efficient binary serialization of payloads
- **Message Types**:
  - `DaemonRequest::Action { method, payload }`
  - `DaemonRequest::Query { method, payload }`
  - `DaemonRequest::Subscribe { event_types, filter }`
  - `DaemonResponse::Ok(bytes)`
  - `DaemonResponse::Error(error)`

### Wire Trait System
All operations implement the `Wire` trait for type-safe communication:

```rust
pub trait Wire {
    const METHOD: &'static str;
}
```

### Registration Macros
Operations are registered using compile-time macros:

```rust
// Query registration
crate::register_query!(NetworkStatusQuery, "network.status");
// Generates method: "query:network.status.v1"

// Library action registration
crate::register_library_action!(FileCopyAction, "files.copy");
// Generates method: "action:files.copy.input.v1"

// Core action registration
crate::register_core_action!(LibraryCreateAction, "libraries.create");
// Generates method: "action:libraries.create.input.v1"
```

### Registry System
- **Location**: `core/src/ops/registry.rs`
- **Mechanism**: Uses `inventory` crate for compile-time registration
- **Global Maps**:
  - `QUERIES`: HashMap of query method strings to handler functions
  - `ACTIONS`: HashMap of action method strings to handler functions
- **Handler Functions**: Generic handlers that decode payloads, execute operations, and encode results

## Event Streaming

### Real-time Events
The daemon supports real-time event streaming for:
- Log messages with filtering by level and target
- Job progress updates and status changes
- Library and location changes
- Network device discovery and pairing events

### Event Subscription
```rust
let mut event_stream = client.subscribe_events(
    vec!["LogMessage".to_string(), "JobUpdate".to_string()],
    Some(EventFilter {
        library_id: Some(library_id),
        job_id: None,
    })
).await?;

while let Some(event) = event_stream.recv().await {
    match event {
        Event::LogMessage { message, level, .. } => {
            println!("[{}] {}", level, message);
        }
        Event::JobUpdate { job_id, status, .. } => {
            println!("Job {} status: {:?}", job_id, status);
        }
    }
}
```

## Development Workflow

### Starting the Daemon
```bash
# Start daemon directly
cargo run --bin daemon

# Or via CLI
cargo run --bin spacedrive -- start
```

### Connecting Clients
```bash
# CLI operations
cargo run --bin spacedrive -- status
cargo run --bin spacedrive -- library list

# GraphQL server
cd apps/graphql && cargo run
# Then visit http://localhost:8080/graphql
```

### Multiple Instances
```bash
# Start named instance
cargo run --bin spacedrive -- --instance dev start

# Connect to named instance
cargo run --bin spacedrive -- --instance dev status
```

## Error Handling

### Connection Errors
- **Socket not found**: Daemon not running
- **Permission denied**: Socket permissions issue
- **Connection refused**: Daemon not accepting connections

### Protocol Errors
- **Method not found**: Operation not registered
- **Deserialization error**: Payload format mismatch
- **Execution error**: Operation-specific errors

### Recovery Strategies
- **Automatic retry**: For transient connection issues
- **Graceful degradation**: Fallback to local operations where possible
- **Error propagation**: Clear error messages to client applications

## Security Considerations

### Access Control
- Unix domain sockets provide process-level access control
- Socket file permissions restrict daemon access
- No network exposure by default

### Data Protection
- All communication stays on local machine
- Binary serialization prevents casual inspection
- Structured logging avoids sensitive data leakage

### Instance Isolation
- Named instances provide logical separation
- Each instance has its own socket and data directory
- Cross-instance communication requires explicit configuration

## Performance Characteristics

### Throughput
- Unix domain sockets: ~10-100x faster than TCP loopback
- Binary serialization: Minimal overhead vs JSON
- Connection pooling: Reuse connections for multiple operations

### Latency
- Local IPC: Sub-millisecond response times
- Event streaming: Real-time delivery with minimal buffering
- Batch operations: Efficient for bulk data transfer

### Resource Usage
- Single daemon process: Shared memory and file handles
- Client connections: Minimal per-connection overhead
- Event subscriptions: Efficient filtering and delivery

## Troubleshooting

### Common Issues
1. **Daemon won't start**: Check data directory permissions
2. **Client can't connect**: Verify socket path and daemon status
3. **Operations fail**: Check daemon logs for detailed errors
4. **Events not received**: Verify subscription filters and event types

### Debugging Tools
```bash
# Check daemon status
cargo run --bin spacedrive -- status

# View daemon logs
tail -f ~/.local/share/spacedrive/logs/daemon.log

# Test socket connection
nc -U ~/.local/share/spacedrive/daemon/daemon.sock
```

### Log Analysis
- **Structured logging**: Use `tracing` fields for filtering
- **Log levels**: DEBUG for development, INFO for production
- **Event correlation**: Track operations across client-daemon boundary
