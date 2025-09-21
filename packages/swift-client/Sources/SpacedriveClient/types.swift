// This file was generated from JSON Schema using quicktype, do not modify it directly.
// To parse the JSON, add this file to your project and do:
//
//   let event = try Event(json)

import Foundation

public enum EventElement: Codable {
    case eventClass(EventClass)
    case string(String)

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let x = try? container.decode(String.self) {
            self = .string(x)
            return
        }
        if let x = try? container.decode(EventClass.self) {
            self = .eventClass(x)
            return
        }
        throw DecodingError.typeMismatch(EventElement.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for EventElement"))
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .eventClass(let x):
            try container.encode(x)
        case .string(let x):
            try container.encode(x)
        }
    }
}

// MARK: - EventClass
public struct EventClass: Codable {
    public let libraryCreated, libraryOpened: Library?
    public let libraryClosed: LibraryClosed?
    public let libraryDeleted: LibraryDeleted?
    public let entryCreated, entryModified, entryDeleted: Entry?
    public let entryMoved: EntryMoved?
    public let jobQueued, jobStarted: JobCancelledClass?
    public let jobProgress: JobProgress?
    public let jobCompleted: JobCompleted?
    public let jobFailed: JobFailed?
    public let jobCancelled: JobCancelledClass?
    public let jobPaused, jobResumed: JobPausedClass?
    public let indexingStarted: IndexingStarted?
    public let indexingProgress: IndexingProgress?
    public let indexingCompleted: IndexingCompleted?
    public let indexingFailed: IndexingFailed?
    public let deviceConnected: DeviceConnected?
    public let deviceDisconnected: DeviceDisconnected?
    public let fsRawChange: FSRawChange?
    public let locationAdded: LocationAdded?
    public let locationRemoved: LocationRemoved?
    public let filesIndexed: FilesIndexed?
    public let thumbnailsGenerated: ThumbnailsGenerated?
    public let fileOperationCompleted: FileOperationCompleted?
    public let filesModified: FilesModified?
    public let logMessage: LogMessage?
    public let custom: Custom?

    public enum CodingKeys: String, CodingKey {
        case libraryCreated = "LibraryCreated"
        case libraryOpened = "LibraryOpened"
        case libraryClosed = "LibraryClosed"
        case libraryDeleted = "LibraryDeleted"
        case entryCreated = "EntryCreated"
        case entryModified = "EntryModified"
        case entryDeleted = "EntryDeleted"
        case entryMoved = "EntryMoved"
        case jobQueued = "JobQueued"
        case jobStarted = "JobStarted"
        case jobProgress = "JobProgress"
        case jobCompleted = "JobCompleted"
        case jobFailed = "JobFailed"
        case jobCancelled = "JobCancelled"
        case jobPaused = "JobPaused"
        case jobResumed = "JobResumed"
        case indexingStarted = "IndexingStarted"
        case indexingProgress = "IndexingProgress"
        case indexingCompleted = "IndexingCompleted"
        case indexingFailed = "IndexingFailed"
        case deviceConnected = "DeviceConnected"
        case deviceDisconnected = "DeviceDisconnected"
        case fsRawChange = "FsRawChange"
        case locationAdded = "LocationAdded"
        case locationRemoved = "LocationRemoved"
        case filesIndexed = "FilesIndexed"
        case thumbnailsGenerated = "ThumbnailsGenerated"
        case fileOperationCompleted = "FileOperationCompleted"
        case filesModified = "FilesModified"
        case logMessage = "LogMessage"
        case custom = "Custom"
    }

