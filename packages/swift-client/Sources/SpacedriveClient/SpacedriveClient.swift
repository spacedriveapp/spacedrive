import Foundation
import Darwin

/// Main client for interacting with the Spacedrive daemon
///
/// This client provides a clean, type-safe interface for executing queries,
/// actions, and subscribing to events from the Spacedrive core.
public class SpacedriveClient {
    private let socketPath: String

    /// Initialize a new Spacedrive client
    /// - Parameter socketPath: Path to the Unix domain socket for the daemon
    public init(socketPath: String) {
        self.socketPath = socketPath
    }

    // MARK: - Core API Methods

    /// Execute a query operation
    /// - Parameters:
    ///   - query: The query input (can be empty struct for parameterless queries)
    ///   - method: The method identifier (e.g., "query:core.status.v1")
    ///   - responseType: The expected response type
    /// - Returns: The query result
    public func executeQuery<Q: Codable, R: Codable>(
        _ query: Q,
        method: String,
        responseType: R.Type
    ) async throws -> R {
        // 1. Encode input to JSON
        let queryData: Data
        do {
            queryData = try JSONEncoder().encode(query)
        } catch {
            throw SpacedriveError.serializationError("Failed to encode query: \(error)")
        }

        // 2. Create daemon request
        let request = DaemonRequest.query(method: method, payload: queryData)

        // 3. Send to daemon and get response
        let response = try await sendRequest(request)

        // 4. Handle response
        switch response {
        case .ok(let data):
            do {
                return try JSONDecoder().decode(responseType, from: data)
            } catch {
                throw SpacedriveError.serializationError("Failed to decode response: \(error)")
            }
        case .error(let error):
            throw SpacedriveError.daemonError(error)
        case .pong, .event, .subscribed, .unsubscribed:
            throw SpacedriveError.invalidResponse("Unexpected response to query")
        }
    }

    /// Execute an action operation
    /// - Parameters:
    ///   - action: The action input
    ///   - method: The method identifier (e.g., "action:libraries.create.input.v1")
    ///   - responseType: The expected response type
    /// - Returns: The action result
    public func executeAction<A: Codable, R: Codable>(
        _ action: A,
        method: String,
        responseType: R.Type
    ) async throws -> R {
        // 1. Encode input to JSON
        let actionData: Data
        do {
            actionData = try JSONEncoder().encode(action)
        } catch {
            throw SpacedriveError.serializationError("Failed to encode action: \(error)")
        }

        // 2. Create daemon request
        let request = DaemonRequest.action(method: method, payload: actionData)

        // 3. Send to daemon and get response
        let response = try await sendRequest(request)

        // 4. Handle response
        switch response {
        case .ok(let data):
            do {
                return try JSONDecoder().decode(responseType, from: data)
            } catch {
                throw SpacedriveError.serializationError("Failed to decode response: \(error)")
            }
        case .error(let error):
            throw SpacedriveError.daemonError(error)
        case .pong, .event, .subscribed, .unsubscribed:
            throw SpacedriveError.invalidResponse("Unexpected response to action")
        }
    }

