import Foundation
import SpacedriveClient

// Now using generated types from SpacedriveClient - no manual definitions needed!

class DaemonConnector: ObservableObject {
    @Published var connectionStatus: ConnectionStatus = .disconnected
    @Published var jobs: [JobInfo] = []

    private let socketPath = "\(NSHomeDirectory())/Library/Application Support/spacedrive/daemon/daemon.sock"
    private let client: SpacedriveClient
    private var eventTask: Task<Void, Never>?

    init() {
        self.client = SpacedriveClient(socketPath: socketPath)
        connect()
    }

    deinit {
        disconnect()
    }

    func connect() {
        guard connectionStatus != .connecting && connectionStatus != .connected else {
            return
        }

        DispatchQueue.main.async {
            self.connectionStatus = .connecting
        }

        print("🔌 DaemonConnector.connect() called - starting connection process")

        // Check if socket file exists
        guard FileManager.default.fileExists(atPath: socketPath) else {
            print("❌ Daemon socket not found at: \(socketPath)")
            DispatchQueue.main.async {
                self.connectionStatus = .error("Daemon socket not found. Is Spacedrive daemon running?")
            }
            return
        }

        print("✅ Daemon socket found at: \(socketPath)")

        // Start connection and event subscription
        Task {
            await startConnection()
        }
    }

    func disconnect() {
        eventTask?.cancel()
        eventTask = nil
        DispatchQueue.main.async {
            self.connectionStatus = .disconnected
        }
    }

    private func startConnection() async {
        do {
            print("🔌 Starting connection to daemon at: \(socketPath)")

            // Test connection with ping (with timeout)
            try await withTimeout(seconds: 5) { [self] in
                print("📡 Sending ping to daemon...")
                try await client.ping()
                print("✅ Ping successful!")
            }

            DispatchQueue.main.async {
                self.connectionStatus = .connected
            }

            print("🎧 Starting event subscription...")
            // Start event subscription
            await subscribeToEvents()

        } catch {
            print("❌ Connection failed: \(error)")
            DispatchQueue.main.async {
                self.connectionStatus = .error("Failed to connect: \(error.localizedDescription)")
            }
        }
    }

    private func withTimeout<T>(seconds: TimeInterval, operation: @escaping () async throws -> T) async throws -> T {
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
            "JobResumed"
        ]

        // First, fetch the current job list to establish baseline state
        await fetchJobList()

