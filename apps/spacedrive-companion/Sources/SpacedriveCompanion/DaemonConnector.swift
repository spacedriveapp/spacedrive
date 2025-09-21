import Foundation
import Darwin

class DaemonConnector: ObservableObject {
    @Published var connectionStatus: ConnectionStatus = .disconnected
    @Published var jobs: [JobInfo] = []

    private let socketPath = "\(NSHomeDirectory())/Library/Application Support/spacedrive/daemon/daemon.sock"
    private let queue = DispatchQueue(label: "daemon-connector", qos: .userInitiated)
    private var isConnected = false

    init() {
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

        // Try to establish connection by sending initial messages
        queue.async {
            self.subscribeToEvents()
            self.fetchInitialJobs()
        }
    }

    func disconnect() {
        isConnected = false
        DispatchQueue.main.async {
            self.connectionStatus = .disconnected
        }
    }

    private func subscribeToEvents() {
        let eventTypes = [
            "JobStarted",
            "JobProgress",
            "JobCompleted",
            "JobFailed",
            "JobPaused",
            "JobResumed"
        ]
        let subscription = DaemonRequest.subscribe(eventTypes: eventTypes, filter: nil)
        sendMessage(subscription)
    }

    private func fetchInitialJobs() {
        // For now, let's try a simple ping to test the connection
        let ping = DaemonRequest.ping
        sendMessage(ping)
    }

    private func sendMessage<T: Codable>(_ message: T) {
        do {
            let jsonData = try JSONEncoder().encode(message)
            let messageString = String(data: jsonData, encoding: .utf8) ?? ""
            let messageWithNewline = messageString + "\n"

            print("Sending message: \(messageString)")

            // Use a simple approach - write to socket and read response
            sendAndReceive(messageWithNewline)
        } catch {
            print("Failed to encode message: \(error)")
        }
    }

    private func sendAndReceive(_ message: String) {
        queue.async {
                // Create socket
                let socketFD = socket(AF_UNIX, SOCK_STREAM, 0)
                guard socketFD != -1 else {
                    print("Failed to create socket")
                    return
                }

                defer { close(socketFD) }

                // Set up address
                var addr = sockaddr_un()
                addr.sun_family = sa_family_t(AF_UNIX)
                let pathBytes = self.socketPath.utf8CString
                let pathSize = MemoryLayout.size(ofValue: addr.sun_path)
                guard pathBytes.count <= pathSize else {
                    print("Socket path too long")
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
                    print("Failed to connect to socket: \(String(cString: strerror(errno)))")
                    return
                }

                // Send message
                let messageData = message.data(using: .utf8) ?? Data()
                let sendResult = messageData.withUnsafeBytes { bytes in
                    send(socketFD, bytes.bindMemory(to: UInt8.self).baseAddress, messageData.count, 0)
                }

                guard sendResult != -1 else {
                    print("Failed to send message: \(String(cString: strerror(errno)))")
                    return
                }

                // Read response
                var buffer = [UInt8](repeating: 0, count: 4096)
                let readResult = recv(socketFD, &buffer, buffer.count, 0)

                guard readResult > 0 else {
                    print("Failed to read response: \(String(cString: strerror(errno)))")
                    return
                }

                let responseData = Data(buffer.prefix(readResult))
                if let responseString = String(data: responseData, encoding: .utf8) {
                    print("Received response: \(responseString)")
                    self.processReceivedData(responseData)
                }
        }
    }


    private func processReceivedData(_ data: Data) {
        guard let string = String(data: data, encoding: .utf8) else {
            return
        }

        // Split by newlines as messages are line-delimited
        let lines = string.components(separatedBy: .newlines).filter { !$0.isEmpty }

        for line in lines {
            guard let lineData = line.data(using: .utf8) else { continue }
            processMessage(lineData)
        }
    }

    private func processMessage(_ data: Data) {
        do {
            // Try to decode as DaemonResponse
            if let response = try? JSONDecoder().decode(DaemonResponse.self, from: data) {
                handleDaemonResponse(response)
                return
            }

            // Log unhandled message for debugging
            if let jsonString = String(data: data, encoding: .utf8) {
                print("Unhandled message: \(jsonString)")
            }
        }
    }

