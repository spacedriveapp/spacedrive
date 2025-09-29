# iOS Embedded Core Design

## Overview

This document outlines the design for integrating the Spacedrive Rust core as an embedded binary within the iOS application, providing a unified API surface between macOS (daemon-based) and iOS (embedded) deployments.

## Rules for development
- NO unique handling of queries. The core is designed to use the Wire trait to automatically route queries and actions
- No fake code or stub implementations, if you need to add something but it will take too long, stop and ask.

## Current Architecture Analysis

### macOS Architecture (Daemon-based)
- **Transport**: Unix domain socket communication to separate daemon process
- **Client**: `SpacedriveClient` with socket-based transport
- **API**: Generated `SpacedriveAPI.swift` with type-safe method calls
- **Protocol**: JSON-RPC over socket with `client.execute()` abstraction
- **Routing**: Daemon routes requests through core registry system (`CORE_QUERIES`, `LIBRARY_QUERIES`, etc.)

### iOS Architecture (Current State)
- **Transport**: Direct FFI calls to simple Rust functions
- **API**: Manual FFI bridge with basic request/response
- **Protocol**: Ad-hoc JSON strings with manual parsing
- **Limitations**: No structured API, no type safety, no code reuse with macOS

## Design Goals

1. **Code Reuse**: Maximize sharing of Swift types and API surface between platforms
2. **Type Safety**: Maintain compile-time type checking for all API calls
3. **Maintainability**: Single source of truth for API definitions
4. **Performance**: Efficient embedded execution without daemon overhead
5. **Event Consistency**: Unified event streaming across platforms

## Proposed Architecture

### 1. Platform-Specific Client Implementation

Create platform-specific implementations of `SpacedriveClient` that expose identical APIs:

```swift
// packages/swift-client/Sources/SpacedriveClient/SpacedriveClient.swift

#if os(macOS)
public class SpacedriveClient {
    // Current socket-based implementation
    private let socketPath: String

    internal func execute<Request: Codable, Response: Codable>(
        _ requestPayload: Request,
        method: String,
        responseType: Response.Type,
        libraryId: String? = nil
    ) async throws -> Response {
        // Build daemon request and send via socket
        // Current implementation remains unchanged
    }
}

#elseif os(iOS)
public class SpacedriveClient {
    private let core: SDIOSCore
    private let dataDirectory: String

    public init(dataDirectory: String) async throws {
        self.dataDirectory = dataDirectory
        self.core = SDIOSCore()

        guard core.initialize(dataDirectory: dataDirectory) else {
            throw SpacedriveError.connectionFailed("Failed to initialize embedded core")
        }
    }

    internal func execute<Request: Codable, Response: Codable>(
        _ requestPayload: Request,
        method: String,
        responseType: Response.Type,
        libraryId: String? = nil
    ) async throws -> Response {
        // Build JSON-RPC request
        let jsonRpc = JSONRPCRequest(
            method: method,
            params: JSONRPCParams(
                input: requestPayload,
                library_id: libraryId
            ),
            id: UUID().uuidString
        )

        // Send via FFI
        let requestJson = try JSONEncoder().encode(jsonRpc)
        let responseJson = try await core.sendMessage(
            String(data: requestJson, encoding: .utf8)!,
            dataDirectory: dataDirectory
        )

        // Parse JSON-RPC response
        let responseData = responseJson.data(using: .utf8)!
        let jsonRpcResponse = try JSONDecoder().decode(JSONRPCResponse<Response>.self, from: responseData)

        if let result = jsonRpcResponse.result {
            return result
        } else if let error = jsonRpcResponse.error {
            throw SpacedriveError.daemonError(error.message)
        } else {
            throw SpacedriveError.invalidResponse("No result or error in response")
        }
    }
}
#endif
```

### 2. Shared API Surface

Both platforms use identical generated API namespaces:

```swift
// Same for both macOS and iOS
public lazy var core = CoreAPI(client: self)
public lazy var libraries = LibrariesAPI(client: self)
public lazy var jobs = JobsAPI(client: self)
public lazy var locations = LocationsAPI(client: self)
public lazy var media = MediaAPI(client: self)
public lazy var network = NetworkAPI(client: self)
public lazy var search = SearchAPI(client: self)
public lazy var tags = TagsAPI(client: self)
public lazy var volumes = VolumesAPI(client: self)
public lazy var files = FilesAPI(client: self)
```

### 3. JSON-RPC Protocol

