import Foundation
import AppKit
import SwiftRs

struct OpenWithApp: Codable {
    let id: String
    let name: String
    let icon: String?
}

enum OpenResult: Codable {
    case success
    case fileNotFound(path: String)
    case appNotFound(appId: String)
    case permissionDenied(path: String)
    case platformError(message: String)
    
    enum CodingKeys: String, CodingKey {
        case status
        case path
        case appId = "app_id"
        case message
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        
        switch self {
        case .success:
            try container.encode("success", forKey: .status)
        case .fileNotFound(let path):
            try container.encode("file_not_found", forKey: .status)
            try container.encode(path, forKey: .path)
        case .appNotFound(let appId):
            try container.encode("app_not_found", forKey: .status)
            try container.encode(appId, forKey: .appId)
        case .permissionDenied(let path):
            try container.encode("permission_denied", forKey: .status)
            try container.encode(path, forKey: .path)
        case .platformError(let message):
            try container.encode("platform_error", forKey: .status)
            try container.encode(message, forKey: .message)
        }
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let status = try container.decode(String.self, forKey: .status)
        
        switch status {
        case "success":
            self = .success
        case "file_not_found":
            let path = try container.decode(String.self, forKey: .path)
            self = .fileNotFound(path: path)
        case "app_not_found":
            let appId = try container.decode(String.self, forKey: .appId)
            self = .appNotFound(appId: appId)
        case "permission_denied":
            let path = try container.decode(String.self, forKey: .path)
            self = .permissionDenied(path: path)
        case "platform_error":
            let message = try container.decode(String.self, forKey: .message)
            self = .platformError(message: message)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .status,
                in: container,
                debugDescription: "Unknown status: \(status)"
            )
        }
    }
}

@_cdecl("get_apps_for_path")
func getAppsForPath(path: SRString) -> SRString {
    let url = URL(fileURLWithPath: path.toString())

    // macOS 12+: Use modern API
    let appURLs: [URL]
    if #available(macOS 12.0, *) {
        appURLs = NSWorkspace.shared.urlsForApplications(toOpen: url)
    } else {
        // Fallback for older macOS
        appURLs = getAppsLegacy(for: url)
    }

    // Filter to standard app directories
    // /Applications/ - user/admin installed apps
    // /System/Applications/ - system apps (macOS 10.15+)
    // ~/Applications/ - user-specific apps
    let homeDir = FileManager.default.homeDirectoryForCurrentUser.path
    let validPrefixes = ["/Applications/", "/System/Applications/", "\(homeDir)/Applications/"]

    let apps: [OpenWithApp] = appURLs
        .filter { appURL in
            validPrefixes.contains { appURL.path.hasPrefix($0) }
        }
        .compactMap { appURL in
            guard let bundle = Bundle(url: appURL),
                  let bundleId = bundle.bundleIdentifier,
                  let displayName = bundle.infoDictionary?["CFBundleDisplayName"] as? String
                    ?? bundle.infoDictionary?["CFBundleName"] as? String else {
                return nil
            }

            return OpenWithApp(id: bundleId, name: displayName, icon: nil)
        }

    let json = (try? JSONEncoder().encode(apps)) ?? Data()
    return SRString(String(data: json, encoding: .utf8) ?? "[]")
}

@_cdecl("open_path_with_default")
func openPathWithDefault(path: SRString) -> SRString {
    let url = URL(fileURLWithPath: path.toString())
    
    let success = NSWorkspace.shared.open(url)
    let result = success
        ? OpenResult.success
        : OpenResult.platformError(message: "Failed to open file")
    
    let json = (try? JSONEncoder().encode(result)) ?? Data()
    return SRString(String(data: json, encoding: .utf8) ?? "{}")
}

@_cdecl("open_path_with_app")
func openPathWithApp(path: SRString, appId: SRString) -> SRString {
    let fileURL = URL(fileURLWithPath: path.toString())
    let bundleId = appId.toString()
    
    guard let appURL = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) else {
        let result = OpenResult.appNotFound(appId: bundleId)
        let json = (try? JSONEncoder().encode(result)) ?? Data()
        return SRString(String(data: json, encoding: .utf8) ?? "{}")
    }
    
    let config = NSWorkspace.OpenConfiguration()
    var openResult = OpenResult.success
    let semaphore = DispatchSemaphore(value: 0)
    
    NSWorkspace.shared.open([fileURL], withApplicationAt: appURL, configuration: config) { _, error in
        if let error = error {
            openResult = OpenResult.platformError(message: error.localizedDescription)
        }
        semaphore.signal()
    }
    
    // Wait for completion with timeout
    let timeoutResult = semaphore.wait(timeout: .now() + 5)
    if timeoutResult == .timedOut {
        openResult = OpenResult.platformError(message: "Operation timed out after 5 seconds")
    }
    
    let json = (try? JSONEncoder().encode(openResult)) ?? Data()
    return SRString(String(data: json, encoding: .utf8) ?? "{}")
}

@_cdecl("open_paths_with_app")
func openPathsWithApp(paths: SRString, appId: SRString) -> SRString {
    let pathStrings = paths.toString().split(separator: "\0")
    let fileURLs = pathStrings.map { URL(fileURLWithPath: String($0)) }
    let bundleId = appId.toString()
    
    guard let appURL = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) else {
        let results = fileURLs.map { _ in OpenResult.appNotFound(appId: bundleId) }
        let json = (try? JSONEncoder().encode(results)) ?? Data()
        return SRString(String(data: json, encoding: .utf8) ?? "{}")
    }
    
    let config = NSWorkspace.OpenConfiguration()
    var openResult = OpenResult.success
    let semaphore = DispatchSemaphore(value: 0)
    
    NSWorkspace.shared.open(fileURLs, withApplicationAt: appURL, configuration: config) { _, error in
        if let error = error {
            openResult = OpenResult.platformError(message: error.localizedDescription)
        }
        semaphore.signal()
    }
    
    // Wait for completion with timeout
    let timeoutResult = semaphore.wait(timeout: .now() + 5)
    if timeoutResult == .timedOut {
        openResult = OpenResult.platformError(message: "Operation timed out after 5 seconds")
    }
    
    let results = fileURLs.map { _ in openResult }
    let json = (try? JSONEncoder().encode(results)) ?? Data()
    return SRString(String(data: json, encoding: .utf8) ?? "{}")
}

func getAppsLegacy(for url: URL) -> [URL] {
    // Legacy implementation for macOS < 12
    // For now, return empty array - can implement LSCopyAllRoleHandlersForContentType if needed
    return []
}
