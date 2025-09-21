import Foundation
import SpacedriveClient

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

        // Check if socket file exists
        guard FileManager.default.fileExists(atPath: socketPath) else {
            DispatchQueue.main.async {
                self.connectionStatus = .error("Daemon socket not found. Is Spacedrive daemon running?")
            }
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
        // For now, we'll parse the event as raw JSON since SpacedriveEvent is a placeholder
        // Once the Event type is properly generated, this will be type-safe

        print("Received event: \(event)")

        // TODO: Parse the actual event structure once Event type has JsonSchema derive
        // For now, we'll create some mock jobs to demonstrate the UI
        DispatchQueue.main.async {
            // Create a sample job for demonstration
            let sampleJob = JobInfo(
                id: UUID().uuidString,
                name: "Sample Job",
                status: .running,
                progress: Double.random(in: 0...1),
                startedAt: Date(),
                completedAt: nil,
                errorMessage: nil
            )
            self.updateOrAddJob(sampleJob)
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

    func reconnect() {
        disconnect()
        DispatchQueue.global().asyncAfter(deadline: .now() + 1.0) {
            self.connect()
        }
    }
}
