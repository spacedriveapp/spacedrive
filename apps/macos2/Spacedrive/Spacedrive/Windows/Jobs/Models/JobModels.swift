import Foundation
import SpacedriveClient

extension JsonValue {
    var stringValue: String? {
        if case let .string(string) = self {
            return string
        }
        return nil
    }

    var dictionaryValue: [String: JsonValue]? {
        if case let .object(dict) = self {
            return dict
        }
        return nil
    }
}

// MARK: - Job Models

struct JobInfo: Codable, Identifiable {
    let id: String
    var name: String
    var status: JobStatus
    var progress: Double
    let startedAt: Date
    var completedAt: Date?
    var errorMessage: String?

    // Enhanced fields from improved job progress events
    var currentPhase: String?
    var completionInfo: String?
    var hasIssues: Bool = false
    var issuesInfo: String?
    var currentPath: String? // Current file/directory being processed
    var jobType: String? // Store the original job type for title display

    // Action context fields for richer job information
    var actionType: String?
    var actionContext: ActionContextInfo?

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case status
        case progress
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case errorMessage = "error_message"
        case currentPhase = "current_phase"
        case completionInfo = "completion_info"
        case hasIssues = "has_issues"
        case issuesInfo = "issues_info"
        case currentPath = "current_path"
        case jobType = "job_type"
        case actionType = "action_type"
        case actionContext = "action_context"
    }

    // Convenience initializer from generated JobListItem
    init(from jobListItem: JobListItem) {
        id = jobListItem.id
        name = jobListItem.name
        status = JobStatus(rawValue: jobListItem.status.rawValue) ?? .failed
        progress = Double(jobListItem.progress)
        startedAt = Date() // TODO: Get actual start time from daemon
        completedAt = nil // TODO: Get actual completion time from daemon
        errorMessage = nil // TODO: Get error message from daemon if status is failed
        currentPhase = nil
        completionInfo = nil
        hasIssues = false
        issuesInfo = nil
        currentPath = nil
        jobType = nil
        actionType = jobListItem.actionType
        actionContext = jobListItem.actionContext
    }

    // Convenience initializer for creating JobInfo with individual parameters
    init(id: String, name: String, status: JobStatus, progress: Double, startedAt: Date, completedAt: Date? = nil, errorMessage: String? = nil, currentPhase: String? = nil, completionInfo: String? = nil, hasIssues: Bool = false, issuesInfo: String? = nil, currentPath: String? = nil, jobType: String? = nil, actionType: String? = nil, actionContext: ActionContextInfo? = nil) {
        self.id = id
        self.name = name
        self.status = status
        self.progress = progress
        self.startedAt = startedAt
        self.completedAt = completedAt
        self.errorMessage = errorMessage
        self.currentPhase = currentPhase
        self.completionInfo = completionInfo
        self.hasIssues = hasIssues
        self.issuesInfo = issuesInfo
        self.currentPath = currentPath
        self.jobType = jobType
        self.actionType = actionType
        self.actionContext = actionContext
    }

    // Computed property for richer job name display using raw data
    var displayName: String {
        // If we have action context, extract meaningful info from the raw data
        if let actionContext = actionContext {
            switch actionContext.actionType {
            case "locations.add":
                let inputObj = actionContext.actionInput.dictionaryValue
                if let pathValue = inputObj?["path"],
                   let path = pathValue.stringValue
                {
                    // Extract directory name from path
                    let directoryName = URL(fileURLWithPath: path).lastPathComponent
                    return "Adding '\(directoryName)'"
                }
                return "Adding Location"
            case "files.copy":
                let inputObj = actionContext.actionInput.dictionaryValue
                if let sourceValue = inputObj?["source"],
                   let source = sourceValue.stringValue
                {
                    let sourceName = URL(fileURLWithPath: source).lastPathComponent
                    return "Copying '\(sourceName)'"
                }
                return "Copying Files"
            case "files.move":
                let inputObj = actionContext.actionInput.dictionaryValue
                if let sourceValue = inputObj?["source"],
                   let source = sourceValue.stringValue
                {
                    let sourceName = URL(fileURLWithPath: source).lastPathComponent
                    return "Moving '\(sourceName)'"
                }
                return "Moving Files"
            case "files.delete":
                let inputObj = actionContext.actionInput.dictionaryValue
                if let targetValue = inputObj?["target"],
                   let target = targetValue.stringValue
                {
                    let targetName = URL(fileURLWithPath: target).lastPathComponent
                    return "Deleting '\(targetName)'"
                }
                return "Deleting Files"
            case "media.thumbnail":
                return "Generating Thumbnails"
            case "media.extract":
                return "Extracting Media"
            default:
                // For unknown action types, try to format them nicely
                return actionContext.actionType.replacingOccurrences(of: ".", with: " ").capitalized
            }
        }

        // Fallback to the original name or job type
        return jobType?.capitalized ?? name.capitalized
    }

    // Computed property for additional context information
    var contextInfo: String? {
        guard let actionContext = actionContext else { return nil }

        // Extract meaningful info from raw action input
        switch actionContext.actionType {
        case "locations.add":
            let inputObj = actionContext.actionInput.dictionaryValue
            if let pathValue = inputObj?["path"],
               let path = pathValue.stringValue
            {
                // Extract directory name from path
                let directoryName = URL(fileURLWithPath: path).lastPathComponent
                return "Adding '\(directoryName)' at \(path)"
            }
        case "files.copy":
            let inputObj = actionContext.actionInput.dictionaryValue
            if let sourceValue = inputObj?["source"],
               let source = sourceValue.stringValue
            {
                if let destValue = inputObj?["destination"],
                   let destination = destValue.stringValue
                {
                    return "Copying from \(source) to \(destination)"
                } else {
                    return "Copying from \(source)"
                }
            }
        case "files.move":
            let inputObj = actionContext.actionInput.dictionaryValue
            if let sourceValue = inputObj?["source"],
               let source = sourceValue.stringValue
            {
                if let destValue = inputObj?["destination"],
                   let destination = destValue.stringValue
                {
                    return "Moving from \(source) to \(destination)"
                } else {
                    return "Moving from \(source)"
                }
            }
        case "files.delete":
            let inputObj = actionContext.actionInput.dictionaryValue
            if let targetValue = inputObj?["target"],
               let target = targetValue.stringValue
            {
                return "Deleting \(target)"
            }
        case "media.thumbnail":
            return "Generating thumbnails for media files"
        case "media.extract":
            return "Extracting metadata from media files"
        default:
            break
        }

        return nil
    }
}

enum JobStatus: String, Codable, CaseIterable {
    case running
    case completed
    case failed
    case paused
    case queued
    case cancelled

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
        case .cancelled:
            return "Cancelled"
        }
    }

    init(from generatedStatus: JobStatus) {
        switch generatedStatus {
        case .running:
            self = .running
        case .completed:
            self = .completed
        case .failed:
            self = .failed
        case .paused:
            self = .paused
        case .queued:
            self = .queued
        case .cancelled:
            self = .cancelled
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
        case .cancelled:
            return "xmark.circle.fill"
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
        case let .error(message):
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