    /// Subscribe to events from the daemon
    /// - Parameter eventTypes: Array of event type names to subscribe to
    /// - Returns: An async stream of events
    public func subscribe(
        to eventTypes: [String] = []
    ) -> AsyncThrowingStream<SpacedriveEvent, Error> {
        AsyncThrowingStream { continuation in
            Task {
                do {
                    // 1. Establish persistent connection to daemon
                    let connection = try await createConnection()

                    // 2. Send subscription request
                    let request = DaemonRequest.subscribe(eventTypes: eventTypes, filter: nil)
                    try await sendRequestOverConnection(request, connection: connection)

                    // 3. Stream events as they arrive
                    while true {
                        let response = try await readResponseFromConnection(connection)
                        if case .event(let eventData) = response {
                            let event = try JSONDecoder().decode(SpacedriveEvent.self, from: eventData)
                            continuation.yield(event)
                        }
                    }
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    // MARK: - Private Implementation

    /// Send a request to the daemon and wait for response
    private func sendRequest(_ request: DaemonRequest) async throws -> DaemonResponse {
        let connection = try await createConnection()
        defer { close(connection) }

        try await sendRequestOverConnection(request, connection: connection)
        return try await readResponseFromConnection(connection)
    }

    /// Create a Unix domain socket connection to the daemon
    private func createConnection() async throws -> Int32 {
        print("🔗 Creating BSD socket connection to: \(socketPath)")

        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async {
                do {
                    // Create socket
                    let socketFD = socket(AF_UNIX, SOCK_STREAM, 0)
                    guard socketFD != -1 else {
                        continuation.resume(throwing: SpacedriveError.connectionFailed("Failed to create socket"))
                        return
                    }

                    // Set up address
                    var addr = sockaddr_un()
                    addr.sun_family = sa_family_t(AF_UNIX)
                    let pathBytes = self.socketPath.utf8CString
                    let pathSize = MemoryLayout.size(ofValue: addr.sun_path)
                    guard pathBytes.count <= pathSize else {
                        close(socketFD)
                        continuation.resume(throwing: SpacedriveError.connectionFailed("Socket path too long"))
                        return
                    }

                    // Copy path bytes to sun_path
                    withUnsafeMutablePointer(to: &addr.sun_path.0) { pathPtr in
                        for (index, byte) in pathBytes.enumerated() {
                            if index >= pathSize { break }
                            pathPtr.advanced(by: index).pointee = byte
                        }
                    }

                    // Connect
                    let connectResult = withUnsafePointer(to: &addr) { ptr in
                        ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                            Darwin.connect(socketFD, sockaddrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
                        }
                    }

                    guard connectResult == 0 else {
                        let errorMsg = String(cString: strerror(errno))
                        close(socketFD)
                        continuation.resume(throwing: SpacedriveError.connectionFailed("Failed to connect: \(errorMsg)"))
                        return
                    }

                    print("✅ BSD socket connected successfully!")
                    continuation.resume(returning: socketFD)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    /// Send a request over an existing connection
    private func sendRequestOverConnection(_ request: DaemonRequest, connection: Int32) async throws {
        let requestData = try JSONEncoder().encode(request)
        let requestLine = requestData + Data("\n".utf8)

        print("📤 Sending request: \(String(data: requestData, encoding: .utf8) ?? "invalid")")

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            DispatchQueue.global().async {
                let sendResult = requestLine.withUnsafeBytes { bytes in
                    send(connection, bytes.bindMemory(to: UInt8.self).baseAddress, requestLine.count, 0)
                }

                if sendResult == -1 {
                    let errorMsg = String(cString: strerror(errno))
                    continuation.resume(throwing: SpacedriveError.connectionFailed("Send failed: \(errorMsg)"))
                } else {
                    print("📤 Sent \(sendResult) bytes")
                    continuation.resume()
                }
            }
        }
    }

    /// Read a response from an existing connection
    private func readResponseFromConnection(_ connection: Int32) async throws -> DaemonResponse {
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async {
                var buffer = [UInt8](repeating: 0, count: 4096)
                let readResult = recv(connection, &buffer, buffer.count, 0)

                guard readResult > 0 else {
                    let errorMsg = String(cString: strerror(errno))
                    continuation.resume(throwing: SpacedriveError.connectionFailed("Receive failed: \(errorMsg)"))
                    return
                }

                let responseData = Data(buffer.prefix(readResult))
                print("📥 Received \(readResult) bytes: \(String(data: responseData, encoding: .utf8) ?? "invalid")")

                do {
                    // Find the newline delimiter and parse JSON
                    if let responseString = String(data: responseData, encoding: .utf8) {
                        let lines = responseString.components(separatedBy: .newlines).filter { !$0.isEmpty }
                        if let firstLine = lines.first {
                            let lineData = Data(firstLine.utf8)
                            let response = try JSONDecoder().decode(DaemonResponse.self, from: lineData)
                            continuation.resume(returning: response)
                        } else {
                            continuation.resume(throwing: SpacedriveError.invalidResponse("No valid response line"))
                        }
                    } else {
                        continuation.resume(throwing: SpacedriveError.invalidResponse("Invalid UTF-8 response"))
                    }
                } catch {
                    continuation.resume(throwing: SpacedriveError.serializationError("Failed to decode response: \(error)"))
                }
            }
        }
    }
}

// MARK: - Daemon Protocol Types

/// Request types that match the Rust daemon protocol
internal enum DaemonRequest: Codable {
    case ping
    case action(method: String, payload: Data)
    case query(method: String, payload: Data)
    case subscribe(eventTypes: [String], filter: EventFilter?)
    case unsubscribe
    case shutdown

    enum CodingKeys: String, CodingKey {
        case ping = "Ping"
        case action = "Action"
        case query = "Query"
        case subscribe = "Subscribe"
        case unsubscribe = "Unsubscribe"
        case shutdown = "Shutdown"
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .ping:
            try container.encode("Ping")
        case .action(let method, let payload):
            let actionRequest = ActionRequest(method: method, payload: payload.base64EncodedString())
            try container.encode(["Action": actionRequest])
        case .query(let method, let payload):
            let queryRequest = QueryRequest(method: method, payload: payload.base64EncodedString())
            try container.encode(["Query": queryRequest])
        case .subscribe(let eventTypes, let filter):
            let subscribeRequest = SubscribeRequest(event_types: eventTypes, filter: filter)
            try container.encode(["Subscribe": subscribeRequest])
        case .unsubscribe:
            try container.encode("Unsubscribe")
        case .shutdown:
            try container.encode("Shutdown")
        }
    }
}

/// Helper structs for proper JSON encoding
private struct ActionRequest: Codable {
    let method: String
    let payload: String
}

private struct QueryRequest: Codable {
    let method: String
    let payload: String
}

private struct SubscribeRequest: Codable {
    let event_types: [String]
    let filter: EventFilter?
}

/// Response types that match the Rust daemon protocol
internal enum DaemonResponse: Codable {
    case pong
    case ok(Data)
    case error(String)
    case event(Data)
    case subscribed
    case unsubscribed

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        // Try to decode as a simple string first (for Pong, Subscribed, Unsubscribed)
        if let stringValue = try? container.decode(String.self) {
            switch stringValue {
            case "Pong":
                self = .pong
            case "Subscribed":
                self = .subscribed
            case "Unsubscribed":
                self = .unsubscribed
            default:
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unknown string response: \(stringValue)")
                )
            }
            return
        }

        // Try to decode as an object with variants
        let variantContainer = try decoder.container(keyedBy: VariantKeys.self)

        if variantContainer.contains(.ok) {
            let okData = try variantContainer.decode([UInt8].self, forKey: .ok)
            self = .ok(Data(okData))
        } else if variantContainer.contains(.error) {
            // For now, decode error as a string - we can improve this later
            let errorData = try variantContainer.decode(Data.self, forKey: .error)
            let errorMsg = String(data: errorData, encoding: .utf8) ?? "Unknown error"
            self = .error(errorMsg)
        } else if variantContainer.contains(.event) {
            let eventData = try variantContainer.decode(Data.self, forKey: .event)
            self = .event(eventData)
        } else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unknown variant response")
            )
        }
    }

