#if os(iOS)
    import Foundation
    import UIKit

    /// Protocol for the iOS core bridge - allows decoupling from specific implementation
    public protocol IOSCoreBridge {
        func initialize(dataDirectory: String, deviceName: String?) -> Bool
        func sendMessage(_ query: String, dataDirectory: String) async throws -> String
        func startEventListener(handler: @escaping (String) -> Void)
        func shutdown()
    }

    /// Main client for interacting with the embedded Spacedrive core
    ///
    /// This client provides a clean, type-safe interface for executing queries,
    /// actions, and subscribing to events from the Spacedrive core running embedded within the iOS app.
    public class SpacedriveClient {
        private let embeddedCore: IOSCoreBridge
        private let dataDirectory: String

        /// The currently active library ID
        /// This is used for library-scoped operations
        private var currentLibraryId: String?

        /// Thread-safe access to current library ID
        private let libraryIdQueue = DispatchQueue(
            label: "com.spacedrive.library-id", attributes: .concurrent)

        /// Initialize a new embedded Spacedrive client
        /// - Parameters:
        ///   - core: The iOS core bridge implementation
        ///   - dataDirectory: Path to the data directory for the embedded core
        ///   - deviceName: Optional device name (defaults to UIDevice.current.name on iOS)
        public init(core: IOSCoreBridge, dataDirectory: String, deviceName: String? = nil)
            async throws
        {
            self.embeddedCore = core
            self.dataDirectory = dataDirectory

            // Get device name from UIDevice if not provided
            #if os(iOS)
                let finalDeviceName = deviceName ?? UIDevice.current.name
            #else
                let finalDeviceName = deviceName
            #endif

            guard embeddedCore.initialize(dataDirectory: dataDirectory, deviceName: finalDeviceName)
            else {
                throw SpacedriveError.connectionFailed("Failed to initialize embedded core")
            }
        }

        // MARK: - API Namespaces

        /// Core API operations (device management, network, etc.)
        public lazy var core = CoreAPI(client: self)

        /// Device management operations
        public lazy var devices = DevicesAPI(client: self)

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
                throw SpacedriveError.invalidResponse(
                    "Multiple libraries found with name '\(libraryName)'. Use switchToLibrary(id:) instead."
                )
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
                throw SpacedriveError.invalidResponse(
                    "This operation requires an active library. Use switchToLibrary() or createAndSwitchToLibrary() first."
                )
            }
        }

        /// Get the current library ID or throw an error if none is set
        /// - Returns: The current library ID
        /// - Throws: SpacedriveError if no library is currently active
        private func getCurrentLibraryIdOrThrow() throws -> String {
            guard let libraryId = getCurrentLibraryId() else {
                throw SpacedriveError.invalidResponse(
                    "This operation requires an active library. Use switchToLibrary() or createAndSwitchToLibrary() first."
                )
            }
            return libraryId
        }

        // MARK: - Core API Methods

        /// Internal method to execute both queries and actions via JSON-RPC over FFI
        /// - Parameters:
        ///   - requestPayload: The input payload (can be empty struct for parameterless operations)
        ///   - method: The method identifier (e.g., "query:core.status" or "action:libraries.create.input")
        ///   - responseType: The expected response type
        ///   - libraryId: Optional library ID to override the current library (for library-scoped operations)
        /// - Returns: The operation result
        internal func execute<Request: Codable, Response: Codable>(
            _ requestPayload: Request,
            method: String,
            responseType: Response.Type,
            libraryId: String? = nil
        ) async throws -> Response {
            // Determine effective library ID
            let effectiveLibraryId: String?
            if method.hasPrefix("query:core.") || method.hasPrefix("action:core.") {
                // Core operations don't use library ID
                effectiveLibraryId = nil
            } else {
                effectiveLibraryId = libraryId ?? getCurrentLibraryId()
            }

            // Build JSON-RPC request
            let jsonRpc = try JSONRPCRequest(
                method: method,
                input: requestPayload,
                library_id: effectiveLibraryId,
                id: UUID().uuidString
            )

            // Send via FFI
            do {
                let requestData = try JSONEncoder().encode(jsonRpc)
                let requestJson = String(data: requestData, encoding: .utf8)!

                let responseJson = try await embeddedCore.sendMessage(
                    requestJson, dataDirectory: dataDirectory)
                let responseData = responseJson.data(using: String.Encoding.utf8)!

                // Parse JSON-RPC response
                let jsonRpcResponse = try JSONDecoder().decode(
                    JSONRPCResponse<Response>.self, from: responseData)

                if let result = jsonRpcResponse.result {
                    return result
                } else if let error = jsonRpcResponse.error {
                    throw SpacedriveError.daemonError(error.message)
                } else {
                    throw SpacedriveError.invalidResponse("No result or error in JSON-RPC response")
                }
            } catch let error as SpacedriveError {
                throw error
            } catch {
                throw SpacedriveError.serializationError("Failed to execute request: \(error)")
            }
        }

        /// Subscribe to events from the embedded core
        /// - Parameter eventTypes: Array of event type names to subscribe to
        /// - Returns: An async stream of events
        public func subscribe(
            to eventTypes: [String] = []
        ) -> AsyncThrowingStream<Event, Error> {
            AsyncThrowingStream { continuation in
                Task {
                    do {
                        // Start the FFI-based event listener
                        embeddedCore.startEventListener { [weak self] (eventJson: String) in
                            guard self != nil else { return }

                            do {
                                let eventData = eventJson.data(using: String.Encoding.utf8)!
                                let event = try JSONDecoder().decode(Event.self, from: eventData)

                                // Filter events if specific types were requested
                                if eventTypes.isEmpty
                                    || self?.shouldIncludeEvent(event, in: eventTypes) == true
                                {
                                    continuation.yield(event)
                                }
                            } catch {
                                continuation.finish(
                                    throwing: SpacedriveError.serializationError(
                                        "Failed to decode event: \(error)"))
                            }
                        }
                    } catch {
                        continuation.finish(throwing: error)
                    }
                }
            }
        }

        /// Check if an event should be included based on the requested event types
        private func shouldIncludeEvent(_ event: Event, in eventTypes: [String]) -> Bool {
            // This is a simplified implementation - in practice you'd want to map
            // the Event enum cases to string identifiers and check against eventTypes
            return true
        }

        // MARK: - Convenience Methods (same as macOS implementation)

        /// Create a library using generated types
        public func createLibrary(name: String, path: String? = nil) async throws
            -> LibraryCreateOutput
        {
            let input = LibraryCreateInput(name: name, path: path)

            return try await execute(
                input,
                method: "action:libraries.create.input",
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
        public func createAndSwitchToLibrary(
            name: String, path: String? = nil, setAsCurrent: Bool = true
        ) async throws -> LibraryCreateOutput {
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
                method: "query:libraries.list",
                responseType: [LibraryInfo].self
            )
        }

        /// Get list of jobs using generated types
        public func getJobs(status: JobStatus? = nil) async throws -> JobListOutput {
            struct JobListQuery: Codable {
                let status: String?
            }

            let query = JobListQuery(status: nil)  // TODO: Convert JobStatus to string

            return try await execute(
                query,
                method: "query:jobs.list",
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

            let query = JobListQuery(status: nil)  // TODO: Convert JobStatus to string

            return try await execute(
                query,
                method: "query:jobs.list",
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

        /// Ping the embedded core to test connectivity
        public func ping() async throws {
            print("Testing embedded core connectivity...")

            // Send a simple core status query as a ping
            do {
                let emptyInput = Empty()
                _ = try await execute(
                    emptyInput,
                    method: "query:core.status",
                    responseType: CoreStatus.self
                )
                print("Embedded core ping successful!")
            } catch {
                print("Embedded core ping failed: \(error)")
                throw error
            }
        }

        /// Shutdown the embedded core
        public func shutdown() {
            embeddedCore.shutdown()
        }

        deinit {
            shutdown()
        }
    }

#endif
