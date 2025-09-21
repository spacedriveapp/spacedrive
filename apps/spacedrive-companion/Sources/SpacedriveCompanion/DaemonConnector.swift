import Foundation
import SpacedriveClient

// Response types that match the Rust daemon output
struct LibraryInfo: Codable {
    let id: String
    let name: String
    let path: String
    let stats: LibraryStatistics?
}

struct LibraryStatistics: Codable {
    let total_files: UInt64
    let total_size: UInt64
    let location_count: UInt32
}

struct JobListOutput: Codable {
    let jobs: [JobListItem]
}

struct JobListItem: Codable {
    let id: String
    let name: String
    let status: String
    let progress: Float
}

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

        // Parse the event and update existing jobs based on the event type
        switch event {
        case .string(let stringEvent):
            // Handle simple string events like "CoreStarted", "CoreShutdown"
            print("📡 Core event: \(stringEvent)")

        case .eventClass(let eventData):
            // Handle structured events
            await handleStructuredEvent(eventData)
        }
    }

    private func handleStructuredEvent(_ eventData: EventClass) async {
        DispatchQueue.main.async {
            // Handle job events by updating existing jobs
            if let jobStarted = eventData.jobStarted {
                self.handleJobStarted(jobId: jobStarted.jobID, jobType: jobStarted.jobType)
            } else if let jobProgress = eventData.jobProgress {
                self.handleJobProgress(
                    jobId: jobProgress.jobID,
                    jobType: jobProgress.jobType,
                    progress: jobProgress.progress,
                    message: jobProgress.message
                )
            } else if let jobCompleted = eventData.jobCompleted {
                self.handleJobCompleted(jobId: jobCompleted.jobID, jobType: jobCompleted.jobType)
            } else if let jobFailed = eventData.jobFailed {
                self.handleJobFailed(jobId: jobFailed.jobID, jobType: jobFailed.jobType, error: jobFailed.error)
            } else if let jobPaused = eventData.jobPaused {
                self.handleJobPaused(jobId: jobPaused.jobID)
            } else if let jobResumed = eventData.jobResumed {
                self.handleJobResumed(jobId: jobResumed.jobID)
            }

            // Handle other event types as needed
            if let libraryCreated = eventData.libraryCreated {
                print("📚 Library created: \(libraryCreated.name) at \(libraryCreated.path)")
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

    private func handleJobProgress(jobId: String, jobType: String, progress: Double, message: String?) {
        if var existingJob = job(withId: jobId) {
            existingJob.progress = progress
            if let msg = message {
                existingJob.name = msg // Use the progress message as the job name for better UX
            }
            existingJob.status = .running
            updateOrAddJob(existingJob)
            print("📊 Job progress: \(jobType) (\(jobId)) - \(Int(progress * 100))%")
        } else {
            // If we receive progress for a job we don't know about, create it
            let newJob = JobInfo(
                id: jobId,
                name: message ?? jobType.capitalized,
                status: .running,
                progress: progress,
                startedAt: Date(),
                completedAt: nil,
                errorMessage: nil
            )
            updateOrAddJob(newJob)
            print("📊 New job from progress event: \(jobType) (\(jobId))")
        }
    }

    private func handleJobCompleted(jobId: String, jobType: String) {
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
            print("📋 Fetching current job list...")

            // Also write to a file for debugging
            var debugLog = "📋 Fetching current job list at \(Date())\n"

            func appendLog(_ message: String) {
                debugLog += message + "\n"
                print(message)
                try? debugLog.write(to: URL(fileURLWithPath: "/tmp/companion_debug.log"), atomically: false, encoding: .utf8)
            }

            // First, get the list of libraries (following CLI approach)
            struct ListLibrariesQuery: Codable {
                let include_stats: Bool

                init() {
                    self.include_stats = false // Basic query like CLI
                }
            }

            // Get libraries list like the CLI does
            appendLog("📚 Getting libraries list...")
            let librariesQuery = ListLibrariesQuery()
            let librariesResponse = try await client.executeQuery(
                librariesQuery,
                method: "query:libraries.list.v1",
                responseType: [LibraryInfo].self
            )

            appendLog("📚 Received libraries response (\(librariesResponse.count) libraries)")

            if librariesResponse.isEmpty {
                appendLog("📚 No libraries found - skipping job list")
                return
            }

            // Use the first library from the response
            let firstLibrary = librariesResponse[0]
            appendLog("📚 Using library: \(firstLibrary.name) (ID: \(firstLibrary.id))")

            // Now query jobs for this library
            // JobListQuery takes an optional status filter
            struct JobListQuery: Codable {
                let status: String?

                init() {
                    self.status = nil
                }
            }

            let jobQuery = JobListQuery()
            let jobsResponse = try await client.executeQuery(
                jobQuery,
                method: "query:jobs.list.v1",
                responseType: JobListOutput.self
            )

            appendLog("📋 Received job list response (\(jobsResponse.jobs.count) jobs)")

            // Convert JobListItem to JobInfo for the UI
            let convertedJobs = jobsResponse.jobs.map { jobItem in
                JobInfo(
                    id: jobItem.id,
                    name: jobItem.name,
                    status: JobStatus(rawValue: jobItem.status.lowercased()) ?? .queued,
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
                appendLog("  \(index + 1). \(job.id) \(job.name) \(Int(job.progress * 100))% \(job.status.displayName)")
            }

        } catch {
            print("❌ Failed to fetch job list: \(error)")
            DispatchQueue.main.async {
                self.connectionStatus = .error("Failed to fetch jobs: \(error.localizedDescription)")
            }
        }
    }
}
