import Foundation

// MARK: - Job Models

struct JobInfo: Codable, Identifiable {
    let id: String
    let name: String
    let status: JobStatus
    let progress: Double
    let startedAt: Date
    let completedAt: Date?
    let errorMessage: String?

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case status
        case progress
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case errorMessage = "error_message"
    }
}

enum JobStatus: String, Codable, CaseIterable {
    case running = "Running"
    case completed = "Completed"
    case failed = "Failed"
    case paused = "Paused"
    case queued = "Queued"

    var displayName: String {
        switch self {
        case .running:
            return "Running"
        case .completed:
            return "Completed"
        case .failed:
            return "Failed"
        case .paused:
            return "Paused"
        case .queued:
            return "Queued"
        }
    }

    var icon: String {
        switch self {
        case .running:
            return "⚡️"
        case .completed:
            return "✅"
        case .failed:
            return "❌"
        case .paused:
            return "⏸️"
        case .queued:
            return "⏳"
        }
    }
}

// MARK: - RPC Models

// Daemon Request enum matching the Rust implementation
enum DaemonRequest: Codable {
    case ping
    case query(method: String, payload: [UInt8])
    case subscribe(eventTypes: [String], filter: EventFilter?)
    case unsubscribe
    case shutdown

    private struct QueryData: Codable {
        let method: String
        let payload: [UInt8]
    }

    private struct SubscribeData: Codable {
        let event_types: [String]
        let filter: EventFilter?
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        switch self {
        case .ping:
            try container.encode("Ping")
        case .query(let method, let payload):
            let queryDict = ["Query": QueryData(method: method, payload: payload)]
            try container.encode(queryDict)
        case .subscribe(let eventTypes, let filter):
            let subscribeDict = ["Subscribe": SubscribeData(event_types: eventTypes, filter: filter)]
            try container.encode(subscribeDict)
        case .unsubscribe:
            try container.encode("Unsubscribe")
        case .shutdown:
            try container.encode("Shutdown")
        }
    }

    init(from decoder: Decoder) throws {
        // This is mainly for decoding responses, we don't expect to decode requests
        throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "DaemonRequest decoding not implemented"))
    }
}

// Event filter for subscriptions
struct EventFilter: Codable {
    let libraryId: String?
    let jobId: String?
    let deviceId: String?

    enum CodingKeys: String, CodingKey {
        case libraryId = "library_id"
        case jobId = "job_id"
        case deviceId = "device_id"
    }
}

// Daemon Response enum
enum DaemonResponse: Codable {
    case pong
    case ok([UInt8])
    case error(String)
    case subscribed
    case unsubscribed
    case event(Event)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if let stringValue = try? container.decode(String.self) {
            switch stringValue {
            case "Pong":
                self = .pong
            case "Subscribed":
                self = .subscribed
            case "Unsubscribed":
                self = .unsubscribed
            default:
                throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unknown string response: \(stringValue)"))
            }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            if let okData = dict["Ok"] {
                if let bytes = okData.value as? [UInt8] {
                    self = .ok(bytes)
                } else {
                    throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Invalid Ok payload"))
                }
            } else if let errorMsg = dict["Error"] {
                if let errorString = errorMsg.value as? String {
                    self = .error(errorString)
                } else {
                    throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Invalid Error payload"))
                }
            } else if let eventData = dict["Event"] {
                // Try to decode the event
                let eventJson = try JSONSerialization.data(withJSONObject: eventData.value)
                let event = try JSONDecoder().decode(Event.self, from: eventJson)
                self = .event(event)
            } else {
                throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unknown dict response"))
            }
        } else {
            throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Invalid response format"))
        }
    }

    func encode(to encoder: Encoder) throws {
        // We don't need to encode responses, only decode them
        throw EncodingError.invalidValue(self, EncodingError.Context(codingPath: encoder.codingPath, debugDescription: "DaemonResponse encoding not implemented"))
    }
}

