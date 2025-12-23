//
//  PairingExtensions.swift
//  SpacedriveClient
//
//  Device pairing functionality built on top of the core NetworkAPI
//

import Foundation

// MARK: - Pairing Session Management

/// High-level pairing session state for UI binding
public enum PairingFlowState {
    case idle
    case generating
    case waitingForConnection(code: String, expiresAt: Date)
    case joining(code: String)
    case connecting(state: SerializablePairingState)
    case completed(deviceName: String, deviceId: String)
    case failed(error: String)

    public var isActive: Bool {
        switch self {
        case .idle, .completed, .failed:
            return false
        default:
            return true
        }
    }

    public var displayMessage: String {
        switch self {
        case .idle:
            return "Ready to pair"
        case .generating:
            return "Generating pairing code..."
        case .waitingForConnection(let code, let expiresAt):
            let timeRemaining = Int(expiresAt.timeIntervalSinceNow)
            return "Share this code: \(code) (expires in \(timeRemaining)s)"
        case .joining(let code):
            return "Joining with code: \(code)"
        case .connecting(let state):
            return state.displayMessage
        case .completed(let deviceName, _):
            return "Successfully paired with \(deviceName)"
        case .failed(let error):
            return "Pairing failed: \(error)"
        }
    }
}

// MARK: - SerializablePairingState Display Extensions

extension SerializablePairingState {
    public var displayMessage: String {
        switch self {
        case .idle:
            return "Ready to pair"
        case .generatingCode:
            return "Generating pairing code..."
        case .broadcasting:
            return "Broadcasting pairing code..."
        case .scanning:
            return "Scanning for devices..."
        case .waitingForConnection:
            return "Waiting for connection..."
        case .connecting:
            return "Connecting..."
        case .authenticating:
            return "Authenticating..."
        case .exchangingKeys:
            return "Exchanging encryption keys..."
        case .awaitingConfirmation:
            return "Awaiting confirmation..."
        case .establishingSession:
            return "Establishing secure session..."
        case .challengeReceived:
            return "Processing authentication..."
        case .responsePending:
            return "Waiting for response..."
        case .responseSent:
            return "Response sent..."
        case .completed:
            return "Pairing completed successfully"
        case .failed(let failed):
            return "Pairing failed: \(failed.reason)"
        }
    }

    public var isTerminalState: Bool {
        switch self {
        case .completed, .failed:
            return true
        default:
            return false
        }
    }
}

// MARK: - SpacedriveClient Pairing Extensions

extension SpacedriveClient {

    // MARK: - High-Level Pairing Methods

    /// Start pairing as initiator (generates a pairing code)
    /// - Returns: Pairing session information with code and expiration
    public func startPairingAsInitiator() async throws -> PairGenerateOutput {
        let input = PairGenerateInput()
        return try await network.pairGenerate(input)
    }

    /// Join an existing pairing session using a code
    /// - Parameter code: The 12-word BIP39 pairing code
    /// - Returns: Information about the newly paired device
    public func joinPairingSession(code: String) async throws -> PairJoinOutput {
        let input = PairJoinInput(code: code)
        return try await network.pairJoin(input)
    }

    /// Cancel an active pairing session
    /// - Parameter sessionId: The UUID of the session to cancel
    /// - Returns: Whether the cancellation was successful
    public func cancelPairingSession(sessionId: String) async throws -> Bool {
        let input = PairCancelInput(sessionId: sessionId)
        let output = try await network.pairCancel(input)
        return output.cancelled
    }

    /// Get the current status of all pairing sessions
    /// - Returns: List of active pairing sessions
    public func getPairingStatus() async throws -> [PairingSessionSummary] {
        let input = PairStatusQueryInput()
        let output = try await network.pairStatus(input)
        return output.sessions
    }

    /// Get list of paired devices
    /// - Parameter connectedOnly: If true, only return currently connected devices
    /// - Returns: List of device information
    public func getPairedDevices(connectedOnly: Bool = false) async throws -> [PairedDeviceInfo] {
        let input = ListPairedDevicesInput(connectedOnly: connectedOnly)
        let output = try await network.devicesList(input)
        return output.devices
    }

    /// Get network status including pairing information
    /// - Returns: Current network status
    public func getNetworkStatus() async throws -> NetworkStatus {
        let input = NetworkStatusQueryInput()
        return try await network.status(input)
    }

    // MARK: - Convenience Methods

    /// Check if networking is available for pairing
    /// - Returns: True if networking is running and ready for pairing
    public func isNetworkingAvailable() async throws -> Bool {
        let status = try await getNetworkStatus()
        return status.running
    }

    /// Find an active pairing session by ID
    /// - Parameter sessionId: The session ID to find
    /// - Returns: The pairing session if found, nil otherwise
    public func findPairingSession(sessionId: String) async throws -> PairingSessionSummary? {
        let sessions = try await getPairingStatus()
        return sessions.first { $0.id == sessionId }
    }

    /// Wait for a pairing session to complete or fail
    /// - Parameters:
    ///   - sessionId: The session ID to monitor
    ///   - timeout: Maximum time to wait in seconds (default: 300 = 5 minutes)
    ///   - pollInterval: How often to check status in seconds (default: 2)
    /// - Returns: Final session state
    public func waitForPairingCompletion(
        sessionId: String,
        timeout: TimeInterval = 300,
        pollInterval: TimeInterval = 2
    ) async throws -> PairingSessionSummary {
        let startTime = Date()

        while Date().timeIntervalSince(startTime) < timeout {
            if let session = try await findPairingSession(sessionId: sessionId) {
                if session.state.isTerminalState {
                    return session
                }
            }

            try await Task.sleep(nanoseconds: UInt64(pollInterval * 1_000_000_000))
        }

        throw SpacedriveError.operationTimeout("Pairing session timed out after \(timeout) seconds")
    }

    /// Parse a pairing code string into individual words
    /// - Parameter code: Space or comma separated pairing code
    /// - Returns: Array of 12 words, or throws if invalid format
    public static func parsePairingCode(_ code: String) throws -> [String] {
        let words = code
            .components(separatedBy: CharacterSet(charactersIn: " ,\n\t"))
            .compactMap { word in
                let trimmed = word.trimmingCharacters(in: .whitespacesAndNewlines)
                return trimmed.isEmpty ? nil : trimmed.lowercased()
            }

        guard words.count == 12 else {
            throw SpacedriveError.invalidInput("Pairing code must contain exactly 12 words, got \(words.count)")
        }

        return words
    }

    /// Format an array of words into a readable pairing code
    /// - Parameter words: Array of 12 BIP39 words
    /// - Returns: Formatted string suitable for display
    public static func formatPairingCode(_ words: [String]) -> String {
        guard words.count == 12 else {
            return words.joined(separator: " ")
        }

        // Format as 3 lines of 4 words each
        let lines = [
            words[0...3].joined(separator: " "),
            words[4...7].joined(separator: " "),
            words[8...11].joined(separator: " ")
        ]
        return lines.joined(separator: "\n")
    }
}

// MARK: - Error Extensions

extension SpacedriveError {
    public static func operationTimeout(_ message: String) -> SpacedriveError {
        return .invalidResponse(message)
    }

    public static func invalidInput(_ message: String) -> SpacedriveError {
        return .invalidResponse(message)
    }
}
