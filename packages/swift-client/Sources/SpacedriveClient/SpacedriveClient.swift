#if os(macOS)
import Foundation
import Darwin

/// Main client for interacting with the Spacedrive daemon
///
/// This client provides a clean, type-safe interface for executing queries,
/// actions, and subscribing to events from the Spacedrive core.
public class SpacedriveClient {
    private let socketPath: String

    /// The currently active library ID
    /// This is used for library-scoped operations
    private var currentLibraryId: String?

    /// Thread-safe access to current library ID
    private let libraryIdQueue = DispatchQueue(label: "com.spacedrive.library-id", attributes: .concurrent)

    /// Initialize a new Spacedrive client
    /// - Parameter socketPath: Path to the Unix domain socket for the daemon
    public init(socketPath: String) {
        self.socketPath = socketPath
    }

    // MARK: - API Namespaces

    /// Core API operations (device management, network, etc.)
    public lazy var core = CoreAPI(client: self)

    /// Library management operations
    public lazy var libraries = LibrariesAPI(client: self)

    /// Job management operations
    public lazy var jobs = JobsAPI(client: self)

    /// Location management operations
    public lazy var locations = LocationsAPI(client: self)

    /// Media operations
    public lazy var media = MediaAPI(client: self)

    /// Network operations
    public lazy var network = NetworkAPI(client: self)

    /// Search operations
    public lazy var search = SearchAPI(client: self)

    /// Tag operations
    public lazy var tags = TagsAPI(client: self)

    /// Volume operations
    public lazy var volumes = VolumesAPI(client: self)

    /// File operations
    public lazy var files = FilesAPI(client: self)

    // MARK: - Library Management

    /// Get the currently active library ID
    /// - Returns: The current library ID, or nil if no library is active
    public func getCurrentLibraryId() -> String? {
        return libraryIdQueue.sync { currentLibraryId }
    }

    /// Set the currently active library
    /// - Parameter libraryId: The ID of the library to make active
    public func setCurrentLibrary(_ libraryId: String) {
        libraryIdQueue.async(flags: .barrier) {
            self.currentLibraryId = libraryId
        }
    }

    /// Clear the currently active library (set to nil)
    public func clearCurrentLibrary() {
        libraryIdQueue.async(flags: .barrier) {
            self.currentLibraryId = nil
        }
    }

    /// Switch to a library by ID
    /// - Parameter libraryId: The ID of the library to switch to
    /// - Throws: SpacedriveError if the library doesn't exist or can't be accessed
    public func switchToLibrary(_ libraryId: String) async throws {
        // Check if the library exists in the list (core-scoped query)
        let libraries = try await getLibraries()
        let libraryExists = libraries.contains { $0.id == libraryId }

        if !libraryExists {
            throw SpacedriveError.invalidResponse("Library with ID '\(libraryId)' not found")
        }

        // Set as current library
        setCurrentLibrary(libraryId)
    }

    /// Switch to a library by name
    /// - Parameter libraryName: The name of the library to switch to
    /// - Throws: SpacedriveError if the library doesn't exist or multiple libraries have the same name
    public func switchToLibrary(named libraryName: String) async throws {
        let libraries = try await getLibraries()
        let matchingLibraries = libraries.filter { $0.name == libraryName }

        switch matchingLibraries.count {
        case 0:
            throw SpacedriveError.invalidResponse("No library found with name '\(libraryName)'")
        case 1:
            setCurrentLibrary(matchingLibraries[0].id)
        default:
            throw SpacedriveError.invalidResponse("Multiple libraries found with name '\(libraryName)'. Use switchToLibrary(id:) instead.")
        }
    }

    /// Get information about the currently active library
    /// - Returns: LibraryInfo for the current library, or nil if no library is active
    /// - Throws: SpacedriveError if the library can't be accessed
    public func getCurrentLibraryInfo() async throws -> LibraryInfo? {
        guard let libraryId = getCurrentLibraryId() else {
            return nil
        }

        let libraries = try await getLibraries()
        return libraries.first { $0.id == libraryId }
    }

    /// Check if a library operation can be performed (requires current library)
    /// - Throws: SpacedriveError if no library is currently active
    private func requireCurrentLibrary() throws {
        guard getCurrentLibraryId() != nil else {
            throw SpacedriveError.invalidResponse("This operation requires an active library. Use switchToLibrary() or createAndSwitchToLibrary() first.")
        }
    }

