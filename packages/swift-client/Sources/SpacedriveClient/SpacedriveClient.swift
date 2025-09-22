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

        // 2. Create daemon request using JSON API
        let jsonPayload = try JSONSerialization.jsonObject(with: queryData) as! [String: Any]
        let request = DaemonRequest.jsonQuery(method: method, payload: jsonPayload)

        // 3. Send to daemon and get response
        print("🔍 Executing query: \(method)")
        let response = try await sendRequest(request)
        print("🔍 Query response received: \(response)")

        // 4. Handle response
        switch response {
        case .jsonOk(let jsonData):
            print("🔍 Query successful (JSON), decoding response")
            do {
                let jsonResponseData = try JSONSerialization.data(withJSONObject: jsonData.value)
                return try JSONDecoder().decode(responseType, from: jsonResponseData)
            } catch {
                print("❌ JSON query decode error: \(error)")
                throw SpacedriveError.serializationError("Failed to decode JSON response: \(error)")
            }
        case .error(let error):
            print("❌ Query daemon error: \(error)")
            throw SpacedriveError.daemonError(error)
        case .pong, .event, .subscribed, .unsubscribed:
            print("❌ Query unexpected response: \(response)")
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

        // 2. Create daemon request using JSON API
        let jsonPayload = try JSONSerialization.jsonObject(with: actionData) as! [String: Any]
        let request = DaemonRequest.jsonAction(method: method, payload: jsonPayload)

        // 3. Send to daemon and get response
        let response = try await sendRequest(request)

        // 4. Handle response
        switch response {
        case .jsonOk(let jsonData):
            do {
                let jsonResponseData = try JSONSerialization.data(withJSONObject: jsonData.value)
                return try JSONDecoder().decode(responseType, from: jsonResponseData)
            } catch {
                throw SpacedriveError.serializationError("Failed to decode JSON response: \(error)")
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
    ) -> AsyncThrowingStream<Event, Error> {
        AsyncThrowingStream { continuation in
            Task { [self] in
                do {
                    // 1. Establish persistent connection to daemon
                    let connection = try await createConnection()

                    // 2. Send subscription request
                    let request = DaemonRequest.subscribe(eventTypes: eventTypes, filter: nil)
                    try await sendRequestOverConnection(request, connection: connection)

                    // 3. Stream events as they arrive
                    while true {
                        let response = try await readStreamingResponseFromConnection(connection)
                        if case .event(let event) = response {
                            continuation.yield(event)
                        }
                    }
                } catch {
                    print("❌ Event subscription error: \(error)")
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
        let requestData: Data

        // Handle JSON requests manually to avoid Swift Codable issues with Any types
        switch request {
        case .ping:
            requestData = Data("\"Ping\"".utf8)

        case .jsonQuery(let method, let payload):
            let jsonString = """
            {"JsonQuery":{"method":"\(method)","payload":\(try jsonStringFromDictionary(payload))}}
            """
            requestData = Data(jsonString.utf8)

        case .jsonAction(let method, let payload):
            let jsonString = """
            {"JsonAction":{"method":"\(method)","payload":\(try jsonStringFromDictionary(payload))}}
            """
            requestData = Data(jsonString.utf8)

        case .subscribe(let eventTypes, let filter):
            let subscribeRequest = SubscribeRequest(event_types: eventTypes, filter: filter)
            requestData = try JSONEncoder().encode(["Subscribe": subscribeRequest])

        case .unsubscribe:
            requestData = Data("\"Unsubscribe\"".utf8)

        case .shutdown:
            requestData = Data("\"Shutdown\"".utf8)
        }

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

    /// Read a complete line-delimited JSON response from a streaming connection
    /// This properly handles JSON messages that span multiple socket reads
    private func readStreamingResponseFromConnection(_ connection: Int32) async throws -> DaemonResponse {
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async {
                var lineBuffer = Data()
                var tempBuffer = [UInt8](repeating: 0, count: 1024)

                // Read byte by byte until we find a complete line (ending with \n)
                while true {
                    let readResult = recv(connection, &tempBuffer, 1, 0) // Read 1 byte at a time

                    guard readResult > 0 else {
                        let errorMsg = String(cString: strerror(errno))
                        print("❌ Stream receive failed: \(errorMsg)")
                        continuation.resume(throwing: SpacedriveError.connectionFailed("Stream receive failed: \(errorMsg)"))
                        return
                    }

                    let byte = tempBuffer[0]

                    // Check for newline (end of JSON message)
                    if byte == 10 { // ASCII newline
                        // We have a complete line, try to parse it
                        if let lineString = String(data: lineBuffer, encoding: .utf8) {
                            let trimmedLine = lineString.trimmingCharacters(in: .whitespacesAndNewlines)
                            if !trimmedLine.isEmpty {
                                print("📥 Received complete JSON line (\(lineBuffer.count) bytes): \(trimmedLine)")

                                do {
                                    let response = try JSONDecoder().decode(DaemonResponse.self, from: Data(trimmedLine.utf8))
                                    continuation.resume(returning: response)
                                    return
                                } catch {
                                    print("❌ Failed to decode JSON line: \(error)")
                                    print("❌ Raw line: \(trimmedLine)")
                                    continuation.resume(throwing: SpacedriveError.serializationError("Failed to decode JSON: \(error)"))
                                    return
                                }
                            }
                        } else {
                            print("❌ Invalid UTF-8 in line buffer")
                            continuation.resume(throwing: SpacedriveError.invalidResponse("Invalid UTF-8 in response"))
                            return
                        }
                    } else {
                        // Accumulate byte into line buffer
                        lineBuffer.append(byte)

                        // Safety check to prevent infinite accumulation
                        if lineBuffer.count > 10 * 1024 * 1024 { // 10MB limit
                            print("❌ JSON line too large (\(lineBuffer.count) bytes)")
                            continuation.resume(throwing: SpacedriveError.invalidResponse("JSON line too large"))
                            return
                        }
                    }
                }
            }
        }
    }

    /// Helper to convert dictionary to JSON string
    private func jsonStringFromDictionary(_ dict: [String: Any]) throws -> String {
        let jsonData = try JSONSerialization.data(withJSONObject: dict)
        return String(data: jsonData, encoding: .utf8) ?? "{}"
    }
}

// MARK: - Daemon Protocol Types

/// Request types that match the Rust daemon protocol
internal enum DaemonRequest {
    case ping
    case jsonAction(method: String, payload: [String: Any])
    case jsonQuery(method: String, payload: [String: Any])
    case subscribe(eventTypes: [String], filter: EventFilter?)
    case unsubscribe
    case shutdown
}

/// Helper structs for proper JSON encoding



private struct SubscribeRequest: Codable {
    let event_types: [String]
    let filter: EventFilter?
}

/// Response types that match the Rust daemon protocol
internal enum DaemonResponse: Codable {
    case pong
    case jsonOk(AnyCodable)
    case error(String)
    case event(Event)
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

        if variantContainer.contains(.jsonOk) {
            // JsonOk contains a JSON value that we need to decode manually
            let jsonValue = try variantContainer.decode(AnyCodable.self, forKey: .jsonOk)
            self = .jsonOk(jsonValue)
        } else if variantContainer.contains(.error) {
            // For now, decode error as a string - we can improve this later
            let errorData = try variantContainer.decode(Data.self, forKey: .error)
            let errorMsg = String(data: errorData, encoding: .utf8) ?? "Unknown error"
            self = .error(errorMsg)
        } else if variantContainer.contains(.event) {
            let event = try variantContainer.decode(Event.self, forKey: .event)
            self = .event(event)
        } else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unknown variant response")
            )
        }
    }

    enum VariantKeys: String, CodingKey {
        case jsonOk = "JsonOk"
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

/// Type alias for the generated Event type from types.swift
public typealias SpacedriveEvent = Event

/// Helper for decoding Any values from JSON
internal struct AnyCodable: Codable {
    let value: Any

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if let intValue = try? container.decode(Int.self) {
            value = intValue
        } else if let doubleValue = try? container.decode(Double.self) {
            value = doubleValue
        } else if let stringValue = try? container.decode(String.self) {
            value = stringValue
        } else if let boolValue = try? container.decode(Bool.self) {
            value = boolValue
        } else if let arrayValue = try? container.decode([AnyCodable].self) {
            value = arrayValue.map { $0.value }
        } else if let dictValue = try? container.decode([String: AnyCodable].self) {
            value = dictValue.mapValues { $0.value }
        } else {
            value = NSNull()
        }
    }

    func encode(to encoder: Encoder) throws {
        // Not needed for our use case
        throw EncodingError.invalidValue(value, EncodingError.Context(codingPath: encoder.codingPath, debugDescription: "AnyCodable encoding not implemented"))
    }
}

// MARK: - Convenience Methods

extension SpacedriveClient {

    /// Create a library using generated types
    public func createLibrary(name: String, path: String? = nil) async throws -> LibraryCreateOutput {
        let input = LibraryCreateInput(name: name, path: path)

        return try await executeAction(
            input,
            method: "action:libraries.create.input.v1",
            responseType: LibraryCreateOutput.self
        )
    }

    /// Get list of libraries using generated types
    public func getLibraries(includeStats: Bool = false) async throws -> [LibraryInfo] {
        struct LibraryListQuery: Codable {
            let include_stats: Bool
        }

        let query = LibraryListQuery(include_stats: includeStats)

        return try await executeQuery(
            query,
            method: "query:libraries.list.v1",
            responseType: [LibraryInfo].self
        )
    }

    /// Get list of jobs using generated types
    public func getJobs(status: JobStatus? = nil) async throws -> JobListOutput {
        struct JobListQuery: Codable {
            let status: String?
        }

        let query = JobListQuery(status: nil) // TODO: Convert JobStatus to string

        return try await executeQuery(
            query,
            method: "query:jobs.list.v1",
            responseType: JobListOutput.self
        )
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
        case .jsonOk, .event, .subscribed, .unsubscribed:
            print("❌ Ping received unexpected response")
            throw SpacedriveError.invalidResponse("Unexpected response to ping")
        }
    }
}