    enum VariantKeys: String, CodingKey {
        case ok = "Ok"
        case error = "Error"
        case event = "Event"
    }
}

/// Event filter for subscriptions
internal struct EventFilter: Codable {
    let libraryId: String?
    let jobId: String?
    let deviceId: String?

    enum CodingKeys: String, CodingKey {
        case libraryId = "library_id"
        case jobId = "job_id"
        case deviceId = "device_id"
    }
}

// MARK: - Error Types

/// Errors that can occur when using the Spacedrive client
public enum SpacedriveError: Error, LocalizedError {
    case connectionFailed(String)
    case serializationError(String)
    case daemonError(String)
    case invalidResponse(String)

    public var errorDescription: String? {
        switch self {
        case .connectionFailed(let message):
            return "Connection failed: \(message)"
        case .serializationError(let message):
            return "Serialization error: \(message)"
        case .daemonError(let message):
            return "Daemon error: \(message)"
        case .invalidResponse(let message):
            return "Invalid response: \(message)"
        }
    }
}

// MARK: - Convenience Types

/// Placeholder event type until types.swift is generated
public struct SpacedriveEvent: Codable {
    // This will be replaced by the generated Event type
}

// MARK: - Convenience Methods

extension SpacedriveClient {
    /// Get core status - demonstrates real type-safe API usage
    /// Once types.swift is generated, this can use the actual OutputProperties type
    public func getCoreStatus() async throws -> Data {
        struct EmptyQuery: Codable {}

        // Return raw data until we have the generated types
        let queryData = try JSONEncoder().encode(EmptyQuery())
        let request = DaemonRequest.query(method: "query:core.status.v1", payload: queryData)
        let response = try await sendRequest(request)

        switch response {
        case .ok(let data):
            return data
        case .error(let error):
            throw SpacedriveError.daemonError(error)
        case .pong, .event, .subscribed, .unsubscribed:
            throw SpacedriveError.invalidResponse("Unexpected response")
        }
    }

    /// Create a library - demonstrates action usage
    /// Once types.swift is generated, this can use the actual LibraryCreateInput/Output types
    public func createLibrary(name: String, path: String? = nil) async throws -> Data {
        struct LibraryCreateInput: Codable {
            let name: String
            let path: String?
        }

        let input = LibraryCreateInput(name: name, path: path)
        let actionData = try JSONEncoder().encode(input)
        let request = DaemonRequest.action(method: "action:libraries.create.input.v1", payload: actionData)
        let response = try await sendRequest(request)

        switch response {
        case .ok(let data):
            return data
        case .error(let error):
            throw SpacedriveError.daemonError(error)
        case .pong, .event, .subscribed, .unsubscribed:
            throw SpacedriveError.invalidResponse("Unexpected response")
        }
    }

    /// Ping the daemon to test connectivity
    public func ping() async throws {
        print("🏓 Sending ping request...")
        let response = try await sendRequest(.ping)
        print("🏓 Received ping response: \(response)")
        switch response {
        case .pong:
            print("✅ Ping successful!")
            return
        case .error(let error):
            print("❌ Ping failed with daemon error: \(error)")
            throw SpacedriveError.daemonError("Ping failed: \(error)")
        case .ok, .event, .subscribed, .unsubscribed:
            print("❌ Ping received unexpected response")
            throw SpacedriveError.invalidResponse("Unexpected response to ping")
        }
    }
}