    /// Get the current library ID or throw an error if none is set
    /// - Returns: The current library ID
    /// - Throws: SpacedriveError if no library is currently active
    private func getCurrentLibraryIdOrThrow() throws -> String {
        guard let libraryId = getCurrentLibraryId() else {
            throw SpacedriveError.invalidResponse("This operation requires an active library. Use switchToLibrary() or createAndSwitchToLibrary() first.")
        }
        return libraryId
    }

    // MARK: - Core API Methods

    /// Internal method to execute both queries and actions
    /// - Parameters:
    ///   - requestPayload: The input payload (can be empty struct for parameterless operations)
    ///   - method: The method identifier (e.g., "query:core.status.v1" or "action:libraries.create.input.v1")
    ///   - responseType: The expected response type
    ///   - libraryId: Optional library ID to override the current library (for library-scoped operations)
    /// - Returns: The operation result
    internal func execute<Request: Codable, Response: Codable>(
        _ requestPayload: Request,
        method: String,
        responseType: Response.Type,
        libraryId: String? = nil
    ) async throws -> Response {
        // 1. Handle unit types (Empty) specially - they should send null payload for unit types
        let jsonPayload: [String: Any]?
        if requestPayload is Empty {
            jsonPayload = nil  // Unit types should have null payload, not empty object
        } else {
            // Encode input to JSON for non-unit types
            let requestData: Data
            do {
                let encoder = JSONEncoder()
                // Don't omit nil values - we need them to be present as null
                // This is handled by the Codable implementation of the structs
                requestData = try encoder.encode(requestPayload)
            } catch {
                throw SpacedriveError.serializationError("Failed to encode request: \(error)")
            }
            jsonPayload = try JSONSerialization.jsonObject(with: requestData) as? [String: Any]
        }

        // 3. Determine request type from method prefix and include library ID if needed
        let request: DaemonRequest
        let effectiveLibraryId = libraryId ?? getCurrentLibraryId()

            if method.hasPrefix("query:") {
                // Core queries (like core.status) don't need library ID
                let queryLibraryId = method.hasPrefix("query:core.") ? nil : effectiveLibraryId
                request = DaemonRequest.query(method: method, libraryId: queryLibraryId, payload: jsonPayload ?? [:])
            } else if method.hasPrefix("action:") {
            request = DaemonRequest.action(method: method, libraryId: effectiveLibraryId, payload: jsonPayload ?? [:])
        } else {
            throw SpacedriveError.invalidResponse("Invalid method format: \(method)")
        }

        // 4. Send to daemon and get response
        let response = try await sendRequest(request)

        // 5. Handle response
        switch response {
        case .jsonOk(let jsonData):
            do {
                let jsonResponseData = try JSONSerialization.data(withJSONObject: jsonData.value)
                return try JSONDecoder().decode(responseType, from: jsonResponseData)
            } catch {
                throw SpacedriveError.serializationError("Failed to decode JSON response: \(error)")
            }
        case .error(let error):
            print("❌ Daemon error: \(error)")
            throw SpacedriveError.daemonError(error)
        case .pong, .event, .subscribed, .unsubscribed:
            print("❌ Unexpected response: \(response)")
            throw SpacedriveError.invalidResponse("Unexpected response to operation")
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
        defer {
            close(connection)
        }

        // Extract method for logging
        let method: String?
        switch request {
        case .query(let m, _, _), .action(let m, _, _):
            method = m
        case .ping, .subscribe, .unsubscribe, .shutdown:
            method = nil
        }

        try await sendRequestOverConnection(request, connection: connection)
        let response = try await readResponseFromConnection(connection, method: method)
        return response
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

        case .query(let method, let libraryId, let payload):
            // Build the JSON string properly, handling trailing commas
            var queryParts: [String] = ["\"method\":\"\(method)\""]
            if let libraryId = libraryId {
                queryParts.append("\"library_id\":\"\(libraryId)\"")
            } else {
                // Always include library_id field, even if null
                queryParts.append("\"library_id\":null")
            }
            // Include payload field - send null for unit types (Empty payloads)
            if payload.isEmpty && method.hasPrefix("query:core.") {
                queryParts.append("\"payload\":null")
            } else {
                queryParts.append("\"payload\":\(try jsonStringFromDictionary(payload))")
            }

            let jsonString = """
            {"Query":{\(queryParts.joined(separator: ","))}}
            """
            print("🔍 Sending query request: \(jsonString)")
            requestData = Data(jsonString.utf8)

        case .action(let method, let libraryId, let payload):
            let libraryIdJson = libraryId.map { "\"library_id\":\"\($0)\"," } ?? ""
            let jsonString = """
            {"Action":{"method":"\(method)",\(libraryIdJson)"payload":\(try jsonStringFromDictionary(payload))}}
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


        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            DispatchQueue.global().async {
                let sendResult = requestLine.withUnsafeBytes { bytes in
                    send(connection, bytes.bindMemory(to: UInt8.self).baseAddress, requestLine.count, 0)
                }

                if sendResult == -1 {
                    let errorMsg = String(cString: strerror(errno))
                    continuation.resume(throwing: SpacedriveError.connectionFailed("Send failed: \(errorMsg)"))
                } else {
                    continuation.resume()
                }
            }
        }
    }

    /// Read a response from an existing connection
    private func readResponseFromConnection(_ connection: Int32, method: String? = nil) async throws -> DaemonResponse {
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async {
                print("🔍 Starting to read response from connection...")

                var allData = Data()
                let bufferSize = 65536
                var buffer = [UInt8](repeating: 0, count: bufferSize)
                var totalBytesRead = 0

                // Keep reading until we get a complete line (ending with newline)
                while true {
                    let readResult = recv(connection, &buffer, buffer.count, 0)
                    print("🔍 Socket read result: \(readResult) bytes")

                    guard readResult > 0 else {
                        let errorMsg = String(cString: strerror(errno))
                        print("❌ Socket read failed: \(errorMsg)")
                        continuation.resume(throwing: SpacedriveError.connectionFailed("Receive failed: \(errorMsg)"))
                        return
                    }

                    allData.append(Data(buffer.prefix(readResult)))
                    totalBytesRead += readResult
                    print("🔍 Total bytes read so far: \(totalBytesRead)")

                    // Check if we have a complete line (ending with newline)
                    if let responseString = String(data: allData, encoding: .utf8) {
                        if responseString.contains("\n") {
                            print("🔍 Found newline, stopping read")
                            break
                        }
                    }

                    // Safety check to prevent infinite loop
                    if totalBytesRead > 10 * 1024 * 1024 { // 10MB limit
                        print("❌ Response too large, stopping read")
                        continuation.resume(throwing: SpacedriveError.invalidResponse("Response too large"))
                        return
                    }
                }

                print("🔍 Final total bytes: \(allData.count)")

                do {
                    // Find the newline delimiter and parse JSON
                    if let responseString = String(data: allData, encoding: .utf8) {
                        print("🔍 Response string length: \(responseString.count) characters")
                        print("🔍 First 200 chars: \(String(responseString.prefix(200)))")

                        let lines = responseString.components(separatedBy: .newlines).filter { !$0.isEmpty }
                        print("🔍 Found \(lines.count) lines in response")

                        if let firstLine = lines.first {
                            print("🔍 First line length: \(firstLine.count) characters")
                            print("🔍 First line preview: \(String(firstLine.prefix(100)))...")

                            let lineData = Data(firstLine.utf8)
                            print("🔍 Attempting to decode JSON from \(lineData.count) bytes...")

                            let response = try JSONDecoder().decode(DaemonResponse.self, from: lineData)
                            print("✅ Successfully decoded response")

                            if let method = method {
                                print("Daemon response for \(method): \(response)")
                            } else {
                                print("Daemon response: \(response)")
                            }
                            continuation.resume(returning: response)
                        } else {
                            print("❌ No valid response line found")
                            continuation.resume(throwing: SpacedriveError.invalidResponse("No valid response line"))
                        }
                    } else {
                        print("❌ Invalid UTF-8 response")
                        continuation.resume(throwing: SpacedriveError.invalidResponse("Invalid UTF-8 response"))
                    }
                } catch {
                    print("❌ JSON decoding failed: \(error)")
                    print("❌ Error details: \(error.localizedDescription)")
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
    case action(method: String, libraryId: String?, payload: [String: Any])
    case query(method: String, libraryId: String?, payload: [String: Any])
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
            // Decode error as a structured object
            let errorObject = try variantContainer.decode(AnyCodable.self, forKey: .error)
            // Convert the error object to a string representation
            if let errorData = try? JSONEncoder().encode(errorObject),
               let errorString = String(data: errorData, encoding: .utf8) {
                self = .error(errorString)
            } else {
                self = .error("Unknown error")
            }
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

// MARK: - Convenience Types
// (Shared types are now in SpacedriveShared.swift)

// MARK: - API Namespace Structs
// These are automatically generated by the Rust build process
// See SpacedriveAPI.swift for the actual implementations

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

        return try await execute(
            input,
            method: "action:libraries.create.input.v1",
            responseType: LibraryCreateOutput.self
        )
    }

    /// Create a library and automatically set it as the current library
    /// - Parameters:
    ///   - name: The name of the library to create
    ///   - path: Optional path for the library
    ///   - setAsCurrent: Whether to automatically set the new library as current (default: true)
    /// - Returns: The created library information
    /// - Throws: SpacedriveError if creation fails
    public func createAndSwitchToLibrary(name: String, path: String? = nil, setAsCurrent: Bool = true) async throws -> LibraryCreateOutput {
        let result = try await createLibrary(name: name, path: path)

        if setAsCurrent {
            setCurrentLibrary(result.libraryId)
        }

        return result
    }

    /// Get list of libraries using generated types
    public func getLibraries(includeStats: Bool = false) async throws -> [LibraryInfo] {
        struct LibraryListQuery: Codable {
            let include_stats: Bool
        }

        let query = LibraryListQuery(include_stats: includeStats)

        return try await execute(
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

        return try await execute(
            query,
            method: "query:jobs.list.v1",
            responseType: JobListOutput.self
        )
    }

    /// Get jobs for the current library
    /// - Parameter status: Optional job status filter
    /// - Returns: List of jobs for the current library
    /// - Throws: SpacedriveError if no library is active or operation fails
    public func getCurrentLibraryJobs(status: JobStatus? = nil) async throws -> JobListOutput {
        let libraryId = try getCurrentLibraryIdOrThrow()

        struct JobListQuery: Codable {
            let status: String?
        }

        let query = JobListQuery(status: nil) // TODO: Convert JobStatus to string

        return try await execute(
            query,
            method: "query:jobs.list.v1",
            responseType: JobListOutput.self,
            libraryId: libraryId
        )
    }

    /// Get the current library status
    /// - Returns: A string describing the current library state
    public func getCurrentLibraryStatus() -> String {
        guard let libraryId = getCurrentLibraryId() else {
            return "No library is currently active"
        }
        return "Library '\(libraryId)' is currently active"
    }

    /// Check if a library is currently active
    /// - Returns: True if a library is active, false otherwise
    public func hasActiveLibrary() -> Bool {
        return getCurrentLibraryId() != nil
    }

    /// Execute a library query
    /// - Parameter query: The query object to execute
    /// - Returns: The query result
    public func query<T: Codable>(_ query: T) async throws -> T {
        // For now, we'll use a simple approach - this should be improved
        // to automatically determine the wire method from the query type
        let wireMethod = "query:files.by_path.v1" // This should be dynamic

        // Encode the query to JSON
        let queryData = try JSONEncoder().encode(query)
        let queryDict = try JSONSerialization.jsonObject(with: queryData) as? [String: Any] ?? [:]

        let response = try await sendRequest(.query(method: wireMethod, libraryId: getCurrentLibraryId(), payload: queryDict))

        switch response {
        case .jsonOk(let anyCodable):
            // Convert AnyCodable to Data, then decode
            let data = try JSONEncoder().encode(anyCodable)
            let result = try JSONDecoder().decode(T.self, from: data)
            return result
        case .error(let error):
            throw SpacedriveError.daemonError("Query failed: \(error)")
        default:
            throw SpacedriveError.invalidResponse("Unexpected response to query")
        }
    }

    /// Execute a FileByPathQuery and return the File result
    public func queryFileByPath(_ query: FileByPathQuery) async throws -> File? {
        let wireMethod = "query:files.by_path.v1"

        // Encode the query to JSON
        let queryData = try JSONEncoder().encode(query)
        let queryDict = try JSONSerialization.jsonObject(with: queryData) as? [String: Any] ?? [:]

        let response = try await sendRequest(.query(method: wireMethod, libraryId: getCurrentLibraryId(), payload: queryDict))

        switch response {
        case .jsonOk(let jsonData):
            print("🔍 Decoding File from JSON data: \(jsonData.value)")
            do {
                let jsonResponseData = try JSONSerialization.data(withJSONObject: jsonData.value)
                let result = try JSONDecoder().decode(File.self, from: jsonResponseData)
                print("✅ Successfully decoded File: \(result.name)")
                return result
            } catch {
                print("❌ Failed to decode File: \(error)")
                print("❌ JSON data: \(jsonData.value)")
                throw SpacedriveError.invalidResponse("Failed to decode File: \(error)")
            }
        case .error(let error):
            throw SpacedriveError.daemonError("Query failed: \(error)")
        default:
            throw SpacedriveError.invalidResponse("Unexpected response to query")
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
        case .jsonOk, .event, .subscribed, .unsubscribed:
            print("❌ Ping received unexpected response")
            throw SpacedriveError.invalidResponse("Unexpected response to ping")
        }
    }
}

#endif