#### Request Format
```json
{
    "jsonrpc": "2.0",
    "method": "query:libraries.list.v1",
    "params": {
        "input": {
            "include_stats": true
        },
        "library_id": null
    },
    "id": "uuid-string"
}
```

#### Response Format
```json
{
    "jsonrpc": "2.0",
    "id": "uuid-string",
    "result": [
        {
            "id": "library-uuid",
            "name": "My Library",
            "path": "/path/to/library",
            "stats": { ... }
        }
    ]
}
```

#### Error Format
```json
{
    "jsonrpc": "2.0",
    "id": "uuid-string",
    "error": {
        "code": -32603,
        "message": "Unknown query method"
    }
}
```

### 4. Rust FFI JSON-RPC Tunnel

Transform the Rust FFI layer into a generic JSON-RPC tunnel:

```rust
// apps/ios/sd-ios-core/src/lib.rs

use sd_core::{
    context::CoreContext,
    infra::api::{dispatcher::ApiDispatcher, SessionContext},
    ops::registry::{CORE_QUERIES, LIBRARY_QUERIES, CORE_ACTIONS, LIBRARY_ACTIONS},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;

// Global state
static RUNTIME: OnceCell<tokio::runtime::Runtime> = OnceCell::const_new();
static CORE_CONTEXT: OnceCell<Arc<CoreContext>> = OnceCell::const_new();
static API_DISPATCHER: OnceCell<ApiDispatcher> = OnceCell::const_new();

#[derive(Serialize, Deserialize)]
struct JSONRPCRequest {
    jsonrpc: String, // "2.0"
    method: String,  // "query:libraries.list.v1"
    params: JSONRPCParams,
    id: String,
}

#[derive(Serialize, Deserialize)]
struct JSONRPCParams {
    input: serde_json::Value,
    library_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct JSONRPCResponse {
    jsonrpc: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JSONRPCError>,
}

#[derive(Serialize, Deserialize)]
struct JSONRPCError {
    code: i32,
    message: String,
}

#[no_mangle]
pub extern "C" fn initialize_core(data_dir: *const std::os::raw::c_char) -> std::os::raw::c_int {
    let data_dir_str = unsafe { CStr::from_ptr(data_dir).to_string_lossy().to_string() };

    println!("🔄 Initializing embedded Spacedrive core with data dir: {}", data_dir_str);

    // Initialize Tokio runtime
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("❌ Failed to create Tokio runtime: {}", e);
            return -1;
        }
    };

    // Initialize core context
    let context = rt.block_on(async {
        let config = sd_core::config::CoreConfig {
            data_directory: PathBuf::from(data_dir_str),
            // Additional config parameters
        };

        CoreContext::new(config).await
    });

    let context = match context {
        Ok(ctx) => Arc::new(ctx),
        Err(e) => {
            println!("❌ Failed to initialize core context: {}", e);
            return -1;
        }
    };

    let dispatcher = ApiDispatcher::new(Arc::clone(&context));

    // Store global state
    let _ = RUNTIME.set(rt);
    let _ = CORE_CONTEXT.set(context);
    let _ = API_DISPATCHER.set(dispatcher);

    println!("✅ Embedded core initialized successfully");
    0 // Success
}

#[no_mangle]
pub extern "C" fn handle_core_msg(
    query: *const std::os::raw::c_char,
    _data_dir: *const std::os::raw::c_char,
    callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char),
    callback_data: *mut std::os::raw::c_void,
) {
    let query_str = unsafe { CStr::from_ptr(query).to_string_lossy().to_string() };

    println!("📡 Received JSON-RPC request: {}", query_str);

    let runtime = RUNTIME.get().unwrap();
    let context = CORE_CONTEXT.get().unwrap();
    let dispatcher = API_DISPATCHER.get().unwrap();

    runtime.spawn(async move {
        let response = handle_json_rpc_request(query_str, context, dispatcher).await;
        let response_json = serde_json::to_string(&response)
            .unwrap_or_else(|_| r#"{"jsonrpc":"2.0","id":"","error":{"code":-32603,"message":"Response serialization failed"}}"#.to_string());

        println!("📡 Sending JSON-RPC response: {}", response_json);

        let response_cstring = CString::new(response_json).unwrap();
        callback(callback_data, response_cstring.as_ptr());
    });
}

async fn handle_json_rpc_request(
    request_json: String,
    context: &Arc<CoreContext>,
    dispatcher: &ApiDispatcher,
) -> JSONRPCResponse {
    // Parse JSON-RPC request
    let request: JSONRPCRequest = match serde_json::from_str(&request_json) {
        Ok(req) => req,
        Err(e) => {
            return JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: "".to_string(),
                result: None,
                error: Some(JSONRPCError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            };
        }
    };

    // Extract method components
    let (scope, method_name) = match parse_method(&request.method) {
        Ok(parsed) => parsed,
        Err(e) => {
            return JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JSONRPCError {
                    code: -32601,
                    message: e,
                }),
            };
        }
    };

    // Create session context
    let mut session = match dispatcher.create_base_session() {
        Ok(s) => s,
        Err(e) => {
            return JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JSONRPCError {
                    code: -32603,
                    message: format!("Failed to create session: {}", e),
                }),
            };
        }
    };

    // Set library context if provided
    if let Some(library_id) = request.params.library_id {
        if let Ok(uuid) = Uuid::parse_str(&library_id) {
            session = session.with_library(uuid);
        }
    }

    // Route to appropriate handler
    let result = match scope.as_str() {
        "query:core" => {
            if let Some(handler) = CORE_QUERIES.get(method_name.as_str()) {
                handler(Arc::clone(context), session, request.params.input).await
            } else {
                Err(format!("Unknown core query: {}", method_name))
            }
        }
        "query:library" => {
            if let Some(handler) = LIBRARY_QUERIES.get(method_name.as_str()) {
                handler(Arc::clone(context), session, request.params.input).await
            } else {
                Err(format!("Unknown library query: {}", method_name))
            }
        }
        "action:core" => {
            if let Some(handler) = CORE_ACTIONS.get(method_name.as_str()) {
                handler(Arc::clone(context), request.params.input).await
            } else {
                Err(format!("Unknown core action: {}", method_name))
            }
        }
        "action:library" => {
            if let Some(handler) = LIBRARY_ACTIONS.get(method_name.as_str()) {
                handler(Arc::clone(context), session, request.params.input).await
            } else {
                Err(format!("Unknown library action: {}", method_name))
            }
        }
        _ => Err(format!("Invalid method scope: {}", scope))
    };

    // Build response
    match result {
        Ok(data) => JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(data),
            error: None,
        },
        Err(err) => JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(JSONRPCError {
                code: -32603,
                message: err,
            }),
        }
    }
}

fn parse_method(method: &str) -> Result<(String, String), String> {
    // Parse "query:libraries.list.v1" into ("query", "libraries.list.v1")
    if let Some((scope, method_name)) = method.split_once(':') {
        let full_scope = if method_name.starts_with("core.") {
            format!("{}:core", scope)
        } else {
            format!("{}:library", scope)
        };
        Ok((full_scope, method_name.to_string()))
    } else {
        Err(format!("Invalid method format: {}", method))
    }
}

#[no_mangle]
pub extern "C" fn spawn_core_event_listener(
    callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char),
    callback_data: *mut std::os::raw::c_void,
) {
    println!("📡 Starting core event listener...");

    let context = CORE_CONTEXT.get().unwrap();
    let runtime = RUNTIME.get().unwrap();

    let mut event_subscriber = context.events.subscribe();

    runtime.spawn(async move {
        while let Ok(event) = event_subscriber.recv().await {
            let event_json = match serde_json::to_string(&event) {
                Ok(json) => json,
                Err(e) => {
                    println!("❌ Failed to serialize event: {}", e);
                    continue;
                }
            };

            println!("📡 Broadcasting event: {}", event_json);

            let event_cstring = CString::new(event_json).unwrap();
            callback(callback_data, event_cstring.as_ptr());
        }
    });
}

#[no_mangle]
pub extern "C" fn shutdown_core() {
    println!("🔄 Shutting down embedded core...");

    // Clean up global state
    // Note: OnceCell doesn't have a reset method, so we rely on process termination

    println!("✅ Core shut down");
}
```

