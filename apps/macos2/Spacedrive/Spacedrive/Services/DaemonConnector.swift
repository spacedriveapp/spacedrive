import Foundation
import os.log
import Combine
@preconcurrency import SpacedriveClient

// Now using generated types from SpacedriveClient - no manual definitions needed!

@MainActor
class DaemonConnector: ObservableObject {
    @Published var connectionStatus: ConnectionStatus = .disconnected
    @Published var jobs: [JobInfo] = []
    @Published var currentLibraryId: String?
    @Published var availableLibraries: [LibraryInfo] = []
    @Published var coreStatus: CoreStatus?

    private let socketPath = "/Users/jamespine/Library/Application Support/spacedrive/daemon/daemon.sock"
    let client: SpacedriveClient
    private var eventTask: Task<Void, Never>?
    private let logger = Logger(subsystem: "com.spacedrive.daemon", category: "DaemonConnector")

    init() {
        logger.info("DaemonConnector initializing with socket path: \(self.socketPath)")
        client = SpacedriveClient(socketPath: self.socketPath)
        connect()
    }

    nonisolated deinit {
        logger.info("DaemonConnector deinitializing")
        eventTask?.cancel()
        eventTask = nil
    }

    func connect() {
        guard connectionStatus != .connecting, connectionStatus != .connected else {
            return
        }

        connectionStatus = .connecting

        // Check if socket file exists
        guard FileManager.default.fileExists(atPath: socketPath) else {
            print("❌ Daemon socket not found at: \(socketPath)")
            connectionStatus = .error("Daemon socket not found. Is Spacedrive daemon running?")
            return
        }

        // Start connection and event subscription
        Task {
            await startConnection()
        }
    }

    func disconnect() {
        eventTask?.cancel()
        eventTask = nil
        connectionStatus = .disconnected
    }

    private func startConnection() async {
        do {
            // Test connection with ping (with timeout)
            try await withTimeout(seconds: 5) { [self] in
                try await client.ping()
            }

            connectionStatus = .connected

            await fetchCoreStatus()

            // Start event subscription
            await subscribeToEvents()

        } catch {
            print("❌ Connection failed: \(error)")
            connectionStatus = .error("Failed to connect: \(error.localizedDescription)")
        }
    }

    private func withTimeout<T: Sendable>(seconds: TimeInterval, operation: @escaping @Sendable () async throws -> T) async throws -> T {
        try await withThrowingTaskGroup(of: T.self) { group in
            group.addTask {
                try await operation()
            }

            group.addTask {
                try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
                throw SpacedriveError.connectionFailed("Connection timeout after \(seconds) seconds")
            }

            let result = try await group.next()!
            group.cancelAll()
            return result
        }
    }

    private func subscribeToEvents() async {
        let eventTypes = [
            "JobStarted",
            "JobProgress",
            "JobCompleted",
            "JobFailed",
            "JobPaused",
            "JobResumed",
            "LibraryCreated",
            "LibraryOpened",
            "LibraryClosed",
            "LibraryDeleted",
            "EntryCreated",
            "EntryModified",
            "EntryDeleted",
            "EntryMoved",
        ]

        // First, fetch the current job list to establish baseline state
        await fetchJobList()

        // Also fetch core status
        await fetchCoreStatus()

        // Then subscribe to events for real-time updates
        eventTask = Task {
            do {
                for try await event in client.subscribe(to: eventTypes) {
                    await handleEvent(event)
                }
            } catch {
                connectionStatus = .error("Event subscription failed: \(error.localizedDescription)")
            }
        }
    }

