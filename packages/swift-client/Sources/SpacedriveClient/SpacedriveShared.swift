import Foundation

// MARK: - Shared Types for All Platforms

/// Errors that can occur when using the Spacedrive client
public enum SpacedriveError: Error, LocalizedError {
    case connectionFailed(String)
    case serializationError(String)
    case daemonError(String)
    case invalidResponse(String)

    public var errorDescription: String? {
        switch self {
        case .connectionFailed(let message):
            return "Connection failed: \(message)"
        case .serializationError(let message):
            return "Serialization error: \(message)"
        case .daemonError(let message):
            return "Daemon error: \(message)"
        case .invalidResponse(let message):
            return "Invalid response: \(message)"
        }
    }
}

/// Helper struct for requests with no parameters
public struct Empty: Codable {
    public init() {}
}

/// Type alias for the generated Event type from types.swift
public typealias SpacedriveEvent = Event

/// Connection status for clients
public enum ConnectionStatus: Equatable {
    case disconnected
    case connecting
    case connected
    case error(String)
}