### 5. Event Streaming

Events work identically on both platforms using the existing core event system:

```swift
// Both macOS and iOS
public func subscribe(to eventTypes: [String] = []) -> AsyncThrowingStream<Event, Error> {
    AsyncThrowingStream { continuation in
        Task {
            #if os(macOS)
            // Existing socket-based event streaming
            #elseif os(iOS)
            // FFI-based event streaming
            core.startEventListener { eventJson in
                do {
                    let eventData = eventJson.data(using: .utf8)!
                    let event = try JSONDecoder().decode(Event.self, from: eventData)
                    continuation.yield(event)
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            #endif
        }
    }
}
```

### 6. iOS App Integration

Update the iOS app to use the new unified client:

```swift
// In EmbeddedCoreManager.swift
@MainActor
class EmbeddedCoreManager: ObservableObject {
    @Published var isInitialized = false
    @Published var libraries: [LibraryInfo] = []
    @Published var jobs: [JobInfo] = []

    private var client: SpacedriveClient?
    private var eventTask: Task<Void, Never>?

    func initializeCore() async throws {
        guard !isInitialized else { return }

        let dataDir = getAppDataDirectory()

        // Initialize embedded client (same API as macOS)
        client = try await SpacedriveClient(dataDirectory: dataDir)

        isInitialized = true

        // Fetch initial data using type-safe APIs
        await fetchLibraries()
        await subscribeToEvents()
    }

    private func fetchLibraries() async {
        guard let client = client else { return }

        do {
            libraries = try await client.libraries.list(ListLibrariesInput(includeStats: true))
        } catch {
            print("Failed to fetch libraries: \(error)")
        }
    }

    private func subscribeToEvents() async {
        guard let client = client else { return }

        eventTask = Task {
            do {
                for try await event in client.subscribe() {
                    await handleEvent(event)
                }
            } catch {
                print("Event subscription failed: \(error)")
            }
        }
    }

    private func handleEvent(_ event: Event) async {
        // Same event handling as macOS
        switch event {
        case .jobProgress(let jobProgressEvent):
            // Update job progress
            break
        case .libraryCreated:
            await fetchLibraries()
            break
        // ... other events
        default:
            break
        }
    }
}
```