    private func handleDaemonResponse(_ response: DaemonResponse) {
        // If we're receiving any response, the connection is working
        DispatchQueue.main.async {
            if self.connectionStatus != .connected {
                self.connectionStatus = .connected
            }
        }

        switch response {
        case .pong:
            print("Received pong from daemon")
        case .ok(let bytes):
            print("Received OK response with \(bytes.count) bytes")
            // For now, we can't easily decode bincode in Swift, so we'll skip job list queries
            // and rely on events for job updates
        case .error(let error):
            print("Daemon error: \(error)")
            DispatchQueue.main.async {
                self.connectionStatus = .error(error)
            }
        case .subscribed:
            print("Successfully subscribed to events")
        case .unsubscribed:
            print("Unsubscribed from events")
        case .event(let event):
            handleEvent(event)
        }
    }

    private func handleEvent(_ event: Event) {
        DispatchQueue.main.async {
            switch event {
            case .jobStarted(let jobId, let jobType):
                print("Job started: \(jobType) [\(String(jobId.prefix(8)))]")
                let jobInfo = JobInfo(
                    id: jobId,
                    name: jobType,
                    status: .running,
                    progress: 0.0,
                    startedAt: Date(),
                    completedAt: nil,
                    errorMessage: nil
                )
                self.updateOrAddJob(jobInfo)

            case .jobProgress(let jobId, let jobType, let progress, let message):
                print("Job progress: \(jobType) [\(String(jobId.prefix(8)))] - \(Int(progress * 100))%")
                let jobInfo = JobInfo(
                    id: jobId,
                    name: jobType,
                    status: .running,
                    progress: Double(progress),
                    startedAt: Date(), // We don't have the original start time
                    completedAt: nil,
                    errorMessage: nil
                )
                self.updateOrAddJob(jobInfo)

            case .jobCompleted(let jobId, let jobType):
                print("Job completed: \(jobType) [\(String(jobId.prefix(8)))]")
                let jobInfo = JobInfo(
                    id: jobId,
                    name: jobType,
                    status: .completed,
                    progress: 1.0,
                    startedAt: Date(), // We don't have the original start time
                    completedAt: Date(),
                    errorMessage: nil
                )
                self.updateOrAddJob(jobInfo)

            case .jobFailed(let jobId, let jobType, let error):
                print("Job failed: \(jobType) [\(String(jobId.prefix(8)))] - \(error)")
                let jobInfo = JobInfo(
                    id: jobId,
                    name: jobType,
                    status: .failed,
                    progress: 0.0,
                    startedAt: Date(), // We don't have the original start time
                    completedAt: Date(),
                    errorMessage: error
                )
                self.updateOrAddJob(jobInfo)

            case .jobPaused(let jobId):
                print("Job paused: [\(String(jobId.prefix(8)))]")
                if let index = self.jobs.firstIndex(where: { $0.id == jobId }) {
                    var updatedJob = self.jobs[index]
                    updatedJob = JobInfo(
                        id: updatedJob.id,
                        name: updatedJob.name,
                        status: .paused,
                        progress: updatedJob.progress,
                        startedAt: updatedJob.startedAt,
                        completedAt: updatedJob.completedAt,
                        errorMessage: updatedJob.errorMessage
                    )
                    self.jobs[index] = updatedJob
                }

            case .jobResumed(let jobId):
                print("Job resumed: [\(String(jobId.prefix(8)))]")
                if let index = self.jobs.firstIndex(where: { $0.id == jobId }) {
                    var updatedJob = self.jobs[index]
                    updatedJob = JobInfo(
                        id: updatedJob.id,
                        name: updatedJob.name,
                        status: .running,
                        progress: updatedJob.progress,
                        startedAt: updatedJob.startedAt,
                        completedAt: nil,
                        errorMessage: updatedJob.errorMessage
                    )
                    self.jobs[index] = updatedJob
                }

            case .jobCancelled(let jobId, let jobType):
                print("Job cancelled: \(jobType) [\(String(jobId.prefix(8)))]")
                // Remove cancelled jobs from the list
                self.jobs.removeAll { $0.id == jobId }

            case .other:
                print("Received other event type")
            }
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