        // Then subscribe to events for real-time updates
        eventTask = Task {
            do {
                for try await event in client.subscribe(to: eventTypes) {
                    await handleEvent(event)
                }
            } catch {
                DispatchQueue.main.async {
                    self.connectionStatus = .error("Event subscription failed: \(error.localizedDescription)")
                }
            }
        }
    }

    private func handleEvent(_ event: SpacedriveEvent) async {
        print("Received event: \(event)")

        // Handle events using the new type-safe Event enum
        DispatchQueue.main.async {
            switch event {
            // Core lifecycle events
            case .coreStarted:
                print("📡 Core started")

            case .coreShutdown:
                print("📡 Core shutdown")

            // Job events - update existing jobs in real-time
            case .jobStarted(let data):
                self.handleJobStarted(jobId: data.jobId, jobType: data.jobType)

            case .jobProgress(let data):
                self.handleJobProgress(
                    jobId: data.jobId,
                    jobType: data.jobType,
                    progress: data.progress,
                    message: data.message,
                    genericProgress: data.genericProgress
                )

            case .jobCompleted(let data):
                self.handleJobCompleted(jobId: data.jobId, jobType: data.jobType, output: data.output)

            case .jobFailed(let data):
                self.handleJobFailed(jobId: data.jobId, jobType: data.jobType, error: data.error)

            case .jobPaused(let data):
                self.handleJobPaused(jobId: data.jobId)

            case .jobResumed(let data):
                self.handleJobResumed(jobId: data.jobId)

            // Library events
            case .libraryCreated(let data):
                print("📚 Library created: \(data.name) at \(data.path)")

            case .libraryOpened(let data):
                print("📚 Library opened: \(data.name)")

            // Other events can be handled as needed
            default:
                print("📡 Other event: \(event)")
            }
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
            print("🚀 Job started: \(jobType) (\(jobId))")
        }
    }

    private func handleJobProgress(jobId: String, jobType: String, progress: Double, message: String?, genericProgress: GenericProgress?) {
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
            if let generic = genericProgress {
                print("📊 Job progress: \(jobType) (\(jobId)) - \(generic.phase) \(Int(generic.percentage * 100))% (\(generic.completion.completed)/\(generic.completion.total))")
            } else {
                print("📊 Job progress: \(jobType) (\(jobId)) - \(Int(progress * 100))%")
            }
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
            print("📊 New job from progress event: \(jobType) (\(jobId))")
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
        case .physical(let pathData):
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


    private func handleJobCompleted(jobId: String, jobType: String, output: JobOutput) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .completed
            existingJob.progress = 1.0
            existingJob.completedAt = Date()
            updateOrAddJob(existingJob)
            print("✅ Job completed: \(jobType) (\(jobId))")
        }
    }

    private func handleJobFailed(jobId: String, jobType: String, error: String) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .failed
            existingJob.errorMessage = error
            updateOrAddJob(existingJob)
            print("❌ Job failed: \(jobType) (\(jobId)) - \(error)")
        }
    }

    private func handleJobPaused(jobId: String) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .paused
            updateOrAddJob(existingJob)
            print("⏸️ Job paused: (\(jobId))")
        }
    }

    private func handleJobResumed(jobId: String) {
        if var existingJob = job(withId: jobId) {
            existingJob.status = .running
            updateOrAddJob(existingJob)
            print("▶️ Job resumed: (\(jobId))")
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
        DispatchQueue.global().asyncAfter(deadline: .now() + 1.0) {
            self.connect()
        }
    }

    // MARK: - Job List Management

    private func fetchJobList() async {
        do {
            // Get libraries using the new type-safe method
            print("📚 Getting libraries list...")
            let librariesResponse = try await client.getLibraries(includeStats: false)

            print("📚 Received libraries response (\(librariesResponse.count) libraries)")

            if librariesResponse.isEmpty {
                print("📚 No libraries found - skipping job list")
                return
            }

            // Use the first library from the response
            let firstLibrary = librariesResponse[0]
            print("📚 Using library: \(firstLibrary.name) (ID: \(firstLibrary.id))")

            // Get jobs using the new type-safe method
            let jobsResponse = try await client.getJobs()

            print("📋 Received job list response (\(jobsResponse.jobs.count) jobs)")

            // Convert JobListItem to JobInfo for the UI
            let convertedJobs = jobsResponse.jobs.map { jobItem in
                // Convert from generated SpacedriveClient.JobStatus to local JobStatus
                let localStatus: SpacedriveCompanion.JobStatus = {
                    switch jobItem.status {
                    case .queued: return .queued
                    case .running: return .running
                    case .paused: return .paused
                    case .completed: return .completed
                    case .failed: return .failed
                    case .cancelled: return .failed // Map cancelled to failed for now
                    }
                }()

                return JobInfo(
                    id: jobItem.id,
                    name: jobItem.name,
                    status: localStatus,
                    progress: Double(jobItem.progress),
                    startedAt: Date(), // TODO: Get actual start time from daemon
                    completedAt: nil,  // TODO: Get actual completion time from daemon
                    errorMessage: nil  // TODO: Get error message from daemon if status is failed
                )
            }

            // Update the UI with the converted jobs
            DispatchQueue.main.async {
                self.jobs = convertedJobs
            }

            // Log the jobs for debugging
            for (index, job) in convertedJobs.enumerated() {
                print("  \(index + 1). \(job.id) \(job.name) \(Int(job.progress * 100))% \(job.status.displayName)")
            }

        } catch {
            print("❌ Failed to fetch job list: \(error)")
            DispatchQueue.main.async {
                self.connectionStatus = .error("Failed to fetch jobs: \(error.localizedDescription)")
            }
        }
    }
}