    public init(libraryCreated: Library?, libraryOpened: Library?, libraryClosed: LibraryClosed?, libraryDeleted: LibraryDeleted?, entryCreated: Entry?, entryModified: Entry?, entryDeleted: Entry?, entryMoved: EntryMoved?, jobQueued: JobCancelledClass?, jobStarted: JobCancelledClass?, jobProgress: JobProgress?, jobCompleted: JobCompleted?, jobFailed: JobFailed?, jobCancelled: JobCancelledClass?, jobPaused: JobPausedClass?, jobResumed: JobPausedClass?, indexingStarted: IndexingStarted?, indexingProgress: IndexingProgress?, indexingCompleted: IndexingCompleted?, indexingFailed: IndexingFailed?, deviceConnected: DeviceConnected?, deviceDisconnected: DeviceDisconnected?, fsRawChange: FSRawChange?, locationAdded: LocationAdded?, locationRemoved: LocationRemoved?, filesIndexed: FilesIndexed?, thumbnailsGenerated: ThumbnailsGenerated?, fileOperationCompleted: FileOperationCompleted?, filesModified: FilesModified?, logMessage: LogMessage?, custom: Custom?) {
        self.libraryCreated = libraryCreated
        self.libraryOpened = libraryOpened
        self.libraryClosed = libraryClosed
        self.libraryDeleted = libraryDeleted
        self.entryCreated = entryCreated
        self.entryModified = entryModified
        self.entryDeleted = entryDeleted
        self.entryMoved = entryMoved
        self.jobQueued = jobQueued
        self.jobStarted = jobStarted
        self.jobProgress = jobProgress
        self.jobCompleted = jobCompleted
        self.jobFailed = jobFailed
        self.jobCancelled = jobCancelled
        self.jobPaused = jobPaused
        self.jobResumed = jobResumed
        self.indexingStarted = indexingStarted
        self.indexingProgress = indexingProgress
        self.indexingCompleted = indexingCompleted
        self.indexingFailed = indexingFailed
        self.deviceConnected = deviceConnected
        self.deviceDisconnected = deviceDisconnected
        self.fsRawChange = fsRawChange
        self.locationAdded = locationAdded
        self.locationRemoved = locationRemoved
        self.filesIndexed = filesIndexed
        self.thumbnailsGenerated = thumbnailsGenerated
        self.fileOperationCompleted = fileOperationCompleted
        self.filesModified = filesModified
        self.logMessage = logMessage
        self.custom = custom
    }
}

// MARK: EventClass convenience initializers and mutators

