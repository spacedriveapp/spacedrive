import Foundation
import os.log
import SpacedriveClient
import SwiftUI
import Combine

/// Custom errors for the Inspector
enum InspectorError: LocalizedError {
    case daemonConnectorNotAvailable
    case invalidFileURL
    case fileNotFound

    var errorDescription: String? {
        switch self {
        case .daemonConnectorNotAvailable:
            return "Daemon connector is not available"
        case .invalidFileURL:
            return "Invalid file URL provided"
        case .fileNotFound:
            return "File not found"
        }
    }
}

/// ViewModel for the Inspector window
@MainActor
class InspectorViewModel: ObservableObject {
    @Published var file: File?
    @Published var isLoading = false
    @Published var errorMessage: String?
    @Published var draggedFileURL: URL?

    private var daemonConnector: DaemonConnector?
    private let logger = Logger(subsystem: "com.spacedrive.inspector", category: "InspectorViewModel")

    init() {
        logger.info("InspectorViewModel initialized")
        setupDaemonConnector()
    }

    deinit {
        logger.info("InspectorViewModel deinitialized")
    }

    private func setupDaemonConnector() {
        logger.info("Setting up DaemonConnector")
        daemonConnector = DaemonConnector()
        logger.info("DaemonConnector setup complete")
    }

    /// Load file information by path
    func loadFileByPath(_ url: URL) {
        logger.info("Starting to load file: \(url.path)")

        isLoading = true
        errorMessage = nil
        draggedFileURL = url

        Task {
            do {
                logger.info("Creating FileByPathQuery for path: \(url.path)")

                // Create the query with the local path directly
                let query = FileByPathQuery(path: url.path)

                logger.info("Query created successfully, executing...")

                // Execute the query
                guard let daemonConnector = daemonConnector else {
                    logger.error("DaemonConnector is nil")
                    throw InspectorError.daemonConnectorNotAvailable
                }

                let result = try await daemonConnector.queryFileByPath(query)

                logger.info("Query executed successfully, result: \(result != nil ? "file found" : "file not found")")

                await MainActor.run {
                    self.file = result
                    self.isLoading = false
                    if result != nil {
                        self.logger.info("File loaded successfully: \(url.lastPathComponent)")
                    } else {
                        self.logger.warning("File not found in database: \(url.path)")
                        self.errorMessage = "File not found in Spacedrive database"
                    }
                }
            } catch {
                logger.error("Failed to load file: \(error.localizedDescription)")
                logger.error("Error details: \(String(describing: error))")

                await MainActor.run {
                    self.errorMessage = "Failed to load file: \(error.localizedDescription)"
                    self.isLoading = false
                }
            }
        }
    }

    /// Clear the current file
    func clearFile() {
        logger.info("Clearing current file")
        file = nil
        errorMessage = nil
        draggedFileURL = nil
        logger.info("File cleared successfully")
    }

    /// Check if a file URL is valid for inspection
    func isValidFileURL(_ url: URL) -> Bool {
        logger.info("Validating file URL: \(url.path)")

        // Check if it's a file URL and the file exists
        guard url.isFileURL else {
            logger.warning("Invalid file URL (not a file URL): \(url)")
            return false
        }

        var isDirectory: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory)

        // Only allow files, not directories
        let isValid = exists && !isDirectory.boolValue

        if !exists {
            logger.warning("File does not exist: \(url.path)")
        } else if isDirectory.boolValue {
            logger.warning("Path is a directory, not a file: \(url.path)")
        } else {
            logger.info("File URL is valid: \(url.lastPathComponent)")
        }

        return isValid
    }
}