    private func handleEvent(_ event: SpacedriveEvent) async {
        // Handle events using the new type-safe Event enum
        switch event {
            // Core lifecycle events
            case .coreStarted:
                break

            case .coreShutdown:
                break

            // Job events - update existing jobs in real-time
            case let .jobStarted(data):
                self.handleJobStarted(jobId: data.jobId, jobType: data.jobType)

            case let .jobProgress(data):
                self.handleJobProgress(
                    jobId: data.jobId,
                    jobType: data.jobType,
                    progress: data.progress,
                    message: data.message,
                    genericProgress: data.genericProgress
                )

            case let .jobCompleted(data):
                self.handleJobCompleted(jobId: data.jobId, jobType: data.jobType, output: data.output)

            case let .jobFailed(data):
                self.handleJobFailed(jobId: data.jobId, jobType: data.jobType, error: data.error)

            case let .jobPaused(data):
                self.handleJobPaused(jobId: data.jobId)

            case let .jobResumed(data):
                self.handleJobResumed(jobId: data.jobId)

            // Library events
            case .libraryCreated:
                Task { await self.refreshLibraryStats() }

            case .libraryOpened:
                Task { await self.refreshLibraryStats() }

            case .libraryClosed:
                Task { await self.refreshLibraryStats() }

            case .libraryDeleted:
                Task { await self.refreshLibraryStats() }

            // Entry events that affect library statistics
            case .entryCreated:
                Task { await self.refreshLibraryStats() }

            case .entryModified:
                Task { await self.refreshLibraryStats() }

            case .entryDeleted:
                Task { await self.refreshLibraryStats() }

            case .entryMoved:
                Task { await self.refreshLibraryStats() }

            // Other events can be handled as needed
            default:
                break
            }
    }

    // MARK: - Job Event Handlers

    private func handleJobStarted(jobId: String, jobType: String) {
        // Only create a new job if it doesn't exist
        if job(withId: jobId) == nil {
            let newJob = JobInfo(
                id: jobId,
                name: jobType.capitalized,
                status: .running,
                progress: 0.0,
                startedAt: Date(),
                completedAt: nil,
                errorMessage: nil
            )
            updateOrAddJob(newJob)
        }
    }

    private func handleJobProgress(jobId: String, jobType: String, progress: Double, message _: String?, genericProgress: GenericProgress?) {
        if var existingJob = job(withId: jobId) {
            existingJob.progress = progress

            // Keep the actual job type as the title
            existingJob.name = formatJobTitle(jobType)
            existingJob.jobType = jobType

            // Use enhanced progress information for better UX
            if let generic = genericProgress {
                // Store additional progress details
                existingJob.currentPhase = generic.phase
                existingJob.completionInfo = "\(generic.completion.completed)/\(generic.completion.total)"

                // Extract current path from genericProgress, avoiding fake status messages
                existingJob.currentPath = extractCurrentPath(from: generic)

                // Check for errors/warnings
                if generic.performance.errorCount > 0 || generic.performance.warningCount > 0 {
                    existingJob.hasIssues = true
                    existingJob.issuesInfo = "⚠️ \(generic.performance.errorCount) errors, \(generic.performance.warningCount) warnings"
                }
            }

            existingJob.status = .running
            updateOrAddJob(existingJob)

            // Enhanced logging with more details
            if genericProgress != nil {
            } else {}
        } else {
            // If we receive progress for a job we don't know about, create it
            let newJob = JobInfo(
                id: jobId,
                name: formatJobTitle(jobType),
                status: .running,
                progress: progress,
                startedAt: Date(),
                completedAt: nil,
                errorMessage: nil,
                currentPhase: genericProgress?.phase,
                completionInfo: genericProgress != nil ? "\(genericProgress!.completion.completed)/\(genericProgress!.completion.total)" : nil,
                hasIssues: false,
                issuesInfo: nil,
                currentPath: genericProgress != nil ? extractCurrentPath(from: genericProgress!) : nil,
                jobType: jobType
            )
            updateOrAddJob(newJob)
        }
    }