public extension EventClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(EventClass.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryCreated: Library?? = nil,
        libraryOpened: Library?? = nil,
        libraryClosed: LibraryClosed?? = nil,
        libraryDeleted: LibraryDeleted?? = nil,
        entryCreated: Entry?? = nil,
        entryModified: Entry?? = nil,
        entryDeleted: Entry?? = nil,
        entryMoved: EntryMoved?? = nil,
        jobQueued: JobCancelledClass?? = nil,
        jobStarted: JobCancelledClass?? = nil,
        jobProgress: JobProgress?? = nil,
        jobCompleted: JobCompleted?? = nil,
        jobFailed: JobFailed?? = nil,
        jobCancelled: JobCancelledClass?? = nil,
        jobPaused: JobPausedClass?? = nil,
        jobResumed: JobPausedClass?? = nil,
        indexingStarted: IndexingStarted?? = nil,
        indexingProgress: IndexingProgress?? = nil,
        indexingCompleted: IndexingCompleted?? = nil,
        indexingFailed: IndexingFailed?? = nil,
        deviceConnected: DeviceConnected?? = nil,
        deviceDisconnected: DeviceDisconnected?? = nil,
        fsRawChange: FSRawChange?? = nil,
        locationAdded: LocationAdded?? = nil,
        locationRemoved: LocationRemoved?? = nil,
        filesIndexed: FilesIndexed?? = nil,
        thumbnailsGenerated: ThumbnailsGenerated?? = nil,
        fileOperationCompleted: FileOperationCompleted?? = nil,
        filesModified: FilesModified?? = nil,
        logMessage: LogMessage?? = nil,
        custom: Custom?? = nil
    ) -> EventClass {
        return EventClass(
            libraryCreated: libraryCreated ?? self.libraryCreated,
            libraryOpened: libraryOpened ?? self.libraryOpened,
            libraryClosed: libraryClosed ?? self.libraryClosed,
            libraryDeleted: libraryDeleted ?? self.libraryDeleted,
            entryCreated: entryCreated ?? self.entryCreated,
            entryModified: entryModified ?? self.entryModified,
            entryDeleted: entryDeleted ?? self.entryDeleted,
            entryMoved: entryMoved ?? self.entryMoved,
            jobQueued: jobQueued ?? self.jobQueued,
            jobStarted: jobStarted ?? self.jobStarted,
            jobProgress: jobProgress ?? self.jobProgress,
            jobCompleted: jobCompleted ?? self.jobCompleted,
            jobFailed: jobFailed ?? self.jobFailed,
            jobCancelled: jobCancelled ?? self.jobCancelled,
            jobPaused: jobPaused ?? self.jobPaused,
            jobResumed: jobResumed ?? self.jobResumed,
            indexingStarted: indexingStarted ?? self.indexingStarted,
            indexingProgress: indexingProgress ?? self.indexingProgress,
            indexingCompleted: indexingCompleted ?? self.indexingCompleted,
            indexingFailed: indexingFailed ?? self.indexingFailed,
            deviceConnected: deviceConnected ?? self.deviceConnected,
            deviceDisconnected: deviceDisconnected ?? self.deviceDisconnected,
            fsRawChange: fsRawChange ?? self.fsRawChange,
            locationAdded: locationAdded ?? self.locationAdded,
            locationRemoved: locationRemoved ?? self.locationRemoved,
            filesIndexed: filesIndexed ?? self.filesIndexed,
            thumbnailsGenerated: thumbnailsGenerated ?? self.thumbnailsGenerated,
            fileOperationCompleted: fileOperationCompleted ?? self.fileOperationCompleted,
            filesModified: filesModified ?? self.filesModified,
            logMessage: logMessage ?? self.logMessage,
            custom: custom ?? self.custom
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Custom
public struct Custom: Codable {
    public let eventType: String
    public let data: DataClass

    public enum CodingKeys: String, CodingKey {
        case eventType = "event_type"
        case data
    }

    public init(eventType: String, data: DataClass) {
        self.eventType = eventType
        self.data = data
    }
}

// MARK: Custom convenience initializers and mutators

public extension Custom {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Custom.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        eventType: String? = nil,
        data: DataClass? = nil
    ) -> Custom {
        return Custom(
            eventType: eventType ?? self.eventType,
            data: data ?? self.data
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DataClass
public struct DataClass: Codable {
    public let key: String

    public init(key: String) {
        self.key = key
    }
}

// MARK: DataClass convenience initializers and mutators

public extension DataClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DataClass.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        key: String? = nil
    ) -> DataClass {
        return DataClass(
            key: key ?? self.key
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DeviceConnected
public struct DeviceConnected: Codable {
    public let deviceID, deviceName: String

    public enum CodingKeys: String, CodingKey {
        case deviceID = "device_id"
        case deviceName = "device_name"
    }

    public init(deviceID: String, deviceName: String) {
        self.deviceID = deviceID
        self.deviceName = deviceName
    }
}

// MARK: DeviceConnected convenience initializers and mutators

public extension DeviceConnected {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DeviceConnected.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        deviceID: String? = nil,
        deviceName: String? = nil
    ) -> DeviceConnected {
        return DeviceConnected(
            deviceID: deviceID ?? self.deviceID,
            deviceName: deviceName ?? self.deviceName
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - DeviceDisconnected
public struct DeviceDisconnected: Codable {
    public let deviceID: String

    public enum CodingKeys: String, CodingKey {
        case deviceID = "device_id"
    }

    public init(deviceID: String) {
        self.deviceID = deviceID
    }
}

// MARK: DeviceDisconnected convenience initializers and mutators

public extension DeviceDisconnected {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(DeviceDisconnected.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        deviceID: String? = nil
    ) -> DeviceDisconnected {
        return DeviceDisconnected(
            deviceID: deviceID ?? self.deviceID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Entry
public struct Entry: Codable {
    public let libraryID, entryID: String

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case entryID = "entry_id"
    }

    public init(libraryID: String, entryID: String) {
        self.libraryID = libraryID
        self.entryID = entryID
    }
}

// MARK: Entry convenience initializers and mutators

public extension Entry {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Entry.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        entryID: String? = nil
    ) -> Entry {
        return Entry(
            libraryID: libraryID ?? self.libraryID,
            entryID: entryID ?? self.entryID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - EntryMoved
public struct EntryMoved: Codable {
    public let libraryID, entryID, oldPath, newPath: String

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case entryID = "entry_id"
        case oldPath = "old_path"
        case newPath = "new_path"
    }

    public init(libraryID: String, entryID: String, oldPath: String, newPath: String) {
        self.libraryID = libraryID
        self.entryID = entryID
        self.oldPath = oldPath
        self.newPath = newPath
    }
}

// MARK: EntryMoved convenience initializers and mutators

public extension EntryMoved {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(EntryMoved.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        entryID: String? = nil,
        oldPath: String? = nil,
        newPath: String? = nil
    ) -> EntryMoved {
        return EntryMoved(
            libraryID: libraryID ?? self.libraryID,
            entryID: entryID ?? self.entryID,
            oldPath: oldPath ?? self.oldPath,
            newPath: newPath ?? self.newPath
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FileOperationCompleted
public struct FileOperationCompleted: Codable {
    public let libraryID, operation: String
    public let affectedFiles: Int

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case operation
        case affectedFiles = "affected_files"
    }

    public init(libraryID: String, operation: String, affectedFiles: Int) {
        self.libraryID = libraryID
        self.operation = operation
        self.affectedFiles = affectedFiles
    }
}

// MARK: FileOperationCompleted convenience initializers and mutators

public extension FileOperationCompleted {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FileOperationCompleted.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        operation: String? = nil,
        affectedFiles: Int? = nil
    ) -> FileOperationCompleted {
        return FileOperationCompleted(
            libraryID: libraryID ?? self.libraryID,
            operation: operation ?? self.operation,
            affectedFiles: affectedFiles ?? self.affectedFiles
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FilesIndexed
public struct FilesIndexed: Codable {
    public let libraryID, locationID: String
    public let count: Int

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case locationID = "location_id"
        case count
    }

    public init(libraryID: String, locationID: String, count: Int) {
        self.libraryID = libraryID
        self.locationID = locationID
        self.count = count
    }
}

// MARK: FilesIndexed convenience initializers and mutators

public extension FilesIndexed {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FilesIndexed.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        locationID: String? = nil,
        count: Int? = nil
    ) -> FilesIndexed {
        return FilesIndexed(
            libraryID: libraryID ?? self.libraryID,
            locationID: locationID ?? self.locationID,
            count: count ?? self.count
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FilesModified
public struct FilesModified: Codable {
    public let libraryID: String
    public let paths: [String]

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case paths
    }

    public init(libraryID: String, paths: [String]) {
        self.libraryID = libraryID
        self.paths = paths
    }
}

// MARK: FilesModified convenience initializers and mutators

public extension FilesModified {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FilesModified.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        paths: [String]? = nil
    ) -> FilesModified {
        return FilesModified(
            libraryID: libraryID ?? self.libraryID,
            paths: paths ?? self.paths
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - FSRawChange
public struct FSRawChange: Codable {
    public let libraryID: String
    public let kind: Kind

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case kind
    }

    public init(libraryID: String, kind: Kind) {
        self.libraryID = libraryID
        self.kind = kind
    }
}

// MARK: FSRawChange convenience initializers and mutators

public extension FSRawChange {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(FSRawChange.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        kind: Kind? = nil
    ) -> FSRawChange {
        return FSRawChange(
            libraryID: libraryID ?? self.libraryID,
            kind: kind ?? self.kind
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Kind
public struct Kind: Codable {
    public let create: Create

    public enum CodingKeys: String, CodingKey {
        case create = "Create"
    }

    public init(create: Create) {
        self.create = create
    }
}

// MARK: Kind convenience initializers and mutators

public extension Kind {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Kind.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        create: Create? = nil
    ) -> Kind {
        return Kind(
            create: create ?? self.create
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Create
public struct Create: Codable {
    public let path: String

    public init(path: String) {
        self.path = path
    }
}

// MARK: Create convenience initializers and mutators

public extension Create {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Create.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        path: String? = nil
    ) -> Create {
        return Create(
            path: path ?? self.path
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - IndexingCompleted
public struct IndexingCompleted: Codable {
    public let locationID: String
    public let totalFiles, totalDirs: Int

    public enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
        case totalFiles = "total_files"
        case totalDirs = "total_dirs"
    }

    public init(locationID: String, totalFiles: Int, totalDirs: Int) {
        self.locationID = locationID
        self.totalFiles = totalFiles
        self.totalDirs = totalDirs
    }
}

// MARK: IndexingCompleted convenience initializers and mutators

public extension IndexingCompleted {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingCompleted.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        locationID: String? = nil,
        totalFiles: Int? = nil,
        totalDirs: Int? = nil
    ) -> IndexingCompleted {
        return IndexingCompleted(
            locationID: locationID ?? self.locationID,
            totalFiles: totalFiles ?? self.totalFiles,
            totalDirs: totalDirs ?? self.totalDirs
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - IndexingFailed
public struct IndexingFailed: Codable {
    public let locationID, error: String

    public enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
        case error
    }

    public init(locationID: String, error: String) {
        self.locationID = locationID
        self.error = error
    }
}

// MARK: IndexingFailed convenience initializers and mutators

public extension IndexingFailed {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingFailed.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        locationID: String? = nil,
        error: String? = nil
    ) -> IndexingFailed {
        return IndexingFailed(
            locationID: locationID ?? self.locationID,
            error: error ?? self.error
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - IndexingProgress
public struct IndexingProgress: Codable {
    public let locationID: String
    public let processed, total: Int

    public enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
        case processed, total
    }

    public init(locationID: String, processed: Int, total: Int) {
        self.locationID = locationID
        self.processed = processed
        self.total = total
    }
}

// MARK: IndexingProgress convenience initializers and mutators

public extension IndexingProgress {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingProgress.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        locationID: String? = nil,
        processed: Int? = nil,
        total: Int? = nil
    ) -> IndexingProgress {
        return IndexingProgress(
            locationID: locationID ?? self.locationID,
            processed: processed ?? self.processed,
            total: total ?? self.total
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - IndexingStarted
public struct IndexingStarted: Codable {
    public let locationID: String

    public enum CodingKeys: String, CodingKey {
        case locationID = "location_id"
    }

    public init(locationID: String) {
        self.locationID = locationID
    }
}

// MARK: IndexingStarted convenience initializers and mutators

public extension IndexingStarted {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(IndexingStarted.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        locationID: String? = nil
    ) -> IndexingStarted {
        return IndexingStarted(
            locationID: locationID ?? self.locationID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobCancelledClass
public struct JobCancelledClass: Codable {
    public let jobID, jobType: String

    public enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case jobType = "job_type"
    }

    public init(jobID: String, jobType: String) {
        self.jobID = jobID
        self.jobType = jobType
    }
}

// MARK: JobCancelledClass convenience initializers and mutators

public extension JobCancelledClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobCancelledClass.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        jobID: String? = nil,
        jobType: String? = nil
    ) -> JobCancelledClass {
        return JobCancelledClass(
            jobID: jobID ?? self.jobID,
            jobType: jobType ?? self.jobType
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobCompleted
public struct JobCompleted: Codable {
    public let jobID, jobType: String
    public let output: Output

    public enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case jobType = "job_type"
        case output
    }

    public init(jobID: String, jobType: String, output: Output) {
        self.jobID = jobID
        self.jobType = jobType
        self.output = output
    }
}

// MARK: JobCompleted convenience initializers and mutators

public extension JobCompleted {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobCompleted.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        jobID: String? = nil,
        jobType: String? = nil,
        output: Output? = nil
    ) -> JobCompleted {
        return JobCompleted(
            jobID: jobID ?? self.jobID,
            jobType: jobType ?? self.jobType,
            output: output ?? self.output
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Output
public struct Output: Codable {
    public let type: String

    public init(type: String) {
        self.type = type
    }
}

// MARK: Output convenience initializers and mutators

public extension Output {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Output.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        type: String? = nil
    ) -> Output {
        return Output(
            type: type ?? self.type
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobFailed
public struct JobFailed: Codable {
    public let jobID, jobType, error: String

    public enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case jobType = "job_type"
        case error
    }

    public init(jobID: String, jobType: String, error: String) {
        self.jobID = jobID
        self.jobType = jobType
        self.error = error
    }
}

// MARK: JobFailed convenience initializers and mutators

public extension JobFailed {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobFailed.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        jobID: String? = nil,
        jobType: String? = nil,
        error: String? = nil
    ) -> JobFailed {
        return JobFailed(
            jobID: jobID ?? self.jobID,
            jobType: jobType ?? self.jobType,
            error: error ?? self.error
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobPausedClass
public struct JobPausedClass: Codable {
    public let jobID: String

    public enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
    }

    public init(jobID: String) {
        self.jobID = jobID
    }
}

// MARK: JobPausedClass convenience initializers and mutators

public extension JobPausedClass {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobPausedClass.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        jobID: String? = nil
    ) -> JobPausedClass {
        return JobPausedClass(
            jobID: jobID ?? self.jobID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - JobProgress
public struct JobProgress: Codable {
    public let jobID, jobType: String
    public let progress: Double
    public let message: String
    public let genericProgress: GenericProgress?

    public enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case jobType = "job_type"
        case progress, message
        case genericProgress = "generic_progress"
    }

    public init(jobID: String, jobType: String, progress: Double, message: String, genericProgress: GenericProgress?) {
        self.jobID = jobID
        self.jobType = jobType
        self.progress = progress
        self.message = message
        self.genericProgress = genericProgress
    }
}

// MARK: JobProgress convenience initializers and mutators

public extension JobProgress {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(JobProgress.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        jobID: String? = nil,
        jobType: String? = nil,
        progress: Double? = nil,
        message: String? = nil,
        genericProgress: GenericProgress?? = nil
    ) -> JobProgress {
        return JobProgress(
            jobID: jobID ?? self.jobID,
            jobType: jobType ?? self.jobType,
            progress: progress ?? self.progress,
            message: message ?? self.message,
            genericProgress: genericProgress ?? self.genericProgress
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - GenericProgress
public struct GenericProgress: Codable {
    public let message: String
    public let percentage: Double
    public let phase: String

    public init(message: String, percentage: Double, phase: String) {
        self.message = message
        self.percentage = percentage
        self.phase = phase
    }
}

// MARK: GenericProgress convenience initializers and mutators

public extension GenericProgress {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(GenericProgress.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        message: String? = nil,
        percentage: Double? = nil,
        phase: String? = nil
    ) -> GenericProgress {
        return GenericProgress(
            message: message ?? self.message,
            percentage: percentage ?? self.percentage,
            phase: phase ?? self.phase
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LibraryClosed
public struct LibraryClosed: Codable {
    public let id, name: String

    public init(id: String, name: String) {
        self.id = id
        self.name = name
    }
}

// MARK: LibraryClosed convenience initializers and mutators

public extension LibraryClosed {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LibraryClosed.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        id: String? = nil,
        name: String? = nil
    ) -> LibraryClosed {
        return LibraryClosed(
            id: id ?? self.id,
            name: name ?? self.name
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Library
public struct Library: Codable {
    public let id, name, path: String

    public init(id: String, name: String, path: String) {
        self.id = id
        self.name = name
        self.path = path
    }
}

// MARK: Library convenience initializers and mutators

public extension Library {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Library.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        id: String? = nil,
        name: String? = nil,
        path: String? = nil
    ) -> Library {
        return Library(
            id: id ?? self.id,
            name: name ?? self.name,
            path: path ?? self.path
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LibraryDeleted
public struct LibraryDeleted: Codable {
    public let id, name: String
    public let deletedData: Bool

    public enum CodingKeys: String, CodingKey {
        case id, name
        case deletedData = "deleted_data"
    }

    public init(id: String, name: String, deletedData: Bool) {
        self.id = id
        self.name = name
        self.deletedData = deletedData
    }
}

// MARK: LibraryDeleted convenience initializers and mutators

public extension LibraryDeleted {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LibraryDeleted.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        id: String? = nil,
        name: String? = nil,
        deletedData: Bool? = nil
    ) -> LibraryDeleted {
        return LibraryDeleted(
            id: id ?? self.id,
            name: name ?? self.name,
            deletedData: deletedData ?? self.deletedData
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LocationAdded
public struct LocationAdded: Codable {
    public let libraryID, locationID, path: String

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case locationID = "location_id"
        case path
    }

    public init(libraryID: String, locationID: String, path: String) {
        self.libraryID = libraryID
        self.locationID = locationID
        self.path = path
    }
}

// MARK: LocationAdded convenience initializers and mutators

public extension LocationAdded {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LocationAdded.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        locationID: String? = nil,
        path: String? = nil
    ) -> LocationAdded {
        return LocationAdded(
            libraryID: libraryID ?? self.libraryID,
            locationID: locationID ?? self.locationID,
            path: path ?? self.path
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LocationRemoved
public struct LocationRemoved: Codable {
    public let libraryID, locationID: String

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case locationID = "location_id"
    }

    public init(libraryID: String, locationID: String) {
        self.libraryID = libraryID
        self.locationID = locationID
    }
}

// MARK: LocationRemoved convenience initializers and mutators

public extension LocationRemoved {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LocationRemoved.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        locationID: String? = nil
    ) -> LocationRemoved {
        return LocationRemoved(
            libraryID: libraryID ?? self.libraryID,
            locationID: locationID ?? self.locationID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - LogMessage
public struct LogMessage: Codable {
    public let timestamp, level, target, message: String
    public let jobID, libraryID: String

    public enum CodingKeys: String, CodingKey {
        case timestamp, level, target, message
        case jobID = "job_id"
        case libraryID = "library_id"
    }

    public init(timestamp: String, level: String, target: String, message: String, jobID: String, libraryID: String) {
        self.timestamp = timestamp
        self.level = level
        self.target = target
        self.message = message
        self.jobID = jobID
        self.libraryID = libraryID
    }
}

// MARK: LogMessage convenience initializers and mutators

public extension LogMessage {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(LogMessage.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        timestamp: String? = nil,
        level: String? = nil,
        target: String? = nil,
        message: String? = nil,
        jobID: String? = nil,
        libraryID: String? = nil
    ) -> LogMessage {
        return LogMessage(
            timestamp: timestamp ?? self.timestamp,
            level: level ?? self.level,
            target: target ?? self.target,
            message: message ?? self.message,
            jobID: jobID ?? self.jobID,
            libraryID: libraryID ?? self.libraryID
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - ThumbnailsGenerated
public struct ThumbnailsGenerated: Codable {
    public let libraryID: String
    public let count: Int

    public enum CodingKeys: String, CodingKey {
        case libraryID = "library_id"
        case count
    }

    public init(libraryID: String, count: Int) {
        self.libraryID = libraryID
        self.count = count
    }
}

// MARK: ThumbnailsGenerated convenience initializers and mutators

public extension ThumbnailsGenerated {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(ThumbnailsGenerated.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func with(
        libraryID: String? = nil,
        count: Int? = nil
    ) -> ThumbnailsGenerated {
        return ThumbnailsGenerated(
            libraryID: libraryID ?? self.libraryID,
            count: count ?? self.count
        )
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

public typealias Event = [EventElement]

public extension Array where Element == Event.Element {
    init(data: Data) throws {
        self = try newJSONDecoder().decode(Event.self, from: data)
    }

    init(_ json: String, using encoding: String.Encoding = .utf8) throws {
        guard let data = json.data(using: encoding) else {
            throw NSError(domain: "JSONDecoding", code: 0, userInfo: nil)
        }
        try self.init(data: data)
    }

    init(fromURL url: URL) throws {
        try self.init(data: try Data(contentsOf: url))
    }

    func jsonData() throws -> Data {
        return try newJSONEncoder().encode(self)
    }

    func jsonString(encoding: String.Encoding = .utf8) throws -> String? {
        return String(data: try self.jsonData(), encoding: encoding)
    }
}

// MARK: - Helper functions for creating encoders and decoders

func newJSONDecoder() -> JSONDecoder {
    let decoder = JSONDecoder()
    if #available(iOS 10.0, OSX 10.12, tvOS 10.0, watchOS 3.0, *) {
        decoder.dateDecodingStrategy = .iso8601
    }
    return decoder
}

func newJSONEncoder() -> JSONEncoder {
    let encoder = JSONEncoder()
    if #available(iOS 10.0, OSX 10.12, tvOS 10.0, watchOS 3.0, *) {
        encoder.dateEncodingStrategy = .iso8601
    }
    return encoder
}