## Implementation Benefits

### 1. **Maximum Code Reuse**
- `SpacedriveTypes.swift` and `SpacedriveAPI.swift` are identical on both platforms
- Business logic, event handling, and UI components can be shared
- Only transport layer implementation differs

### 2. **Type Safety**
- Compile-time verification of all API calls
- No manual JSON parsing in application code
- Automatic serialization/deserialization

### 3. **Maintainability**
- Single source of truth for API definitions
- Changes to core API automatically propagate to both platforms
- No platform-specific API code to maintain

### 4. **Performance**
- Direct in-process core execution on iOS (no daemon overhead)
- Efficient FFI calls with minimal serialization overhead
- Full Rust performance benefits

### 5. **Developer Experience**
- Identical APIs between platforms
- Rich type information and autocompletion
- Consistent error handling

## Implementation Steps

1. **Phase 1: Swift Client Infrastructure**
   - Create platform-specific `SpacedriveClient` implementations
   - Add JSON-RPC protocol types
   - Update package structure for conditional compilation

2. **Phase 2: Rust FFI Tunnel**
   - Implement JSON-RPC parsing and routing
   - Integrate with existing core registry system
   - Add proper error handling and logging

3. **Phase 3: Event Streaming**
   - Implement FFI-based event streaming
   - Ensure event format compatibility with macOS
   - Add event filtering and subscription management

4. **Phase 4: iOS App Integration**
   - Update `EmbeddedCoreManager` to use new client
   - Replace manual FFI calls with type-safe API calls
   - Test all existing functionality

5. **Phase 5: Testing & Validation**
   - Comprehensive API testing on both platforms
   - Performance benchmarking
   - Event streaming validation

## Migration Path

### Backward Compatibility
- Existing iOS app continues to work during migration
- Gradual migration of features to new client
- Ability to test both implementations side-by-side

### Rollback Strategy
- Keep existing FFI interface until new implementation is stable
- Feature flags for enabling/disabling new client
- Clear rollback procedures for each phase

## Success Criteria

1. **Functional Parity**: iOS app has same capabilities as macOS app
2. **Performance**: Embedded core performs as well or better than daemon
3. **Code Quality**: Reduced code duplication and improved maintainability
4. **Developer Experience**: Easier to add new features across platforms
5. **Stability**: Robust error handling and graceful failure modes

## Future Considerations

### Cross-Platform Expansion
- This architecture can be extended to other embedded platforms (Android, Windows)
- Same JSON-RPC tunnel approach works for any FFI-based integration

### API Evolution
- Generated types and API surface evolve automatically
- JSON-RPC provides versioning and backward compatibility
- Clear migration path for breaking changes

### Performance Optimization
- Binary protocol can replace JSON-RPC if needed
- Streaming interfaces for large data transfers
- Memory-mapped communication for high-frequency operations

This design provides a solid foundation for unified Spacedrive client development while maintaining platform-specific optimizations where needed.