    // Helper to format job type into a user-friendly title
    private func formatJobTitle(_ jobType: String) -> String {
        switch jobType.lowercased() {
        case "indexer":
            return "File Indexer"
        case "file_copy":
            return "File Copy"
        case "thumbnail_generator":
            return "Thumbnail Generator"
        case "file_move":
            return "File Move"
        case "file_delete":
            return "File Delete"
        case "media_processor":
            return "Media Processor"
        default:
            return jobType.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }

    // Helper to extract real current path from genericProgress, avoiding fake status messages
    private func extractCurrentPath(from genericProgress: GenericProgress) -> String? {
        guard let currentPath = genericProgress.currentPath else { return nil }

        switch currentPath {
        case let .physical(pathData):
            let path = pathData.path

            // Check if this looks like a real file path vs a status message
            // Status messages often contain patterns like "(X/Y)" or "- XX.X%"
            let isStatusMessage = path.contains(#"\(\d+/\d+\)"#) ||
                path.contains(#" - \d+\.?\d*%"#) ||
                path.hasPrefix("Generating content identities") ||
                path.hasPrefix("Aggregating directory")

            // Only return if it looks like a real file path
            return isStatusMessage ? nil : path

        default:
            return nil
        }
    }

    private func handleJobCompleted(jobId: String, jobType _: String, output _: JobOutput) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .completed
            existingJob.progress = 1.0
            existingJob.completedAt = Date()
            updateOrAddJob(existingJob)
        }
    }

    private func handleJobFailed(jobId: String, jobType _: String, error: String) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .failed
            existingJob.errorMessage = error
            updateOrAddJob(existingJob)
        }
    }

    private func handleJobPaused(jobId: String) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .paused
            updateOrAddJob(existingJob)
        }
    }

    private func handleJobResumed(jobId: String) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .running
            updateOrAddJob(existingJob)
        }
    }

    private func updateOrAddJob(_ jobInfo: JobInfo) {
        if let index = jobs.firstIndex(where: { $0.id == jobInfo.id }) {
            jobs[index] = jobInfo
        } else {
            jobs.append(jobInfo)
        }

        // Sort jobs by start date (newest first)
        jobs.sort { $0.startedAt > $1.startedAt }
    }

    private func job(withId id: String) -> JobInfo? {
        return jobs.first { $0.id == id }
    }

    func reconnect() {
        disconnect()
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_000_000_000) // 1 second
            connect()
        }
    }

    // MARK: - Job Control

    func pauseJob(_ jobId: String) {
        guard connectionStatus == .connected else {
            return
        }

        Task {
            do {
                // Call the jobs.pause action using the new type-safe method
                _ = try await client.jobs.pause(JobPauseInput(jobId: jobId))

            } catch {}
        }
    }

    func resumeJob(_ jobId: String) {
        guard connectionStatus == .connected else {
            return
        }

        Task {
            do {
                // Call the jobs.resume action using the new type-safe method
                _ = try await client.jobs.resume(JobResumeInput(jobId: jobId))

            } catch {}
        }
    }

    // MARK: - Library Management


    func switchToLibrary(_ library: LibraryInfo) async {
        do {
            // Switch to the library using the Swift client's async method
            try await client.switchToLibrary(library.id)

            // Set the current library ID synchronously
            currentLibraryId = library.id

            // Refresh job list for the new library
            await fetchJobList()

        } catch {}
    }

    func getCurrentLibraryInfo() -> LibraryInfo? {
        return availableLibraries.first { $0.id == currentLibraryId }
    }

    // MARK: - Library Statistics Refresh

    private func refreshLibraryStats() async {
        do {
            let libraries = try await client.libraries.list(ListLibrariesInput(includeStats: true))

            availableLibraries = libraries.map { library in
                LibraryInfo(
                    id: library.id,
                    name: library.name,
                    path: library.path,
                    isDefault: false, // TODO: Determine if this is the default library
                    stats: library.stats
                )
            }
        } catch {
            logger.error("Failed to refresh library statistics: \(error.localizedDescription)")
        }
    }

    // MARK: - Job List Management

    private func fetchJobList() async {
        do {
            // Check if we have a current library
            guard let libraryId = currentLibraryId else {
                logger.info("No current library ID, skipping job fetch")
                return
            }

            logger.info("Fetching jobs for library: \(libraryId)")

            // Get jobs using the new type-safe method
            let jobsResponse = try await client.jobs.list(JobListInput(status: nil))

            logger.info("Received \(jobsResponse.jobs.count) jobs from daemon")

            // Convert JobListItem to JobInfo for the UI using convenience initializer
            let convertedJobs = jobsResponse.jobs.map { jobItem in
                JobInfo(from: jobItem)
            }

            // Update the UI with the converted jobs
            jobs = convertedJobs
            logger.info("Updated UI with \(convertedJobs.count) jobs")

        } catch {
            logger.error("Failed to fetch jobs: \(error.localizedDescription)")
            connectionStatus = .error("Failed to fetch jobs: \(error.localizedDescription)")
        }
    }

    private func fetchCoreStatus() async {
        do {
            // Create empty input for core status query
            let emptyData = Data("{}".utf8)
            let emptyInput = try JSONDecoder().decode(Empty.self, from: emptyData)

            // Get libraries with stats
            let libraries = try await client.getLibraries(includeStats: true)

            // Update available libraries
            availableLibraries = libraries.map { library in
                LibraryInfo(
                    id: library.id,
                    name: library.name,
                    path: library.path,
                    isDefault: false, // TODO: Determine if this is the default library
                    stats: library.stats
                )
            }

            // Auto-select first library if none is selected
            if currentLibraryId == nil, !libraries.isEmpty {
                do {
                    try await client.switchToLibrary(libraries[0].id)
                    // Set the library ID synchronously
                    currentLibraryId = libraries[0].id
                    // Fetch jobs after setting the library ID
                    await fetchJobList()
                } catch {
                    logger.error("Failed to auto-select first library: \(error.localizedDescription)")
                }
            } else if currentLibraryId != nil {
                // Fetch jobs for the existing library
                await fetchJobList()
            }

            // Now try core status
            let status = try await client.core.status(emptyInput)

            // Update the UI with the core status
            coreStatus = status

        } catch {
            logger.error("Failed to fetch core status: \(error.localizedDescription)")
            // Don't update connection status for core status failures
        }
    }

    // MARK: - Query Execution

    /// Execute a library query
    nonisolated func executeLibraryQuery<T: Codable>(_ query: T) async throws -> T {
        logger.info("Executing library query: \(String(describing: type(of: query)))")
        do {
            let result = try await client.query(query)
            logger.info("Library query executed successfully")
            return result
        } catch {
            logger.error("Library query failed: \(error.localizedDescription)")
            logger.error("Query error details: \(String(describing: error))")
            throw error
        }
    }

    /// Execute a query using the internal client
    nonisolated func query<T: Codable>(_ query: T) async throws -> T {
        logger.info("Executing generic query: \(String(describing: type(of: query)))")
        do {
            let result = try await client.query(query)
            logger.info("Generic query executed successfully")
            return result
        } catch {
            logger.error("Generic query failed: \(error.localizedDescription)")
            logger.error("Query error details: \(String(describing: error))")
            throw error
        }
    }

    /// Execute a FileByPathQuery and return the File result
    nonisolated func queryFileByPath(_ query: FileByPathQuery) async throws -> File? {
        logger.info("Executing FileByPathQuery for path: \(query.path)")
        logger.info("Query details - path: \(query.path)")

        do {
            let result = try await client.queryFileByPath(query)
            if let file = result {
                logger.info("FileByPathQuery successful - found file: \(file.name)")
                logger.info("File details - size: \(file.size), extension: \(file.extension ?? "none")")
            } else {
                logger.warning("FileByPathQuery successful but no file found for path: \(query.path)")
            }
            return result
        } catch {
            logger.error("FileByPathQuery failed for path: \(query.path)")
            logger.error("Error: \(error.localizedDescription)")
            logger.error("Error details: \(String(describing: error))")
            throw error
        }
    }
}
