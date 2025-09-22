import Foundation
import SpacedriveClient

// MARK: - Job Models

struct JobInfo: Codable, Identifiable {
    let id: String
    var name: String
    var status: JobStatus
    var progress: Double
    let startedAt: Date
    var completedAt: Date?
    var errorMessage: String?

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
            return "circle.fill"
        case .completed:
            return "checkmark.circle.fill"
        case .failed:
            return "xmark.circle.fill"
        case .paused:
            return "pause.circle.fill"
        case .queued:
            return "clock.fill"
        }
    }
}

// MARK: - Legacy Models (to be replaced with generated types)

// TODO: Replace these with generated types once Event has JsonSchema derive
// For now, keeping minimal models for the companion app

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