// Helper for decoding arbitrary JSON values
struct AnyCodable: Codable {
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
            throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Cannot decode value"))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        if let intValue = value as? Int {
            try container.encode(intValue)
        } else if let doubleValue = value as? Double {
            try container.encode(doubleValue)
        } else if let stringValue = value as? String {
            try container.encode(stringValue)
        } else if let boolValue = value as? Bool {
            try container.encode(boolValue)
        } else {
            throw EncodingError.invalidValue(value, EncodingError.Context(codingPath: encoder.codingPath, debugDescription: "Cannot encode value"))
        }
    }
}

// Job List Query Input (for bincode encoding)
struct JobListQueryInput: Codable {
    let status: String?

    init(status: String? = nil) {
        self.status = status
    }
}

// Job List Output (matches Rust JobListOutput)
struct JobListOutput: Codable {
    let jobs: [JobListItem]
}

struct JobListItem: Codable {
    let id: String
    let name: String
    let status: JobStatus
    let progress: Float
}

// Event enum matching the Rust Event enum
enum Event: Codable {
    case jobStarted(jobId: String, jobType: String)
    case jobProgress(jobId: String, jobType: String, progress: Float, message: String?)
    case jobCompleted(jobId: String, jobType: String)
    case jobFailed(jobId: String, jobType: String, error: String)
    case jobCancelled(jobId: String, jobType: String)
    case jobPaused(jobId: String)
    case jobResumed(jobId: String)
    case other

    enum CodingKeys: String, CodingKey {
        case jobStarted = "JobStarted"
        case jobProgress = "JobProgress"
        case jobCompleted = "JobCompleted"
        case jobFailed = "JobFailed"
        case jobCancelled = "JobCancelled"
        case jobPaused = "JobPaused"
        case jobResumed = "JobResumed"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        if let jobStartedData = try? container.decode([String: String].self, forKey: .jobStarted) {
            self = .jobStarted(
                jobId: jobStartedData["job_id"] ?? "",
                jobType: jobStartedData["job_type"] ?? ""
            )
        } else if let jobProgressData = try? container.decode([String: AnyCodable].self, forKey: .jobProgress) {
            self = .jobProgress(
                jobId: (jobProgressData["job_id"]?.value as? String) ?? "",
                jobType: (jobProgressData["job_type"]?.value as? String) ?? "",
                progress: Float((jobProgressData["progress"]?.value as? Double) ?? 0.0),
                message: jobProgressData["message"]?.value as? String
            )
        } else if let jobCompletedData = try? container.decode([String: String].self, forKey: .jobCompleted) {
            self = .jobCompleted(
                jobId: jobCompletedData["job_id"] ?? "",
                jobType: jobCompletedData["job_type"] ?? ""
            )
        } else if let jobFailedData = try? container.decode([String: String].self, forKey: .jobFailed) {
            self = .jobFailed(
                jobId: jobFailedData["job_id"] ?? "",
                jobType: jobFailedData["job_type"] ?? "",
                error: jobFailedData["error"] ?? ""
            )
        } else if let jobCancelledData = try? container.decode([String: String].self, forKey: .jobCancelled) {
            self = .jobCancelled(
                jobId: jobCancelledData["job_id"] ?? "",
                jobType: jobCancelledData["job_type"] ?? ""
            )
        } else if let jobPausedData = try? container.decode([String: String].self, forKey: .jobPaused) {
            self = .jobPaused(jobId: jobPausedData["job_id"] ?? "")
        } else if let jobResumedData = try? container.decode([String: String].self, forKey: .jobResumed) {
            self = .jobResumed(jobId: jobResumedData["job_id"] ?? "")
        } else {
            self = .other
        }
    }

    func encode(to encoder: Encoder) throws {
        // We don't need to encode events, only decode them
        throw EncodingError.invalidValue(self, EncodingError.Context(codingPath: encoder.codingPath, debugDescription: "Event encoding not implemented"))
    }
}

// MARK: - Connection Status

enum ConnectionStatus: Equatable {
    case disconnected
    case connecting
    case connected
    case error(String)

    var displayName: String {
        switch self {
        case .disconnected:
            return "Disconnected"
        case .connecting:
            return "Connecting..."
        case .connected:
            return "Connected"
        case .error(let message):
            return "Error: \(message)"
        }
    }

    var color: String {
        switch self {
        case .disconnected, .error:
            return "red"
        case .connecting:
            return "yellow"
        case .connected:
            return "green"
        }
    }
}


